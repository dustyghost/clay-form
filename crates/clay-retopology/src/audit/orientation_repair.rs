//! Controlled contraction tests: preserve every previously nonnegative cell.
use super::Snapshot;
use crate::{Result, coarse::Graph};
use glam::IVec2;
use std::time::{Duration, Instant};

/// Walk the directed-edge fans used by QuadriFlow's CheckShrink. The returned
/// quarter turn is the grid transport around a closed vertex fan, not the
/// smooth field index on a triangle. Open fans have no closed-loop index.
fn fan_turns(graph: &Graph) -> Result<Vec<(Vec<usize>, i32)>> {
    let twins = graph.halfedges()?;
    let mut visited = vec![false; twins.len()];
    let mut result = Vec::new();
    for start in 0..twins.len() {
        if visited[start] {
            continue;
        }
        let mut h = start;
        let mut fan = Vec::new();
        let mut turn = 0;
        loop {
            if visited[h] {
                break;
            }
            visited[h] = true;
            fan.push(h);
            let twin = twins[h];
            if twin == crate::coarse::NONE {
                break;
            }
            let next = twin / 3 * 3 + (twin + 1) % 3;
            turn += 2 + graph.rotations[twin / 3][twin % 3] - graph.rotations[next / 3][next % 3];
            h = next;
            if h == start {
                result.push((fan, turn.rem_euclid(4)));
                break;
            }
        }
    }
    Ok(result)
}

fn face_distance(graph: &Graph, seeds: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let mut distance = vec![usize::MAX; graph.faces.len()];
    let mut queue = std::collections::VecDeque::new();
    for f in seeds {
        if distance[f] != 0 {
            distance[f] = 0;
            queue.push_back(f);
        }
    }
    while let Some(f) = queue.pop_front() {
        for &e in &graph.faces[f] {
            for g in graph.edge_faces[e] {
                if g != crate::coarse::NONE && distance[g] == usize::MAX {
                    distance[g] = distance[f] + 1;
                    queue.push_back(g);
                }
            }
        }
    }
    distance
}

fn histogram(values: impl IntoIterator<Item = usize>) -> std::collections::BTreeMap<String, usize> {
    let mut result = std::collections::BTreeMap::new();
    for n in values {
        *result
            .entry(if n == usize::MAX {
                "unreachable".into()
            } else {
                n.to_string()
            })
            .or_default() += 1;
    }
    result
}

pub(super) fn write_initial_grid_input(
    field: &crate::field::Field,
    mut writer: impl std::io::Write,
) -> Result<()> {
    let initial = crate::integer::Lattice::new(field, &|| false)?;
    let mut write = || -> std::io::Result<()> {
        writeln!(
            writer,
            "{} {} {}",
            field.origins.len(),
            initial.faces.len(),
            initial.edges.len()
        )?;
        for i in 0..field.origins.len() {
            for p in [
                field.surface.positions[i],
                field.normals[i],
                field.directions[i],
                field.origins[i],
            ] {
                write!(writer, "{:.17e} {:.17e} {:.17e} ", p.x, p.y, p.z)?;
            }
            writeln!(writer, "{:.17e}", field.spacing[i])?;
        }
        for (tri, face) in field.surface.triangles.iter().zip(&initial.faces) {
            writeln!(
                writer,
                "{} {} {} {} {} {} {} {} {}",
                tri[0],
                tri[1],
                tri[2],
                face.edges[0],
                face.edges[1],
                face.edges[2],
                face.rotations[0],
                face.rotations[1],
                face.rotations[2]
            )?;
        }
        for e in &initial.edges {
            writeln!(writer, "{} {} {} {}", e.ends[0], e.ends[1], e.d.x, e.d.y)?;
        }
        Ok(())
    };
    write().map_err(|e| e.to_string())
}

impl Snapshot {
    pub fn write_initial_grid_input(&self, writer: impl std::io::Write) -> Result<()> {
        write_initial_grid_input(&self.field, writer)
    }

    /// Compare the saved grid's rotational defects to the unresolved cells.
    /// Read-only: no field rotations, geometry, guides or offsets are changed.
    pub fn orientation_diagnostic(&self) -> Result<serde_json::Value> {
        use crate::symmetry::{Extrinsic, Frame, RoSy4};
        let graph = Graph::from_lattice(&self.lattice)?;
        let inverted: Vec<_> = (0..graph.faces.len())
            .filter(|&f| graph.area(f) < 0)
            .collect();
        let mut zero_components = crate::topology::Dsu::new(self.field.origins.len());
        for e in &self.lattice.edges {
            if e.d == IVec2::ZERO {
                zero_components.join(e.ends[0], e.ends[1]);
            }
        }
        let bad_roots: std::collections::BTreeSet<_> = inverted
            .iter()
            .flat_map(|&f| self.field.surface.triangles[f])
            .map(|v| zero_components.find(v))
            .collect();
        let mut grid_poles = Vec::new();
        let mut grid_seeds = Vec::new();
        for (fan, turn) in fan_turns(&graph)? {
            if turn == 0 {
                continue;
            }
            let h = fan[0];
            let vertex = self.field.surface.triangles[h / 3][h % 3];
            if fan
                .iter()
                .any(|&h| self.field.surface.triangles[h / 3][h % 3] != vertex)
            {
                return Err("Grid fan does not follow a source vertex".into());
            }
            grid_seeds.extend(fan.iter().map(|h| h / 3));
            let root = zero_components.find(vertex);
            grid_poles.push(serde_json::json!({"vertex":vertex,"quarter_turn_mod4":turn,"position":self.field.surface.positions[vertex].to_array(),"zero_component":root,"shares_zero_component_with_inversion":bad_roots.contains(&root)}));
        }
        // Same edge-compatibility index sum as upstream
        // Parametrizer::ComputeOrientationSingularities, without its Q sign edit.
        let mut field_poles = Vec::new();
        let mut field_seeds = Vec::new();
        for (f, tri) in self.field.surface.triangles.iter().enumerate() {
            let mut index = 0;
            for k in 0..3 {
                let [a, b] = [tri[k], tri[(k + 1) % 3]];
                let pair = Extrinsic::match_orientation(
                    Frame::new(self.field.directions[a], self.field.normals[a]),
                    Frame::new(self.field.directions[b], self.field.normals[b]),
                );
                index += pair.rhs.1 - pair.lhs.1;
            }
            let index = index.rem_euclid(4);
            if index != 0 {
                field_seeds.push(f);
                let center = tri
                    .iter()
                    .map(|&v| self.field.surface.positions[v])
                    .sum::<crate::Vec3>()
                    / 3.;
                field_poles.push(
                    serde_json::json!({"face":f,"index_mod4":index,"position":center.to_array()}),
                );
            }
        }
        let grid_distance = face_distance(&graph, grid_seeds);
        let field_distance = face_distance(&graph, field_seeds);
        Ok(
            serde_json::json!({"audit_only":true,"read_only":true,"faces":graph.faces.len(),"inversions":inverted.len(),
            "negative_integer_area":inverted.iter().map(|&f| -graph.area(f) as i64).sum::<i64>(),
            "grid_poles":grid_poles,"field_poles":field_poles,
            "inverted_distance_to_grid_pole":histogram(inverted.iter().map(|&f| grid_distance[f])),
            "all_faces_distance_to_grid_pole":histogram(grid_distance),
            "inverted_distance_to_field_pole":histogram(inverted.iter().map(|&f| field_distance[f])),
            "all_faces_distance_to_field_pole":histogram(field_distance),
            "note":"Distances are face-adjacency hops, not geodesic length. Grid fan holonomy and the current interpolated field's triangle indices are distinct diagnostics; proximity alone does not establish causation. Index 2 is reported too, although upstream only labels indices 1 and 3."}),
        )
    }
    pub fn contract_preserving_orientation(
        &mut self,
        max_length: i32,
        seconds: f64,
    ) -> Result<serde_json::Value> {
        if self.repaired
            || !(1..=2).contains(&max_length)
            || !seconds.is_finite()
            || seconds <= 0.
            || seconds > 120.
        {
            return Err("Strict contraction needs an unrepaired snapshot, length 1 or 2 and a budget in (0,120]".into());
        }
        let start = Instant::now();
        let deadline = start + Duration::from_secs_f64(seconds);
        let cancel = || Instant::now() >= deadline;
        let mut graph = Graph::from_lattice(&self.lattice)?;
        let old_areas: Vec<_> = (0..graph.faces.len()).map(|f| graph.area(f)).collect();
        let negative_area_before: i64 = old_areas.iter().map(|&a| i64::from((-a).max(0))).sum();
        let contractions = graph.shrink_preserving_orientation(max_length, &cancel)?;
        let negative_area_contracted: i64 = (0..graph.faces.len())
            .map(|f| i64::from((-graph.area(f)).max(0)))
            .sum();
        let new_inversions = (0..graph.faces.len())
            .filter(|&f| old_areas[f] >= 0 && graph.area(f) < 0)
            .count();
        let closure_defects = (0..graph.faces.len())
            .filter(|&f| {
                (0..3)
                    .map(|k| {
                        crate::coarse::rot(graph.offsets[graph.faces[f][k]], graph.rotations[f][k])
                    })
                    .sum::<IVec2>()
                    != IVec2::ZERO
            })
            .count();
        let changed_locks = self
            .lattice
            .edges
            .iter()
            .zip(&graph.offsets)
            .filter(|(e, d)| e.fixed_axis.is_some_and(|a| e.d[a] != d[a]))
            .count();
        if new_inversions != 0 || closure_defects != 0 || changed_locks != 0 {
            return Err("Strict contraction failed full-graph validation".into());
        }
        let changed_edges = self
            .lattice
            .edges
            .iter()
            .zip(&graph.offsets)
            .filter(|(e, d)| e.d != **d)
            .count();
        for (e, d) in self.lattice.edges.iter_mut().zip(graph.offsets) {
            e.d = d;
        }
        let before_subdivision = self.lattice.flipped();
        let long_edges = self.lattice.long_edges();
        let vertices_before = self.field.origins.len();
        self.lattice
            .subdivide_preserving_orientation(&mut self.field, 150_000, &cancel)?;
        self.diagnostics.samples = self.field.origins.len();
        self.diagnostics.position_defects_after = self.lattice.defects();
        self.diagnostics.flipped_after = self.lattice.flipped();
        self.repaired = self.lattice.flipped() == 0;
        Ok(
            serde_json::json!({"audit_only":true,"integer_repaired":self.repaired,"contractions":contractions,
            "newly_inverted_existing_cells":new_inversions,"closure_defects":self.lattice.defects(),"changed_locks":changed_locks,
            "changed_edges":changed_edges,"inversions_before_subdivision":before_subdivision,"inversions_after_subdivision":self.lattice.flipped(),
            "negative_area_before":negative_area_before,"negative_area_after_contraction":negative_area_contracted,
            "long_edges_before_subdivision":long_edges,"added_samples":self.field.origins.len()-vertices_before,"seconds":start.elapsed().as_secs_f64(),
            "acceptance":"Integer repair alone is not mesh acceptance; extraction and all topology/geometry checks are still required."}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fan_index_is_invariant_under_independent_face_frame_rotations() {
        let faces = vec![[0, 1, 2], [2, 3, 4], [1, 5, 3], [0, 4, 5]];
        let mut graph = Graph {
            faces,
            rotations: vec![[0, 0, 0], [2, 0, 0], [2, 0, 2], [2, 2, 2]],
            offsets: vec![IVec2::ZERO; 6],
            allow: vec![[1, 1]; 6],
            edge_faces: vec![[0, 3], [0, 2], [0, 1], [1, 2], [1, 3], [2, 3]],
            source_faces: vec![0, 1, 2, 3],
        };
        assert!(
            fan_turns(&graph)
                .unwrap()
                .iter()
                .all(|(_, turn)| *turn == 0)
        );
        // A single changed edge frame creates opposite quarter-turn defects
        // at its endpoints. A whole-face gauge change must not erase them.
        graph.rotations[0][0] += 1;
        let before = fan_turns(&graph).unwrap();
        assert_eq!(before.len(), 4);
        let mut nonzero: Vec<_> = before
            .iter()
            .map(|(_, turn)| *turn)
            .filter(|&t| t != 0)
            .collect();
        nonzero.sort();
        assert_eq!(nonzero, vec![1, 3]);
        for (f, rotations) in graph.rotations.iter_mut().enumerate() {
            for r in rotations {
                *r = (*r + f as i32 + 1).rem_euclid(4);
            }
        }
        assert_eq!(before, fan_turns(&graph).unwrap());
    }
}

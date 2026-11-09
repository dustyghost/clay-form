//! Frozen local problems, retaining the exact boundary and coordinate locks.
use super::Snapshot;
use crate::{
    Result,
    coarse::Graph,
    coarse_sat::{Observer, PatchReport},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

#[derive(Serialize, Deserialize)]
pub struct RepairPatch {
    pub version: u32,
    pub depth: usize,
    pub radius: usize,
    pub source_faces: Vec<usize>,
    pub graph_edges: Vec<usize>,
    pub faces: Vec<[usize; 3]>,
    pub rotations: Vec<[i32; 3]>,
    pub preferred: Vec<u8>,
    pub domains: Vec<u16>,
    pub inversions: usize,
}
impl RepairPatch {
    pub fn solve(&self, millis: u64) -> Result<serde_json::Value> {
        self.solve_using(millis, false)
    }
    pub fn solve_scalar(&self, millis: u64) -> Result<serde_json::Value> {
        self.solve_using(millis, true)
    }
    fn solve_using(&self, millis: u64, scalar: bool) -> Result<serde_json::Value> {
        if self.version != 1
            || !(1..=30_000).contains(&millis)
            || self.faces.len() != self.rotations.len()
            || self.preferred.len() != self.domains.len()
            || self.preferred.iter().any(|&v| v > 8)
            || self.domains.iter().any(|&d| d == 0 || d & !0x1ff != 0)
            || self
                .faces
                .iter()
                .flatten()
                .any(|&v| v >= self.domains.len())
        {
            return Err("Invalid frozen repair patch or budget".into());
        }
        let rules: Vec<_> = self
            .faces
            .iter()
            .zip(&self.rotations)
            .map(|(&f, &r)| crate::coarse_sat::face_rule(f, r))
            .collect();
        let attempt = if scalar {
            crate::sat::solve_grid(
                &self.domains,
                &self.faces,
                &self.rotations,
                &self.preferred,
                &|| false,
                millis,
            )?
        } else {
            crate::sat::solve_reported(
                self.domains.clone(),
                &rules,
                &self.preferred,
                &|| false,
                millis,
            )?
        };
        Ok(serde_json::json!({"stats":attempt.stats,"solution":attempt.solution}))
    }
}
struct Export<'a> {
    directory: &'a Path,
    count: usize,
}
impl Observer for Export<'_> {
    fn problem(
        &mut self,
        report: &PatchReport,
        graph: &Graph,
        faces: &BTreeSet<usize>,
        edges: &BTreeSet<usize>,
        flexible: &BTreeSet<usize>,
    ) -> Result<bool> {
        let ids: BTreeMap<_, _> = edges.iter().enumerate().map(|(i, &e)| (e, i)).collect();
        let preferred: Vec<_> = edges
            .iter()
            .map(|&e| ((graph.offsets[e].x + 1) * 3 + graph.offsets[e].y + 1) as u8)
            .collect();
        let domains = edges
            .iter()
            .zip(&preferred)
            .map(|(&e, &p)| {
                if !flexible.contains(&e) {
                    return 1 << p;
                }
                (0..9)
                    .filter(|&v| {
                        (0..2).all(|k| {
                            graph.allow[e][k] != 0
                                || [v / 3 - 1, v % 3 - 1][k] == graph.offsets[e][k]
                        })
                    })
                    .fold(0, |d, v| d | (1 << v))
            })
            .collect();
        let patch = RepairPatch {
            version: 1,
            depth: report.depth,
            radius: report.radius,
            source_faces: faces.iter().map(|&f| graph.source_faces[f]).collect(),
            graph_edges: edges.iter().copied().collect(),
            faces: faces
                .iter()
                .map(|&f| graph.faces[f].map(|e| ids[&e]))
                .collect(),
            rotations: faces.iter().map(|&f| graph.rotations[f]).collect(),
            preferred,
            domains,
            inversions: report.inverted_source_faces_before.len(),
        };
        let path = self.directory.join(format!(
            "patch-{:03}-{}-folds.json",
            self.count, patch.inversions
        ));
        let mut writer = BufWriter::new(File::create_new(path).map_err(|e| e.to_string())?);
        serde_json::to_writer(&mut writer, &patch).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        self.count += 1;
        Ok(true)
    }
}
impl Snapshot {
    /// Audit-only global closure repair after requesting zero offsets on folded
    /// cells. Flow may alter surrounding cells; this is never accepted output.
    pub fn reflow_inversions(
        &mut self,
        seconds: f64,
        pin_requests: bool,
    ) -> Result<serde_json::Value> {
        use std::time::{Duration, Instant};
        if self.repaired
            || !seconds.is_finite()
            || !(0.0..=300.0).contains(&seconds)
            || seconds == 0.0
        {
            return Err("Reflow needs an unrepaired snapshot and a budget in (0, 300]".into());
        }
        let started = Instant::now();
        let deadline = started + Duration::from_secs_f64(seconds);
        let cancel = || Instant::now() >= deadline;
        let graph = Graph::from_lattice(&self.lattice)?;
        let inverted: BTreeSet<_> = (0..graph.faces.len())
            .filter(|&f| graph.area(f) < 0)
            .collect();
        let mut edges = BTreeSet::new();
        let initial: Vec<_> = self.lattice.edges.iter().map(|e| e.d).collect();
        let mut pinned = vec![false; 2 * initial.len()];
        let mut passes = Vec::new();
        for pass in 0..if pin_requests { 8 } else { 1 } {
            let current = Graph::from_lattice(&self.lattice)?;
            let before = self.lattice.flipped();
            if before == 0 {
                break;
            }
            edges.extend(
                (0..current.faces.len())
                    .filter(|&f| current.area(f) < 0)
                    .flat_map(|f| current.faces[f]),
            );
            for &id in &edges {
                let edge = &mut self.lattice.edges[id];
                for axis in 0..2 {
                    if edge.fixed_axis != Some(axis) {
                        edge.d[axis] = 0;
                        pinned[2 * id + axis] = true;
                    }
                }
            }
            let requested_defects = self.lattice.defects();
            let result = if pin_requests {
                self.lattice.correct_pinned(&pinned, &cancel)
            } else {
                self.lattice.correct(&cancel)
            };
            if let Err(error) = result {
                for (edge, value) in self.lattice.edges.iter_mut().zip(initial) {
                    edge.d = value;
                }
                return Ok(
                    serde_json::json!({"audit_only":true,"error":error,"rolled_back":true,"passes":passes,"seconds":started.elapsed().as_secs_f64()}),
                );
            }
            let after = self.lattice.flipped();
            passes.push(serde_json::json!({"pass":pass,"seconds":started.elapsed().as_secs_f64(),"before":before,"after":after,"requested_defects":requested_defects,"requested_edges":edges.len()}));
            if after > before {
                for (edge, value) in self.lattice.edges.iter_mut().zip(initial) {
                    edge.d = value;
                }
                return Ok(
                    serde_json::json!({"audit_only":true,"error":"Global reflow increased inversions","rolled_back":true,"passes":passes,"seconds":started.elapsed().as_secs_f64()}),
                );
            }
            if pin_requests
                && self
                    .lattice
                    .edges
                    .iter()
                    .enumerate()
                    .any(|(i, e)| (0..2).any(|a| pinned[2 * i + a] && e.d[a] != 0))
            {
                for (edge, value) in self.lattice.edges.iter_mut().zip(initial) {
                    edge.d = value;
                }
                return Err("Reflow changed a temporary lock; rolled back".into());
            }
        }
        let changed = self
            .lattice
            .edges
            .iter()
            .zip(&initial)
            .filter(|(e, v)| e.d != **v)
            .count();
        let changed_outside = self
            .lattice
            .edges
            .iter()
            .zip(&initial)
            .enumerate()
            .filter(|(i, (e, v))| !edges.contains(i) && e.d != **v)
            .count();
        let locks_changed = self
            .lattice
            .edges
            .iter()
            .zip(&initial)
            .filter(|(e, v)| e.fixed_axis.is_some_and(|a| e.d[a] != v[a]))
            .count();
        if locks_changed != 0 {
            for (edge, value) in self.lattice.edges.iter_mut().zip(initial) {
                edge.d = value;
            }
            return Err("Reflow changed a locked coordinate; rolled back".into());
        }
        let after = Graph::from_lattice(&self.lattice)?;
        let new_inversions: Vec<_> = (0..after.faces.len())
            .filter(|&f| after.area(f) < 0 && !inverted.contains(&f))
            .collect();
        let inversion_count = self.lattice.flipped();
        let long_edges = self.lattice.long_edges();
        self.lattice.subdivide(&mut self.field, 150_000, &cancel)?;
        self.diagnostics.samples = self.field.surface.positions.len();
        self.diagnostics.position_defects_after = self.lattice.defects();
        self.diagnostics.flipped_after = self.lattice.flipped();
        Ok(
            serde_json::json!({"audit_only":true,"error":null,"pin_requests":pin_requests,"passes":passes,"seconds":started.elapsed().as_secs_f64(),"inversions_before":inverted.len(),"requested_edges":edges.len(),"changed_edges":changed,"changed_outside_requested_edges":changed_outside,"locks_changed":locks_changed,"new_inverted_faces":new_inversions,"inversions_before_subdivision":inversion_count,"long_edges_before_subdivision":long_edges,"inversions_after_subdivision":self.lattice.flipped(),"closure_defects":self.lattice.defects()}),
        )
    }
    pub fn export_repair_patches(
        &mut self,
        directory: &Path,
        depth: usize,
        radius: usize,
    ) -> Result<usize> {
        if self.repaired || depth > 5 || !(1..=32).contains(&radius) {
            return Err("Use an unrepaired snapshot, depth 0..5 and radius 1..32".into());
        }
        fs::create_dir(directory).map_err(|e| e.to_string())?;
        crate::coarse::repair_lattice(&mut self.lattice, &|| false)?;
        let mut graph = Graph::from_lattice(&self.lattice)?;
        for _ in 0..depth {
            graph = graph.coarsen(&|| false)?.0;
        }
        let mut writer = BufWriter::new(
            File::create_new(directory.join("graph.json")).map_err(|e| e.to_string())?,
        );
        serde_json::to_writer(&mut writer, &graph).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        let mut observer = Export {
            directory,
            count: 0,
        };
        crate::coarse_sat::solve_patches(&mut graph, depth, radius, &|| false, &mut observer)?;
        Ok(observer.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Diagnostics, Surface, Vec3,
        field::{Constraint, Field},
        integer::Lattice,
    };
    use glam::IVec2;

    #[test]
    fn reflow_preserves_locks_source_and_unaccepted_state_and_rolls_back_cancellation() {
        let positions = vec![Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y];
        let field = Field {
            surface: Surface {
                positions: positions.clone(),
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            },
            normals: vec![Vec3::Z; 4],
            directions: vec![Vec3::X; 4],
            origins: positions.clone(),
            spacing: vec![1.; 4],
            areas: vec![1.; 4],
            constraints: vec![Constraint::default(); 4],
        };
        let mut lattice = Lattice::new(&field, &|| false).unwrap();
        let xy = [IVec2::ZERO, IVec2::X, -IVec2::Y, -IVec2::X];
        for edge in &mut lattice.edges {
            edge.d = xy[edge.ends[1]] - xy[edge.ends[0]];
            edge.fixed_axis = (0..2).find(|&a| edge.d[a] == 0);
        }
        let initial: Vec<_> = lattice.edges.iter().map(|e| e.d).collect();
        let mut snapshot = Snapshot {
            version: 1,
            repaired: false,
            source: field.surface.clone(),
            field,
            lattice,
            diagnostics: Diagnostics::default(),
        };
        assert!(snapshot.reflow_inversions(f64::NAN, true).is_err());
        let stopped = snapshot.reflow_inversions(f64::MIN_POSITIVE, true).unwrap();
        assert_eq!(stopped["rolled_back"], true);
        assert_eq!(
            snapshot
                .lattice
                .edges
                .iter()
                .map(|e| e.d)
                .collect::<Vec<_>>(),
            initial
        );
        let report = snapshot.reflow_inversions(2., true).unwrap();
        assert_eq!(report["error"], serde_json::Value::Null);
        assert_eq!(report["closure_defects"], 0);
        assert_eq!(report["locks_changed"], 0);
        assert_eq!(snapshot.lattice.flipped(), 0);
        assert!(!snapshot.repaired);
        assert_eq!(snapshot.source.positions, positions);
    }
}

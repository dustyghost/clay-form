//! Directed move welds keep stationary targets and surviving face-corner attributes.
use super::*;

impl Mesh {
    pub(crate) fn weld_moved_vertices(
        &self,
        pairs: &[[usize; 2]],
        protect: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<(Mesh, BTreeMap<usize, usize>), String> {
        manifold(self)?;
        let touched: Vec<_> = pairs.iter().flatten().copied().collect();
        self.protect_model_vertices(&touched, protect)?;
        let mut mapping = BTreeMap::new();
        for map in &symmetry.vertices {
            for &[source, target] in pairs {
                let (source, target) = (map[source], map[target]);
                if source == target
                    || mapping
                        .insert(source, target)
                        .is_some_and(|old| old != target)
                {
                    return Err("Symmetry gives this point conflicting weld targets".into());
                }
            }
        }
        if mapping.values().any(|v| mapping.contains_key(v)) {
            return Err("The weld target must remain stationary".into());
        }
        let touched: Vec<_> = mapping.iter().flat_map(|(&a, &b)| [a, b]).collect();
        self.protect_model_vertices(&touched, protect)?;
        let targets = mapping.values().copied().collect();
        self.finish_move_weld(self.clone(), &mapping, targets, protect, symmetry)
    }

    fn finish_move_weld(
        &self,
        mut out: Mesh,
        mapping: &BTreeMap<usize, usize>,
        targets: BTreeSet<usize>,
        protect: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<(Mesh, BTreeMap<usize, usize>), String> {
        if !self.uv.is_empty() && !self.uv.current(self) {
            return Err("Restore or clear the inactive UV map before welding".into());
        }
        let mapped = |v: usize| *mapping.get(&v).unwrap_or(&v);
        let edges = |values: &[[usize; 2]]| -> Vec<[usize; 2]> {
            values
                .iter()
                .filter_map(|&[a, b]| {
                    let (a, b) = (mapped(a), mapped(b));
                    (a != b).then_some(edge(a, b))
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        let mut records = Vec::new();
        for (face, &triangle) in self.triangles.iter().enumerate() {
            let t = triangle.map(mapped);
            let mut key = t;
            key.sort_unstable();
            if key.windows(2).any(|v| v[0] == v[1]) {
                continue;
            }
            records.push((t != triangle, face, t));
        }
        records.sort_by_key(|r| r.0);
        let first_changed = records.iter().take_while(|r| !r.0).count();
        out.triangles = records.iter().map(|r| r.2).collect();
        out.polygroups = records
            .iter()
            .map(|r| self.polygroups.get(r.1).copied().unwrap_or(0))
            .collect();
        out.hidden_faces = records
            .iter()
            .map(|r| self.hidden_faces.get(r.1).copied().unwrap_or(false))
            .collect();
        out.quads = self
            .quads
            .iter()
            .filter_map(|q| {
                let q = q.map(mapped);
                (q.into_iter().collect::<BTreeSet<_>>().len() == 4).then_some(q)
            })
            .collect();
        out.loose_edges = edges(&self.loose_edges);
        out.loose_vertices = self.loose_vertices.iter().copied().map(mapped).collect();
        out.retain_loose_vertices();
        let surface: BTreeSet<_> = out.polygon_edges().into_iter().collect();
        out.loose_edges.retain(|e| !surface.contains(e));
        if out.triangles.is_empty() && out.loose_edges.is_empty() && out.loose_vertices.is_empty() {
            return Err("This weld would remove the entire mesh".into());
        }
        out.creases = edges(&self.creases)
            .into_iter()
            .filter(|e| surface.contains(e))
            .collect();
        let shading: Vec<_> = edges(self.shading_edges())
            .into_iter()
            .filter(|e| surface.contains(e))
            .collect();
        if self.uv.current(self) {
            out.uv.topology = out.triangles.clone();
            out.uv.corners = records.iter().map(|r| self.uv.corners[r.1]).collect();
            out.uv.removed = records.iter().map(|r| !self.uv.mapped(r.1)).collect();
            if !self.uv.masks.is_empty() {
                out.uv.masks = records.iter().map(|r| self.uv.masks[r.1]).collect();
            }
            out.uv.seams = edges(&self.uv.seams)
                .into_iter()
                .filter(|e| surface.contains(e))
                .collect();
        }
        self.check_model_protection(&out, protect)?;
        // Compaction also reindexes materials, group weights, masks and UV seams.
        let used: BTreeSet<_> = out
            .triangles
            .iter()
            .flatten()
            .chain(out.loose_edges.iter().flatten())
            .chain(out.loose_vertices.iter())
            .copied()
            .collect();
        let indices: BTreeMap<_, _> = used.into_iter().enumerate().map(|(i, v)| (v, i)).collect();
        let targets: BTreeMap<_, _> = targets
            .iter()
            .filter_map(|v| indices.get(v).map(|&i| (*v, i)))
            .collect();
        if targets.is_empty() {
            return Err("This weld has no surviving target".into());
        }
        out.compact_model_vertices();
        out.set_shading_seams(shading.iter().map(|e| e.map(|v| indices[&v])).collect());
        manifold(&out)?;
        crate::sculpt::fusion::validate_added_faces(&out, first_changed, &|| false)
            .map_err(|_| "The weld would overlap another face".to_string())?;
        symmetry.validate_result(&out)?;
        Ok((out, targets))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::Symmetry;
    fn panels() -> Mesh {
        let mut mesh = Mesh {
            vertices: vec![
                Vec3(-1., -0.5, 0.),
                Vec3(-0.3, -0.5, 0.),
                Vec3(-0.3, 0.5, 0.),
                Vec3(-1., 0.5, 0.),
                Vec3(0.3, -0.5, 0.),
                Vec3(1., -0.5, 0.),
                Vec3(1., 0.5, 0.),
                Vec3(0.3, 0.5, 0.),
            ],
            mask: vec![0.; 8],
            ..Default::default()
        };
        mesh.push_polygon(&[0, 1, 2, 3], 0, false);
        mesh.push_polygon(&[4, 5, 6, 7], 0, false);
        mesh.uv.topology = mesh.triangles.clone();
        mesh.uv.corners = (0..4)
            .map(|i| [[i as f32, 0.], [1., 0.], [1., 1.]])
            .collect();
        mesh.uv.masks = vec![[0.2, 0.5, 0.8]; 4];
        mesh.uv.seams = vec![[1, 2], [4, 7]];
        mesh.creases = mesh.uv.seams.clone();
        mesh.set_shading_seams(mesh.creases.clone());
        mesh
    }
    #[test]
    fn move_snap_weld_reuses_target_and_combines_duplicate_edges() {
        let mesh = Mesh {
            vertices: vec![
                Vec3(-1., 0., 0.),
                Vec3(0., 0.1, 0.),
                Vec3(0., 0., 0.),
                Vec3(1., 0., 0.),
            ],
            mask: vec![0.; 4],
            loose_edges: vec![[0, 1], [0, 2], [2, 3]],
            ..Default::default()
        };
        let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
        let (out, indices) = mesh.weld_moved_vertices(&[[1, 2]], true, &sym).unwrap();
        assert_eq!(out.vertices.len(), 3);
        assert_eq!(out.vertices[indices[&2]], mesh.vertices[2]);
        assert_eq!(out.loose_edges, vec![[0, 1], [1, 2]]);
        assert!(out.triangles.is_empty());
    }
    #[test]
    fn move_snap_weld_rejects_pinched_surface_join_without_changing_attributes() {
        let mesh = panels();
        let before = mesh.clone();
        let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
        // One endpoint joins two disconnected surface fans. Joining both
        // endpoints instead produces the valid boundary seam tested below.
        let error = mesh.weld_moved_vertices(&[[1, 4]], true, &sym).unwrap_err();
        assert!(error.contains("connected surface fan"), "{error}");
        assert_eq!(mesh, before);
    }
    #[test]
    fn move_snap_weld_preserves_uvs_creases_and_stationary_edge() {
        let mesh = panels();
        let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
        let (out, indices) = mesh
            .weld_moved_vertices(&[[1, 4], [2, 7]], true, &sym)
            .unwrap();
        assert_eq!(out.vertices.len(), 6);
        assert_eq!(out.quads.len(), 2);
        assert_eq!(out.model_edges().len(), 7);
        assert_eq!(out.open_model_edges().len(), 6);
        let seam = edge(indices[&4], indices[&7]);
        assert_eq!(out.creases, vec![seam]);
        assert_eq!(out.uv.seams, vec![seam]);
        assert_eq!(out.shading_edges(), &[seam]);
        assert_eq!(
            out.uv.corners,
            vec![
                mesh.uv.corners[2],
                mesh.uv.corners[3],
                mesh.uv.corners[0],
                mesh.uv.corners[1]
            ]
        );
        assert_eq!(out.uv.masks, mesh.uv.masks);
        assert!(out.uv.current(&out));
        for (&old, &new) in &indices {
            assert_eq!(out.vertices[new], mesh.vertices[old]);
        }
        manifold(&out).unwrap();
    }
    #[test]
    fn move_snap_weld_rejects_protection_duplicate_faces_and_obstructions() {
        let mesh = panels();
        let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
        assert!(mesh.weld_moved_vertices(&[[1, 99]], true, &sym).is_err());
        assert!(
            mesh.weld_moved_vertices(&[[1, 4], [4, 1]], true, &sym)
                .is_err()
        );
        assert!(
            mesh.weld_moved_vertices(&[[0, 4], [1, 5], [2, 6], [3, 7]], true, &sym)
                .is_err()
        );
        for (mask, hidden) in [(true, false), (false, true)] {
            let mut protected = mesh.clone();
            if mask {
                protected.mask[4] = 1.;
            } else {
                protected.hidden_faces[2] = hidden;
            }
            assert!(
                protected
                    .weld_moved_vertices(&[[1, 4]], true, &sym)
                    .is_err()
            );
        }
        let mut obstructed = mesh.clone();
        obstructed.vertices.extend([
            Vec3(0., -0.4, -0.5),
            Vec3(0., -0.2, -0.5),
            Vec3(0., -0.3, 0.5),
        ]);
        obstructed.mask.resize(11, 0.);
        obstructed.push_polygon(&[8, 9, 10], 0, false);
        obstructed.uv = Default::default();
        let sym = ModelingSymmetry::new(&obstructed, &Default::default()).unwrap();
        assert!(
            obstructed
                .weld_moved_vertices(&[[1, 4], [2, 7]], true, &sym)
                .unwrap_err()
                .contains("overlap")
        );
    }
    #[test]
    fn move_snap_weld_applies_exact_symmetry_atomically() {
        let mesh = Mesh {
            vertices: vec![
                Vec3(-2., 0., 0.),
                Vec3(-1., 0., 0.),
                Vec3(-1., 0.2, 0.),
                Vec3(-2., 0.2, 0.),
                Vec3(2., 0., 0.),
                Vec3(1., 0., 0.),
                Vec3(1., 0.2, 0.),
                Vec3(2., 0.2, 0.),
            ],
            mask: vec![0.; 8],
            loose_edges: vec![[0, 1], [2, 3], [4, 5], [6, 7]],
            ..Default::default()
        };
        let settings = Symmetry {
            enabled: true,
            mirror: [true, false, false],
            ..Default::default()
        };
        let sym = ModelingSymmetry::new(&mesh, &settings).unwrap();
        let (out, indices) = mesh.weld_moved_vertices(&[[1, 2]], true, &sym).unwrap();
        assert_eq!(out.vertices.len(), 6);
        assert_eq!(indices.len(), 2);
        sym.validate_result(&out).unwrap();
        let mut masked = mesh.clone();
        masked.mask[6] = 1.;
        assert!(masked.weld_moved_vertices(&[[1, 2]], true, &sym).is_err());
    }
}

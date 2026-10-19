//! Transactional transforms of the active model and stored levels.
use super::*;
use crate::sculpt::{Primitive, Transform, Vec3};

enum Transformed {
    Primitive(Box<(Mesh, Primitive)>),
    Levels(Vec<Vec<Vec3>>),
}
impl Document {
    /// Prepare only changed coordinates; topology and inactive models remain in
    /// place. Validation finishes before mutation, and the first changed update
    /// captures the entire scene once for undo.
    pub fn apply_transform(
        &mut self,
        transform: Transform,
        respect_mask: bool,
    ) -> Result<bool, String> {
        transform.validate()?;
        if transform.is_identity() {
            return Ok(false);
        }
        if !self.active_visible() {
            return Err("Show the active submodel before transforming it".into());
        }
        self.end_edit();
        let update = if let Some(primitive) = self.primitive() {
            let transformed = primitive.transformed(transform)?;
            let mesh = transformed.mesh()?;
            if &transformed == primitive && mesh == self.geometry.mesh {
                return Ok(false);
            }
            Transformed::Primitive(Box::new((mesh, transformed)))
        } else {
            let mut changed = false;
            let mut levels = Vec::new();
            for mesh in std::iter::once(&self.geometry.mesh)
                .chain(self.geometry.settings.lower.iter())
                .chain(self.geometry.settings.upper.iter())
            {
                let mut vertices = Vec::with_capacity(mesh.vertices.len());
                for (&point, &mask) in mesh.vertices.iter().zip(&mesh.mask) {
                    let mobility = if respect_mask { 1. - mask } else { 1. };
                    let next = transform.weighted_point(point, mobility);
                    if !next.0.is_finite() || !next.1.is_finite() || !next.2.is_finite() {
                        return Err("Transform would produce invalid vertex coordinates".into());
                    }
                    changed |= next != point;
                    vertices.push(next);
                }
                levels.push(vertices);
            }
            if !changed {
                return Ok(false);
            }
            Transformed::Levels(levels)
        };
        self.begin_stroke();
        if let Some(before) = self.stroke_before.take() {
            self.record_edit_owned(before, "Transform submodel");
        }
        match update {
            Transformed::Primitive(update) => {
                let (mesh, primitive) = *update;
                self.geometry.mesh = mesh;
                self.geometry.settings.primitive = Some(primitive);
            }
            Transformed::Levels(levels) => {
                for (mesh, vertices) in std::iter::once(&mut self.geometry.mesh)
                    .chain(self.geometry.settings.lower.iter_mut())
                    .chain(self.geometry.settings.upper.iter_mut())
                    .zip(levels)
                {
                    mesh.vertices = vertices;
                }
            }
        }
        self.dirty = true;
        self.display_changed();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::PrimitiveKind;
    #[test]
    fn coordinate_updates_keep_topology_and_inactive_storage_and_undo_once() {
        let primitive = Primitive::for_kind(PrimitiveKind::Plane);
        let mut geometry = Geometry::new(primitive.mesh().unwrap());
        geometry.subdivide().unwrap();
        let mut doc = Document::default();
        doc.replace_geometry(geometry).unwrap();
        doc.duplicate_model(false).unwrap();
        let before = doc.geometry_snapshot();
        let count = doc.history_counts().0;
        let topology = doc.mesh().triangles.as_ptr();
        let inactive = doc.model(1).unwrap().mesh.vertices.as_ptr();
        for _ in 0..3 {
            doc.apply_transform(
                Transform {
                    translation: Vec3(0.1, 0., 0.),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
            assert_eq!(doc.mesh().triangles.as_ptr(), topology);
            assert_eq!(doc.model(1).unwrap().mesh.vertices.as_ptr(), inactive);
        }
        assert_eq!(doc.history_counts().0, count + 1);
        let after = doc.geometry_snapshot();
        assert_eq!(after.submodels, before.submodels);
        assert_eq!(after.mesh.triangles, before.mesh.triangles);
        assert_eq!(after.mesh.mask, before.mesh.mask);
        for (mesh, old) in std::iter::once(&after.mesh)
            .chain(after.settings.lower.iter())
            .zip(std::iter::once(&before.mesh).chain(before.settings.lower.iter()))
        {
            for (&p, &q) in mesh.vertices.iter().zip(&old.vertices) {
                assert!((p - q - Vec3(0.3, 0., 0.)).len() < 1e-6);
            }
        }
        doc.end_stroke();
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), after);
    }
    #[test]
    fn invalid_upper_level_coordinates_reject_the_entire_transform() {
        let primitive = Primitive::for_kind(PrimitiveKind::Plane);
        let mut geometry = Geometry::new(primitive.mesh().unwrap());
        geometry.subdivide().unwrap();
        geometry.set_level(1).unwrap();
        geometry.settings.upper[0].vertices.last_mut().unwrap().0 = f32::MAX;
        let mut doc = Document::default();
        doc.replace_geometry(geometry).unwrap();
        doc.mark_saved("transform.clay");
        let before = doc.geometry_snapshot();
        let history = doc.history_counts();
        assert!(
            doc.apply_transform(
                Transform {
                    scale: Vec3(2., 1., 1.),
                    ..Default::default()
                },
                false
            )
            .is_err()
        );
        assert_eq!(doc.geometry_snapshot(), before);
        assert_eq!(doc.history_counts(), history);
        assert!(!doc.is_dirty());
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    use crate::sculpt::PrimitiveKind;
    use std::f32::consts::FRAC_PI_4;
    #[test]
    fn rotation_keeps_all_levels_masks_groups_visibility_and_inactive_models_with_one_undo() {
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        mesh.mask = (0..mesh.vertices.len())
            .map(|i| [0., 0.5, 1.][i % 3])
            .collect();
        mesh.create_vertex_group("weights".into(), 0.1, false)
            .unwrap();
        mesh.hidden_faces = vec![true; mesh.triangles.len()];
        let mut geometry = Geometry::new(mesh);
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.set_level(2).unwrap();
        let mut doc = Document::default();
        doc.replace_geometry(geometry).unwrap();
        doc.duplicate_model(false).unwrap();
        let before = doc.geometry_snapshot();
        let count = doc.history_counts().0;
        for _ in 0..2 {
            doc.apply_transform(
                Transform {
                    rotation: Some((Vec3(0., 0., 1.), FRAC_PI_4)),
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        }
        doc.end_stroke();
        assert_eq!(doc.history_counts().0, count + 1);
        let after = doc.geometry_snapshot();
        let mut attributes = after.clone();
        for (mesh, original) in std::iter::once(&mut attributes.mesh)
            .chain(attributes.settings.lower.iter_mut())
            .chain(attributes.settings.upper.iter_mut())
            .zip(
                std::iter::once(&before.mesh)
                    .chain(before.settings.lower.iter())
                    .chain(before.settings.upper.iter()),
            )
        {
            for ((&p, &q), &mask) in mesh
                .vertices
                .iter()
                .zip(&original.vertices)
                .zip(&original.mask)
            {
                let angle = 2. * FRAC_PI_4 * (1. - mask);
                let expected = Vec3(
                    q.0 * angle.cos() - q.1 * angle.sin(),
                    q.0 * angle.sin() + q.1 * angle.cos(),
                    q.2,
                );
                assert!((p - expected).len() < 1e-5);
            }
            mesh.vertices.clone_from(&original.vertices);
        }
        assert_eq!(attributes, before);
        doc.mesh_settings().validate(doc.mesh()).unwrap();
        let mut loaded = Document::default();
        loaded
            .replace_project(
                serde_json::from_slice(&serde_json::to_vec(&doc.project()).unwrap()).unwrap(),
                "rotation.clay",
            )
            .unwrap();
        assert_eq!(loaded.geometry_snapshot(), after);
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), after);
    }
    #[test]
    fn fully_masked_and_invalid_rotations_preserve_redo_and_clean_state() {
        let mut doc = Document::default();
        doc.invert_mask().unwrap();
        doc.clear_mask();
        doc.undo();
        doc.mark_saved("rotation.clay");
        let before = doc.geometry_snapshot();
        let history = doc.history_counts();
        assert!(
            !doc.apply_transform(
                Transform {
                    rotation: Some((Vec3(0., 1., 0.), 1.)),
                    ..Default::default()
                },
                true
            )
            .unwrap()
        );
        assert!(
            doc.apply_transform(
                Transform {
                    rotation: Some((Vec3::default(), 1.)),
                    ..Default::default()
                },
                false
            )
            .is_err()
        );
        assert_eq!(doc.geometry_snapshot(), before);
        assert_eq!(doc.history_counts(), history);
        assert!(!doc.is_dirty());
    }
}

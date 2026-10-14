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
                    let next = if mobility == 0. {
                        point
                    } else {
                        point + (transform.point(point) - point) * mobility
                    };
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
            self.history.record_owned(before, "Transform submodel");
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

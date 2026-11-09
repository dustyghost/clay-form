use super::*;
impl Document {
    pub(crate) fn paint_retopology_density(
        &mut self,
        brush: &Brush,
        dab: Dab,
        mode: crate::sculpt::retopology::DensityPaint,
    ) -> Result<bool, String> {
        self.require_retopology_target()?;
        for mesh in std::iter::once(&self.geometry.mesh)
            .chain(&self.geometry.settings.lower)
            .chain(&self.geometry.settings.upper)
        {
            mesh.require_density_channels()?;
        }
        self.begin_stroke();
        let dabs = self.symmetry.dabs(dab);
        let changed = self
            .geometry
            .mesh
            .paint_retopology_density(brush, &dabs, mode)?;
        if changed {
            if let Some(before) = self.stroke_before.take() {
                self.history
                    .record_owned(before, "Retopology density stroke", None);
            }
            // Use the same spatial dabs on stored levels; geometry never changes.
            for mesh in self
                .geometry
                .settings
                .lower
                .iter_mut()
                .chain(&mut self.geometry.settings.upper)
            {
                mesh.ensure_density_channels()?;
                mesh.paint_retopology_density(brush, &dabs, mode)?;
            }
            self.dirty = true;
            self.display_changed();
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{
        Alpha, GroupPurpose, Vec3,
        retopology::{DensityPaint, tests::fixture},
    };
    #[test]
    fn retopology_density_undo_save_subdivision_and_group_all_preserve_independence() {
        let (mesh, _) = fixture(true);
        let mut geometry = Geometry::new(mesh);
        geometry.subdivide().unwrap();
        let mut document = Document::default();
        document.replace_geometry(geometry).unwrap();
        let before = document.project();
        let i = 320;
        let normal = document.mesh().normals()[i];
        let mut brush = Brush::new(Kind::Mask);
        brush.alpha = Alpha::Hard;
        brush.strength = 1.;
        brush.pressure = false;
        let dab = Dab {
            center: document.mesh().vertices[i],
            normal,
            right: Vec3(0., 1., 0.),
            up: normal.cross(Vec3(0., 1., 0.)),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        document
            .paint_retopology_density(&brush, dab, DensityPaint::More)
            .unwrap();
        document.end_stroke();
        assert_eq!(document.undo_label(), Some("Retopology density stroke"));
        assert_eq!(document.mesh().mask, before.mesh.mask);
        assert_eq!(document.mesh().vertices, before.mesh.vertices);
        assert_eq!(document.mesh().retopology_detail()[i], 1.);
        document.mesh_settings().validate(document.mesh()).unwrap();
        let saved = document.project();
        let mut reopened = Document::default();
        reopened
            .replace_project(
                serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap(),
                "density.clay",
            )
            .unwrap();
        assert!(reopened.project() == saved);
        document.undo();
        assert!(document.project() == before);
        document.redo();
        assert!(document.project() == saved);
        let density = document.mesh().retopology_detail();
        document
            .create_vertex_group("New selection".into(), true)
            .unwrap();
        assert_eq!(document.mesh().retopology_detail(), density);
        assert_eq!(
            document
                .mesh()
                .vertex_groups
                .iter()
                .filter(|g| g.purpose == GroupPurpose::Selection)
                .count(),
            1
        );
    }
}

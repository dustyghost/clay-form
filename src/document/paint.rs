use super::*;

impl Document {
    pub fn fill_material(&mut self, brush: &Brush) -> Result<(), String> {
        if brush.kind != Kind::Paint {
            return Err("Select a Paint brush before filling materials".into());
        }
        if !self.active_visible() {
            return Err("Show the active model before painting".into());
        }
        crate::sculpt::validate_brush(brush)?;
        self.edit_group_meshes("Fill material", |mesh| {
            mesh.fill_material(brush);
            Ok(())
        })
    }
    pub(crate) fn paint_stroke_active(&self) -> bool {
        self.paint_stroke.is_some()
    }
    pub(super) fn apply_paint_dab(
        &mut self,
        brush: &Brush,
        dab: Dab,
        stencil: Option<&crate::sculpt::alpha::Projection>,
    ) -> bool {
        if self.paint_stroke.is_none() {
            self.end_stroke();
            let dirty = self.dirty;
            self.begin_stroke();
            let mut settings = self.capture_edit_settings();
            settings.active_brush = Some(brush.clone());
            self.paint_stroke = Some((dirty, settings));
        }
        let dabs = self.symmetry.dabs(dab);
        let stencils = stencil.map_or_else(Vec::new, |s| s.copies(&self.symmetry));
        let changed = self.geometry.mesh.paint_material(brush, &dabs, &stencils);
        if changed {
            self.dirty = true;
            self.display_changed();
        }
        changed
    }
    pub(super) fn finish_paint_stroke(&mut self) {
        if let Some((_, settings)) = self.paint_stroke.take()
            && let Some(before) = self.stroke_before.take()
            && before != self.geometry
        {
            self.history
                .record_owned(before, "Paint material", Some(settings));
        }
    }
    pub(crate) fn cancel_paint_stroke(&mut self) -> bool {
        let Some((dirty, _)) = self.paint_stroke.take() else {
            return false;
        };
        if let Some(before) = self.stroke_before.take() {
            self.geometry = before;
            self.display_changed();
        }
        self.dirty = dirty;
        self.end_stroke();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Material, Vec3};
    #[test]
    fn material_strokes_history_cancel_save_fill_and_level_switching() {
        let mut doc = Document::default();
        doc.set_symmetry(Symmetry {
            enabled: false,
            ..Default::default()
        })
        .unwrap();
        let before = doc.geometry_snapshot();
        let history = doc.history_counts();
        let brush = Brush::new(Kind::Paint);
        let dab = Dab {
            center: Vec3(0., 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.5,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        assert!(doc.apply_dab(&brush, dab));
        assert!(doc.apply_dab(&brush, dab));
        assert_eq!(doc.history_counts(), history, "history waits until release");
        assert!(
            !doc.end_stroke(),
            "material-only edits must not request remeshing"
        );
        let painted = doc.geometry_snapshot();
        assert_eq!(painted.mesh.vertices, before.mesh.vertices);
        assert_eq!(doc.history_counts().0, history.0 + 1);
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        let undone = doc.history_counts();
        assert!(doc.apply_dab(&brush, dab));
        assert!(doc.cancel_paint_stroke());
        assert_eq!(doc.history_counts(), undone);
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), painted);
        let file = serde_json::to_string(&doc.project()).unwrap();
        let project: Project = serde_json::from_str(&file).unwrap();
        project.validate().unwrap();
        let mut reopened = Document::default();
        reopened.replace_project(project, "paint.clay").unwrap();
        assert_eq!(reopened.mesh(), doc.mesh());
        let mut geometry = doc.geometry_snapshot();
        geometry.subdivide().unwrap();
        doc.replace_geometry(geometry).unwrap();
        let mut fill = Brush::new(Kind::Paint);
        fill.material = Material::examples()[4].1;
        fill.strength = 1.;
        doc.fill_material(&fill).unwrap();
        assert!(doc.mesh().materials.iter().all(|m| *m == fill.material));
        let history = doc.history_counts();
        doc.fill_material(&fill).unwrap();
        assert_eq!(history, doc.history_counts(), "same fill is a no-op");
        let mut geometry = doc.geometry_snapshot();
        geometry.set_level(1).unwrap();
        assert!(geometry.mesh.materials.iter().all(|m| *m == fill.material));
        geometry.set_level(2).unwrap();
        assert!(geometry.mesh.materials.iter().all(|m| *m == fill.material));
    }
}

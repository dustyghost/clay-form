//! Owns persistent content and mesh edits. No UI, window, or tablet dependencies.
use crate::{
    history::History,
    sculpt::{
        AdaptiveBudget, AdaptiveSettings, Alpha, Brush, Dab, Geometry, Kind, Mesh, MeshSettings,
        Project, Symmetry, validate_brush,
    },
};
pub struct Document {
    geometry: Geometry,
    brushes: Vec<Brush>,
    history: History,
    symmetry: Symmetry,
    dirty: bool,
    stroke_active: bool,
    stroke_deformed: bool,
    stroke_before: Option<Geometry>,
    current_path: String,
    adaptive_budget: AdaptiveBudget,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            geometry: Geometry::new(Mesh::sphere()),
            brushes: Kind::ALL.into_iter().map(Brush::new).collect(),
            history: History::default(),
            symmetry: Symmetry::default(),
            dirty: false,
            stroke_active: false,
            stroke_deformed: false,
            stroke_before: None,
            current_path: "sculpt.clay".into(),
            adaptive_budget: AdaptiveBudget::default(),
        }
    }
}
impl Document {
    pub fn mesh(&self) -> &Mesh {
        &self.geometry.mesh
    }
    pub fn mesh_settings(&self) -> &MeshSettings {
        &self.geometry.settings
    }
    pub(crate) fn geometry_snapshot(&self) -> Geometry {
        self.geometry.clone()
    }
    #[cfg(test)]
    pub(crate) fn replace_geometry(&mut self, geometry: Geometry) -> Result<(), String> {
        self.replace_geometry_named(geometry, "Mesh change")
    }
    pub(crate) fn replace_geometry_named(
        &mut self,
        geometry: Geometry,
        label: &'static str,
    ) -> Result<(), String> {
        geometry.settings.validate(&geometry.mesh)?;
        self.end_stroke();
        self.history.record_named(&self.geometry, label);
        self.geometry = geometry;
        self.dirty = true;
        Ok(())
    }
    pub fn set_mesh_level(&mut self, level: usize) -> Result<(), String> {
        if level == self.geometry.settings.level() {
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        geometry.set_level(level)?;
        self.replace_geometry_named(geometry, "Change subdivision level")
    }
    /// Keep the active surface and the opposite side of the subdivision stack.
    pub fn delete_mesh_levels(&mut self, lower: bool) -> Result<(), String> {
        let levels = if lower {
            &self.geometry.settings.lower
        } else {
            &self.geometry.settings.upper
        };
        if levels.is_empty() {
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        if lower {
            geometry.settings.lower.clear();
        } else {
            geometry.settings.upper.clear();
        }
        self.replace_geometry_named(
            geometry,
            if lower {
                "Delete lower levels"
            } else {
                "Delete higher levels"
            },
        )
    }
    pub fn set_dynamic_mesh(&mut self, enabled: bool) -> Result<(), String> {
        if self.geometry.settings.dynamic == enabled && !self.geometry.settings.adaptive.enabled {
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        geometry.set_dynamic(enabled);
        self.replace_geometry_named(geometry, "Switch mesh mode")
    }
    pub fn set_adaptive(&mut self, settings: AdaptiveSettings) -> Result<(), String> {
        settings.validate()?;
        if self.geometry.settings.adaptive == settings {
            return Ok(());
        }
        if self.geometry.settings.adaptive.enabled == settings.enabled {
            self.end_stroke();
            self.geometry.settings.adaptive = settings;
            self.dirty = true;
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        if settings.enabled && !geometry.settings.adaptive.enabled {
            geometry.settings.lower.clear();
            geometry.settings.upper.clear();
            geometry.settings.dynamic = false;
        }
        geometry.settings.adaptive = settings;
        self.replace_geometry_named(geometry, "Adaptive sculpt settings")
    }
    pub(crate) fn begin_sculpt_frame(&mut self) {
        self.adaptive_budget = AdaptiveBudget::default();
    }
    pub fn set_remesh_settings(&mut self, resolution: u32, auto: bool) -> Result<(), String> {
        if !(16..=96).contains(&resolution) {
            return Err("Remesh resolution must be between 16 and 96".into());
        }
        if self.geometry.settings.resolution != resolution
            || self.geometry.settings.auto_remesh != auto
        {
            self.geometry.settings.resolution = resolution;
            self.geometry.settings.auto_remesh = auto;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn brushes(&self) -> &[Brush] {
        &self.brushes
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn current_path(&self) -> &str {
        &self.current_path
    }
    pub fn undo_label(&self) -> Option<&'static str> {
        self.history.undo_label()
    }
    pub fn redo_label(&self) -> Option<&'static str> {
        self.history.redo_label()
    }
    pub fn history_counts(&self) -> (usize, usize) {
        self.history.counts()
    }
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    pub fn project(&self) -> Project {
        Project {
            version: Project::VERSION,
            mesh: self.geometry.mesh.clone(),
            brushes: self.brushes.clone(),
            symmetry: self.symmetry.clone(),
            mesh_settings: self.geometry.settings.clone(),
        }
    }
    pub fn mark_saved(&mut self, path: &str) {
        self.current_path = path.into();
        self.dirty = false;
    }
    pub fn new_sphere(&mut self) {
        self.geometry = Geometry::new(Mesh::sphere());
        self.history = History::default();
        self.dirty = false;
        self.end_stroke();
        self.current_path = "untitled.clay".into();
    }
    /// Validate before changing anything, so an invalid open leaves the current document intact.
    pub fn replace_project(&mut self, project: Project, path: &str) -> Result<(), String> {
        project.validate()?;
        self.geometry = Geometry {
            mesh: project.mesh,
            settings: project.mesh_settings,
        };
        self.brushes = project.brushes;
        self.symmetry = project.symmetry;
        self.history = History::default();
        self.end_stroke();
        self.mark_saved(path);
        Ok(())
    }
    pub fn begin_stroke(&mut self) {
        if !self.stroke_active {
            self.stroke_before = Some(self.geometry.clone());
            self.stroke_active = true;
            self.stroke_deformed = false;
            self.adaptive_budget = AdaptiveBudget::default();
        }
    }
    pub fn end_stroke(&mut self) -> bool {
        let deformed = self.stroke_active && self.stroke_deformed;
        self.stroke_active = false;
        self.stroke_deformed = false;
        self.stroke_before = None;
        deformed
    }
    pub fn symmetry(&self) -> &Symmetry {
        &self.symmetry
    }
    pub fn set_symmetry(&mut self, symmetry: Symmetry) -> Result<(), String> {
        symmetry.validate()?;
        if self.symmetry != symmetry {
            self.symmetry = symmetry;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn apply_dab(&mut self, brush: &Brush, dab: Dab) -> bool {
        self.begin_stroke();
        let dabs = self.symmetry.dabs(dab);
        let settings = &self.geometry.settings.adaptive;
        let before_topology =
            self.geometry
                .mesh
                .adapt(brush, &dabs, settings, &mut self.adaptive_budget, true);
        let old_mask = (brush.kind == Kind::Mask).then(|| self.geometry.mesh.mask.clone());
        let deformed = self.geometry.mesh.stamp_many(brush, &dabs);
        let mut after_dabs = dabs.clone();
        if brush.kind == Kind::Snake {
            for dab in &mut after_dabs {
                dab.center = dab.center + brush.snake_motion(*dab);
            }
        }
        let after_topology = self.geometry.mesh.adapt(
            brush,
            &after_dabs,
            settings,
            &mut self.adaptive_budget,
            false,
        );
        self.stroke_deformed |= deformed || before_topology || after_topology;
        if deformed
            || before_topology
            || after_topology
            || old_mask.is_some_and(|mask| mask != self.geometry.mesh.mask)
        {
            if let Some(before) = self.stroke_before.take() {
                self.history.record_owned(
                    before,
                    if settings.enabled {
                        "Adaptive brush stroke"
                    } else if brush.kind == Kind::Snake {
                        "Snake stroke"
                    } else {
                        "Brush stroke"
                    },
                );
            }
            self.dirty = true;
        }
        deformed
    }
    pub fn undo(&mut self) {
        self.end_stroke();
        if self.history.undo(&mut self.geometry) {
            self.dirty = true;
        }
    }
    pub fn redo(&mut self) {
        self.end_stroke();
        if self.history.redo(&mut self.geometry) {
            self.dirty = true;
        }
    }
    pub fn clear_mask(&mut self) {
        self.end_stroke();
        self.history.record_named(&self.geometry, "Clear mask");
        self.geometry.mesh.mask.fill(0.);
        for level in self
            .geometry
            .settings
            .lower
            .iter_mut()
            .chain(self.geometry.settings.upper.iter_mut())
        {
            level.mask.fill(0.);
        }
        self.dirty = true;
    }
    pub fn update_brush(&mut self, index: usize, brush: Brush) -> Result<(), String> {
        validate_brush(&brush)?;
        let target = self
            .brushes
            .get_mut(index)
            .ok_or("Brush no longer exists")?;
        if *target != brush {
            *target = brush;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn set_alpha(&mut self, index: usize, alpha: Alpha) -> Result<(), String> {
        let mut brush = self
            .brushes
            .get(index)
            .ok_or("Brush no longer exists")?
            .clone();
        brush.alpha = alpha;
        self.update_brush(index, brush)
    }
    pub fn append_brush(&mut self, brush: Brush) -> Result<usize, String> {
        validate_brush(&brush)?;
        if self.brushes.len() >= 256 {
            return Err("Brush library is limited to 256 presets".into());
        }
        self.brushes.push(brush);
        self.dirty = true;
        Ok(self.brushes.len() - 1)
    }
    pub fn duplicate_brush(&mut self, index: usize) -> Result<usize, String> {
        let mut brush = self
            .brushes
            .get(index)
            .ok_or("Brush no longer exists")?
            .clone();
        brush.name.push_str(" copy");
        self.append_brush(brush)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::Vec3;
    #[test]
    fn whole_stroke_is_one_undo_step_and_new_edits_clear_redo() {
        let mut document = Document::default();
        let original = document.geometry.mesh.vertices[0].1;
        let brush = document.brushes[0].clone();
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        document.apply_dab(&brush, dab);
        document.apply_dab(&brush, dab);
        document.end_stroke();
        let sculpted = document.geometry.mesh.vertices[0].1;
        assert!(sculpted > original);
        document.undo();
        assert_eq!(document.geometry.mesh.vertices[0].1, original);
        assert!(!document.can_undo());
        document.redo();
        assert_eq!(document.geometry.mesh.vertices[0].1, sculpted);
        document.undo();
        document.apply_dab(&brush, dab);
        assert!(!document.can_redo());
    }
    #[test]
    fn invalid_project_does_not_replace_content_or_history() {
        let mut document = Document::default();
        document.clear_mask();
        let original = serde_json::to_value(document.project()).unwrap();
        let mut invalid = document.project();
        invalid.mesh.mask.clear();
        assert!(document.replace_project(invalid, "invalid.clay").is_err());
        assert_eq!(serde_json::to_value(document.project()).unwrap(), original);
        assert!(document.is_dirty());
        assert!(document.can_undo());
        assert_eq!(document.current_path(), "sculpt.clay");
    }
}

#[cfg(test)]
mod mesh_tests {
    use super::*;
    #[test]
    fn topology_changes_restore_the_entire_stack_with_undo_and_project_roundtrip() {
        let mut document = Document::default();
        let original = document.mesh().clone();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document.replace_geometry(geometry).unwrap();
        assert_eq!(document.mesh_settings().level(), 2);
        document.set_mesh_level(1).unwrap();
        let project = document.project();
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
        loaded.validate().unwrap();
        let mut restored = Document::default();
        restored.replace_project(loaded, "roundtrip.clay").unwrap();
        restored.set_mesh_level(2).unwrap();
        assert_eq!(
            restored.mesh().triangles.len(),
            original.triangles.len() * 4
        );
        document.set_dynamic_mesh(true).unwrap();
        assert_eq!(document.mesh_settings().levels(), 1);
        document.undo();
        assert!(!document.mesh_settings().dynamic);
        assert_eq!(document.mesh_settings().levels(), 2);
        assert_eq!(document.mesh_settings().level(), 1);
        document.undo();
        assert_eq!(document.mesh_settings().level(), 2);
        document.undo();
        assert_eq!(document.mesh(), &original);
        assert_eq!(document.mesh_settings().levels(), 1);
        document.redo();
        assert_eq!(document.mesh_settings().levels(), 2);
    }
    #[test]
    fn invalid_stack_is_transactional_and_mask_strokes_do_not_request_remeshing() {
        let mut document = Document::default();
        let original = document.geometry_snapshot();
        let mut invalid = original.clone();
        invalid.settings.lower.push(original.mesh.clone());
        assert!(document.replace_geometry(invalid).is_err());
        assert_eq!(document.geometry_snapshot(), original);
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
        let brush = Brush::new(Kind::Mask);
        document.apply_dab(
            &brush,
            Dab {
                center: crate::sculpt::Vec3(0., 1., 0.),
                normal: crate::sculpt::Vec3(0., 1., 0.),
                right: crate::sculpt::Vec3(1., 0., 0.),
                up: crate::sculpt::Vec3(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: crate::sculpt::Vec3::default(),
                invert: false,
            },
        );
        assert!(!document.end_stroke());
        assert!(document.mesh().mask[0] > 0.);
    }
}

#[cfg(test)]
mod protected_stroke_tests {
    use super::*;
    #[test]
    fn black_alpha_and_fully_masked_strokes_do_not_trigger_auto_remesh() {
        let mut document = Document::default();
        let dab = Dab {
            center: crate::sculpt::Vec3(0., 1., 0.),
            normal: crate::sculpt::Vec3(0., 1., 0.),
            right: crate::sculpt::Vec3(1., 0., 0.),
            up: crate::sculpt::Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: crate::sculpt::Vec3::default(),
            invert: false,
        };
        let mut brush = Brush::new(Kind::Standard);
        brush.alpha = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.].into(),
        };
        document.apply_dab(&brush, dab);
        assert!(!document.end_stroke());
        brush.alpha = Alpha::Soft;
        document.geometry.mesh.mask.fill(1.);
        document.apply_dab(&brush, dab);
        assert!(!document.end_stroke());
    }
}

#[cfg(test)]
mod delete_level_tests {
    use super::*;
    #[test]
    fn deleting_either_side_preserves_active_surface_and_restores_stack_on_undo() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.mesh.vertices.last_mut().unwrap().0 += 0.1;
        *geometry.mesh.mask.last_mut().unwrap() = 0.6;
        geometry.set_level(2).unwrap();
        document.replace_geometry(geometry).unwrap();
        let original = document.geometry_snapshot();
        document.delete_mesh_levels(true).unwrap();
        assert_eq!(document.mesh(), &original.mesh);
        assert_eq!(document.mesh_settings().level(), 1);
        assert_eq!(document.mesh_settings().upper, original.settings.upper);
        document.set_mesh_level(2).unwrap();
        document.mesh_settings().validate(document.mesh()).unwrap();
        document.undo();
        document.undo();
        assert_eq!(document.geometry_snapshot(), original);
        document.delete_mesh_levels(false).unwrap();
        assert_eq!(document.mesh(), &original.mesh);
        assert_eq!(document.mesh_settings().level(), 2);
        assert_eq!(document.mesh_settings().levels(), 2);
        assert_eq!(document.mesh_settings().lower, original.settings.lower);
        let mut fresh = document.geometry_snapshot();
        fresh.subdivide().unwrap();
        fresh.settings.validate(&fresh.mesh).unwrap();
        document.undo();
        assert_eq!(document.geometry_snapshot(), original);
        document.redo();
        assert!(document.mesh_settings().upper.is_empty());
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(&document.project()).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.mesh_settings, document.mesh_settings().clone());
    }
    #[test]
    fn deleting_absent_levels_is_a_clean_no_op() {
        let mut document = Document::default();
        document.delete_mesh_levels(true).unwrap();
        document.delete_mesh_levels(false).unwrap();
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
    }
}

#[cfg(test)]
mod history_feedback_tests {
    use super::*;
    #[test]
    fn no_effect_strokes_keep_redo_and_each_mesh_edit_has_a_history_label() {
        let mut document = Document::default();
        document.set_dynamic_mesh(true).unwrap();
        document.undo();
        assert_eq!(document.redo_label(), Some("Switch mesh mode"));
        let mut brush = Brush::new(Kind::Standard);
        brush.alpha = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.].into(),
        };
        document.apply_dab(
            &brush,
            Dab {
                center: crate::sculpt::Vec3(0., 1., 0.),
                normal: crate::sculpt::Vec3(0., 1., 0.),
                right: crate::sculpt::Vec3(1., 0., 0.),
                up: crate::sculpt::Vec3(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: crate::sculpt::Vec3::default(),
                invert: false,
            },
        );
        document.end_stroke();
        assert_eq!(document.history_counts(), (0, 1));
        document.redo();
        assert!(document.mesh_settings().dynamic);
        document.undo();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document
            .replace_geometry_named(geometry, "Subdivide mesh")
            .unwrap();
        assert_eq!(document.undo_label(), Some("Subdivide mesh"));
        assert!(!document.can_redo());
        document.delete_mesh_levels(true).unwrap();
        assert_eq!(document.undo_label(), Some("Delete lower levels"));
        document.undo();
        assert_eq!(document.redo_label(), Some("Delete lower levels"));
        document.set_mesh_level(1).unwrap();
        assert_eq!(document.undo_label(), Some("Change subdivision level"));
        document.delete_mesh_levels(false).unwrap();
        assert_eq!(document.undo_label(), Some("Delete higher levels"));
    }
}

#[cfg(test)]
mod adaptive_mode_tests {
    use super::*;
    #[test]
    fn adaptive_mode_discards_levels_undo_restores_them_and_other_modes_exclude_it() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document.replace_geometry(geometry).unwrap();
        let before = document.geometry_snapshot();
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(document.mesh_settings().levels(), 1);
        assert!(!document.mesh_settings().dynamic);
        assert_eq!(document.mesh(), &before.mesh);
        document.undo();
        assert_eq!(document.geometry_snapshot(), before);
        document.redo();
        assert!(document.mesh_settings().adaptive.enabled);
        document.set_dynamic_mesh(true).unwrap();
        assert!(!document.mesh_settings().adaptive.enabled);
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        document.set_dynamic_mesh(false).unwrap();
        assert!(!document.mesh_settings().adaptive.enabled);
    }
}

#[cfg(test)]
mod adaptive_validation_tests {
    use super::*;
    #[test]
    fn invalid_adaptive_project_is_rejected_and_live_detail_edits_do_not_flood_history() {
        let mut document = Document::default();
        let original = serde_json::to_value(document.project()).unwrap();
        let mut invalid = document.project();
        invalid.mesh.triangles.pop();
        invalid.mesh_settings.adaptive.enabled = true;
        assert!(
            document
                .replace_project(invalid, "invalid-adaptive.clay")
                .is_err()
        );
        assert_eq!(serde_json::to_value(document.project()).unwrap(), original);
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        let count = document.history_counts();
        for detail in [4., 5., 6., 7.] {
            let mut settings = document.mesh_settings().adaptive.clone();
            settings.detail = detail;
            document.set_adaptive(settings).unwrap();
        }
        assert_eq!(document.history_counts(), count);
        document.undo();
        assert!(!document.mesh_settings().adaptive.enabled);
    }
}

#[cfg(test)]
mod adaptive_smooth_tests {
    use super::*;
    use crate::sculpt::Vec3;

    #[test]
    fn smooth_cleanup_reduces_density_preserves_masks_and_undoes_as_one_stroke() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.mesh = geometry.mesh.subdivide().unwrap();
        geometry.mesh.vertices[0].1 += 0.06;
        for (v, mask) in geometry.mesh.vertices.iter().zip(&mut geometry.mesh.mask) {
            if v.1 < 0.5 {
                *mask = 1.;
            }
        }
        document
            .replace_geometry_named(geometry, "Dense test surface")
            .unwrap();
        document
            .set_adaptive(crate::sculpt::AdaptiveSettings {
                enabled: true,
                detail: 16.,
                ..Default::default()
            })
            .unwrap();
        let before = document.mesh().clone();
        let history = document.history_counts().0;
        let mut brush = Brush::new(Kind::Smooth);
        brush.strength = 0.6;
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.4,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        for _ in 0..6 {
            document.begin_sculpt_frame();
            let count = document.mesh().vertices.len();
            document.apply_dab(&brush, dab);
            assert!(document.mesh().vertices.len() <= count);
        }
        document.end_stroke();
        let after = document.mesh().clone();
        assert!(after.vertices.len() < before.vertices.len() - 50);
        assert!(
            after
                .vertices
                .iter()
                .map(|v| v.1)
                .fold(f32::NEG_INFINITY, f32::max)
                < 1.03
        );
        after.validate_closed().unwrap();
        let protected: std::collections::HashSet<_> = after
            .vertices
            .iter()
            .zip(&after.mask)
            .filter(|(_, m)| **m == 1.)
            .map(|(v, _)| (v.0.to_bits(), v.1.to_bits(), v.2.to_bits()))
            .collect();
        for (v, mask) in before.vertices.iter().zip(&before.mask) {
            if *mask == 1. {
                assert!(
                    protected.contains(&(v.0.to_bits(), v.1.to_bits(), v.2.to_bits())),
                    "protected vertex or mask changed"
                );
            }
        }
        assert_eq!(document.history_counts().0, history + 1);
        document.undo();
        assert_eq!(document.mesh(), &before);
        document.redo();
        assert_eq!(document.mesh(), &after);
    }
}

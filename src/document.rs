//! Owns persistent content and mesh edits. No UI, window, or tablet dependencies.
use crate::{
    history::History,
    sculpt::{Alpha, Brush, Dab, Kind, Mesh, Project, Vec3, validate_brush},
};
pub struct Document {
    mesh: Mesh,
    brushes: Vec<Brush>,
    history: History,
    dirty: bool,
    stroke_active: bool,
    current_path: String,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            mesh: Mesh::sphere(),
            brushes: Kind::ALL.into_iter().map(Brush::new).collect(),
            history: History::default(),
            dirty: false,
            stroke_active: false,
            current_path: "sculpt.clay".into(),
        }
    }
}
impl Document {
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
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
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    pub fn project(&self) -> Project {
        Project {
            version: Project::VERSION,
            mesh: self.mesh.clone(),
            brushes: self.brushes.clone(),
        }
    }
    pub fn mark_saved(&mut self, path: &str) {
        self.current_path = path.into();
        self.dirty = false;
    }
    pub fn new_sphere(&mut self) {
        self.mesh = Mesh::sphere();
        self.history = History::default();
        self.dirty = false;
        self.stroke_active = false;
        self.current_path = "untitled.clay".into();
    }
    /// Validate before changing anything, so an invalid open leaves the current document intact.
    pub fn replace_project(&mut self, project: Project, path: &str) -> Result<(), String> {
        project.validate()?;
        self.mesh = project.mesh;
        self.brushes = project.brushes;
        self.history = History::default();
        self.stroke_active = false;
        self.mark_saved(path);
        Ok(())
    }
    pub fn begin_stroke(&mut self) {
        if !self.stroke_active {
            self.history.record(&self.mesh);
            self.stroke_active = true;
        }
    }
    pub fn end_stroke(&mut self) {
        self.stroke_active = false;
    }
    pub fn apply_dab(&mut self, brush: &Brush, dab: Dab, mirror_x: bool) {
        self.begin_stroke();
        self.mesh.stamp(brush, dab);
        if mirror_x && dab.center.0.abs() > dab.radius {
            let mirror = |v: Vec3| Vec3(-v.0, v.1, v.2);
            self.mesh.stamp(
                brush,
                Dab {
                    center: mirror(dab.center),
                    normal: mirror(dab.normal),
                    right: mirror(dab.right),
                    up: mirror(dab.up),
                    delta: mirror(dab.delta),
                    ..dab
                },
            );
        }
        self.dirty = true;
    }
    pub fn undo(&mut self) {
        self.end_stroke();
        if self.history.undo(&mut self.mesh) {
            self.dirty = true;
        }
    }
    pub fn redo(&mut self) {
        self.end_stroke();
        if self.history.redo(&mut self.mesh) {
            self.dirty = true;
        }
    }
    pub fn clear_mask(&mut self) {
        self.end_stroke();
        self.history.record(&self.mesh);
        self.mesh.mask.fill(0.);
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
    #[test]
    fn whole_stroke_is_one_undo_step_and_new_edits_clear_redo() {
        let mut document = Document::default();
        let original = document.mesh.vertices[0].1;
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
        document.apply_dab(&brush, dab, false);
        document.apply_dab(&brush, dab, false);
        document.end_stroke();
        let sculpted = document.mesh.vertices[0].1;
        assert!(sculpted > original);
        document.undo();
        assert_eq!(document.mesh.vertices[0].1, original);
        assert!(!document.can_undo());
        document.redo();
        assert_eq!(document.mesh.vertices[0].1, sculpted);
        document.undo();
        document.apply_dab(&brush, dab, false);
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

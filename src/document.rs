//! Owns persistent content and mesh edits. No UI, window, or tablet dependencies.
use crate::{
    history::History,
    sculpt::{
        AdaptiveBudget, AdaptiveSettings, Alpha, Brush, Dab, Geometry, Kind, Mesh, MeshSettings,
        Project, Symmetry, validate_brush,
    },
};
mod attributes;
mod brushes;
mod edit;
mod mask;
mod mesh;
mod modeling;
mod primitive;
mod references;
mod scene;
mod stroke;
#[cfg(test)]
mod tests;
mod thickness;
mod transform;
pub(crate) use edit::EditKind;
use edit::EditSession;

pub struct ModelRef<'a> {
    pub name: &'a str,
    pub metadata: &'a crate::sculpt::ModelMetadata,
    pub mesh: &'a Mesh,
    pub settings: &'a MeshSettings,
}
impl ModelRef<'_> {
    pub fn display_mesh(&self, active: bool) -> &Mesh {
        if active {
            self.mesh
        } else {
            self.settings.lower.first().unwrap_or(self.mesh)
        }
    }
}
pub struct Document {
    references: Vec<crate::reference::ReferenceImage>,
    geometry: Geometry,
    display_revision: u64,
    brushes: Vec<Brush>,
    history: History,
    symmetry: Symmetry,
    dirty: bool,
    edit: Option<EditSession>,
    stroke_active: bool,
    stroke_deformed: bool,
    stroke_before: Option<Geometry>,
    current_path: String,
    adaptive_budget: AdaptiveBudget,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            references: Vec::new(),
            geometry: Geometry::new(Mesh::starting_sphere()),
            display_revision: 0,
            brushes: Kind::ALL.into_iter().map(Brush::new).collect(),
            history: History::default(),
            symmetry: Symmetry::default(),
            dirty: false,
            edit: None,
            stroke_active: false,
            stroke_deformed: false,
            stroke_before: None,
            current_path: "sculpt.clay".into(),
            adaptive_budget: AdaptiveBudget::default(),
        }
    }
}
impl Document {
    /// Session-only invalidation token for display geometry, materials, and ownership.
    pub(crate) fn display_revision(&self) -> u64 {
        self.display_revision
    }
    fn display_changed(&mut self) {
        self.display_revision = self.display_revision.wrapping_add(1);
    }
    pub fn brushes(&self) -> &[Brush] {
        &self.brushes
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub(crate) fn edit_in_progress(&self) -> bool {
        self.edit.is_some() || self.stroke_active
    }
    pub(crate) fn recover_project(&mut self, project: Project) -> Result<(), String> {
        self.replace_project(project, "recovered.clay")?;
        self.dirty = true;
        Ok(())
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
            references: self.references.clone(),
            version: Project::VERSION,
            scene: self.geometry.scene.clone(),
            model_name: self.geometry.name.clone(),
            submodels: self.geometry.submodels.clone(),
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
        self.end_edit();
        self.references.clear();
        self.geometry = Geometry::new(Mesh::starting_sphere());
        self.display_changed();
        self.history = History::default();
        self.dirty = false;
        self.end_stroke();
        self.current_path = "untitled.clay".into();
    }
    /// Validate before changing anything, so an invalid open leaves the current document intact.
    pub fn replace_project(&mut self, project: Project, path: &str) -> Result<(), String> {
        project.validate()?;
        self.end_edit();
        self.geometry = Geometry {
            scene: project.scene,
            name: project.model_name,
            submodels: project.submodels,
            mesh: project.mesh,
            settings: project.mesh_settings,
        };
        self.display_changed();
        self.references = project.references;
        self.brushes = project.brushes;
        self.symmetry = project.symmetry;
        self.history = History::default();
        self.end_stroke();
        self.mark_saved(path);
        Ok(())
    }
    pub fn undo(&mut self) {
        self.end_edit();
        self.end_stroke();
        if self.history.undo(&mut self.geometry) {
            self.dirty = true;
            self.display_changed();
        }
    }
    pub fn redo(&mut self) {
        self.end_edit();
        self.end_stroke();
        if self.history.redo(&mut self.geometry) {
            self.dirty = true;
            self.display_changed();
        }
    }
}

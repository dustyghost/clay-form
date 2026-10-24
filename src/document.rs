//! Owns persistent content and mesh edits. No UI, window, or tablet dependencies.
use crate::{
    history::History,
    sculpt::{
        AdaptiveBudget, AdaptiveSettings, Alpha, Brush, Dab, Geometry, Kind, Mesh, MeshSettings,
        Project, Symmetry, validate_brush,
    },
};
mod alphas;
mod attributes;
mod brushes;
mod diagnostics;
mod edit;
mod mask;
mod mesh;
mod modeling;
mod path_stroke;
mod paths;
mod primitive;
pub use path_stroke::{PathStrokeOptions, PathStrokePreview, PathStrokeRequest, PathStrokeSession};
mod projection;
pub use projection::ProjectionSource;
mod references;
mod relax;
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
    alphas: Vec<crate::sculpt::alpha::Asset>,
    stencil: Option<crate::sculpt::alpha::Stencil>,
    references: Vec<crate::reference::ReferenceImage>,
    geometry: Geometry,
    display_revision: u64,
    brushes: Vec<Brush>,
    history: History,
    diagnostic_context: Option<serde_json::Value>,
    symmetry: Symmetry,
    dirty: bool,
    edit: Option<EditSession>,
    stroke_active: bool,
    relax_surface: Option<(u64, crate::sculpt::RelaxationSurface)>,
    relax_stroke: Option<(bool, crate::diagnostics::EditSettings)>,
    pub(crate) relax_status: Option<String>,
    stroke_deformed: bool,
    stroke_before: Option<Geometry>,
    stroke_flatten_planes: Vec<(crate::sculpt::Vec3, crate::sculpt::Vec3)>,
    current_path: String,
    has_saved_path: bool,
    adaptive_budget: AdaptiveBudget,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            alphas: Vec::new(),
            stencil: None,
            references: Vec::new(),
            geometry: Geometry::new(Mesh::starting_sphere()),
            display_revision: 0,
            brushes: Kind::ALL
                .into_iter()
                .map(Brush::new)
                .chain(std::iter::once(Brush::alpha_drag()))
                .collect(),
            history: History::default(),
            diagnostic_context: None,
            symmetry: Symmetry::default(),
            dirty: false,
            edit: None,
            stroke_active: false,
            relax_surface: None,
            relax_stroke: None,
            relax_status: None,
            stroke_deformed: false,
            stroke_before: None,
            stroke_flatten_planes: Vec::new(),
            current_path: "sculpt.clay".into(),
            has_saved_path: false,
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
            paths: self.geometry.paths.clone(),
            alphas: self.alphas.clone(),
            stencil: self.stencil.clone(),
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
    pub fn has_saved_path(&self) -> bool {
        self.has_saved_path
    }
    pub fn mark_saved(&mut self, path: &str) {
        self.has_saved_path = true;
        self.current_path = path.into();
        self.dirty = false;
    }
    pub fn new_sphere(&mut self) {
        self.end_edit();
        self.references.clear();
        self.stencil = None;
        self.geometry = Geometry::new(Mesh::starting_sphere());
        self.display_changed();
        self.history.reset();
        self.dirty = false;
        self.end_stroke();
        self.current_path = "untitled.clay".into();
        self.has_saved_path = false;
    }
    /// Validate before changing anything, so an invalid open leaves the current document intact.
    pub fn replace_project(&mut self, project: Project, path: &str) -> Result<(), String> {
        project.validate()?;
        self.end_edit();
        self.geometry = Geometry {
            paths: project.paths,
            scene: project.scene,
            name: project.model_name,
            submodels: project.submodels,
            mesh: project.mesh,
            settings: project.mesh_settings,
        };
        self.display_changed();
        self.alphas = project.alphas;
        self.stencil = project.stencil;
        self.references = project.references;
        self.brushes = project.brushes;
        self.symmetry = project.symmetry;
        self.history.reset();
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

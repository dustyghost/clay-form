//! Reference selection and transactional detail projection.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionSource {
    Submodel(usize),
    Level(usize),
}
impl Document {
    /// A normal saved submodel keeps the exact active surface, even after remesh
    /// or adaptive sculpting discards the target's subdivision stack.
    pub fn preserve_projection_reference(&mut self) -> Result<usize, String> {
        self.require_projection_target()?;
        if self.model_count() >= 64 {
            return Err("Submodels are limited to 64".into());
        }
        self.end_edit();
        self.end_stroke();
        let mut models = self.ordered_models();
        let active = self.active_model_index();
        let mut reference = models[active].clone();
        let mut number = 1;
        loop {
            reference.name = format!("Projection reference {number}");
            if !self.submodel_names().any(|n| n == reference.name) {
                break;
            }
            number += 1;
        }
        reference.metadata.visible = false;
        models.insert(active + 1, reference);
        self.record_edit("Keep projection reference");
        self.install_models(models, active);
        Ok(active + 1)
    }
    pub(crate) fn require_projection_target(&self) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if !self.active_visible() {
            return Err("Show the target submodel before projecting".into());
        }
        if self.geometry.scene.active_metadata.temporary_thickness > 0. {
            return Err("Apply or remove the target's Thickness modifier before projecting".into());
        }
        if self.mesh().triangles.is_empty() {
            return Err("The target has no surface to project".into());
        }
        Ok(())
    }
    pub fn projection_source(&self, source: ProjectionSource) -> Result<Mesh, String> {
        self.require_projection_target()?;
        match source {
            ProjectionSource::Submodel(index) => {
                if index == self.active_model_index() {
                    return Err("Choose a reference different from the target".into());
                }
                let model = self
                    .model(index)
                    .ok_or("Reference submodel no longer exists")?;
                if model.metadata.temporary_thickness > 0. {
                    return Err("Apply or remove the reference's Thickness modifier first".into());
                }
                // Whole-model visibility is deliberately ignored: hidden reference
                // copies remain usable. Individually hidden faces stay excluded.
                Ok(model.mesh.clone())
            }
            ProjectionSource::Level(level) => {
                if level == self.mesh_settings().level() {
                    return Err("Choose a different subdivision level as the reference".into());
                }
                let mut geometry = Geometry::new(self.mesh().clone());
                geometry.settings = self.mesh_settings().clone();
                geometry.settings.validate(&geometry.mesh)?;
                geometry.set_level(level)?;
                Ok(geometry.mesh)
            }
        }
    }
    pub(crate) fn apply_projection(
        &mut self,
        mesh: Mesh,
        revision: u64,
        settings: Option<crate::diagnostics::EditSettings>,
    ) -> Result<(), String> {
        self.require_projection_target()?;
        if revision != self.display_revision() {
            return Err("The scene changed; preview the projection again".into());
        }
        if mesh == *self.mesh() {
            return Ok(());
        }
        let mut original_attributes = mesh.clone();
        original_attributes.vertices = self.mesh().vertices.clone();
        if mesh.vertices.len() != self.mesh().vertices.len() || original_attributes != *self.mesh()
        {
            return Err("Projection cannot replace topology or attributes".into());
        }
        let mut geometry = self.geometry_snapshot();
        geometry.mesh = mesh;
        self.replace_geometry_recorded(geometry, "Project details", settings)
    }
}

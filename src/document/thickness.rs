//! Temporary shell parameters and thickness application.
use super::*;

impl Document {
    pub fn set_temporary_thickness(&mut self, value: f32) -> Result<(), String> {
        let mut metadata = self.geometry.scene.active_metadata.clone();
        metadata.temporary_thickness = value;
        metadata.validate()?;
        self.prepare_edit(Some(EditKind::Thickness));
        if metadata == self.geometry.scene.active_metadata {
            return Ok(());
        }
        if value > 0. {
            self.mesh().solidify_with_metadata(&metadata)?;
        }
        self.begin_edit(EditSession::Thickness);
        self.geometry.scene.active_metadata = metadata;
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn apply_thickness(&mut self) -> Result<(), String> {
        if !self.active_visible() {
            return Err("Show the active model before applying thickness".into());
        }
        let shell = self
            .mesh()
            .solidify_with_metadata(&self.geometry.scene.active_metadata)?;
        self.end_edit();
        let mut geometry = self.geometry.clone();
        geometry.mesh = shell;
        geometry.settings = MeshSettings {
            resolution: geometry.settings.resolution,
            ..Default::default()
        };
        geometry.scene.active_metadata.temporary_thickness = 0.;
        self.replace_geometry_named(geometry, "Apply thickness")
    }
}

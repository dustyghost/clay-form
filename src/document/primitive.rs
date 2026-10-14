//! Parametric primitive editing and conversion.
use super::*;

impl Document {
    pub fn primitive(&self) -> Option<&crate::sculpt::Primitive> {
        self.mesh_settings().primitive.as_ref()
    }
    pub fn require_sculpt_mesh(&self) -> Result<(), String> {
        if self.primitive().is_some() {
            Err("Convert the primitive to a sculpting mesh first".into())
        } else {
            Ok(())
        }
    }
    pub fn initialize_primitive(&mut self) -> Result<(), String> {
        let mut primitive = crate::sculpt::Primitive::default();
        let mut min = crate::sculpt::Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = crate::sculpt::Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for p in &self.mesh().vertices {
            min = crate::sculpt::Vec3(min.0.min(p.0), min.1.min(p.1), min.2.min(p.2));
            max = crate::sculpt::Vec3(max.0.max(p.0), max.1.max(p.1), max.2.max(p.2));
        }
        primitive.center = if self.mesh().vertices.is_empty() {
            crate::sculpt::Vec3::default()
        } else {
            (min + max) * 0.5
        };
        let mut geometry = self.geometry.clone();
        geometry.mesh = primitive.mesh()?;
        geometry.settings = MeshSettings {
            primitive: Some(primitive),
            ..Default::default()
        };
        self.end_edit();
        self.replace_geometry_named(geometry, "Initialize primitive")
    }
    /// Continuous slider updates are one edit until the pointer is released.
    pub fn update_primitive(&mut self, primitive: crate::sculpt::Primitive) -> Result<(), String> {
        self.prepare_edit(Some(EditKind::Primitive));
        let Some(current) = self.primitive() else {
            return Err("The active submodel is no longer a primitive".into());
        };
        if *current == primitive {
            return Ok(());
        }
        let mesh = primitive.mesh()?;
        self.begin_edit(EditSession::Primitive);
        self.geometry.mesh = mesh;
        self.geometry.settings = MeshSettings {
            primitive: Some(primitive),
            ..Default::default()
        };
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn convert_primitive(&mut self) -> Result<(), String> {
        if self.primitive().is_none() {
            return Err("The active submodel is already a sculpting mesh".into());
        }
        if self.mesh().triangles.is_empty() {
            return Err("Fill the circle before converting it to a sculpting mesh".into());
        }
        self.end_edit();
        let mut geometry = self.geometry.clone();
        geometry.settings.primitive = None;
        self.replace_geometry_named(geometry, "Convert primitive to sculpting mesh")
    }
}

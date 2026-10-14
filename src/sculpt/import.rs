//! Add a parsed surface to a worker-owned scene before committing one history step.
use super::{Geometry, Mesh, Submodel};
impl Geometry {
    pub(crate) fn import_mesh(&mut self, mesh: Mesh, name: &str) -> Result<(), String> {
        mesh.validate()?;
        if mesh.triangles.is_empty() {
            return Err("Imported mesh has no faces".into());
        }
        if self.submodels.len() >= 63 {
            return Err("Submodels are limited to 64".into());
        }
        let mut base = name.trim().to_owned();
        while base.len() > 112 {
            base.pop();
        }
        if base.is_empty() {
            base = "Imported OBJ".into();
        }
        let mut name = base.clone();
        let mut suffix = 2;
        while self.name == name || self.submodels.iter().any(|m| m.name == name) {
            name = format!("{base} ({suffix})");
            suffix += 1;
        }
        let previous = Submodel {
            name: std::mem::replace(&mut self.name, name),
            mesh: std::mem::replace(&mut self.mesh, mesh),
            mesh_settings: std::mem::take(&mut self.settings),
            metadata: std::mem::take(&mut self.scene.active_metadata),
        };
        self.submodels.insert(self.scene.active_index, previous);
        self.scene.active_index = self.submodels.len();
        Ok(())
    }
}

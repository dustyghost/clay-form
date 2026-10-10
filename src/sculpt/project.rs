use super::{Brush, Mesh, MeshSettings, Symmetry, validate_brush};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub mesh: Mesh,
    pub brushes: Vec<Brush>,
    #[serde(default, skip_serializing_if = "Symmetry::is_default")]
    pub symmetry: Symmetry,
    #[serde(default, skip_serializing_if = "MeshSettings::is_default")]
    pub mesh_settings: MeshSettings,
}
impl Project {
    pub const VERSION: u32 = 1;
    pub fn validate(&self) -> Result<(), String> {
        if self.version != Self::VERSION {
            return Err("Unsupported project version".into());
        }
        self.mesh.validate()?;
        self.symmetry.validate()?;
        self.mesh_settings.validate(&self.mesh)?;
        if self.brushes.is_empty() || self.brushes.len() > 256 {
            return Err("Invalid brush library".into());
        }
        for brush in &self.brushes {
            validate_brush(brush)?;
        }
        Ok(())
    }
}

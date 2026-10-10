use super::{Brush, Mesh, validate_brush};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub mesh: Mesh,
    pub brushes: Vec<Brush>,
}
impl Project {
    pub const VERSION: u32 = 1;
    pub fn validate(&self) -> Result<(), String> {
        if self.version != Self::VERSION {
            return Err("Unsupported project version".into());
        }
        self.mesh.validate()?;
        if self.brushes.is_empty() || self.brushes.len() > 256 {
            return Err("Invalid brush library".into());
        }
        for brush in &self.brushes {
            validate_brush(brush)?;
        }
        Ok(())
    }
}

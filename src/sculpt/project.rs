use super::{Brush, Mesh, MeshSettings, Symmetry, validate_brush};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Submodel {
    pub name: String,
    #[serde(default, skip_serializing_if = "ModelMetadata::is_default")]
    pub metadata: ModelMetadata,
    pub mesh: Mesh,
    pub mesh_settings: MeshSettings,
}
fn visible_default() -> bool {
    true
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelMetadata {
    #[serde(default = "visible_default")]
    pub visible: bool,
    #[serde(default)]
    pub group: String,
}
impl Default for ModelMetadata {
    fn default() -> Self {
        Self {
            visible: true,
            group: String::new(),
        }
    }
}
impl ModelMetadata {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneSettings {
    pub active_index: usize,
    pub active_metadata: ModelMetadata,
    pub groups: Vec<String>,
}
impl SceneSettings {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}
fn is_default_model_name(name: &str) -> bool {
    name == "Model"
}
fn default_model_name() -> String {
    "Model".into()
}
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    #[serde(default, skip_serializing_if = "SceneSettings::is_default")]
    pub scene: SceneSettings,
    #[serde(
        default = "default_model_name",
        skip_serializing_if = "is_default_model_name"
    )]
    pub model_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submodels: Vec<Submodel>,
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
        if self.scene.active_index > self.submodels.len() || self.scene.groups.len() > 64 {
            return Err("Invalid scene organization".into());
        }
        for (i, group) in self.scene.groups.iter().enumerate() {
            if group.trim().is_empty()
                || group.len() > 128
                || self.scene.groups[..i].contains(group)
            {
                return Err("Invalid or duplicate group name".into());
            }
        }
        for metadata in std::iter::once(&self.scene.active_metadata)
            .chain(self.submodels.iter().map(|m| &m.metadata))
        {
            if !metadata.group.is_empty() && !self.scene.groups.contains(&metadata.group) {
                return Err("Submodel refers to a missing group".into());
            }
        }
        if self.submodels.len() >= 64
            || self.model_name.trim().is_empty()
            || self.model_name.len() > 128
        {
            return Err("Invalid submodel collection".into());
        }
        for model in &self.submodels {
            if model.name.trim().is_empty() || model.name.len() > 128 {
                return Err("Invalid submodel name".into());
            }
            model.mesh_settings.validate(&model.mesh)?;
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

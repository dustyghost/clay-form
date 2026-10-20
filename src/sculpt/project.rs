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
fn default_threshold() -> f32 {
    0.5
}
fn threshold_default(value: &f32) -> bool {
    *value == 0.5
}
fn is_zero_loops(value: &u32) -> bool {
    *value == 0
}
fn disabled(value: &bool) -> bool {
    !*value
}
fn zero_thickness(value: &f32) -> bool {
    *value == 0.
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
    #[serde(default, skip_serializing_if = "zero_thickness")]
    pub temporary_thickness: f32,
    #[serde(
        default = "default_threshold",
        skip_serializing_if = "threshold_default"
    )]
    pub mask_threshold: f32,
    #[serde(default, skip_serializing_if = "disabled")]
    pub thickness_groups: bool,
    #[serde(default, skip_serializing_if = "is_zero_loops")]
    pub thickness_loops: u32,
    #[serde(default, skip_serializing_if = "disabled")]
    pub thickness_crease: bool,
    #[serde(default, skip_serializing_if = "disabled")]
    pub thickness_mask_rim: bool,
    #[serde(default, skip_serializing_if = "disabled")]
    pub thickness_group_mask: bool,
}
impl Default for ModelMetadata {
    fn default() -> Self {
        Self {
            visible: true,
            group: String::new(),
            temporary_thickness: 0.,
            mask_threshold: 0.5,
            thickness_groups: false,
            thickness_loops: 0,
            thickness_crease: false,
            thickness_mask_rim: false,
            thickness_group_mask: false,
        }
    }
}
impl ModelMetadata {
    pub fn validate(&self) -> Result<(), String> {
        if self.thickness_loops > 8 {
            return Err("Thickness modifier supports up to 8 rim loops".into());
        }
        if !self.mask_threshold.is_finite() || !(0. ..=1.).contains(&self.mask_threshold) {
            return Err("Mask threshold must be between 0% and 100%".into());
        }
        if !self.temporary_thickness.is_finite()
            || !(0. ..=100.).contains(&self.temporary_thickness)
        {
            return Err("Invalid thickness modifier".into());
        }
        Ok(())
    }
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
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alphas: Vec<super::alpha::Asset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stencil: Option<super::alpha::Stencil>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "crate::reference::deserialize_list"
    )]
    pub references: Vec<crate::reference::ReferenceImage>,
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
        super::alpha::validate_library(&self.alphas)?;
        if let Some(stencil) = &self.stencil {
            stencil.validate()?;
        }
        crate::reference::validate(&self.references)?;
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
            metadata.validate()?;
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
        let mut identities = std::collections::HashSet::new();
        for brush in &self.brushes {
            validate_brush(brush)?;
            if let Some(id) = &brush.id
                && !identities.insert(id.to_ascii_lowercase())
            {
                return Err("Duplicate brush identity in project".into());
            }
        }
        Ok(())
    }
}

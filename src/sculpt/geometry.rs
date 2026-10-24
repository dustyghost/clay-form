//! Persistent scene geometry, independent of mesh algorithms.
use super::{Mesh, MeshSettings};

#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub paths: Vec<super::paths::SurfacePath>,
    pub scene: super::SceneSettings,
    pub name: String,
    pub submodels: Vec<super::project::Submodel>,
    pub mesh: Mesh,
    pub settings: MeshSettings,
}
impl Geometry {
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh,
            paths: Vec::new(),
            name: "Model".into(),
            scene: super::SceneSettings::default(),
            submodels: Vec::new(),
            settings: MeshSettings::default(),
        }
    }
}

/// A result validated before crossing the worker/document commit boundary.
pub(crate) struct ValidatedGeometry(Geometry);
impl ValidatedGeometry {
    pub fn new(geometry: Geometry, cancel: &impl Fn() -> bool) -> Result<Self, String> {
        geometry
            .settings
            .validate_cancellable(&geometry.mesh, cancel)?;
        if cancel() {
            return Err("Operation cancelled".into());
        }
        Ok(Self(geometry))
    }
    pub fn geometry(&self) -> &Geometry {
        &self.0
    }
    pub fn into_geometry(self) -> Geometry {
        self.0
    }
}

//! Topology choices shared by voxel reconstruction and local adaptation.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Topology {
    #[default]
    Quads,
    Triangles,
}
impl Topology {
    pub(crate) fn triangles() -> Self {
        Self::Triangles
    }
}

/// User-facing quad remeshing choices. Retired names migrate to Fast.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuadRemeshAlgorithm {
    #[default]
    #[serde(alias = "SurfaceNets", alias = "Relaxed", alias = "VoxelSkin")]
    Fast,
    Optimized,
}
impl QuadRemeshAlgorithm {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::Fast
    }
}

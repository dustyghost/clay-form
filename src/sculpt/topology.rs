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

//! Sculpting data and algorithms, independent of windowing and UI.
mod adaptive;
mod brush;
mod clay;
mod deformation;
mod detail;
mod math;
mod mesh;
mod overlap;
mod polish;
mod project;
mod remesh;
mod smooth;
mod subdivision;
mod symmetry;
pub use adaptive::AdaptiveSettings;
pub(crate) use adaptive::{Budget as AdaptiveBudget, Region as AdaptiveRegion};
pub use brush::{Alpha, Brush, BrushIcon, Kind, StrokeMode, validate_brush};
pub use math::Vec3;
pub use mesh::{Dab, Mesh};
pub use project::{ModelMetadata, Project, SceneSettings, Submodel};
pub use subdivision::{Geometry, MeshSettings};
pub(crate) use subdivision::{MAX_TRIANGLES, MAX_VERTICES};
mod import;
pub use symmetry::{Axis, GuidePlane, Symmetry};
#[cfg(test)]
mod tests;

mod transform;
pub use transform::Transform;

mod primitive;
pub use primitive::{Primitive, PrimitiveKind};

mod thickness;

mod groups;
pub use groups::{VertexGroup, mask_selected};

mod group_transfer;

mod slice;
mod slice_path;
pub use slice::{SliceMode, SliceOptions, SlicePlane};
pub use slice_path::SlicePath;
mod shading;
pub(crate) use shading::SurfaceShading;

#[cfg(test)]
mod group_tests;

mod visibility;

mod merge;
mod mirror;
pub use mirror::MirrorOptions;

#[cfg(test)]
mod mirror_tests;

mod edges;
pub use edges::{BorderTarget, EdgeEdit};
mod modeling;
pub use modeling::{
    ComponentBuild, ComponentBuildOptions, ComponentTransform, Construction, ExtrudeContact,
    Extrusion, LoopCut, ModelingSymmetry, TopologyEdit,
};

mod projection;
pub use projection::{ProjectionOptions, ProjectionReport};

pub mod alpha;

pub mod path_geometry;
pub mod paths;

//! Sculpting data and algorithms, independent of windowing and UI.
mod adaptive;
mod brush;
mod math;
mod mesh;
mod project;
mod remesh;
mod subdivision;
mod symmetry;
pub use adaptive::AdaptiveSettings;
pub(crate) use adaptive::Budget as AdaptiveBudget;
pub use brush::{Alpha, Brush, BrushIcon, Kind, StrokeMode, validate_brush};
pub use math::Vec3;
pub use mesh::{Dab, Mesh};
pub use project::{ModelMetadata, Project, SceneSettings, Submodel};
pub use subdivision::{Geometry, MeshSettings};
pub use symmetry::{Axis, GuidePlane, Symmetry};
#[cfg(test)]
mod tests;

mod transform;
pub use transform::Transform;

mod primitive;
pub use primitive::{HeadOptions, Primitive, PrimitiveKind};

mod thickness;

mod groups;
pub use groups::{VertexGroup, mask_selected};

mod group_transfer;

#[cfg(test)]
mod group_tests;

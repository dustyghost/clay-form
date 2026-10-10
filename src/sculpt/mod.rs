//! Sculpting data and algorithms, independent of windowing and UI.
mod brush;
mod math;
mod mesh;
mod project;
mod remesh;
mod subdivision;
mod symmetry;
pub use brush::{Alpha, Brush, Kind, validate_brush};
pub use math::Vec3;
pub use mesh::{Dab, Mesh};
pub use project::Project;
pub use subdivision::{Geometry, MeshSettings};
pub use symmetry::{Axis, GuidePlane, Symmetry};
#[cfg(test)]
mod tests;

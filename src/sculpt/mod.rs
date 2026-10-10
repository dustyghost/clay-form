//! Sculpting data and algorithms, independent of windowing and UI.
mod brush;
mod math;
mod mesh;
mod project;
pub use brush::{Alpha, Brush, Kind, validate_brush};
pub use math::Vec3;
pub use mesh::{Dab, Mesh};
pub use project::Project;
#[cfg(test)]
mod tests;

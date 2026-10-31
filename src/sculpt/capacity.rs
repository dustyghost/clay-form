//! Process-wide mesh budgets, installed once before geometry workers start.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct MeshCapacity {
    pub vertices: usize,
    pub triangles: usize,
}
impl MeshCapacity {
    pub const DEFAULT: Self = Self {
        vertices: 500_000,
        triangles: 1_000_000,
    };
    pub const MAX: Self = Self {
        vertices: 16_000_000,
        triangles: 32_000_000,
    };
    pub fn validate(self) -> Result<(), String> {
        if !(Self::DEFAULT.vertices..=Self::MAX.vertices).contains(&self.vertices)
            || !(Self::DEFAULT.triangles..=Self::MAX.triangles).contains(&self.triangles)
        {
            return Err("Mesh capacity must be 500,000–16,000,000 vertices and 1,000,000–32,000,000 triangles".into());
        }
        Ok(())
    }
}
impl Default for MeshCapacity {
    fn default() -> Self {
        Self::DEFAULT
    }
}
static ACTIVE: OnceLock<MeshCapacity> = OnceLock::new();
/// Preferences may change during a job. Only a fresh process installs new budgets.
pub(crate) fn initialize(capacity: MeshCapacity) -> Result<(), String> {
    capacity.validate()?;
    ACTIVE.get_or_init(|| capacity);
    Ok(())
}
pub(crate) fn current() -> MeshCapacity {
    #[cfg(test)]
    if let Some(capacity) = TEST_CAPACITY.with(|c| c.get()) {
        return capacity;
    }
    ACTIVE.get().copied().unwrap_or_default()
}
#[inline]
pub(crate) fn max_vertices() -> usize {
    current().vertices
}
#[inline]
pub(crate) fn max_triangles() -> usize {
    current().triangles
}

// Small budgets exercise real operation boundaries without allocating huge meshes.
#[cfg(test)]
thread_local! { static TEST_CAPACITY: std::cell::Cell<Option<MeshCapacity>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
pub(crate) fn with_test_capacity<T>(capacity: MeshCapacity, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<MeshCapacity>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_CAPACITY.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(TEST_CAPACITY.with(|c| c.replace(Some(capacity))));
    f()
}

#[cfg(test)]
mod tests;

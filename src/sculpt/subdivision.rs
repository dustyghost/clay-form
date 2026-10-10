//! Linear triangle subdivision and a detail-preserving multiresolution stack.
use super::{AdaptiveSettings, Mesh, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAX_VERTICES: usize = 500_000;
pub const MAX_TRIANGLES: usize = 1_000_000;
const MAX_LEVELS: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MeshSettings {
    /// Lowest to nearest lower level; higher levels are stored highest to nearest.
    pub lower: Vec<Mesh>,
    pub upper: Vec<Mesh>,
    pub dynamic: bool,
    pub resolution: u32,
    pub auto_remesh: bool,
    #[serde(default, skip_serializing_if = "AdaptiveSettings::is_default")]
    pub adaptive: AdaptiveSettings,
}
impl Default for MeshSettings {
    fn default() -> Self {
        Self {
            lower: Vec::new(),
            upper: Vec::new(),
            dynamic: false,
            resolution: 48,
            auto_remesh: false,
            adaptive: AdaptiveSettings::default(),
        }
    }
}
impl MeshSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn level(&self) -> usize {
        self.lower.len() + 1
    }
    pub fn levels(&self) -> usize {
        self.lower.len() + self.upper.len() + 1
    }
    pub fn validate(&self, mesh: &Mesh) -> Result<(), String> {
        self.adaptive.validate()?;
        if (self.adaptive.enabled && (self.dynamic || self.levels() != 1))
            || !(16..=96).contains(&self.resolution)
            || self.levels() > MAX_LEVELS
            || (self.dynamic && self.levels() != 1)
        {
            return Err("Invalid mesh management settings".into());
        }
        if self.adaptive.enabled {
            mesh.validate_closed()?;
        }
        let levels: Vec<&Mesh> = self
            .lower
            .iter()
            .chain(std::iter::once(mesh))
            .chain(self.upper.iter().rev())
            .collect();
        if levels.iter().map(|m| m.vertices.len()).sum::<usize>() > MAX_VERTICES * 2 {
            return Err("Subdivision stack exceeds memory limit".into());
        }
        for level in &levels {
            level.validate()?;
        }
        for pair in levels.windows(2) {
            let refined = pair[0].subdivide()?;
            if refined.triangles != pair[1].triangles
                || refined.vertices.len() != pair[1].vertices.len()
            {
                return Err("Subdivision levels have incompatible topology".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub mesh: Mesh,
    pub settings: MeshSettings,
}
impl Geometry {
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh,
            settings: MeshSettings::default(),
        }
    }
    pub fn subdivide(&mut self) -> Result<(), String> {
        if self.settings.dynamic || self.settings.adaptive.enabled {
            return Err("Switch to subdivision mode first".into());
        }
        if !self.settings.upper.is_empty() {
            return Err("Select the highest level before adding another".into());
        }
        if self.settings.levels() >= MAX_LEVELS {
            return Err("Subdivision level limit reached".into());
        }
        let refined = self.mesh.subdivide()?;
        self.settings
            .lower
            .push(std::mem::replace(&mut self.mesh, refined));
        Ok(())
    }
    pub fn set_level(&mut self, level: usize) -> Result<(), String> {
        if level == 0 || level > self.settings.levels() {
            return Err("Subdivision level does not exist".into());
        }
        while self.settings.level() > level {
            let mut lower = self.settings.lower.pop().expect("validated level");
            // Original vertices remain the first vertices of the refined mesh.
            let count = lower.vertices.len();
            lower.vertices.copy_from_slice(&self.mesh.vertices[..count]);
            lower.mask.copy_from_slice(&self.mesh.mask[..count]);
            self.settings
                .upper
                .push(std::mem::replace(&mut self.mesh, lower));
        }
        while self.settings.level() < level {
            let mut higher = self.settings.upper.pop().expect("validated level");
            let count = self.mesh.vertices.len();
            let mut baseline = self.mesh.clone();
            baseline.vertices.copy_from_slice(&higher.vertices[..count]);
            baseline.mask.copy_from_slice(&higher.mask[..count]);
            let old = baseline.subdivide()?;
            let new = self.mesh.subdivide()?;
            for i in 0..higher.vertices.len() {
                higher.vertices[i] = higher.vertices[i] + (new.vertices[i] - old.vertices[i]);
                higher.mask[i] = (higher.mask[i] + new.mask[i] - old.mask[i]).clamp(0., 1.);
            }
            self.settings
                .lower
                .push(std::mem::replace(&mut self.mesh, higher));
        }
        Ok(())
    }
    pub fn set_dynamic(&mut self, enabled: bool) {
        if self.settings.dynamic != enabled || self.settings.adaptive.enabled {
            self.settings.lower.clear();
            self.settings.upper.clear();
            self.settings.dynamic = enabled;
            self.settings.adaptive.enabled = false;
        }
    }
}
impl Mesh {
    /// Split each triangle into four; shared edges share midpoint vertices.
    /// Original vertices retain their indices, which is the multiresolution contract.
    pub fn subdivide(&self) -> Result<Self, String> {
        self.validate()?;
        if self.triangles.is_empty() {
            return Err("Mesh has no faces to subdivide".into());
        }
        if self.triangles.len() > MAX_TRIANGLES / 4 {
            return Err("Subdivision would exceed the one-million-face limit".into());
        }
        let mut result = Self {
            vertices: self.vertices.clone(),
            mask: self.mask.clone(),
            triangles: Vec::with_capacity(self.triangles.len() * 4),
        };
        let mut edges = HashMap::new();
        for &[a, b, c] in &self.triangles {
            let mut mids = [0; 3];
            for (i, (x, y)) in [(a, b), (b, c), (c, a)].into_iter().enumerate() {
                let edge = if x < y { (x, y) } else { (y, x) };
                mids[i] = *edges.entry(edge).or_insert_with(|| {
                    let index = result.vertices.len();
                    result
                        .vertices
                        .push((self.vertices[x] + self.vertices[y]) * 0.5);
                    result.mask.push((self.mask[x] + self.mask[y]) * 0.5);
                    index
                });
            }
            if result.vertices.len() > MAX_VERTICES {
                return Err("Subdivision would exceed the 500,000-vertex limit".into());
            }
            let [ab, bc, ca] = mids;
            result
                .triangles
                .extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        }
        Ok(result)
    }
    pub(crate) fn smooth_remesh(&mut self) {
        // Two mild relaxation passes remove the voxel staircase.
        for _ in 0..2 {
            let mut sum = vec![Vec3::default(); self.vertices.len()];
            let mut count = vec![0u32; self.vertices.len()];
            for &[a, b, c] in &self.triangles {
                for (i, j, k) in [(a, b, c), (b, a, c), (c, a, b)] {
                    sum[i] = sum[i] + self.vertices[j] + self.vertices[k];
                    count[i] += 2;
                }
            }
            for i in 0..self.vertices.len() {
                if count[i] > 0 {
                    self.vertices[i] = self.vertices[i] * 0.65 + sum[i] * (0.35 / count[i] as f32);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tetrahedron() -> Mesh {
        Mesh {
            vertices: vec![
                Vec3(1., 1., 1.),
                Vec3(-1., -1., 1.),
                Vec3(-1., 1., -1.),
                Vec3(1., -1., -1.),
            ],
            triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            mask: vec![0., 1., 0., 0.],
        }
    }
    #[test]
    fn subdivision_shares_edges_preserves_winding_and_interpolates_masks() {
        let original = tetrahedron();
        let refined = original.subdivide().unwrap();
        assert_eq!(refined.vertices.len(), 10);
        assert_eq!(refined.triangles.len(), 16);
        assert_eq!(&refined.vertices[..4], &original.vertices);
        assert!(refined.mask[4..].contains(&0.5));
        refined.validate_closed().unwrap();
        let volume = |m: &Mesh| {
            m.triangles
                .iter()
                .map(|&[a, b, c]| m.vertices[a].dot(m.vertices[b].cross(m.vertices[c])) / 6.)
                .sum::<f32>()
        };
        assert!((volume(&original) - volume(&refined)).abs() < 1e-5);
    }
    #[test]
    fn coarse_edits_propagate_through_levels_without_losing_high_detail() {
        let mut geometry = Geometry::new(tetrahedron());
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        let high_vertex = geometry.mesh.vertices.len() - 1;
        geometry.mesh.vertices[high_vertex].2 += 0.2;
        geometry.mesh.mask[high_vertex] = 0.7;
        let detailed = geometry.mesh.clone();
        geometry.set_level(1).unwrap();
        for v in &mut geometry.mesh.vertices {
            *v = *v + Vec3(0.2, 0.1, -0.3);
        }
        geometry.set_level(3).unwrap();
        for (a, b) in geometry.mesh.vertices.iter().zip(detailed.vertices.iter()) {
            assert!((*a - *b - Vec3(0.2, 0.1, -0.3)).len() < 1e-5);
        }
        assert!((geometry.mesh.mask[high_vertex] - 0.7).abs() < 1e-6);
        let before = geometry.clone();
        geometry.set_level(1).unwrap();
        geometry.set_level(3).unwrap();
        for (a, b) in geometry
            .mesh
            .vertices
            .iter()
            .zip(before.mesh.vertices.iter())
        {
            assert!((*a - *b).len() < 1e-5);
        }
        geometry.settings.validate(&geometry.mesh).unwrap();
    }
    #[test]
    fn saved_stack_rejects_incompatible_topology_and_dynamic_levels() {
        let mut geometry = Geometry::new(tetrahedron());
        geometry.subdivide().unwrap();
        geometry.settings.validate(&geometry.mesh).unwrap();
        geometry.mesh.triangles[0].swap(0, 1);
        assert!(geometry.settings.validate(&geometry.mesh).is_err());
        geometry.set_dynamic(true);
        assert_eq!(geometry.settings.levels(), 1);
        assert!(geometry.subdivide().is_err());
    }
}

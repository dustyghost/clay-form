//! Linear polygon subdivision and a detail-preserving multiresolution stack.
use super::{AdaptiveSettings, Geometry, Mesh, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAX_VERTICES: usize = 500_000;
pub const MAX_TRIANGLES: usize = 1_000_000;
const MAX_LEVELS: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MeshSettings {
    /// Lowest to nearest lower level; higher levels are stored highest to nearest.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_primitive"
    )]
    pub primitive: Option<super::Primitive>,
    pub lower: Vec<Mesh>,
    pub upper: Vec<Mesh>,
    pub dynamic: bool,
    pub resolution: u32,
    pub auto_remesh: bool,
    #[serde(default, skip_serializing_if = "AdaptiveSettings::is_default")]
    pub adaptive: AdaptiveSettings,
}
// Removed head recipes load as ordinary sculpt meshes. The project already
// stores their complete geometry; never regenerate or discard that surface.
fn deserialize_primitive<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<super::Primitive>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        Some(value) if value.get("kind").and_then(|kind| kind.as_str()) == Some("HumanHead") => {
            Ok(None)
        }
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

impl Default for MeshSettings {
    fn default() -> Self {
        Self {
            primitive: None,
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
        self.validate_cancellable(mesh, &|| false)
    }
    pub(crate) fn validate_cancellable(
        &self,
        mesh: &Mesh,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        check_cancel(cancel)?;
        if let Some(primitive) = &self.primitive
            && (self.levels() != 1
                || self.dynamic
                || self.adaptive.enabled
                || self.auto_remesh
                || {
                    let mut expected = primitive.mesh()?;
                    expected.quads.clear();
                    let mut actual = mesh.clone();
                    actual.quads.clear();
                    expected != actual
                })
        {
            return Err(
                "Primitive preview does not match its parameters; convert before sculpting".into(),
            );
        }
        self.adaptive.validate()?;
        if (self.adaptive.enabled && (self.dynamic || self.levels() != 1))
            || !(16..=96).contains(&self.resolution)
            || self.levels() > MAX_LEVELS
            || (self.dynamic && self.levels() != 1)
        {
            return Err("Invalid mesh management settings".into());
        }
        if (self.adaptive.enabled || self.dynamic) && !mesh.creases.is_empty() {
            return Err("Clear creases before enabling adaptive sculpt or voxel remeshing".into());
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
            check_cancel(cancel)?;
            level.validate()?;
            if level
                .vertex_groups
                .iter()
                .map(|g| (&g.name, g.visible))
                .ne(mesh.vertex_groups.iter().map(|g| (&g.name, g.visible)))
            {
                return Err("Subdivision levels have incompatible vertex groups".into());
            }
        }
        for pair in levels.windows(2) {
            let refined = pair[0].subdivide_cancellable(cancel)?;
            if refined.triangles != pair[1].triangles
                || refined.vertices.len() != pair[1].vertices.len()
            {
                return Err("Subdivision levels have incompatible topology".into());
            }
        }
        Ok(())
    }
}
impl Geometry {
    pub fn subdivide(&mut self) -> Result<(), String> {
        self.subdivide_cancellable(&|| false)
    }
    pub(crate) fn subdivide_cancellable(
        &mut self,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        check_cancel(cancel)?;
        if self.settings.primitive.is_some() {
            return Err("Convert the primitive to a sculpting mesh first".into());
        }
        if self.settings.dynamic || self.settings.adaptive.enabled {
            return Err("Switch to subdivision mode first".into());
        }
        if !self.settings.upper.is_empty() {
            return Err("Select the highest level before adding another".into());
        }
        if self.settings.levels() >= MAX_LEVELS {
            return Err("Subdivision level limit reached".into());
        }
        let refined = self.mesh.subdivide_cancellable(cancel)?;
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
            lower.materials = self.mesh.reindex_materials(0..count);
            for (low, high) in lower.vertex_groups.iter_mut().zip(&self.mesh.vertex_groups) {
                low.weights.copy_from_slice(&high.weights[..count]);
            }
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
            baseline.materials = higher.reindex_materials(0..count);
            for (base, high) in baseline.vertex_groups.iter_mut().zip(&higher.vertex_groups) {
                base.weights.copy_from_slice(&high.weights[..count]);
            }
            let old = baseline.subdivide()?;
            let new = self.mesh.subdivide()?;
            if !higher.materials.is_empty() || !new.materials.is_empty() {
                higher.materials = (0..higher.vertices.len())
                    .map(|i| higher.material(i).delta(new.material(i), old.material(i)))
                    .collect();
            }
            for i in 0..higher.vertices.len() {
                higher.vertices[i] = higher.vertices[i] + (new.vertices[i] - old.vertices[i]);
                higher.mask[i] = (higher.mask[i] + new.mask[i] - old.mask[i]).clamp(0., 1.);
                for ((high, new), old) in higher
                    .vertex_groups
                    .iter_mut()
                    .zip(&new.vertex_groups)
                    .zip(&old.vertex_groups)
                {
                    high.weights[i] =
                        (high.weights[i] + new.weights[i] - old.weights[i]).clamp(0., 1.);
                }
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
    /// Split each quad or triangle into four; shared edges share midpoint vertices.
    /// Original vertices retain their indices, which is the multiresolution contract.
    pub fn subdivide(&self) -> Result<Self, String> {
        self.subdivide_cancellable(&|| false)
    }
    pub(crate) fn subdivide_cancellable(&self, cancel: &impl Fn() -> bool) -> Result<Self, String> {
        check_cancel(cancel)?;
        self.validate()?;
        if self.triangles.is_empty() {
            return Err("Mesh has no faces to subdivide".into());
        }
        if self.triangles.len() > MAX_TRIANGLES / 4 {
            return Err("Subdivision would exceed the one-million-face limit".into());
        }
        let mut result = Self {
            creases: Vec::new(),
            materials: self.materials.clone(),
            vertex_groups: self.vertex_groups.clone(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            quads: Vec::new(),
            vertices: self.vertices.clone(),
            mask: self.mask.clone(),
            triangles: Vec::with_capacity(self.triangles.len() * 4),
        };
        let key = |mut t: [usize; 3]| {
            t.sort_unstable();
            t
        };
        let mut polygons = std::collections::HashSet::new();
        for &[a, b, c, d] in &self.quads {
            check_cancel(cancel)?;
            for t in [[a, b, c], [a, c, d], [a, b, d], [b, c, d]] {
                polygons.insert(key(t));
            }
        }
        let mut edges = HashMap::new();
        let midpoint =
            |result: &mut Mesh, edges: &mut HashMap<(usize, usize), usize>, x: usize, y: usize| {
                let edge = if x < y { (x, y) } else { (y, x) };
                *edges.entry(edge).or_insert_with(|| {
                    let i = result.vertices.len();
                    result
                        .vertices
                        .push((self.vertices[x] + self.vertices[y]) * 0.5);
                    result.mask.push((self.mask[x] + self.mask[y]) * 0.5);
                    result.append_material(self, &[(x, 0.5), (y, 0.5)]);
                    for (group, old) in result.vertex_groups.iter_mut().zip(&self.vertex_groups) {
                        group.weights.push((old.weights[x] + old.weights[y]) * 0.5);
                    }
                    i
                })
            };
        let face_ids: HashMap<_, _> = if self.polygroups.is_empty() {
            HashMap::new()
        } else {
            self.triangles
                .iter()
                .enumerate()
                .map(|(i, &t)| (key(t), self.polygroups[i]))
                .collect()
        };
        let hidden: HashMap<_, _> = if self.hidden_faces.is_empty() {
            HashMap::new()
        } else {
            self.triangles
                .iter()
                .enumerate()
                .map(|(i, &t)| (key(t), self.hidden_faces[i]))
                .collect()
        };
        for (fi, &[a, b, c]) in self.triangles.iter().enumerate() {
            check_cancel(cancel)?;
            if polygons.contains(&key([a, b, c])) {
                continue;
            }
            let ab = midpoint(&mut result, &mut edges, a, b);
            let bc = midpoint(&mut result, &mut edges, b, c);
            let ca = midpoint(&mut result, &mut edges, c, a);
            result
                .triangles
                .extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
            if !self.polygroups.is_empty() {
                result.polygroups.extend([self.polygroups[fi]; 4]);
            }
            if !self.hidden_faces.is_empty() {
                result.hidden_faces.extend([self.hidden_faces[fi]; 4]);
            }
        }
        for &[a, b, c, d] in &self.quads {
            check_cancel(cancel)?;
            let ab = midpoint(&mut result, &mut edges, a, b);
            let bc = midpoint(&mut result, &mut edges, b, c);
            let cd = midpoint(&mut result, &mut edges, c, d);
            let da = midpoint(&mut result, &mut edges, d, a);
            let center = result.vertices.len();
            result.append_material(self, &[(a, 0.25), (b, 0.25), (c, 0.25), (d, 0.25)]);
            result.vertices.push(
                (self.vertices[a] + self.vertices[b] + self.vertices[c] + self.vertices[d]) * 0.25,
            );
            result
                .mask
                .push((self.mask[a] + self.mask[b] + self.mask[c] + self.mask[d]) * 0.25);
            for (group, old) in result.vertex_groups.iter_mut().zip(&self.vertex_groups) {
                group.weights.push(
                    (old.weights[a] + old.weights[b] + old.weights[c] + old.weights[d]) * 0.25,
                );
            }
            let id = [[a, b, c], [a, c, d], [a, b, d], [b, c, d]]
                .into_iter()
                .find_map(|t| face_ids.get(&key(t)).copied())
                .unwrap_or(0);
            let hide = [[a, b, c], [a, c, d], [a, b, d], [b, c, d]]
                .iter()
                .any(|&t| hidden.get(&key(t)).copied().unwrap_or(false));
            for q in [
                [a, ab, center, da],
                [ab, b, bc, center],
                [center, bc, c, cd],
                [da, center, cd, d],
            ] {
                result.quads.push(q);
                let [a, b, c, d] = q;
                result.triangles.extend([[a, b, c], [a, c, d]]);
                if !self.polygroups.is_empty() {
                    result.polygroups.extend([id; 2]);
                }
                if !self.hidden_faces.is_empty() {
                    result.hidden_faces.extend([hide; 2]);
                }
            }
        }
        for &[a, b] in &self.creases {
            check_cancel(cancel)?;
            let m = edges[&(a.min(b), a.max(b))];
            result
                .creases
                .extend([super::edges::edge(a, m), super::edges::edge(m, b)]);
        }
        result.creases.sort_unstable();
        check_cancel(cancel)?;
        result.validate()?;
        check_cancel(cancel)?;
        Ok(result)
    }
    pub(crate) fn smooth_remesh(&mut self) {
        // Mild relaxation regularizes the extracted triangulation. Surface
        // placement comes from the signed-distance field, not this smoothing.
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
    #[test]
    fn primitive_quads_survive_subdivision_and_serialization() {
        use crate::sculpt::{Primitive, PrimitiveKind};
        for kind in [
            PrimitiveKind::Cube,
            PrimitiveKind::Grid,
            PrimitiveKind::UvSphere,
            PrimitiveKind::Cylinder,
            PrimitiveKind::Torus,
        ] {
            let primitive = Primitive::for_kind(kind);

            let mesh = primitive.mesh().unwrap();
            assert!(!mesh.quads.is_empty());
            let refined = mesh.subdivide().unwrap();
            assert_eq!(refined.quads.len(), mesh.quads.len() * 4);
            assert_eq!(refined.triangles.len(), mesh.triangles.len() * 4);
            refined.validate().unwrap();
            let saved = serde_json::to_string(&refined).unwrap();
            assert_eq!(serde_json::from_str::<Mesh>(&saved).unwrap(), refined);
            if kind != PrimitiveKind::Grid {
                refined.validate_closed().unwrap();
            }
        }
    }
    #[test]
    fn triangle_primitive_option_and_legacy_meshes_are_supported() {
        use crate::sculpt::Primitive;
        let mut p = Primitive {
            divisions: [1; 3],
            ..Default::default()
        };
        let quad_mesh = p.mesh().unwrap();
        p.triangulated = true;
        let triangle_mesh = p.mesh().unwrap();
        assert!(triangle_mesh.quads.is_empty());
        assert_eq!(triangle_mesh.triangles, quad_mesh.triangles);
        let mut legacy = serde_json::to_value(&quad_mesh).unwrap();
        legacy.as_object_mut().unwrap().remove("quads");
        let legacy: Mesh = serde_json::from_value(legacy).unwrap();
        legacy.validate().unwrap();
        assert!(legacy.quads.is_empty());
        let mut invalid = quad_mesh;
        invalid.quads[0][1] = invalid.quads[0][0];
        assert!(invalid.validate().is_err());
    }
    fn tetrahedron() -> Mesh {
        Mesh {
            creases: Vec::new(),
            materials: Vec::new(),
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            quads: Vec::new(),
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

fn check_cancel(cancel: &impl Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Subdivision cancelled".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[test]
    fn cancellation_during_subdivision_keeps_the_geometry_and_levels() {
        let mut geometry = Geometry::new(Mesh::sphere());
        let before = geometry.clone();
        let checks = std::cell::Cell::new(0);
        let result = geometry.subdivide_cancellable(&|| {
            checks.set(checks.get() + 1);
            checks.get() > 30
        });
        assert!(result.unwrap_err().contains("cancelled"));
        assert_eq!(geometry, before);
        assert!(checks.get() > 30);
    }
}

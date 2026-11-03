//! Linear polygon subdivision and a detail-preserving multiresolution stack.
use super::{AdaptiveSettings, Geometry, Mesh, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub(crate) use super::capacity::{max_triangles, max_vertices};
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
    #[serde(default = "super::Topology::triangles")]
    pub remesh_topology: super::Topology,
    #[serde(
        default,
        skip_serializing_if = "super::QuadRemeshAlgorithm::is_default"
    )]
    pub quad_remesh_algorithm: super::QuadRemeshAlgorithm,
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
            remesh_topology: super::Topology::Quads,
            quad_remesh_algorithm: super::QuadRemeshAlgorithm::default(),
            adaptive: AdaptiveSettings::default(),
        }
    }
}
impl MeshSettings {
    pub(crate) fn legacy_default() -> Self {
        Self {
            remesh_topology: super::Topology::Triangles,
            ..Self::default()
        }
    }
    pub(crate) fn is_legacy_default(&self) -> bool {
        *self == Self::legacy_default()
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn level(&self) -> usize {
        self.lower.len() + 1
    }
    pub fn levels(&self) -> usize {
        self.lower.len() + self.upper.len() + 1
    }
    pub(super) fn validate_stack_memory(&self, mesh: &Mesh) -> Result<(), String> {
        let vertices = self
            .lower
            .iter()
            .chain(std::iter::once(mesh))
            .chain(&self.upper)
            .map(|m| m.vertices.len())
            .sum::<usize>();
        if vertices > max_vertices() * 2 {
            return Err("Subdivision stack exceeds memory limit".into());
        }
        Ok(())
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
            && (self.levels() != 1 || self.dynamic || self.adaptive.enabled || {
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
            if self.adaptive.topology == super::Topology::Quads {
                mesh.require_adaptive_quads()?;
            }
        }
        let levels: Vec<&Mesh> = self
            .lower
            .iter()
            .chain(std::iter::once(mesh))
            .chain(self.upper.iter().rev())
            .collect();
        self.validate_stack_memory(mesh)?;
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
            pair[0].validate_subdivision_topology(pair[1], cancel)?;
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
        self.select_level(level).map(|_| ())
    }
    /// Returns whether coarse edits changed any stored higher-level attributes.
    pub(crate) fn select_level(&mut self, level: usize) -> Result<bool, String> {
        let mut propagated = false;
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
            propagated |= self.mesh.propagate_level_edits(&mut higher)?;
            self.settings
                .lower
                .push(std::mem::replace(&mut self.mesh, higher));
        }
        Ok(propagated)
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
enum RefinementStep {
    Edge([usize; 2]),
    Center([usize; 4]),
    Triangles {
        parent: usize,
        faces: [[usize; 3]; 4],
    },
    Quads {
        parent: usize,
        faces: [[usize; 4]; 4],
    },
}
struct RefinementIndices {
    next: usize,
    midpoints: HashMap<(usize, usize), usize>,
}
impl RefinementIndices {
    fn midpoint(
        &mut self,
        x: usize,
        y: usize,
        emit: &mut impl FnMut(RefinementStep) -> Result<(), String>,
    ) -> Result<usize, String> {
        let edge = (x.min(y), x.max(y));
        match self.midpoints.entry(edge) {
            std::collections::hash_map::Entry::Occupied(entry) => Ok(*entry.get()),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let index = self.next;
                self.next += 1;
                emit(RefinementStep::Edge([x, y]))?;
                entry.insert(index);
                Ok(index)
            }
        }
    }
}
fn refined_position(values: &[Vec3], parents: &[usize]) -> Vec3 {
    match *parents {
        [a, b] => (values[a] + values[b]) * 0.5,
        [a, b, c, d] => (values[a] + values[b] + values[c] + values[d]) * 0.25,
        _ => unreachable!("subdivision creates edge midpoints and quad centres"),
    }
}
fn refined_weight(values: &[f32], parents: &[usize]) -> f32 {
    match *parents {
        [a, b] => (values[a] + values[b]) * 0.5,
        [a, b, c, d] => (values[a] + values[b] + values[c] + values[d]) * 0.25,
        _ => unreachable!("subdivision creates edge midpoints and quad centres"),
    }
}
impl Mesh {
    fn matches_higher_prefix(&self, higher: &Self) -> bool {
        let count = self.vertices.len();
        higher.vertices.get(..count) == Some(self.vertices.as_slice())
            && higher.mask.get(..count) == Some(self.mask.as_slice())
            && (self.materials.is_empty() && higher.materials.is_empty()
                || higher.materials.get(..count).is_some_and(|prefix| {
                    super::Material::slices_bitwise_equal(&self.materials, prefix)
                }))
            && self.vertex_groups.len() == higher.vertex_groups.len()
            && self
                .vertex_groups
                .iter()
                .zip(&higher.vertex_groups)
                .all(|(low, high)| high.weights.get(..count) == Some(low.weights.as_slice()))
    }

    /// Transfer edits through the subdivision stencils while keeping the saved
    /// high-resolution detail and topology. Parent values are updated last.
    fn propagate_level_edits(&self, higher: &mut Self) -> Result<bool, String> {
        if self.matches_higher_prefix(higher) {
            return Ok(false);
        }
        self.validate()?;
        let count = self.vertices.len();
        if higher.vertices.len() < count || higher.vertex_groups.len() != self.vertex_groups.len() {
            return Err("Subdivision levels have incompatible topology".into());
        }
        let old_painted = !higher.materials.is_empty();
        let new_painted = !self.materials.is_empty();
        let paint = old_painted || new_painted;
        if paint && !old_painted {
            higher
                .materials
                .resize(higher.vertices.len(), super::Material::default());
        }
        let mut index = count;
        self.refine_topology(&|| false, |step| {
            let parents: &[usize] = match &step {
                RefinementStep::Edge(parents) => parents,
                RefinementStep::Center(parents) => parents,
                RefinementStep::Triangles { .. } | RefinementStep::Quads { .. } => return Ok(()),
            };
            if index >= higher.vertices.len() {
                return Err("Subdivision levels have incompatible topology".into());
            }
            let old = refined_position(&higher.vertices, parents);
            let new = refined_position(&self.vertices, parents);
            higher.vertices[index] = higher.vertices[index] + (new - old);
            higher.mask[index] = (higher.mask[index] + refined_weight(&self.mask, parents)
                - refined_weight(&higher.mask, parents))
            .clamp(0., 1.);
            for (low, high) in self.vertex_groups.iter().zip(&mut higher.vertex_groups) {
                high.weights[index] = (high.weights[index] + refined_weight(&low.weights, parents)
                    - refined_weight(&high.weights, parents))
                .clamp(0., 1.);
            }
            if paint {
                let weight = 1. / parents.len() as f32;
                let old = if old_painted {
                    super::Material::blend(parents.iter().map(|&i| (higher.material(i), weight)))
                } else {
                    super::Material::default()
                };
                let new = if new_painted {
                    super::Material::blend(parents.iter().map(|&i| (self.material(i), weight)))
                } else {
                    super::Material::default()
                };
                higher.materials[index] = higher.material(index).delta(new, old);
            }
            index += 1;
            Ok(())
        })?;
        if index != higher.vertices.len() {
            return Err("Subdivision levels have incompatible topology".into());
        }
        for i in 0..count {
            if paint {
                let old = higher.material(i);
                higher.materials[i] = old.clone().delta(self.material(i), old);
            }
            higher.vertices[i] = higher.vertices[i] + (self.vertices[i] - higher.vertices[i]);
            higher.mask[i] = (higher.mask[i] + self.mask[i] - higher.mask[i]).clamp(0., 1.);
            for (low, high) in self.vertex_groups.iter().zip(&mut higher.vertex_groups) {
                high.weights[i] =
                    (high.weights[i] + low.weights[i] - high.weights[i]).clamp(0., 1.);
            }
        }
        higher.validate()?;
        Ok(true)
    }

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
        if self.triangles.len() > max_triangles() / 4 {
            return Err(format!(
                "Subdivision would exceed the configured triangle limit ({})",
                max_triangles()
            ));
        }
        let mut result = Self {
            uv: Default::default(),
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
        let edges = self.refine_topology(cancel, |step| {
            match step {
                RefinementStep::Edge([x, y]) => {
                    result
                        .vertices
                        .push((self.vertices[x] + self.vertices[y]) * 0.5);
                    result.mask.push((self.mask[x] + self.mask[y]) * 0.5);
                    result.append_material(self, &[(x, 0.5), (y, 0.5)]);
                    for (group, old) in result.vertex_groups.iter_mut().zip(&self.vertex_groups) {
                        group.weights.push((old.weights[x] + old.weights[y]) * 0.5);
                    }
                }
                RefinementStep::Center([a, b, c, d]) => {
                    result.append_material(self, &[(a, 0.25), (b, 0.25), (c, 0.25), (d, 0.25)]);
                    result.vertices.push(
                        (self.vertices[a] + self.vertices[b] + self.vertices[c] + self.vertices[d])
                            * 0.25,
                    );
                    result
                        .mask
                        .push((self.mask[a] + self.mask[b] + self.mask[c] + self.mask[d]) * 0.25);
                    for (group, old) in result.vertex_groups.iter_mut().zip(&self.vertex_groups) {
                        group.weights.push(
                            (old.weights[a] + old.weights[b] + old.weights[c] + old.weights[d])
                                * 0.25,
                        );
                    }
                }
                RefinementStep::Triangles { parent, faces } => {
                    result.triangles.extend(faces);
                    if !self.polygroups.is_empty() {
                        result.polygroups.extend([self.polygroups[parent]; 4]);
                    }
                    if !self.hidden_faces.is_empty() {
                        result.hidden_faces.extend([self.hidden_faces[parent]; 4]);
                    }
                }
                RefinementStep::Quads { parent, faces } => {
                    let [a, b, c, d] = self.quads[parent];
                    let id = [[a, b, c], [a, c, d], [a, b, d], [b, c, d]]
                        .into_iter()
                        .find_map(|t| face_ids.get(&key(t)).copied())
                        .unwrap_or(0);
                    let hide = [[a, b, c], [a, c, d], [a, b, d], [b, c, d]]
                        .iter()
                        .any(|&t| hidden.get(&key(t)).copied().unwrap_or(false));
                    for q in faces {
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
            }
            Ok(())
        })?;
        for &[a, b] in &self.creases {
            check_cancel(cancel)?;
            let m = edges.midpoints[&(a.min(b), a.max(b))];
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
    /// Check the exact index sequence without rebuilding positions, materials, masks or UV data.
    fn validate_subdivision_topology(
        &self,
        higher: &Self,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        let incompatible = || "Subdivision levels have incompatible topology".to_owned();
        let mut cursor = 0;
        let indices = self.refine_topology(cancel, |step| {
            match step {
                RefinementStep::Triangles { faces, .. } => {
                    if higher.triangles.get(cursor..cursor + 4) != Some(faces.as_slice()) {
                        return Err(incompatible());
                    }
                    cursor += 4;
                }
                RefinementStep::Quads { faces, .. } => {
                    for [a, b, c, d] in faces {
                        let triangles = [[a, b, c], [a, c, d]];
                        if higher.triangles.get(cursor..cursor + 2) != Some(triangles.as_slice()) {
                            return Err(incompatible());
                        }
                        cursor += 2;
                    }
                }
                RefinementStep::Edge(_) | RefinementStep::Center(_) => {}
            }
            Ok(())
        })?;
        if indices.next != higher.vertices.len() || cursor != higher.triangles.len() {
            return Err(incompatible());
        }
        Ok(())
    }

    fn refine_topology(
        &self,
        cancel: &impl Fn() -> bool,
        mut emit: impl FnMut(RefinementStep) -> Result<(), String>,
    ) -> Result<RefinementIndices, String> {
        check_cancel(cancel)?;
        if self.triangles.is_empty() {
            return Err("Mesh has no faces to subdivide".into());
        }
        if self.triangles.len() > max_triangles() / 4 {
            return Err(format!(
                "Subdivision would exceed the configured triangle limit ({})",
                max_triangles()
            ));
        }
        let key = |mut t: [usize; 3]| {
            t.sort_unstable();
            t
        };
        let mut polygons = std::collections::HashSet::new();
        for &[a, b, c, d] in &self.quads {
            check_cancel(cancel)?;
            polygons.extend([[a, b, c], [a, c, d], [a, b, d], [b, c, d]].map(key));
        }
        let mut indices = RefinementIndices {
            next: self.vertices.len(),
            midpoints: HashMap::new(),
        };
        for (parent, &[a, b, c]) in self.triangles.iter().enumerate() {
            check_cancel(cancel)?;
            if polygons.contains(&key([a, b, c])) {
                continue;
            }
            let ab = indices.midpoint(a, b, &mut emit)?;
            let bc = indices.midpoint(b, c, &mut emit)?;
            let ca = indices.midpoint(c, a, &mut emit)?;
            emit(RefinementStep::Triangles {
                parent,
                faces: [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]],
            })?;
        }
        for (parent, &[a, b, c, d]) in self.quads.iter().enumerate() {
            check_cancel(cancel)?;
            let ab = indices.midpoint(a, b, &mut emit)?;
            let bc = indices.midpoint(b, c, &mut emit)?;
            let cd = indices.midpoint(c, d, &mut emit)?;
            let da = indices.midpoint(d, a, &mut emit)?;
            let center = indices.next;
            indices.next += 1;
            emit(RefinementStep::Center([a, b, c, d]))?;
            emit(RefinementStep::Quads {
                parent,
                faces: [
                    [a, ab, center, da],
                    [ab, b, bc, center],
                    [center, bc, c, cd],
                    [da, center, cd, d],
                ],
            })?;
        }
        if indices.next > max_vertices() {
            return Err(format!(
                "Mesh exceeds configured limits ({} vertices, {} triangles). Increase Settings → Mesh capacity and restart.",
                max_vertices(),
                max_triangles()
            ));
        }
        check_cancel(cancel)?;
        Ok(indices)
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
    fn streamed_subdivision_validation_matches_triangle_quad_and_mixed_layouts() {
        use crate::sculpt::{Primitive, PrimitiveKind};
        let quad = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
        let tri = tetrahedron();
        let mixed = quad.merged_with(&tri).unwrap();
        for source in [quad, tri, mixed] {
            let higher = source.subdivide().unwrap();
            source
                .validate_subdivision_topology(&higher, &|| false)
                .unwrap();
            let mut invalid = higher.clone();
            invalid.triangles.last_mut().unwrap().swap(0, 1);
            assert!(
                source
                    .validate_subdivision_topology(&invalid, &|| false)
                    .is_err()
            );
            let mut invalid = higher;
            invalid.vertices.push(Vec3::default());
            invalid.mask.push(0.);
            assert!(
                source
                    .validate_subdivision_topology(&invalid, &|| false)
                    .is_err()
            );
        }
    }
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
            uv: Default::default(),
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
    fn reference_level_transfer(low: &Mesh, mut high: Mesh) -> Mesh {
        let count = low.vertices.len();
        let mut baseline = low.clone();
        baseline.vertices.copy_from_slice(&high.vertices[..count]);
        baseline.mask.copy_from_slice(&high.mask[..count]);
        baseline.materials = high.reindex_materials(0..count);
        for (base, upper) in baseline.vertex_groups.iter_mut().zip(&high.vertex_groups) {
            base.weights.copy_from_slice(&upper.weights[..count]);
        }
        let old = baseline.subdivide().unwrap();
        let new = low.subdivide().unwrap();
        if !high.materials.is_empty() || !new.materials.is_empty() {
            high.materials = (0..high.vertices.len())
                .map(|i| high.material(i).delta(new.material(i), old.material(i)))
                .collect();
        }
        for i in 0..high.vertices.len() {
            high.vertices[i] = high.vertices[i] + (new.vertices[i] - old.vertices[i]);
            high.mask[i] = (high.mask[i] + new.mask[i] - old.mask[i]).clamp(0., 1.);
            for ((high, new), old) in high
                .vertex_groups
                .iter_mut()
                .zip(&new.vertex_groups)
                .zip(&old.vertex_groups)
            {
                high.weights[i] = (high.weights[i] + new.weights[i] - old.weights[i]).clamp(0., 1.);
            }
        }
        high
    }

    #[test]
    fn direct_level_transfer_matches_reference_for_detail_masks_groups_and_paint() {
        use crate::sculpt::{Material, Primitive, PrimitiveKind};
        let quad = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
        let tri = tetrahedron();
        for mesh in [quad.clone(), tri.clone(), quad.merged_with(&tri).unwrap()] {
            for (old_paint, new_paint) in
                [(false, false), (true, true), (false, true), (true, false)]
            {
                let mut mesh = mesh.clone();
                mesh.create_vertex_group("Detail".into(), 0.35, true)
                    .unwrap();
                mesh.mask.fill(0.17);
                if old_paint {
                    mesh.materials =
                        vec![Material::from_srgb([0.3, 0.4, 0.5], 0.63, 0.2); mesh.vertices.len()];
                }
                let mut geometry = Geometry::new(mesh);
                geometry.subdivide().unwrap();
                geometry.subdivide().unwrap();
                let last = geometry.mesh.vertices.len() - 1;
                geometry.mesh.vertices[last].2 += 0.137;
                geometry.mesh.mask[last] = 0.79;
                geometry.mesh.vertex_groups[0].weights[last] = 0.83;
                let detailed = geometry.mesh.clone();
                geometry.set_level(1).unwrap();
                geometry.set_level(3).unwrap();
                assert_eq!(
                    geometry.mesh, detailed,
                    "unchanged navigation preserves all high-level detail"
                );
                geometry.set_level(1).unwrap();
                geometry.mesh.vertices[0] = geometry.mesh.vertices[0] + Vec3(0.031, -0.027, 0.019);
                geometry.mesh.mask[0] = 0.61;
                geometry.mesh.vertex_groups[0].weights[0] = 0.49;
                geometry.mesh.materials = if new_paint {
                    vec![
                        Material::from_srgb([0.7, 0.2, 0.3], 0.31, 0.8);
                        geometry.mesh.vertices.len()
                    ]
                } else {
                    Vec::new()
                };
                let mut reference = geometry.clone();
                while let Some(high) = reference.settings.upper.pop() {
                    let high = reference_level_transfer(&reference.mesh, high);
                    reference
                        .settings
                        .lower
                        .push(std::mem::replace(&mut reference.mesh, high));
                }
                geometry.set_level(3).unwrap();
                assert_eq!(
                    geometry, reference,
                    "old paint={old_paint}, new paint={new_paint}"
                );
                geometry.settings.validate(&geometry.mesh).unwrap();
            }
        }
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

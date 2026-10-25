//! Independent weighted vertex selections and categorical face groups.
use super::{Geometry, Mesh};
use serde::{Deserialize, Serialize};
fn visible_default() -> bool {
    true
}
fn is_visible(value: &bool) -> bool {
    *value
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VertexGroup {
    #[serde(default = "visible_default", skip_serializing_if = "is_visible")]
    pub visible: bool,
    pub name: String,
    pub weights: Vec<f32>,
}
pub fn mask_selected(value: f32, threshold: f32) -> bool {
    value > 0. && value >= threshold
}
impl Mesh {
    pub fn face_visible(&self, face: usize) -> bool {
        let id = self.polygroups.get(face).copied().unwrap_or(0);
        !self.hidden_faces.get(face).copied().unwrap_or(false)
            && (id == 0 || self.vertex_groups[id - 1].visible)
    }
    /// Shared boundary vertices remain fixed while either adjoining face is hidden.
    pub(crate) fn sculptable_vertices(&self) -> Vec<bool> {
        let mut visible = vec![true; self.vertices.len()];
        if !self.has_hidden_faces() {
            return visible;
        }
        for (i, t) in self.triangles.iter().enumerate() {
            if !self.face_visible(i) {
                for &v in t {
                    visible[v] = false;
                }
            }
        }
        visible
    }
    pub fn starting_sphere() -> Self {
        let mut mesh = Self::sphere();
        mesh.create_vertex_group("Base".into(), 0.5, true)
            .expect("starting group is valid");
        mesh
    }
    pub fn validate_groups(&self) -> Result<(), String> {
        if (!self.hidden_faces.is_empty() && self.hidden_faces.len() != self.triangles.len())
            || self.vertex_groups.len() > 64
            || (!self.polygroups.is_empty() && self.polygroups.len() != self.triangles.len())
        {
            return Err("Invalid group data size".into());
        }
        let mut names = std::collections::HashSet::new();
        for g in &self.vertex_groups {
            if g.name.trim().is_empty()
                || g.name.len() > 128
                || !names.insert(&g.name)
                || g.weights.len() != self.vertices.len()
                || g.weights
                    .iter()
                    .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            {
                return Err("Invalid vertex group".into());
            }
        }
        if self
            .polygroups
            .iter()
            .any(|&id| id > self.vertex_groups.len())
        {
            return Err("Invalid vertex group face reference".into());
        }
        Ok(())
    }
    /// Muted, deliberately separated hues shared by viewport faces and UI swatches.
    /// Registry order avoids collisions between similarly named groups and keeps
    /// a group's color unchanged when renamed. Eight shade sets cover all 64 groups.
    pub fn group_color(index: usize) -> [f32; 3] {
        const PALETTE: [[f32; 3]; 8] = [
            [0.76, 0.49, 0.52], // rose
            [0.45, 0.64, 0.78], // blue
            [0.57, 0.73, 0.56], // sage
            [0.67, 0.55, 0.78], // lavender
            [0.80, 0.71, 0.45], // ochre
            [0.45, 0.73, 0.70], // teal
            [0.80, 0.60, 0.44], // peach
            [0.73, 0.51, 0.68], // mauve
        ];
        const SHADES: [f32; 8] = [0., 0.06, -0.06, 0.12, -0.12, 0.03, -0.03, 0.09];
        PALETTE[index % PALETTE.len()].map(|v| v + SHADES[(index / PALETTE.len()) % SHADES.len()])
    }
    pub fn canonicalize_polygroups(&mut self) {
        if self.polygroups.iter().all(|&id| id == 0) {
            self.polygroups.clear();
        }
    }
    pub fn unique_group_name(&self, base: &str) -> String {
        if !self.vertex_groups.iter().any(|g| g.name == base) {
            return base.into();
        }
        for n in 2.. {
            let name = format!("{base} {n}");
            if !self.vertex_groups.iter().any(|g| g.name == name) {
                return name;
            }
        }
        unreachable!()
    }
    pub fn create_vertex_group(
        &mut self,
        name: String,
        threshold: f32,
        all: bool,
    ) -> Result<usize, String> {
        self.create_group_with_limit(name, threshold, all, 64)
    }
    // Replacement may briefly need one extra slot before empty groups are removed.
    pub(crate) fn create_replacement_group(
        &mut self,
        name: String,
        threshold: f32,
        all: bool,
    ) -> Result<usize, String> {
        self.create_group_with_limit(name, threshold, all, 65)
    }
    fn create_group_with_limit(
        &mut self,
        name: String,
        threshold: f32,
        all: bool,
        limit: usize,
    ) -> Result<usize, String> {
        let name = name.trim().to_string();
        if name.is_empty()
            || name.len() > 128
            || self.vertex_groups.len() >= limit
            || self.vertex_groups.iter().any(|g| g.name == name)
        {
            return Err("Choose a unique group name (up to 128 bytes; maximum 64 groups)".into());
        }
        let weights = self
            .mask
            .iter()
            .map(|&m| {
                if all {
                    1.
                } else if mask_selected(m, threshold) {
                    m
                } else {
                    0.
                }
            })
            .collect();
        let index = self.vertex_groups.len();
        self.vertex_groups.push(VertexGroup {
            visible: true,
            name,
            weights,
        });
        self.assign_group_faces(index, threshold, all, false);
        Ok(index)
    }
    fn assign_group_faces(&mut self, index: usize, threshold: f32, all: bool, remove: bool) {
        if self.polygroups.is_empty() {
            self.polygroups = vec![0; self.triangles.len()];
        }
        let old = self.polygroups.clone();
        for (i, t) in self.triangles.iter().enumerate() {
            let selected = if remove {
                t.iter().any(|&v| mask_selected(self.mask[v], threshold))
            } else {
                t.iter().all(|&v| mask_selected(self.mask[v], threshold))
            };
            if all || selected {
                if !remove {
                    self.polygroups[i] = index + 1;
                } else if self.polygroups[i] == index + 1 {
                    self.polygroups[i] = 0;
                }
            }
        }
        let key = |mut t: [usize; 3]| {
            t.sort_unstable();
            t
        };
        let face_index: std::collections::HashMap<_, _> = self
            .triangles
            .iter()
            .enumerate()
            .map(|(i, &t)| (key(t), i))
            .collect();
        for &[a, b, c, d] in &self.quads {
            let selected = if remove {
                [a, b, c, d]
                    .iter()
                    .any(|&v| mask_selected(self.mask[v], threshold))
            } else {
                [a, b, c, d]
                    .iter()
                    .all(|&v| mask_selected(self.mask[v], threshold))
            };
            for t in [[a, b, c], [a, c, d], [a, b, d], [b, c, d]] {
                if let Some(&i) = face_index.get(&key(t)) {
                    if !all && !selected {
                        self.polygroups[i] = old[i];
                    } else if remove && old[i] == index + 1 {
                        self.polygroups[i] = 0;
                    }
                }
            }
        }
        self.canonicalize_polygroups();
    }
    pub fn assign_mask_to_group(
        &mut self,
        index: usize,
        threshold: f32,
        remove: bool,
    ) -> Result<(), String> {
        let group = self
            .vertex_groups
            .get_mut(index)
            .ok_or("Vertex group no longer exists")?;
        for (w, &m) in group.weights.iter_mut().zip(&self.mask) {
            if mask_selected(m, threshold) {
                *w = if remove { 0. } else { w.max(m) };
            }
        }
        self.assign_group_faces(index, threshold, false, remove);
        Ok(())
    }
    /// Face regions own their boundary vertices. Those corners may be shared by
    /// neighboring groups, but reassigned interiors must not retain old weights.
    pub(crate) fn replace_group_memberships(
        &mut self,
        index: usize,
        threshold: f32,
        remove: bool,
        previous: &[VertexGroup],
        previous_labels: &[usize],
    ) {
        let mut required = vec![std::collections::BTreeSet::new(); self.vertex_groups.len()];
        for (face, triangle) in self.triangles.iter().enumerate() {
            if let Some(id) = self.polygroups.get(face).copied().filter(|&id| id > 0) {
                required[id - 1].extend(triangle.iter().copied());
            }
        }
        for (g, group) in self.vertex_groups.iter_mut().enumerate() {
            if (remove && g != index) || (!remove && g == index) {
                continue;
            }
            // Older assignments could leave weights beyond the visible region.
            // Once its last face is reassigned, that region has no remainder.
            if required[g].is_empty() && previous_labels.contains(&(g + 1)) {
                group.weights.fill(0.);
                continue;
            }
            for (v, weight) in group.weights.iter_mut().enumerate() {
                if mask_selected(self.mask[v], threshold) {
                    *weight = if required[g].contains(&v) {
                        previous.get(g).map_or(0., |old| old.weights[v])
                    } else {
                        0.
                    };
                }
            }
        }
    }
    pub fn mask_from_group(&mut self, index: usize) -> Result<(), String> {
        self.mask = self
            .vertex_groups
            .get(index)
            .ok_or("Vertex group no longer exists")?
            .weights
            .clone();
        Ok(())
    }
    pub fn remove_vertex_group(&mut self, index: usize) -> Result<(), String> {
        if index >= self.vertex_groups.len() {
            return Err("Vertex group no longer exists".into());
        }
        self.vertex_groups.remove(index);
        for id in &mut self.polygroups {
            if *id == index + 1 {
                *id = 0;
            } else if *id > index + 1 {
                *id -= 1;
            }
        }
        self.canonicalize_polygroups();
        Ok(())
    }
    pub fn rename_vertex_group(&mut self, index: usize, name: String) -> Result<(), String> {
        let name = name.trim().to_string();
        if name.is_empty()
            || name.len() > 128
            || self
                .vertex_groups
                .iter()
                .enumerate()
                .any(|(i, g)| i != index && g.name == name)
        {
            return Err("Choose a unique group name (up to 128 bytes)".into());
        }
        self.vertex_groups
            .get_mut(index)
            .ok_or("Vertex group no longer exists")?
            .name = name;
        Ok(())
    }
    /// Half a step toward the mean of unique polygon-edge neighbours.
    /// Read the original mask throughout, so face order cannot bias the blur.
    pub fn blur_mask(&mut self) {
        let key = |a, b| if a < b { (a, b) } else { (b, a) };
        let diagonals: std::collections::HashSet<_> = self
            .quads
            .iter()
            .flat_map(|&[a, b, c, d]| [key(a, c), key(b, d)])
            .collect();
        let mut edges: Vec<_> = self
            .triangles
            .iter()
            .flat_map(|&[a, b, c]| [key(a, b), key(b, c), key(c, a)])
            .filter(|edge| !diagonals.contains(edge))
            .collect();
        edges.sort_unstable();
        edges.dedup();
        let mut changes = vec![(0_f64, 0_usize); self.mask.len()];
        for (a, b) in edges {
            let delta = self.mask[b] as f64 - self.mask[a] as f64;
            changes[a].0 += delta;
            changes[a].1 += 1;
            changes[b].0 -= delta;
            changes[b].1 += 1;
        }
        for (value, (delta, count)) in self.mask.iter_mut().zip(changes) {
            if count > 0 {
                *value = (*value as f64 + 0.5 * delta / count as f64).clamp(0., 1.) as f32;
            }
        }
    }
    /// Double contrast around 50%, keeping fully clear/protected vertices fixed.
    pub fn sharpen_mask(&mut self) {
        for value in &mut self.mask {
            *value = (2. * *value - 0.5).clamp(0., 1.);
        }
    }
    /// One topological ring per click; soft weights propagate by local extrema.
    pub fn grow_mask(&mut self, grow: bool) {
        let old = self.mask.clone();
        let key = |a, b| if a < b { (a, b) } else { (b, a) };
        let hidden: std::collections::HashSet<_> = self
            .quads
            .iter()
            .flat_map(|&[a, b, c, d]| [key(a, c), key(b, d)])
            .collect();
        for &[a, b, c] in &self.triangles {
            for (i, j) in [(a, b), (b, c), (c, a)] {
                if hidden.contains(&key(i, j)) {
                    continue;
                }
                if grow {
                    self.mask[i] = self.mask[i].max(old[j]);
                    self.mask[j] = self.mask[j].max(old[i]);
                } else {
                    self.mask[i] = self.mask[i].min(old[j]);
                    self.mask[j] = self.mask[j].min(old[i]);
                }
            }
        }
    }
    pub(crate) fn reindex_groups(
        &self,
        result: &mut Mesh,
        old_vertices: &[usize],
        old_faces: &[usize],
    ) {
        if self.uv.current(self) {
            result.uv.textures = self.uv.textures.clone();
            result.uv.corners = old_faces.iter().map(|&f| self.uv.corners[f]).collect();
            result.uv.topology = result.triangles.clone();
        }
        result.materials = self.reindex_materials(old_vertices.iter().copied());
        result.vertex_groups = self
            .vertex_groups
            .iter()
            .map(|g| VertexGroup {
                visible: g.visible,
                name: g.name.clone(),
                weights: old_vertices.iter().map(|&i| g.weights[i]).collect(),
            })
            .collect();
        result.polygroups = if self.polygroups.is_empty() {
            Vec::new()
        } else {
            old_faces.iter().map(|&i| self.polygroups[i]).collect()
        };
        result.hidden_faces = if self.hidden_faces.is_empty() {
            Vec::new()
        } else {
            old_faces.iter().map(|&i| self.hidden_faces[i]).collect()
        };
        let mut indices = vec![usize::MAX; self.vertices.len()];
        for (new, &old) in old_vertices.iter().enumerate() {
            indices[old] = new;
        }
        self.reindex_creases(result, &indices);
        result.canonicalize_polygroups();
    }
}

impl Geometry {
    /// Compact one shared registry across every stored subdivision level.
    pub(crate) fn compact_vertex_groups(&mut self) -> Vec<Option<usize>> {
        let mut used = vec![false; self.mesh.vertex_groups.len()];
        for mesh in std::iter::once(&self.mesh)
            .chain(self.settings.lower.iter())
            .chain(self.settings.upper.iter())
        {
            for (g, group) in mesh.vertex_groups.iter().enumerate() {
                used[g] |= group.weights.iter().any(|&w| w > 0.);
            }
            for &id in &mesh.polygroups {
                if id > 0 {
                    used[id - 1] = true;
                }
            }
        }
        let mut next = 0;
        let mapping: Vec<_> = used
            .iter()
            .map(|&keep| {
                if keep {
                    let index = next;
                    next += 1;
                    Some(index)
                } else {
                    None
                }
            })
            .collect();
        for mesh in std::iter::once(&mut self.mesh)
            .chain(self.settings.lower.iter_mut())
            .chain(self.settings.upper.iter_mut())
        {
            mesh.vertex_groups = mesh
                .vertex_groups
                .drain(..)
                .enumerate()
                .filter_map(|(g, group)| used[g].then_some(group))
                .collect();
            for id in &mut mesh.polygroups {
                if *id > 0 {
                    *id = mapping[*id - 1].map_or(0, |g| g + 1);
                }
            }
            mesh.canonicalize_polygroups();
        }
        mapping
    }
}

//! Explicit polygon edits. Results are built separately so failed previews never mutate a mesh.
use super::{
    Mesh, Vec3,
    edges::{Polygon, edge},
    max_triangles, max_vertices,
};
use std::collections::{BTreeMap, BTreeSet};
mod bevel;
mod components;
mod delete;
mod detached;
mod move_weld;
mod snap_join;
pub use components::{ComponentBuild, ComponentBuildOptions};
mod construction;
mod contact;
pub use construction::Construction;
mod dissolve;
mod join;
mod loops;
mod notch;
mod selection;
mod symmetry;
mod topology;
mod transform;
pub use loops::LoopCut;
pub use symmetry::ModelingSymmetry;
pub use topology::TopologyEdit;
pub use transform::ComponentTransform;

#[derive(Clone, Copy)]
pub struct ExtrudeContact {
    pub join: bool,
    pub loops: bool,
    pub snap: f32,
}
/// Source-space polygons remain identifiable after a join compacts the mesh.
#[derive(Default)]
pub(crate) struct FusePreview {
    pub targets: Vec<Vec<Vec3>>,
    pub walls: Vec<Vec<Vec3>>,
    pub loop_edges: Vec<[Vec3; 2]>,
}

pub struct Extrusion {
    pub mesh: Mesh,
    pub distance: f32,
    pub joined: bool,
    pub collapsed: bool,
    /// At least one region snapped while another continued freely.
    pub partial_contact: bool,
    /// Regions moved different distances; `distance` retains the requested amount.
    pub varying_distance: bool,
    pub rim: Vec<[Vec3; 2]>,
    pub(crate) preview: FusePreview,
}

fn normal(mesh: &Mesh, polygon: &Polygon) -> Vec3 {
    let origin = mesh.vertices[polygon.vertices[0]];
    polygon
        .edges()
        .fold(Vec3::default(), |n, (a, b)| {
            n + (mesh.vertices[a] - origin).cross(mesh.vertices[b] - origin)
        })
        .norm()
}

/// Check surface fans, orientation, duplicate faces and area as well as schema limits.
fn manifold(mesh: &Mesh) -> Result<(), String> {
    mesh.validate()?;
    let mut incidence = BTreeMap::<_, Vec<_>>::new();
    let mut unique = BTreeSet::new();
    for p in mesh.polygons() {
        let mut key = p.vertices.clone();
        key.sort_unstable();
        if key.windows(2).any(|v| v[0] == v[1]) || !unique.insert(key) {
            return Err("This edit would create overlapping or collapsed faces".into());
        }
        for (a, b) in p.edges() {
            incidence.entry(edge(a, b)).or_default().push(a);
        }
    }
    if incidence
        .values()
        .any(|v| v.len() > 2 || (v.len() == 2 && v[0] == v[1]))
    {
        return Err("Modeling requires consistently oriented manifold faces".into());
    }
    for &[a, b, c] in &mesh.triangles {
        let ab = mesh.vertices[b] - mesh.vertices[a];
        let ac = mesh.vertices[c] - mesh.vertices[a];
        if ab.cross(ac).len() <= (ab.len() * ac.len() * 1e-6).max(1e-12) {
            return Err("This edit would create a zero-area face".into());
        }
    }
    // Edge incidence alone accepts separate sheets joined at a single vertex.
    // The triangulated link must be one directed cycle (interior) or path
    // (boundary). Explicit wires and isolated vertices have no surface link.
    let corners = super::corners::VertexCorners::new(mesh)
        .ok_or("Mesh is too large to validate modeling surface fans")?;
    for vertex in 0..mesh.vertices.len() {
        let mut next = BTreeMap::new();
        let mut incoming = BTreeSet::new();
        for &corner in corners.incident(vertex) {
            let triangle = mesh.triangles[corner as usize / 3];
            let i = corner as usize % 3;
            let (a, b) = (triangle[(i + 1) % 3], triangle[(i + 2) % 3]);
            if next.insert(a, b).is_some() || !incoming.insert(b) {
                return Err("Modeling requires one connected surface fan at each vertex".into());
            }
        }
        let Some(&first) = next.keys().next() else {
            continue;
        };
        let mut cursor = next
            .keys()
            .find(|v| !incoming.contains(*v))
            .copied()
            .unwrap_or(first);
        while let Some(vertex) = next.remove(&cursor) {
            cursor = vertex;
        }
        if !next.is_empty() {
            return Err("Modeling requires one connected surface fan at each vertex".into());
        }
    }
    Ok(())
}

impl Mesh {
    /// Keep corner data on unchanged faces and translated cap copies. Newly
    /// constructed faces are explicitly unmapped. Inactive maps cannot safely
    /// identify corners after earlier compaction; retain their textures, with
    /// the original map remaining available in the geometry Undo snapshot.
    fn preserve_model_uvs(&mut self, source: &Mesh, copies: &BTreeMap<usize, usize>) {
        if source.uv.is_empty() {
            return;
        }
        let origin: BTreeMap<_, _> = copies
            .iter()
            .filter(|(_, b)| **b >= source.vertices.len())
            .map(|(&a, &b)| (b, a))
            .collect();
        let key = |mut t: [usize; 3]| {
            t.sort_unstable();
            t
        };
        let records: BTreeMap<_, _> = if source.uv.current(source) {
            source
                .triangles
                .iter()
                .enumerate()
                .map(|(i, &t)| (key(t), i))
                .collect()
        } else {
            BTreeMap::new()
        };
        self.uv = source.uv.clone();
        self.uv.corners.clear();
        self.uv.masks.clear();
        self.uv.removed.clear();
        for &triangle in &self.triangles {
            let old = triangle.map(|v| origin.get(&v).copied().unwrap_or(v));
            let record = records.get(&key(old)).copied();
            if let Some(i) = record {
                let order = old.map(|v| source.triangles[i].iter().position(|&w| w == v).unwrap());
                self.uv.corners.push(order.map(|c| source.uv.corners[i][c]));
                self.uv.masks.push(order.map(|c| source.uv.mask(i, c)));
                self.uv.removed.push(!source.uv.mapped(i));
            } else {
                self.uv.corners.push([[0.; 2]; 3]);
                self.uv.masks.push([0.; 3]);
                self.uv.removed.push(true);
            }
        }
        self.uv.topology = self.triangles.clone();
        let edges: BTreeSet<_> = self.polygon_edges().into_iter().collect();
        self.uv.seams.retain(|e| edges.contains(e));
    }

    pub fn extrude_faces(
        &self,
        selected: &[usize],
        distance: f32,
        join: bool,
    ) -> Result<Extrusion, String> {
        self.extrude_contact(
            selected,
            distance,
            ExtrudeContact {
                join,
                loops: false,
                snap: 0.04,
            },
        )
    }
    pub fn extrude_contact(
        &self,
        selected: &[usize],
        distance: f32,
        contact: ExtrudeContact,
    ) -> Result<Extrusion, String> {
        let join = contact.join;
        self.validate()?;
        if !contact.snap.is_finite() || !(0.0..=0.25).contains(&contact.snap) {
            return Err("Snap range must be between 0 and 0.25 of the local edge length".into());
        }
        if !distance.is_finite() {
            return Err("Extrusion distance must be finite".into());
        }
        let polygons = self.polygons();
        let selected: BTreeSet<_> = selected.iter().copied().collect();
        let first = selected
            .first()
            .and_then(|&i| polygons.get(i))
            .ok_or("Select faces to extrude")?;
        if selected
            .iter()
            .any(|&i| polygons.get(i).is_none_or(|p| !p.visible))
        {
            return Err("Select visible faces on the active mesh".into());
        }
        let direction = normal(self, first);
        let origin = self.vertices[first.vertices[0]];
        let size = first
            .edges()
            .map(|(a, b)| (self.vertices[a] - self.vertices[b]).len())
            .fold(0_f32, f32::max);
        if selected.iter().any(|&i| {
            normal(self, &polygons[i]).dot(direction) < 0.9999
                || polygons[i]
                    .vertices
                    .iter()
                    .any(|&v| (self.vertices[v] - origin).dot(direction).abs() > size * 1e-4)
        }) {
            return Err("Extrude a flat region with faces pointing in the same direction".into());
        }
        if distance.abs() < size * 1e-6 {
            return Ok(Extrusion {
                mesh: self.clone(),
                distance: 0.,
                joined: false,
                collapsed: false,
                partial_contact: false,
                varying_distance: false,
                rim: Vec::new(),
                preview: FusePreview::default(),
            });
        }
        manifold(self)?;
        let mut boundary = BTreeMap::new();
        let mut vertices = BTreeSet::new();
        for &i in &selected {
            let p = &polygons[i];
            vertices.extend(p.vertices.iter().copied());
            for (a, b) in p.edges() {
                let e = edge(a, b);
                if boundary.remove(&e).is_none() {
                    boundary.insert(e, (a, b, p.group));
                }
            }
        }
        let mut boundary_degree = BTreeMap::<usize, usize>::new();
        for &(a, b, _) in boundary.values() {
            *boundary_degree.entry(a).or_default() += 1;
            *boundary_degree.entry(b).or_default() += 1;
        }
        if boundary_degree.values().any(|&n| n != 2) {
            return Err("The selection touches itself at a vertex; select separate regions".into());
        }
        if boundary.is_empty() {
            return Err("The selected region has no boundary to extrude".into());
        }
        if self.vertices.len() + vertices.len() > max_vertices()
            || self.triangles.len() + boundary.len() * 2 > max_triangles()
        {
            return Err("Extrusion would exceed the mesh size limit".into());
        }
        if contact.loops && !contact::connected(&polygons, &selected) {
            return Err("Each fuse region must be connected".into());
        }
        let region = contact::ContactRegion {
            polygons: &polygons,
            selected: &selected,
            vertices: &vertices,
            boundary: &boundary,
            direction,
            distance,
            snap: contact.snap,
        };
        let mut target = if join {
            self.cap_contact(&region, size)
        } else {
            None
        };
        if contact.loops
            && distance < 0.
            && let Some(loop_target) =
                self.loop_contact(&region, target.as_ref().map(|cap| cap.depth))?
        {
            target = Some(loop_target);
        }
        if target
            .as_ref()
            .is_some_and(|t| t.caps.iter().any(|&i| !polygons[i].visible))
        {
            return Err("Show the matching target faces before joining this region".into());
        }
        let collapsed = target.as_ref().is_some_and(|t| t.caps.is_empty());
        let mut out = self.empty_topology();
        let (distance, mut mapping, removed) = if let Some(target) = target {
            (target.depth, target.mapping, target.caps)
        } else {
            (distance, BTreeMap::new(), BTreeSet::new())
        };
        // The contact rim uses existing vertices. A tiled cap may also contain
        // interior vertices with no counterpart inside the solid; translate these.
        if removed.is_empty() {
            for &v in &vertices {
                mapping.entry(v).or_insert_with(|| {
                    out.append_blend(self, &[(v, 1.)], self.vertices[v] + direction * distance)
                });
            }
        }
        let sides: Vec<_> = boundary
            .values()
            .map(|&(a, b, group)| ([a, b, mapping[&b], mapping[&a]], group))
            .collect();
        let joined_walls = if !removed.is_empty() || collapsed {
            let excluded = selected.iter().chain(&removed).copied().collect();
            Some(self.joined_walls(&polygons, &sides, &excluded, !removed.is_empty(), &mut out)?)
        } else {
            None
        };
        for (i, p) in polygons.iter().enumerate() {
            if removed.contains(&i)
                || joined_walls
                    .as_ref()
                    .is_some_and(|walls| walls.remove.contains(&i))
            {
                continue;
            }
            if selected.contains(&i) {
                if removed.is_empty() {
                    out.push_polygon(
                        &p.vertices.iter().map(|v| mapping[v]).collect::<Vec<_>>(),
                        p.group,
                        p.hidden,
                    );
                }
            } else {
                out.push_polygon(&p.vertices, p.group, p.hidden);
            }
        }
        if let Some(walls) = &joined_walls {
            for (vertices, group) in &walls.walls {
                out.push_polygon(vertices, *group, false);
            }
        } else {
            for (vertices, group) in sides {
                out.push_polygon(&vertices, group, false);
            }
        }
        if out.triangles.is_empty() {
            return Err("This join would remove the entire mesh".into());
        }
        out.creases = self.creases.clone();
        out.creases.extend(
            self.creases
                .iter()
                .filter_map(|[a, b]| Some(edge(*mapping.get(a)?, *mapping.get(b)?))),
        );
        let mut preview = FusePreview::default();
        if contact.loops
            && let Some(walls) = &joined_walls
        {
            let positions = |&f: &usize| {
                polygons[f]
                    .vertices
                    .iter()
                    .map(|&v| self.vertices[v])
                    .collect()
            };
            preview.targets = removed.iter().map(positions).collect();
            preview.walls = walls.remove.iter().map(positions).collect();
        }
        if let Some(walls) = joined_walls
            && !walls.seam.is_empty()
        {
            out.stitch_join_seam(&walls.seam, &polygons)?;
        }
        out.retain_creases();
        manifold(&out)?;
        // Remove unused interior vertices after region extrusion, including group weights.
        let rim: Vec<_> = boundary
            .values()
            .map(|&(a, b, _)| [out.vertices[mapping[&a]], out.vertices[mapping[&b]]])
            .collect();
        if collapsed {
            preview.loop_edges.clone_from(&rim);
        }
        out.preserve_model_uvs(self, &mapping);
        out.compact_model_vertices();
        Ok(Extrusion {
            mesh: out,
            distance,
            joined: !removed.is_empty(),
            collapsed,
            partial_contact: false,
            varying_distance: false,
            rim,
            preview,
        })
    }

    fn compact_model_vertices(&mut self) {
        let uv_current = self.uv.current(self);
        let shading = self.shading_edges().to_vec();
        let used: BTreeSet<_> = self
            .triangles
            .iter()
            .flatten()
            .chain(self.loose_edges.iter().flatten())
            .chain(self.loose_vertices.iter())
            .copied()
            .collect();
        let mut mapping = vec![usize::MAX; self.vertices.len()];
        let mut vertices = Vec::with_capacity(used.len());
        let mut mask = Vec::with_capacity(used.len());
        for &v in &used {
            mapping[v] = vertices.len();
            vertices.push(self.vertices[v]);
            mask.push(self.mask[v]);
        }
        self.materials = self.reindex_materials(used.iter().copied());
        for group in &mut self.vertex_groups {
            group.weights = used.iter().map(|&v| group.weights[v]).collect();
        }
        for t in &mut self.triangles {
            *t = t.map(|v| mapping[v]);
        }
        for q in &mut self.quads {
            *q = q.map(|v| mapping[v]);
        }
        for v in &mut self.loose_vertices {
            *v = mapping[*v];
        }
        for e in &mut self.loose_edges {
            *e = e.map(|v| mapping[v]);
        }
        self.creases = self
            .creases
            .iter()
            .map(|&[a, b]| edge(mapping[a], mapping[b]))
            .collect();
        self.creases.sort_unstable();
        self.vertices = vertices;
        self.mask = mask;
        self.set_shading_seams(
            shading
                .iter()
                .filter_map(|&[a, b]| {
                    (mapping[a] != usize::MAX && mapping[b] != usize::MAX)
                        .then_some(edge(mapping[a], mapping[b]))
                })
                .collect(),
        );
        if uv_current {
            self.uv.topology = self.triangles.clone();
            self.uv.seams = self
                .uv
                .seams
                .iter()
                .filter_map(|&[a, b]| {
                    (mapping[a] != usize::MAX && mapping[b] != usize::MAX)
                        .then_some(edge(mapping[a], mapping[b]))
                })
                .collect();
            self.uv.seams.sort_unstable();
        }
    }
}

#[cfg(test)]
mod invariant_tests;
#[cfg(test)]
mod notes_tests;
#[cfg(test)]
mod tests;

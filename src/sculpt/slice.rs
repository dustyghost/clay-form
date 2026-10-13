//! Plane cuts with shared edge intersections and transactional scene results.
use super::{Geometry, Mesh, MeshSettings, Submodel, Vec3, VertexGroup};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SliceMode {
    #[default]
    Polygroups,
    Trim,
    Separate,
}
impl SliceMode {
    pub const ALL: [Self; 3] = [Self::Polygroups, Self::Trim, Self::Separate];
    pub fn label(self) -> &'static str {
        match self {
            Self::Polygroups => "Polygroups",
            Self::Trim => "Trim",
            Self::Separate => "Separate",
        }
    }
    pub fn history_label(self) -> &'static str {
        match self {
            Self::Polygroups => "Slice polygroups",
            Self::Trim => "Trim surface",
            Self::Separate => "Separate sliced submodels",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct SlicePlane {
    pub origin: Vec3,
    /// Positive half-space is kept by Trim.
    pub normal: Vec3,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliceOptions {
    pub mode: SliceMode,
    pub cap: bool,
    pub flip: bool,
}
impl Default for SliceOptions {
    fn default() -> Self {
        Self {
            mode: SliceMode::Polygroups,
            cap: true,
            flip: false,
        }
    }
}
fn check_cancel(cancel: &impl Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Slice cancelled".into())
    } else {
        Ok(())
    }
}
fn key(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

impl Geometry {
    pub fn slice(
        &mut self,
        plane: SlicePlane,
        options: SliceOptions,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        self.check_slice(options)?;
        let result = self.mesh.slice(plane, options, cancel)?;
        self.commit_slice(result);
        Ok(())
    }
    pub fn slice_path(
        &mut self,
        path: &super::SlicePath,
        options: SliceOptions,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        self.check_slice(options)?;
        let result = self.mesh.slice_path(path, options, cancel)?;
        self.commit_slice(result);
        Ok(())
    }
    fn check_slice(&self, options: SliceOptions) -> Result<(), String> {
        if self.settings.primitive.is_some() {
            return Err("Convert the primitive to a sculpting mesh before slicing".into());
        }
        if !self.scene.active_metadata.visible || self.mesh.vertex_groups.iter().any(|g| !g.visible)
        {
            return Err("Show the active model and all of its polygroups before slicing".into());
        }
        if self.scene.active_metadata.temporary_thickness > 0. {
            return Err("Apply or remove temporary thickness before slicing".into());
        }
        if options.mode == SliceMode::Separate && self.submodels.len() >= 63 {
            return Err("Submodels are limited to 64".into());
        }
        Ok(())
    }
    fn commit_slice(&mut self, (mesh, other): (Mesh, Option<Mesh>)) {
        let settings = MeshSettings {
            resolution: self.settings.resolution,
            ..Default::default()
        };
        if let Some(other) = other {
            let mut number = 1;
            let name = loop {
                let name = format!("Slice {number}");
                if name != self.name && !self.submodels.iter().any(|m| m.name == name) {
                    break name;
                }
                number += 1;
            };
            self.submodels.insert(
                self.scene.active_index,
                Submodel {
                    name,
                    metadata: self.scene.active_metadata.clone(),
                    mesh: other,
                    mesh_settings: settings.clone(),
                },
            );
        }
        self.mesh = mesh;
        self.settings = settings;
    }
}
impl Mesh {
    pub fn slice(
        &self,
        plane: SlicePlane,
        options: SliceOptions,
        cancel: &impl Fn() -> bool,
    ) -> Result<(Self, Option<Self>), String> {
        self.validate()?;
        let finite = |v: Vec3| v.0.is_finite() && v.1.is_finite() && v.2.is_finite();
        if !finite(plane.origin)
            || !finite(plane.normal)
            || !plane.normal.len().is_finite()
            || plane.normal.len() < 1e-8
        {
            return Err("Invalid slice plane".into());
        }
        check_cancel(cancel)?;
        let normal = plane.normal.norm()
            * if options.flip && options.mode == SliceMode::Trim {
                -1.
            } else {
                1.
            };
        let capped = options.cap && options.mode != SliceMode::Polygroups;
        if capped {
            self.validate_closed().map_err(|_| "Capped cuts require a closed, oriented surface; apply thickness or turn off Cap cuts".to_string())?;
        }
        let (work, sides) = self.split_plane(
            SlicePlane {
                origin: plane.origin,
                normal,
            },
            cancel,
        )?;
        work.finish_slice(&sides, options, |mesh, positive| {
            if capped {
                mesh.cap_slice(normal * if positive { -1. } else { 1. }, cancel)?;
            }
            Ok(())
        })
    }
    pub(super) fn split_plane(
        &self,
        plane: SlicePlane,
        cancel: &impl Fn() -> bool,
    ) -> Result<(Self, Vec<bool>), String> {
        let normal = plane.normal;
        // Use f64 predicates to avoid cancellation at large model translations.
        let distance = |v: Vec3| {
            (v.0 as f64 - plane.origin.0 as f64) * normal.0 as f64
                + (v.1 as f64 - plane.origin.1 as f64) * normal.1 as f64
                + (v.2 as f64 - plane.origin.2 as f64) * normal.2 as f64
        };
        let extent = self
            .vertices
            .iter()
            .map(|&p| distance(p).abs())
            .fold(0_f64, f64::max);
        let eps = (extent * 1e-7).max(1e-10);
        let mut distances: Vec<_> = self
            .vertices
            .iter()
            .map(|&p| {
                let d = distance(p);
                if d.abs() <= eps { 0. } else { d }
            })
            .collect();
        let mut work = self.clone();
        work.triangles.clear();
        work.polygroups.clear();
        work.quads.clear();
        let mut sides = Vec::new();
        let mut intersections = HashMap::new();
        for (fi, &face) in self.triangles.iter().enumerate() {
            if fi % 1024 == 0 {
                check_cancel(cancel)?;
            }
            let group = self.polygroups.get(fi).copied().unwrap_or(0);
            let positive = face.iter().any(|&i| distances[i] > 0.);
            let negative = face.iter().any(|&i| distances[i] < 0.);
            if !(positive && negative) {
                work.triangles.push(face);
                work.polygroups.push(group);
                // Coplanar faces belong to one side only, preserving oriented solid boundaries.
                let coplanar_positive = (self.vertices[face[1]] - self.vertices[face[0]])
                    .cross(self.vertices[face[2]] - self.vertices[face[0]])
                    .dot(normal)
                    < 0.;
                sides.push(if positive {
                    true
                } else if negative {
                    false
                } else {
                    coplanar_positive
                });
                continue;
            }
            for side in [false, true] {
                let mut polygon = Vec::with_capacity(4);
                for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
                    let da = distances[a];
                    let db = distances[b];
                    if if side { da >= 0. } else { da <= 0. } {
                        polygon.push(a);
                    }
                    if da * db < 0. {
                        let index = *intersections.entry(key(a, b)).or_insert_with(|| {
                            let t = (da / (da - db)) as f32;
                            let index = work.vertices.len();
                            work.vertices
                                .push(self.vertices[a] + (self.vertices[b] - self.vertices[a]) * t);
                            work.mask
                                .push(self.mask[a] + (self.mask[b] - self.mask[a]) * t);
                            for (out, original) in
                                work.vertex_groups.iter_mut().zip(&self.vertex_groups)
                            {
                                out.weights.push(
                                    original.weights[a]
                                        + (original.weights[b] - original.weights[a]) * t,
                                );
                            }
                            distances.push(0.);
                            index
                        });
                        polygon.push(index);
                    }
                }
                for i in 1..polygon.len() - 1 {
                    work.triangles
                        .push([polygon[0], polygon[i], polygon[i + 1]]);
                    work.polygroups.push(group);
                    sides.push(side);
                }
            }
            if work.vertices.len() > super::subdivision::MAX_VERTICES
                || work.triangles.len() > super::subdivision::MAX_TRIANGLES
            {
                return Err("Slice exceeds the mesh size limit; reduce resolution first".into());
            }
        }
        // Complete untouched quads survive; intersected quads become triangles.
        work.quads = self
            .quads
            .iter()
            .copied()
            .filter(|q| {
                !(q.iter().any(|&i| distances[i] > 0.) && q.iter().any(|&i| distances[i] < 0.))
            })
            .collect();
        Ok((work, sides))
    }
    pub(super) fn finish_slice(
        mut self,
        sides: &[bool],
        options: SliceOptions,
        cap: impl Fn(&mut Self, bool) -> Result<(), String>,
    ) -> Result<(Self, Option<Self>), String> {
        if !sides.contains(&true) || !sides.contains(&false) {
            return Err(
                "The slice must pass through the model, leaving surface on both sides".into(),
            );
        }
        let work = &mut self;
        if options.mode == SliceMode::Polygroups {
            let negative: BTreeSet<_> = work
                .polygroups
                .iter()
                .zip(sides)
                .filter(|(_, s)| !**s)
                .map(|(&g, _)| g)
                .collect();
            let positive: BTreeSet<_> = work
                .polygroups
                .iter()
                .zip(sides)
                .filter(|(_, s)| **s)
                .map(|(&g, _)| g)
                .collect();
            // Preserve existing categories on one side and split their counterparts on the other.
            for old in positive.intersection(&negative) {
                let faces: Vec<_> = work
                    .polygroups
                    .iter()
                    .zip(sides)
                    .enumerate()
                    .filter(|(_, (g, s))| **g == *old && **s)
                    .map(|(i, _)| i)
                    .collect();
                work.group_faces(&faces, "Slice")?;
            }
            if work.polygroups.contains(&0) {
                let faces: Vec<_> = work
                    .polygroups
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| **g == 0)
                    .map(|(i, _)| i)
                    .collect();
                work.group_faces(&faces, "Slice")?;
            }
            work.retain_valid_quads();
            work.validate()?;
            return Ok((self, None));
        }
        let mut positive = work.slice_subset(sides, true);
        cap(&mut positive, true)?;
        positive.validate()?;
        if options.mode == SliceMode::Trim {
            return Ok((positive, None));
        }
        let mut negative = work.slice_subset(sides, false);
        cap(&mut negative, false)?;
        negative.validate()?;
        Ok((positive, Some(negative)))
    }
    fn group_faces(&mut self, faces: &[usize], base: &str) -> Result<(), String> {
        if self.vertex_groups.len() >= 64 {
            return Err("Slice would exceed the 64 vertex group limit".into());
        }
        let mut weights = vec![0.; self.vertices.len()];
        let id = self.vertex_groups.len() + 1;
        for &f in faces {
            self.polygroups[f] = id;
            for &v in &self.triangles[f] {
                weights[v] = 1.;
            }
        }
        self.vertex_groups.push(VertexGroup {
            visible: true,
            name: self.unique_group_name(base),
            weights,
        });
        Ok(())
    }
    fn retain_valid_quads(&mut self) {
        let face_key = |mut f: [usize; 3]| {
            f.sort_unstable();
            f
        };
        let faces: HashMap<_, _> = self
            .triangles
            .iter()
            .enumerate()
            .map(|(i, &f)| (face_key(f), self.polygroups[i]))
            .collect();
        self.quads.retain(|&[a, b, c, d]| {
            [[[a, b, c], [a, c, d]], [[a, b, d], [b, c, d]]]
                .iter()
                .any(|pair| {
                    let x = faces.get(&face_key(pair[0]));
                    let y = faces.get(&face_key(pair[1]));
                    x.is_some() && x == y
                })
        });
    }
    fn slice_subset(&self, sides: &[bool], side: bool) -> Self {
        let mut out = self.clone();
        out.triangles = self
            .triangles
            .iter()
            .zip(sides)
            .filter(|(_, s)| **s == side)
            .map(|(&t, _)| t)
            .collect();
        out.polygroups = self
            .polygroups
            .iter()
            .zip(sides)
            .filter(|(_, s)| **s == side)
            .map(|(&g, _)| g)
            .collect();
        out.retain_valid_quads();
        let mut indices = vec![usize::MAX; self.vertices.len()];
        for &v in out.triangles.iter().flatten() {
            indices[v] = 0;
        }
        let vertices: Vec<_> = indices
            .iter()
            .enumerate()
            .filter(|(_, v)| **v == 0)
            .map(|(i, _)| i)
            .collect();
        for (new, &old) in vertices.iter().enumerate() {
            indices[old] = new;
        }
        out.vertices = vertices.iter().map(|&i| self.vertices[i]).collect();
        out.mask = vertices.iter().map(|&i| self.mask[i]).collect();
        for (g, original) in out.vertex_groups.iter_mut().zip(&self.vertex_groups) {
            g.weights = vertices.iter().map(|&i| original.weights[i]).collect();
        }
        for t in &mut out.triangles {
            *t = t.map(|i| indices[i]);
        }
        for q in &mut out.quads {
            *q = q.map(|i| indices[i]);
        }
        out
    }
    fn cap_slice(&mut self, normal: Vec3, cancel: &impl Fn() -> bool) -> Result<(), String> {
        let reference = if normal.0.abs() < 0.8 {
            Vec3(1., 0., 0.)
        } else {
            Vec3(0., 1., 0.)
        };
        let u = normal.cross(reference).norm();
        let v = normal.cross(u);
        let origin = self.vertices[0];
        let coordinates: Vec<_> = self
            .vertices
            .iter()
            .map(|&p| {
                let p = p - origin;
                [p.dot(u) as f64, p.dot(v) as f64]
            })
            .collect();
        self.cap_slice_projected(&coordinates, cancel)?;
        self.validate_closed().map_err(|_| {
            "Could not close the slice cleanly; move the line slightly or turn off Cap cuts"
                .to_string()
        })
    }
    pub(super) fn cap_slice_projected(
        &mut self,
        coordinates: &[[f64; 2]],
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        let project = |i: usize| coordinates[i];
        let mut edges: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for &[a, b, c] in &self.triangles {
            for (a, b) in [(a, b), (b, c), (c, a)] {
                edges.entry(key(a, b)).or_default().push((a, b));
            }
        }
        let mut next = BTreeMap::new();
        let mut incoming = BTreeSet::new();
        for edge in edges.values() {
            if edge.len() == 1 {
                let (a, b) = edge[0];
                if next.insert(b, a).is_some() || !incoming.insert(a) {
                    return Err("Slice meets a branching boundary; move the line slightly".into());
                }
            } else if edge.len() != 2 || edge[0] != (edge[1].1, edge[1].0) {
                return Err("Slice produced a nonmanifold boundary".into());
            }
        }
        if next.is_empty() {
            return Ok(());
        }
        let mut remaining = next.clone();
        let mut loops = Vec::new();
        while let Some((&start, _)) = remaining.first_key_value() {
            check_cancel(cancel)?;
            let mut ring = vec![start];
            let mut current = start;
            loop {
                current = remaining
                    .remove(&current)
                    .ok_or("Slice has an open boundary; turn off Cap cuts")?;
                if current == start {
                    break;
                }
                ring.push(current);
                if ring.len() > next.len() {
                    return Err("Invalid slice boundary".into());
                }
            }
            loops.push(ring);
        }
        // Remove almost-collinear points only from the triangulation input. Keep
        // their exact directed edge chains for re-insertion into the cap below.
        let cap_epsilon = loops
            .iter()
            .flatten()
            .map(|&i| {
                let p = project(i);
                p[0].hypot(p[1])
            })
            .fold(0_f64, f64::max)
            * 1e-7;
        for ring in &mut loops {
            loop {
                check_cancel(cancel)?;
                let redundant = (0..ring.len()).find(|&i| {
                    if ring.len() <= 3 {
                        return false;
                    }
                    let a = project(ring[(i + ring.len() - 1) % ring.len()]);
                    let b = project(ring[i]);
                    let c = project(ring[(i + 1) % ring.len()]);
                    let ac = [c[0] - a[0], c[1] - a[1]];
                    let ab = [b[0] - a[0], b[1] - a[1]];
                    let len2 = ac[0] * ac[0] + ac[1] * ac[1];
                    let t = (ac[0] * ab[0] + ac[1] * ab[1]) / len2;
                    t > 0.
                        && t < 1.
                        && (ac[0] * ab[1] - ac[1] * ab[0]).abs() <= len2.sqrt() * cap_epsilon
                });
                if let Some(i) = redundant {
                    ring.remove(i);
                } else {
                    break;
                }
            }
        }
        let areas: Vec<_> = loops
            .iter()
            .map(|ring| {
                ring.iter()
                    .zip(ring.iter().cycle().skip(1))
                    .map(|(&a, &b)| {
                        let a = project(a);
                        let b = project(b);
                        a[0] * b[1] - a[1] * b[0]
                    })
                    .sum::<f64>()
                    * 0.5
            })
            .collect();
        let contains = |ring: &[usize], p: [f64; 2]| {
            let mut inside = false;
            for (&a, &b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
                let a = project(a);
                let b = project(b);
                if (a[1] > p[1]) != (b[1] > p[1])
                    && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
                {
                    inside = !inside;
                }
            }
            inside
        };
        let mut holes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, &area) in areas.iter().enumerate() {
            if area >= 0. {
                continue;
            }
            let parent = (0..loops.len())
                .filter(|&j| areas[j] > 0. && contains(&loops[j], project(loops[i][0])))
                .min_by(|&a, &b| areas[a].total_cmp(&areas[b]))
                .ok_or("Slice cap orientation is invalid; check surface normals")?;
            holes.entry(parent).or_default().push(i);
        }
        let first_cap = self.triangles.len();
        let mut caps = Vec::new();
        for (i, ring) in loops.iter().enumerate().filter(|(i, _)| areas[*i] > 0.) {
            check_cancel(cancel)?;
            let mut ids = ring.clone();
            let mut starts = Vec::new();
            for &hole in holes.get(&i).into_iter().flatten() {
                starts.push(ids.len());
                ids.extend(&loops[hole]);
            }
            let points: Vec<_> = ids.iter().flat_map(|&id| project(id)).collect();
            let triangles = earcutr::earcut(&points, &starts, 2)
                .map_err(|_| "Could not triangulate slice cap")?;
            for t in triangles.chunks_exact(3) {
                let mut face = [ids[t[0]], ids[t[1]], ids[t[2]]];
                let [a, b, c] = face.map(project);
                if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) < 0. {
                    face.swap(1, 2);
                }
                caps.push(face);
            }
        }
        // Inspect the triangulator's actual boundary because it can remove more
        // collinear vertices while resolving holes. Reinsert all skipped points.
        let mut cap_edges: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for &[a, b, c] in &caps {
            for (a, b) in [(a, b), (b, c), (c, a)] {
                cap_edges.entry(key(a, b)).or_default().push((a, b));
            }
        }
        let mut chains = HashMap::new();
        for edges in cap_edges.values().filter(|edges| edges.len() == 1) {
            let (a, b) = edges[0];
            let mut chain = vec![a];
            let mut current = *next.get(&a).ok_or("Invalid cap boundary")?;
            let pa = project(a);
            let pb = project(b);
            let d = [pb[0] - pa[0], pb[1] - pa[1]];
            let len2 = d[0] * d[0] + d[1] * d[1];
            while current != b {
                check_cancel(cancel)?;
                let p = project(current);
                let offset = [p[0] - pa[0], p[1] - pa[1]];
                let t = (offset[0] * d[0] + offset[1] * d[1]) / len2;
                if current == a
                    || !(0. ..=1.).contains(&t)
                    || (offset[0] * d[1] - offset[1] * d[0]).abs() > len2.sqrt() * cap_epsilon * 4.
                {
                    return Err("Could not triangulate the slice boundary accurately".into());
                }
                chain.push(current);
                current = next[&current];
            }
            chain.push(b);
            if chain.len() > 2 {
                chains.insert((a, b), chain);
            }
        }
        // Restore each simplified boundary chain exactly once. This guarantees
        // that cap triangles use the same segmented edges as the cut surface.
        while let Some(face) = caps.pop() {
            check_cancel(cancel)?;
            let mut split = false;
            for (a, b, c) in [
                (face[0], face[1], face[2]),
                (face[1], face[2], face[0]),
                (face[2], face[0], face[1]),
            ] {
                if let Some(chain) = chains.remove(&(a, b)) {
                    caps.extend(chain.windows(2).map(|edge| [edge[0], edge[1], c]));
                    split = true;
                    break;
                }
            }
            if !split {
                self.triangles.push(face);
                self.polygroups.push(0);
            }
            if self.triangles.len() + caps.len() > super::subdivision::MAX_TRIANGLES {
                return Err("Slice cap exceeds the mesh size limit".into());
            }
        }
        let faces: Vec<_> = (first_cap..self.triangles.len()).collect();
        if faces.is_empty() {
            return Err("Could not construct a valid slice cap".into());
        }
        self.group_faces(&faces, "Cut")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    fn plane() -> SlicePlane {
        SlicePlane {
            origin: Vec3(0.13, 0., 0.),
            normal: Vec3(1., 0., 0.),
        }
    }
    fn options(mode: SliceMode) -> SliceOptions {
        SliceOptions {
            mode,
            ..Default::default()
        }
    }
    fn cube() -> Mesh {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
        primitive.divisions = [2; 3];
        primitive.mesh().unwrap()
    }
    fn volume(mesh: &Mesh) -> f32 {
        mesh.triangles
            .iter()
            .map(|&[a, b, c]| mesh.vertices[a].dot(mesh.vertices[b].cross(mesh.vertices[c])) / 6.)
            .sum()
    }
    fn assert_surface(mesh: &Mesh) {
        mesh.validate_closed().unwrap();
        for &[a, b, c] in &mesh.triangles {
            assert!(
                (mesh.vertices[b] - mesh.vertices[a])
                    .cross(mesh.vertices[c] - mesh.vertices[a])
                    .len()
                    > 1e-9,
                "degenerate face"
            );
        }
        assert!(volume(mesh) > 0.);
    }
    #[test]
    fn slice_all_modes_keep_welded_geometry_and_interpolate_data() {
        let mut original = cube();
        original.mask = original.vertices.iter().map(|p| (p.0 + 1.) * 0.5).collect();
        original
            .create_vertex_group("Soft".into(), 0., false)
            .unwrap();
        for mode in SliceMode::ALL {
            let (a, b) = original.slice(plane(), options(mode), &|| false).unwrap();
            assert_surface(&a);
            for (i, p) in a.vertices.iter().enumerate() {
                assert!((a.mask[i] - (p.0 + 1.) * 0.5).abs() < 1e-6);
                assert!((a.vertex_groups[0].weights[i] - a.mask[i]).abs() < 1e-6);
            }
            assert!(!a.quads.is_empty());
            if mode == SliceMode::Polygroups {
                assert!((volume(&a) - volume(&original)).abs() < 1e-5);
                assert!(a.vertex_groups.len() > original.vertex_groups.len());
            } else {
                assert!(a.vertices.iter().all(|v| v.0 >= 0.13 - 1e-6));
                assert!(a.vertex_groups.iter().any(|g| g.name == "Cut"));
            }
            if let Some(b) = b {
                assert_surface(&b);
                assert!((volume(&a) + volume(&b) - volume(&original)).abs() < 1e-5);
            }
        }
    }
    #[test]
    fn slice_caps_exact_vertices_edges_and_oblique_planes() {
        for normal in [Vec3(1., 0., 0.), Vec3(1., 1., 0.), Vec3(1., 1., 1.)] {
            for origin in [Vec3::default(), Vec3(0.19, 0.11, 0.03)] {
                let mesh = cube();
                let (a, b) = mesh
                    .slice(
                        SlicePlane { origin, normal },
                        options(SliceMode::Separate),
                        &|| false,
                    )
                    .unwrap();
                assert_surface(&a);
                assert_surface(b.as_ref().unwrap());
                assert!((volume(&a) + volume(b.as_ref().unwrap()) - volume(&mesh)).abs() < 1e-5);
            }
        }
    }
    #[test]
    fn slice_torus_cap_preserves_the_hole_and_volume() {
        let mesh = Primitive::for_kind(PrimitiveKind::Torus).mesh().unwrap();
        for y in [0., 0.07] {
            let (a, b) = mesh
                .slice(
                    SlicePlane {
                        origin: Vec3(0., y, 0.),
                        normal: Vec3(0., 1., 0.),
                    },
                    options(SliceMode::Separate),
                    &|| false,
                )
                .unwrap();
            assert_surface(&a);
            assert_surface(b.as_ref().unwrap());
            assert!((volume(&a) + volume(b.as_ref().unwrap()) - volume(&mesh)).abs() < 1e-4);
            for (i, &[x, y, z]) in a.triangles.iter().enumerate() {
                if a.polygroups[i] == a.vertex_groups.len() {
                    let center = (a.vertices[x] + a.vertices[y] + a.vertices[z]) * (1. / 3.);
                    assert!(
                        center.0 * center.0 + center.2 * center.2 > 0.01,
                        "cap filled the torus hole"
                    );
                }
            }
        }
    }
    #[test]
    fn slice_open_surfaces_allow_uncapped_results_and_reject_caps() {
        let mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        assert!(
            mesh.slice(plane(), options(SliceMode::Trim), &|| false)
                .is_err()
        );
        for mode in SliceMode::ALL {
            let (a, b) = mesh
                .slice(
                    plane(),
                    SliceOptions {
                        mode,
                        cap: false,
                        flip: false,
                    },
                    &|| false,
                )
                .unwrap();
            a.validate().unwrap();
            if let Some(b) = b {
                b.validate().unwrap();
            }
        }
    }
    #[test]
    fn slice_miss_tangent_invalid_cancel_and_flip() {
        let mesh = cube();
        for origin in [Vec3(3., 0., 0.), Vec3(1., 0., 0.)] {
            assert!(
                mesh.slice(
                    SlicePlane { origin, ..plane() },
                    options(SliceMode::Trim),
                    &|| false
                )
                .is_err()
            );
        }
        assert!(
            mesh.slice(
                SlicePlane {
                    normal: Vec3::default(),
                    ..plane()
                },
                options(SliceMode::Trim),
                &|| false
            )
            .is_err()
        );
        assert!(
            mesh.slice(plane(), options(SliceMode::Trim), &|| true)
                .is_err()
        );
        let (a, _) = mesh
            .slice(
                plane(),
                SliceOptions {
                    flip: true,
                    ..options(SliceMode::Trim)
                },
                &|| false,
            )
            .unwrap();
        assert_surface(&a);
        assert!(a.vertices.iter().all(|p| p.0 <= 0.13 + 1e-6));
    }
    #[test]
    fn slice_sphere_keeps_closed_surface() {
        let (a, b) = Mesh::sphere()
            .slice(plane(), options(SliceMode::Separate), &|| false)
            .unwrap();
        assert_surface(&a);
        assert_surface(b.as_ref().unwrap());
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    fn plane() -> SlicePlane {
        SlicePlane {
            origin: Vec3(0.1, 0., 0.),
            normal: Vec3(1., 0., 0.),
        }
    }
    #[test]
    fn slice_multiple_shells_and_concave_sections_are_closed() {
        let mut mesh = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
        let mut second = mesh.clone();
        for p in &mut second.vertices {
            p.1 += 3.;
        }
        let count = mesh.vertices.len();
        mesh.vertices.extend(second.vertices);
        mesh.mask.extend(second.mask);
        mesh.triangles
            .extend(second.triangles.into_iter().map(|f| f.map(|i| i + count)));
        mesh.quads
            .extend(second.quads.into_iter().map(|f| f.map(|i| i + count)));
        for source in [
            mesh,
            Primitive::for_kind(PrimitiveKind::Torus).mesh().unwrap(),
        ] {
            for normal in [Vec3(1., 0., 0.), Vec3(0.3, 0.8, 0.1), Vec3(0., 0., 1.)] {
                let (a, b) = source
                    .slice(
                        SlicePlane { normal, ..plane() },
                        SliceOptions {
                            mode: SliceMode::Separate,
                            ..Default::default()
                        },
                        &|| false,
                    )
                    .unwrap();
                a.validate_closed().unwrap();
                b.unwrap().validate_closed().unwrap();
            }
        }
    }
    #[test]
    fn slice_guards_and_group_limit_are_transactional() {
        let mut geometry = Geometry::new(Mesh::starting_sphere());
        geometry.scene.active_metadata.temporary_thickness = 0.2;
        let original = geometry.clone();
        assert!(
            geometry
                .slice(plane(), Default::default(), &|| false)
                .unwrap_err()
                .contains("thickness")
        );
        assert_eq!(geometry, original);
        geometry.scene.active_metadata.temporary_thickness = 0.;
        geometry.mesh.vertex_groups[0].visible = false;
        let original = geometry.clone();
        assert!(
            geometry
                .slice(plane(), Default::default(), &|| false)
                .unwrap_err()
                .contains("Show")
        );
        assert_eq!(geometry, original);
        geometry.mesh.vertex_groups[0].visible = true;
        for n in 1..64 {
            geometry
                .mesh
                .create_vertex_group(format!("Group {n}"), 0.5, false)
                .unwrap();
        }
        let original = geometry.clone();
        assert!(
            geometry
                .slice(plane(), Default::default(), &|| false)
                .unwrap_err()
                .contains("64")
        );
        assert_eq!(geometry, original);
    }
}

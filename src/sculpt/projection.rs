//! Bounded detail transfer along a coherent normal field. Topology stays intact.
use super::{Mesh, Vec3, math::closest_triangle};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionOptions {
    pub strength: f32,
    pub max_distance: f32,
}
impl Default for ProjectionOptions {
    fn default() -> Self {
        Self {
            strength: 1.,
            max_distance: 0.1,
        }
    }
}
impl ProjectionOptions {
    pub fn validate(self) -> Result<(), String> {
        if !self.strength.is_finite()
            || !(0. ..=1.).contains(&self.strength)
            || !self.max_distance.is_finite()
            || self.max_distance <= 0.
            || self.max_distance > 1000.
        {
            return Err(
                "Projection needs strength 0–1 and distance greater than 0 (at most 1000)".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectionReport {
    pub moved: usize,
    pub protected: usize,
    pub missed: usize,
    pub opposed: usize,
    pub ambiguous: usize,
    pub limited: usize,
}
impl ProjectionReport {
    pub fn summary(&self) -> String {
        format!(
            "{} moved · {} protected · {} outside distance · {} opposing · {} ambiguous · {} safety-limited",
            self.moved, self.protected, self.missed, self.opposed, self.ambiguous, self.limited
        )
    }
}

struct Node {
    min: [f32; 3],
    max: [f32; 3],
    range: std::ops::Range<usize>,
    children: Option<[usize; 2]>,
    component: Option<usize>,
}
struct Surface<'a> {
    mesh: &'a Mesh,
    faces: Vec<usize>,
    components: Vec<usize>,
    nodes: Vec<Node>,
}
fn xyz(v: Vec3) -> [f32; 3] {
    [v.0, v.1, v.2]
}
fn root(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}
impl<'a> Surface<'a> {
    fn new(mesh: &'a Mesh, cancel: &impl Fn() -> bool) -> Result<Self, String> {
        let mut parents: Vec<_> = (0..mesh.vertices.len()).collect();
        for (i, &[a, b, c]) in mesh.triangles.iter().enumerate() {
            if i % 1024 == 0 && cancel() {
                return Err("Projection cancelled".into());
            }
            if !mesh.face_visible(i) {
                continue;
            }
            let a = root(&mut parents, a);
            for v in [b, c] {
                let b = root(&mut parents, v);
                parents[b] = a;
            }
        }
        let components = mesh
            .triangles
            .iter()
            .map(|t| root(&mut parents, t[0]))
            .collect();
        let faces = (0..mesh.triangles.len())
            .filter(|&i| mesh.face_visible(i))
            .collect();
        let mut result = Self {
            mesh,
            faces,
            components,
            nodes: Vec::new(),
        };
        if result.faces.is_empty() {
            return Err("The reference has no visible faces".into());
        }
        result.build(0..result.faces.len(), cancel)?;
        Ok(result)
    }
    fn build(
        &mut self,
        range: std::ops::Range<usize>,
        cancel: &impl Fn() -> bool,
    ) -> Result<usize, String> {
        if cancel() {
            return Err("Projection cancelled".into());
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let first = self.components[self.faces[range.start]];
        let mut component = Some(first);
        for &face in &self.faces[range.clone()] {
            if self.components[face] != first {
                component = None;
            }
            for v in self.mesh.triangles[face] {
                for (axis, value) in xyz(self.mesh.vertices[v]).into_iter().enumerate() {
                    min[axis] = min[axis].min(value);
                    max[axis] = max[axis].max(value);
                }
            }
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            min,
            max,
            range: range.clone(),
            children: None,
            component,
        });
        if range.len() > 8 {
            let axis = (0..3)
                .max_by(|&a, &b| (max[a] - min[a]).total_cmp(&(max[b] - min[b])))
                .unwrap();
            let mesh = self.mesh;
            let center = |i: usize| {
                mesh.triangles[i]
                    .iter()
                    .map(|&v| xyz(mesh.vertices[v])[axis])
                    .sum::<f32>()
            };
            let mid = range.len() / 2;
            self.faces[range.clone()].select_nth_unstable_by(mid, |&a, &b| {
                center(a).total_cmp(&center(b)).then(a.cmp(&b))
            });
            let split = range.start + mid;
            let left = self.build(range.start..split, cancel)?;
            let right = self.build(split..range.end, cancel)?;
            self.nodes[index].children = Some([left, right]);
        }
        Ok(index)
    }
    fn nearest(
        &self,
        p: Vec3,
        distance: f32,
        exclude: Option<usize>,
        budget: &mut usize,
    ) -> Result<Option<(usize, Vec3, f32)>, String> {
        let mut stack = vec![0];
        let mut best = distance * distance;
        let mut hit = None;
        let bounds_distance = |n: &Node| {
            xyz(p)
                .into_iter()
                .enumerate()
                .map(|(i, v)| (v - v.clamp(n.min[i], n.max[i])).powi(2))
                .sum::<f32>()
        };
        while let Some(index) = stack.pop() {
            if *budget == 0 {
                return Err("Projection search budget exceeded; reduce the distance or simplify the reference".into());
            }
            *budget -= 1;
            let node = &self.nodes[index];
            if exclude.is_some() && node.component == exclude {
                continue;
            }
            if bounds_distance(node) > best {
                continue;
            }
            if let Some([a, b]) = node.children {
                if bounds_distance(&self.nodes[a]) < bounds_distance(&self.nodes[b]) {
                    stack.extend([b, a]);
                } else {
                    stack.extend([a, b]);
                }
            } else {
                for &face in &self.faces[node.range.clone()] {
                    if Some(self.components[face]) == exclude {
                        continue;
                    }
                    let [a, b, c] = self.mesh.triangles[face].map(|i| self.mesh.vertices[i]);
                    let q = closest_triangle(p, a, b, c);
                    let d = (q - p).dot(q - p);
                    if d <= best {
                        best = d;
                        hit = Some((face, q, d.sqrt()));
                    }
                }
            }
        }
        Ok(hit)
    }
}
impl Surface<'_> {
    fn along_normal(
        &self,
        p: Vec3,
        normal: Vec3,
        distance: f32,
        budget: &mut usize,
    ) -> Result<Option<(usize, Vec3, f32)>, String> {
        let mut stack = vec![0];
        let mut best = distance as f64;
        let mut hit = None;
        let origin = xyz(p).map(f64::from);
        let direction = xyz(normal).map(f64::from);
        let sub = |a: [f64; 3], b: [f64; 3]| std::array::from_fn(|i| a[i] - b[i]);
        let cross = |a: [f64; 3], b: [f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let dot = |a: [f64; 3], b: [f64; 3]| a.into_iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
        while let Some(i) = stack.pop() {
            if *budget == 0 {
                return Err("Projection search budget exceeded; reduce the distance or simplify the reference".into());
            }
            *budget -= 1;
            let node = &self.nodes[i];
            let mut low = -best;
            let mut high = best;
            for axis in 0..3 {
                let a = node.min[axis] as f64 - origin[axis];
                let b = node.max[axis] as f64 - origin[axis];
                if direction[axis] == 0. {
                    if a > 0. || b < 0. {
                        low = f64::INFINITY;
                    }
                } else {
                    let a = a / direction[axis];
                    let b = b / direction[axis];
                    low = low.max(a.min(b));
                    high = high.min(a.max(b));
                }
            }
            if low > high {
                continue;
            }
            if let Some(c) = node.children {
                stack.extend(c);
                continue;
            }
            for &face in &self.faces[node.range.clone()] {
                let [a, b, c] =
                    self.mesh.triangles[face].map(|i| xyz(self.mesh.vertices[i]).map(f64::from));
                let ab = sub(b, a);
                let ac = sub(c, a);
                let h = cross(direction, ac);
                let det = dot(ab, h);
                if det == 0. {
                    continue;
                }
                let s = sub(origin, a);
                let u = dot(s, h) / det;
                let q = cross(s, ab);
                let v = dot(direction, q) / det;
                if u < -1e-8 || v < -1e-8 || u + v > 1. + 1e-8 {
                    continue;
                }
                let t = dot(ac, q) / det;
                if t.abs() <= best {
                    best = t.abs();
                    hit = Some((face, p + normal * t as f32, best as f32));
                }
            }
        }
        Ok(hit)
    }
}

// Average directions without smoothing vertex positions. A raw vertex normal
// can point to a different side of a steep rim than its neighbour; using a
// local direction field keeps correspondence stable through subdivision.
fn projection_directions(mesh: &Mesh, cancel: &impl Fn() -> bool) -> Result<Vec<Vec3>, String> {
    let mut normals = mesh.normals();
    for _ in 0..8 {
        if cancel() {
            return Err("Projection cancelled".into());
        }
        let old = normals.clone();
        for &[a, b, c] in &mesh.triangles {
            for (v, x, y) in [(a, b, c), (b, a, c), (c, a, b)] {
                normals[v] = normals[v] + old[x] + old[y];
            }
        }
        for normal in &mut normals {
            *normal = normal.norm();
        }
    }
    Ok(normals)
}

impl Mesh {
    pub fn project_details(
        &self,
        source: &Mesh,
        options: ProjectionOptions,
        cancel: &impl Fn() -> bool,
    ) -> Result<(Mesh, ProjectionReport), String> {
        options.validate()?;
        self.validate()?;
        source.validate()?;
        if cancel() {
            return Err("Projection cancelled".into());
        }
        let surface = Surface::new(source, cancel)?;
        let normals = projection_directions(self, cancel)?;
        let editable = self.sculptable_vertices();
        let mut mesh = self.clone();
        let mut report = ProjectionReport::default();
        let mut budget = 30_000_000;
        for (i, &p) in self.vertices.iter().enumerate() {
            if i % 128 == 0 && cancel() {
                return Err("Projection cancelled".into());
            }
            if !editable[i] || self.mask[i] >= 1. {
                report.protected += 1;
                continue;
            }
            let Some((face, q, distance)) =
                surface.along_normal(p, normals[i], options.max_distance, &mut budget)?
            else {
                report.missed += 1;
                continue;
            };
            let [a, b, c] = source.triangles[face].map(|v| source.vertices[v]);
            // Reject the nearest opposing sheet instead of searching through it.
            if (b - a).cross(c - a).norm().dot(normals[i]) < 0.2 {
                report.opposed += 1;
                continue;
            }
            let ambiguity_radius = (distance * 1.25 + 1e-6).min(options.max_distance);
            if surface
                .nearest(
                    p,
                    ambiguity_radius,
                    Some(surface.components[face]),
                    &mut budget,
                )?
                .is_some()
            {
                report.ambiguous += 1;
                continue;
            }
            if distance > 1e-7 {
                mesh.vertices[i] = p + (q - p) * (options.strength * (1. - self.mask[i]));
            }
        }
        if cancel() {
            return Err("Projection cancelled".into());
        }
        let proposed = mesh.vertices.clone();
        mesh.limit_projection_deformation(&self.vertices);
        report.limited = mesh
            .vertices
            .iter()
            .zip(&proposed)
            .filter(|(a, b)| a != b)
            .count();
        report.moved = mesh
            .vertices
            .iter()
            .zip(&self.vertices)
            .filter(|(a, b)| a != b)
            .count();
        mesh.validate()?;
        if cancel() {
            return Err("Projection cancelled".into());
        }
        Ok((mesh, report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};

    fn plane(z: f32) -> Mesh {
        let mut p = Primitive::for_kind(PrimitiveKind::Plane);
        p.divisions = [4, 4, 1];
        let mut mesh = p.mesh().unwrap();
        for v in &mut mesh.vertices {
            v.2 += z;
        }
        mesh
    }
    fn options() -> ProjectionOptions {
        ProjectionOptions {
            strength: 1.,
            max_distance: 0.3,
        }
    }

    #[test]
    fn projection_of_a_steep_bowl_stays_stable_through_subdivision() {
        fn bowl(divisions: usize, depth: f32, rim: f32) -> Mesh {
            let mut mesh = Mesh::default();
            for y in 0..=divisions {
                for x in 0..=divisions {
                    let x = 2. * x as f32 / divisions as f32 - 1.;
                    let y = 2. * y as f32 / divisions as f32 - 1.;
                    let t = ((x.hypot(y) - 0.28) / rim).clamp(0., 1.);
                    mesh.vertices
                        .push(Vec3(x, y, -depth * (1. - t * t * (3. - 2. * t))));
                    mesh.mask.push(0.);
                }
            }
            for y in 0..divisions {
                for x in 0..divisions {
                    let a = y * (divisions + 1) + x;
                    mesh.triangles.extend([
                        [a, a + 1, a + divisions + 2],
                        [a, a + divisions + 2, a + divisions + 1],
                    ]);
                }
            }
            mesh
        }
        let reference = bowl(56, 0.25, 0.08);
        let mut target = bowl(28, 0.2, 0.15);
        for level in 0..3 {
            if level > 0 {
                target = target.subdivide().unwrap();
            }
            let (result, report) = target
                .project_details(&reference, ProjectionOptions::default(), &|| false)
                .unwrap();
            assert!(report.moved > 0, "{report:?}");
            assert_eq!(
                report.limited, 0,
                "Correspondence should stay coherent: {report:?}"
            );
            assert_eq!(report.missed + report.opposed, 0, "{report:?}");
            target = result;
        }
        // Independent closest points produced folded, jagged rim faces here,
        // and projecting again kept moving them. The result should be stable.
        let mut edges = std::collections::HashMap::new();
        for &[a, b, c] in &target.triangles {
            let normal = (target.vertices[b] - target.vertices[a])
                .cross(target.vertices[c] - target.vertices[a])
                .norm();
            for (a, b) in [(a, b), (b, c), (c, a)] {
                let edge = if a < b { (a, b) } else { (b, a) };
                if let Some(other) = edges.insert(edge, normal) {
                    assert!(
                        normal.dot(other) > 0.1,
                        "Projection introduced a sharp rim fold"
                    );
                }
            }
        }
        let (repeated, _) = target
            .project_details(&reference, ProjectionOptions::default(), &|| false)
            .unwrap();
        assert!(
            repeated
                .vertices
                .iter()
                .zip(&target.vertices)
                .all(|(&a, &b)| (a - b).len() < 1e-6)
        );
    }

    #[test]
    fn projection_cannot_fold_neighbouring_faces_back_against_each_other() {
        // Each face can keep its winding while the pair folds into a spike.
        // This used to pass the individual triangle safety checks.
        for scale in [0.01, 1., 100.] {
            let target = Mesh {
                vertices: [
                    Vec3(0., -1., 0.),
                    Vec3(0., 1., 0.),
                    Vec3(-1., 0., 0.),
                    Vec3(1., 0., 0.),
                ]
                .map(|p| p * scale)
                .into(),
                triangles: vec![[0, 1, 2], [1, 0, 3]],
                mask: vec![0.; 4],
                ..Mesh::default()
            };
            let mut source = target.clone();
            source.vertices[2] = Vec3(-1., 0., 4.) * scale;
            source.vertices[3] = Vec3(1., 0., 4.) * scale;
            let (result, report) = target
                .project_details(
                    &source,
                    ProjectionOptions {
                        max_distance: 5. * scale,
                        ..options()
                    },
                    &|| false,
                )
                .unwrap();
            let normals: Vec<_> = result
                .triangles
                .iter()
                .map(|t| {
                    let [a, b, c] = t.map(|v| result.vertices[v]);
                    (b - a).cross(c - a).norm()
                })
                .collect();
            assert!(normals[0].dot(normals[1]) >= 0., "{report:?}");
            assert!(report.moved > 0 && report.limited > 0, "{report:?}");
            assert_eq!(result.triangles, target.triangles);
        }
    }

    #[test]
    fn projection_blends_positions_preserving_topology_masks_groups_and_hidden_boundaries() {
        let mut target = plane(0.);
        target
            .create_vertex_group("Protected".into(), 0.5, true)
            .unwrap();
        target.mask[0] = 1.;
        target.mask[1] = 0.5;
        target.hidden_faces = vec![false; target.triangles.len()];
        target.hidden_faces[10] = true;
        let source = plane(0.2);
        let before_source = source.clone();
        let (result, report) = target
            .project_details(
                &source,
                ProjectionOptions {
                    strength: 0.5,
                    ..options()
                },
                &|| false,
            )
            .unwrap();
        let editable = target.sculptable_vertices();
        for (i, p) in result.vertices.iter().enumerate() {
            let expected = if editable[i] {
                0.1 * (1. - target.mask[i])
            } else {
                0.
            };
            assert!((p.2 - expected).abs() < 1e-6, "vertex {i}: {p:?}");
        }
        let mut attrs = result.clone();
        attrs.vertices = target.vertices.clone();
        assert_eq!(attrs, target);
        assert!(report.moved > 0 && report.protected > 0);
        assert_eq!(source, before_source);
    }
    #[test]
    fn projection_skips_misses_opposing_sheets_and_ambiguous_disconnected_parts() {
        let target = plane(0.);
        let (result, report) = target
            .project_details(&plane(2.), options(), &|| false)
            .unwrap();
        assert_eq!(result, target);
        assert_eq!(report.missed, target.vertices.len());
        let mut opposite = plane(0.05);
        for t in &mut opposite.triangles {
            t.swap(1, 2);
        }
        opposite.quads.clear();
        // A compatible but more distant surface must not be found through the opposing one.
        let source = opposite.merged_with(&plane(0.15)).unwrap();
        let (result, report) = target
            .project_details(&source, options(), &|| false)
            .unwrap();
        assert_eq!(result, target);
        assert_eq!(report.opposed, target.vertices.len());
        let source = plane(0.1).merged_with(&plane(0.11)).unwrap();
        let (result, report) = target
            .project_details(&source, options(), &|| false)
            .unwrap();
        assert_eq!(result, target);
        assert_eq!(report.ambiguous, target.vertices.len());
    }
    #[test]
    fn projection_hidden_reference_faces_cancellation_invalid_options_and_noops() {
        let target = plane(0.);
        let mut source = plane(0.1);
        source.set_face_visibility(vec![false; source.triangles.len()]);
        assert!(
            target
                .project_details(&source, options(), &|| false)
                .unwrap_err()
                .contains("visible faces")
        );
        assert!(
            target
                .project_details(&target, options(), &|| true)
                .is_err()
        );
        for invalid in [f32::NAN, -1., f32::INFINITY, 0.] {
            assert!(
                target
                    .project_details(
                        &target,
                        ProjectionOptions {
                            max_distance: invalid,
                            ..options()
                        },
                        &|| false
                    )
                    .is_err()
            );
        }
        assert_eq!(
            target
                .project_details(&target, options(), &|| false)
                .unwrap()
                .0,
            target
        );
        assert_eq!(
            target
                .project_details(
                    &plane(0.1),
                    ProjectionOptions {
                        strength: 0.,
                        ..options()
                    },
                    &|| false
                )
                .unwrap()
                .0,
            target
        );
        let calls = std::cell::Cell::new(0);
        assert!(
            target
                .project_details(&plane(0.1), options(), &|| {
                    calls.set(calls.get() + 1);
                    calls.get() > 3
                })
                .is_err()
        );
    }
    #[test]
    fn projection_recovers_remeshed_surface_with_a_bounded_triangle_search() {
        let mut source = Mesh::sphere();
        for p in &mut source.vertices {
            let bump = 0.08 * (p.0 * 9.).sin() * (p.1 * 9.).cos();
            *p = *p * (1. + bump);
        }
        let target = source.remesh_cancellable(24, &|| false).unwrap();
        assert_ne!(target.triangles.len(), source.triangles.len());
        let (result, report) = target
            .project_details(&source, options(), &|| false)
            .unwrap();
        assert!(report.moved > target.vertices.len() / 2, "{report:?}");
        let surface = Surface::new(&source, &|| false).unwrap();
        let error = |m: &Mesh| {
            let mut budget = 30_000_000;
            m.vertices
                .iter()
                .map(|&p| {
                    surface
                        .nearest(p, 1., None, &mut budget)
                        .unwrap()
                        .unwrap()
                        .2 as f64
                })
                .sum::<f64>()
        };
        let before = error(&target);
        let after = error(&result);
        assert!(
            after < before * 0.2,
            "before {before}, after {after}, {report:?}"
        );
        assert_eq!(target.triangles, result.triangles);
        result.validate_closed().unwrap();
    }
    #[test]
    fn projection_bvh_matches_exhaustive_nearest_surface() {
        let source = Mesh::sphere();
        let surface = Surface::new(&source, &|| false).unwrap();
        for p in [
            Vec3(0.2, 0.3, 1.2),
            Vec3(-0.7, 0.2, 0.3),
            Vec3(2., -1., 0.4),
        ] {
            let expected = source
                .triangles
                .iter()
                .map(|t| {
                    let [a, b, c] = t.map(|i| source.vertices[i]);
                    (closest_triangle(p, a, b, c) - p).len()
                })
                .fold(f32::INFINITY, f32::min);
            let got = surface
                .nearest(p, 5., None, &mut 30_000_000)
                .unwrap()
                .unwrap()
                .2;
            assert!((got - expected).abs() < 1e-6);
        }
    }
}

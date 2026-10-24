//! Connectivity-preserving tangential redistribution against an immutable local surface.
use super::{Mesh, Vec3, math::closest_triangle};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelaxationOptions {
    pub strength: f32,
    pub iterations: u32,
    /// World-space bound sampled at triangle centers and edge midpoints.
    pub max_deviation: f32,
}
impl Default for RelaxationOptions {
    fn default() -> Self {
        Self {
            strength: 0.5,
            iterations: 5,
            max_deviation: 0.002,
        }
    }
}
impl RelaxationOptions {
    pub fn validate(self) -> Result<(), String> {
        if !self.strength.is_finite()
            || !(0. ..=1.).contains(&self.strength)
            || !(1..=30).contains(&self.iterations)
            || !self.max_deviation.is_finite()
            || !(0.000001..=1.).contains(&self.max_deviation)
        {
            return Err(
                "Relaxation needs strength 0–1, 1–30 iterations and surface tolerance 0.000001–1"
                    .into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default)]
pub struct RelaxationReport {
    pub moved: usize,
    pub protected: usize,
    pub limited: usize,
    pub max_deviation: f32,
}
impl RelaxationReport {
    pub fn summary(&self) -> String {
        format!(
            "{} moved · {} protected · {} limited · {:.6} maximum sampled surface deviation",
            self.moved, self.protected, self.limited, self.max_deviation
        )
    }
}
fn nearest(reference: &Mesh, faces: &[usize], p: Vec3, budget: &mut usize) -> Result<Vec3, String> {
    let mut best = p;
    let mut distance = f32::INFINITY;
    for &f in faces {
        if *budget == 0 {
            return Err(
                "Relaxation work limit reached; use fewer iterations or a smaller unmasked region"
                    .into(),
            );
        }
        *budget -= 1;
        let [a, b, c] = reference.triangles[f].map(|v| reference.vertices[v]);
        let q = closest_triangle(p, a, b, c);
        let d = (q - p).len();
        if d < distance {
            distance = d;
            best = q;
        }
    }
    Ok(best)
}
fn samples(p: [Vec3; 3]) -> [Vec3; 4] {
    [
        (p[0] + p[1] + p[2]) * (1. / 3.),
        (p[0] + p[1]) * 0.5,
        (p[1] + p[2]) * 0.5,
        (p[2] + p[0]) * 0.5,
    ]
}
impl Mesh {
    pub fn relax_mesh(
        &self,
        reference: &Mesh,
        options: RelaxationOptions,
        cancel: &impl Fn() -> bool,
    ) -> Result<(Mesh, RelaxationReport), String> {
        options.validate()?;
        self.validate()?;
        reference.validate()?;
        if self.triangles != reference.triangles
            || self.quads != reference.quads
            || self.vertices.len() != reference.vertices.len()
        {
            return Err("Relaxation reference topology changed; capture a new reference".into());
        }
        let n = self.vertices.len();
        let mut incident = vec![Vec::new(); n];
        let mut edges: BTreeMap<[usize; 2], Vec<usize>> = BTreeMap::new();
        let mut face_normals = Vec::new();
        for (f, &t) in reference.triangles.iter().enumerate() {
            if f % 256 == 0 && cancel() {
                return Err("Relaxation cancelled".into());
            }
            for v in t {
                incident[v].push(f);
            }
            let [a, b, c] = t.map(|v| reference.vertices[v]);
            face_normals.push((b - a).cross(c - a));
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                edges.entry([a.min(b), a.max(b)]).or_default().push(f);
            }
        }
        let mut editable = self.sculptable_vertices();
        for (i, can) in editable.iter_mut().enumerate() {
            *can &= self.mask[i] < 1. && !incident[i].is_empty();
        }
        for (&[a, b], faces) in &edges {
            if faces.len() != 2
                || face_normals[faces[0]]
                    .norm()
                    .dot(face_normals[faces[1]].norm())
                    < 0.819152
            {
                editable[a] = false;
                editable[b] = false;
            }
        }
        for &[a, b] in &self.creases {
            editable[a] = false;
            editable[b] = false;
        }
        // Polygon edges avoid bias from the diagonals used only for rendering quads.
        let mut neighbors = vec![Vec::new(); n];
        for [a, b] in self.polygon_edges() {
            neighbors[a].push(b);
            neighbors[b].push(a);
        }
        let mut patches = Vec::with_capacity(self.triangles.len());
        if incident.iter().any(|faces| faces.len() > 64) {
            return Err("Relaxation supports at most 64 triangles at a vertex; simplify the crowded junction first".into());
        }
        for (f, t) in self.triangles.iter().enumerate() {
            if f % 256 == 0 && cancel() {
                return Err("Relaxation cancelled".into());
            }
            let mut patch: Vec<_> = t
                .iter()
                .flat_map(|&v| incident[v].iter().copied())
                .collect();
            patch.sort_unstable();
            patch.dedup();
            patches.push(patch);
        }
        let normals = reference.normals();
        let mut out = self.clone();
        let mut budget = 80_000_000;
        let mut limited = vec![false; n];
        for _ in 0..options.iterations {
            if cancel() {
                return Err("Relaxation cancelled".into());
            }
            let old = out.vertices.clone();
            for i in 0..n {
                if i % 256 == 0 && cancel() {
                    return Err("Relaxation cancelled".into());
                }
                if !editable[i] || neighbors[i].is_empty() {
                    continue;
                }
                let mean = neighbors[i]
                    .iter()
                    .fold(Vec3::default(), |p, &j| p + old[j])
                    * (1. / neighbors[i].len() as f32);
                let d = mean - old[i];
                let d =
                    (d - normals[i] * d.dot(normals[i])) * (options.strength * (1. - self.mask[i]));
                let shortest = neighbors[i]
                    .iter()
                    .map(|&j| (old[j] - old[i]).len())
                    .fold(f32::INFINITY, f32::min);
                let d = d * (0.15 * shortest / d.len().max(1e-20)).min(1.);
                if d.len() < 1e-8 {
                    continue;
                }
                // Only the original topological fan is searchable: no jumps to another wall/sheet.
                out.vertices[i] = nearest(reference, &incident[i], old[i] + d, &mut budget)?;
            }
            // Freeze unsafe neighborhoods, then recheck faces adjacent to frozen vertices.
            let mut safe = false;
            for _ in 0..8 {
                let mut reject = vec![false; n];
                let mut invalid = false;
                for (f, &t) in self.triangles.iter().enumerate() {
                    if f % 256 == 0 && cancel() {
                        return Err("Relaxation cancelled".into());
                    }
                    if t.iter().all(|&v| out.vertices[v] == old[v]) {
                        continue;
                    }
                    let p = t.map(|v| out.vertices[v]);
                    let cross = (p[1] - p[0]).cross(p[2] - p[0]);
                    let before = face_normals[f];
                    let mut bad = cross.dot(before) <= 0.1 * before.dot(before)
                        || !super::deformation::safe_face(t.map(|v| old[v]), p, None);
                    for point in samples(p) {
                        if (nearest(reference, &patches[f], point, &mut budget)? - point).len()
                            > options.max_deviation
                        {
                            bad = true;
                            break;
                        }
                    }
                    if bad {
                        invalid = true;
                        for v in t {
                            reject[v] = true;
                        }
                    }
                }
                if !invalid {
                    safe = true;
                    break;
                }
                for i in 0..n {
                    if reject[i] {
                        limited[i] |= out.vertices[i] != old[i];
                        out.vertices[i] = old[i];
                    }
                }
            }
            if !safe {
                out.vertices = old;
                break;
            }
            if out.vertices == old {
                break;
            }
        }
        let mut report = RelaxationReport {
            moved: out
                .vertices
                .iter()
                .zip(&self.vertices)
                .filter(|(a, b)| a != b)
                .count(),
            protected: editable.iter().filter(|&&v| !v).count(),
            limited: limited.iter().filter(|&&v| v).count(),
            max_deviation: 0.,
        };
        for (f, &t) in out.triangles.iter().enumerate() {
            if f % 256 == 0 && cancel() {
                return Err("Relaxation cancelled".into());
            }
            if t.iter().all(|&v| out.vertices[v] == reference.vertices[v]) {
                continue;
            }
            for p in samples(t.map(|v| out.vertices[v])) {
                report.max_deviation = report
                    .max_deviation
                    .max((nearest(reference, &patches[f], p, &mut budget)? - p).len());
            }
        }
        if report.max_deviation > options.max_deviation + 1e-6 {
            return Err("The current surface already exceeds this tolerance against the retained reference; Undo the previous relaxation or use a larger tolerance".into());
        }
        out.validate()?;
        Ok((out, report))
    }
}

#[cfg(test)]
pub(crate) fn relaxation_fixture() -> Mesh {
    use super::{Primitive, PrimitiveKind};
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [24, 24, 4];
    let mut mesh = primitive.mesh().unwrap();
    for p in &mut mesh.vertices {
        p.1 += 0.22 * (std::f32::consts::PI * p.1).sin();
        if p.2 > 0.99 {
            p.2 += 0.18 * (1. - p.0 * p.0) * (1. - p.1 * p.1)
                - 0.4 * (-p.0 * p.0 / 0.025).exp() * (1. - p.1 * p.1).powi(2);
        }
    }
    mesh
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    fn stretch(mesh: &Mesh) -> f32 {
        mesh.quads
            .iter()
            .filter(|q| q.iter().all(|&v| mesh.vertices[v].2 > 0.6))
            .map(|q| {
                let lengths: Vec<_> = (0..4)
                    .map(|i| (mesh.vertices[q[i]] - mesh.vertices[q[(i + 1) % 4]]).len())
                    .collect();
                lengths.iter().copied().fold(0., f32::max)
                    / lengths.iter().copied().fold(f32::INFINITY, f32::min)
            })
            .sum()
    }
    #[test]
    fn relaxation_groove_repeated_passes_preserve_shape_topology_and_attributes() {
        let mut source = relaxation_fixture();
        source
            .create_vertex_group("Surface".into(), 0., true)
            .unwrap();
        let mut out = source.clone();
        let options = RelaxationOptions {
            iterations: 10,
            ..Default::default()
        };
        let mut moved = 0;
        for _ in 0..3 {
            let (next, report) = out.relax_mesh(&source, options, &|| false).unwrap();
            assert!(
                report.max_deviation <= options.max_deviation * 1.001,
                "{report:?}"
            );
            moved += report.moved;
            out = next;
        }
        assert!(moved > 100, "moved {moved}");
        assert!(
            stretch(&out) < stretch(&source),
            "stretch {} -> {}",
            stretch(&source),
            stretch(&out)
        );
        let min_z = |m: &Mesh| {
            m.vertices
                .iter()
                .filter(|p| p.2 > 0.5)
                .map(|p| p.2)
                .fold(f32::INFINITY, f32::min)
        };
        assert!((min_z(&out) - min_z(&source)).abs() < 0.002);
        let volume = |m: &Mesh| {
            m.triangles
                .iter()
                .map(|t| m.vertices[t[0]].dot(m.vertices[t[1]].cross(m.vertices[t[2]])) / 6.)
                .sum::<f32>()
        };
        assert!((volume(&out) - volume(&source)).abs() / volume(&source).abs() < 0.001);
        out.validate_closed().unwrap();
        let moved_positions = out.vertices.clone();
        out.vertices = source.vertices.clone();
        assert_eq!(out, source);
        assert_ne!(moved_positions, source.vertices);
    }
    #[test]
    fn relaxation_plane_boundaries_masks_hidden_faces_and_thin_sheets_are_protected() {
        let mut p = Primitive::for_kind(PrimitiveKind::Plane);
        p.divisions = [12, 12, 1];
        let mut mesh = p.mesh().unwrap();
        for v in &mut mesh.vertices {
            v.0 += 0.25 * (v.0 * std::f32::consts::PI).sin();
        }
        for (i, v) in mesh.vertices.iter().enumerate() {
            if v.0 < 0. {
                mesh.mask[i] = 1.;
            }
        }
        let (out, report) = mesh
            .relax_mesh(&mesh, Default::default(), &|| false)
            .unwrap();
        assert!(report.moved > 0);
        for (i, &v) in mesh.vertices.iter().enumerate() {
            if mesh.mask[i] == 1. || v.0.abs() > 0.999 || v.1.abs() > 0.999 {
                assert_eq!(out.vertices[i], v);
            }
        }
        let mut thin = mesh.clone();
        let offset = thin.vertices.len();
        thin.vertices
            .extend(mesh.vertices.iter().map(|p| *p + Vec3(0., 0., 0.0001)));
        thin.mask.extend(&mesh.mask);
        thin.quads
            .extend(mesh.quads.iter().map(|q| q.map(|i| i + offset)));
        thin.triangles
            .extend(mesh.triangles.iter().map(|t| t.map(|i| i + offset)));
        let (out, _) = thin
            .relax_mesh(&thin, Default::default(), &|| false)
            .unwrap();
        for i in 0..offset {
            assert!((out.vertices[i + offset].2 - out.vertices[i].2 - 0.0001).abs() < 1e-7);
        }
        let mut hidden = mesh.clone();
        hidden.set_face_visibility(vec![false; hidden.triangles.len()]);
        assert_eq!(
            hidden
                .relax_mesh(&hidden, Default::default(), &|| false)
                .unwrap()
                .0,
            hidden
        );
        assert!(
            mesh.relax_mesh(&mesh, Default::default(), &|| true)
                .unwrap_err()
                .contains("cancelled")
        );
        let zero = RelaxationOptions {
            strength: 0.,
            ..Default::default()
        };
        assert_eq!(mesh.relax_mesh(&mesh, zero, &|| false).unwrap().0, mesh);
    }
}

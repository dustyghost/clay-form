//! Keep brush proposals from folding or crushing incident triangles.
use super::{Mesh, Vec3};

type Vector = [f64; 3];
fn difference(a: Vec3, b: Vec3) -> Vector {
    [
        a.0 as f64 - b.0 as f64,
        a.1 as f64 - b.1 as f64,
        a.2 as f64 - b.2 as f64,
    ]
}
fn cross(a: Vector, b: Vector) -> Vector {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: Vector, b: Vector) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn quality(a: Vector, b: Vector) -> f64 {
    let c = std::array::from_fn(|i| b[i] - a[i]);
    let normal = cross(a, b);
    2. * 3_f64.sqrt() * dot(normal, normal).sqrt() / (dot(a, a) + dot(b, b) + dot(c, c))
}

fn safe_face(before: [Vec3; 3], after: [Vec3; 3], direction: Option<Vec3>) -> bool {
    let a = difference(before[1], before[0]);
    let b = difference(before[2], before[0]);
    let n = cross(a, b);
    let area_squared = dot(n, n);
    if area_squared == 0. {
        return true;
    } // Existing degenerate faces need topology repair.
    // A directional relief can turn a steep side past the old face normal
    // without folding: its footprint in the stamp plane remains unchanged.
    // Check winding in that plane for front-facing faces. Keep the original
    // guard at the silhouette and on the back, where this projection is unsafe.
    let axis = direction.map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]);
    let n = axis
        .filter(|&axis| dot(n, axis) > 0.1 * (area_squared * dot(axis, axis)).sqrt())
        .unwrap_or(n);
    let projected_area = dot(cross(a, b), n);
    let x = difference(after[1], after[0]);
    let y = difference(after[2], after[0]);
    let dx = std::array::from_fn(|i| x[i] - a[i]);
    let dy = std::array::from_fn(|i| y[i] - b[i]);
    // Projected area during linear movement is 1 + linear*t + quadratic*t².
    // Check its minimum, not only the endpoint: two vertices could otherwise
    // cross and leave an apparently correctly oriented triangle at release.
    let quadratic = dot(cross(dx, dy), n) / projected_area;
    let linear = (dot(cross(a, dy), n) + dot(cross(dx, b), n)) / projected_area;
    let mut minimum = (1. + linear + quadratic).min(1.);
    if quadratic > 0. {
        let t = -linear / (2. * quadratic);
        if (0. ..1.).contains(&t) {
            minimum = minimum.min(1. + linear * t + quadratic * t * t);
        }
    }
    minimum.is_finite() && minimum >= 0.05 && quality(x, y) >= quality(a, b).min(0.02) * (1. - 1e-6)
}

fn face_normal(points: [Vec3; 3]) -> Vector {
    cross(
        difference(points[1], points[0]),
        difference(points[2], points[0]),
    )
}

fn normal_agreement(a: Vector, b: Vector) -> f64 {
    let length = (dot(a, a) * dot(b, b)).sqrt();
    if length == 0. { 1. } else { dot(a, b) / length }
}

impl Mesh {
    pub(super) fn limit_sculpt_deformation(&mut self, before: &[Vec3]) -> bool {
        self.limit_deformation(before, false, None, false)
    }

    pub(super) fn limit_stamp_deformation(&mut self, before: &[Vec3], direction: Vec3) -> bool {
        self.limit_deformation(before, false, Some(direction), false)
    }

    pub(super) fn limit_projection_deformation(&mut self, before: &[Vec3]) -> bool {
        self.limit_deformation(before, true, None, false)
    }

    pub(super) fn limit_smoothing_deformation(&mut self, before: &[Vec3]) -> bool {
        self.limit_deformation(before, false, None, true)
    }

    fn limit_deformation(
        &mut self,
        before: &[Vec3],
        prevent_folds: bool,
        direction: Option<Vec3>,
        local_rejection: bool,
    ) -> bool {
        let active: Vec<_> = self
            .triangles
            .iter()
            .filter(|t| t.iter().any(|&v| before[v] != self.vertices[v]))
            .copied()
            .collect();
        if active.is_empty() {
            return self.vertices.iter().zip(before).any(|(a, b)| a != b);
        }
        // Individually well-oriented faces can still fold back against their
        // neighbour. Check shared edges too when moving onto a reference surface.
        // Preserve existing sharp folds, but never create a new opposing pair.
        let mut pairs = Vec::new();
        if prevent_folds {
            let mut edges = std::collections::HashMap::new();
            for (face, &[a, b, c]) in self.triangles.iter().enumerate() {
                for (a, b) in [(a, b), (b, c), (c, a)] {
                    let edge = if a < b { (a, b) } else { (b, a) };
                    if let Some(other) = edges.insert(edge, face) {
                        let first = self.triangles[other];
                        let second = self.triangles[face];
                        if first
                            .iter()
                            .chain(&second)
                            .any(|&v| before[v] != self.vertices[v])
                        {
                            let agreement = normal_agreement(
                                face_normal(first.map(|v| before[v])),
                                face_normal(second.map(|v| before[v])),
                            );
                            pairs.push((other, face, agreement.min(0.)));
                        }
                    }
                }
            }
        }
        let safe_pair = |vertices: &[Vec3], &(a, b, minimum): &(usize, usize, f64)| {
            normal_agreement(
                face_normal(self.triangles[a].map(|v| vertices[v])),
                face_normal(self.triangles[b].map(|v| vertices[v])),
            ) >= minimum - 1e-6
        };
        let mut reduce = vec![false; self.vertices.len()];
        for pass in 0..if local_rejection { 20 } else { 12 } {
            let mut unsafe_face = false;
            for &t in &active {
                if !safe_face(t.map(|v| before[v]), t.map(|v| self.vertices[v]), direction) {
                    unsafe_face = true;
                    for v in t {
                        reduce[v] = true;
                    }
                }
            }
            for pair in &pairs {
                if !safe_pair(&self.vertices, pair) {
                    unsafe_face = true;
                    for v in self.triangles[pair.0]
                        .into_iter()
                        .chain(self.triangles[pair.1])
                    {
                        reduce[v] = true;
                    }
                }
            }
            if !unsafe_face {
                return self.vertices.iter().zip(before).any(|(a, b)| a != b);
            }
            for (v, flag) in reduce.iter_mut().enumerate() {
                if *flag {
                    self.vertices[v] = if local_rejection && pass >= 8 {
                        // A few pre-existing slivers must not cancel smoothing
                        // everywhere else. Pin failed proposals, then recheck
                        // neighboring faces against the resulting mixed update.
                        before[v]
                    } else {
                        before[v] + (self.vertices[v] - before[v]) * 0.5
                    };
                    *flag = false;
                }
            }
        }
        // Never commit an unchecked last reduction; pinning one vertex can
        // affect its neighbours. Reject a pathological dab without changing positions.
        if active
            .iter()
            .any(|&t| !safe_face(t.map(|v| before[v]), t.map(|v| self.vertices[v]), direction))
            || pairs.iter().any(|pair| !safe_pair(&self.vertices, pair))
        {
            self.vertices.copy_from_slice(before);
            return false;
        }
        self.vertices.iter().zip(before).any(|(a, b)| a != b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothing_does_not_make_existing_slivers_worse() {
        let before = vec![Vec3(0., 0., 0.), Vec3(1., 0., 0.), Vec3(0.5, 0.004, 0.)];
        let mut mesh = Mesh {
            vertices: before.clone(),
            triangles: vec![[0, 1, 2]],
            mask: vec![0.; 3],
            ..Default::default()
        };
        mesh.vertices[2].1 = 0.001;
        mesh.limit_smoothing_deformation(&before);
        let q = |v: &[Vec3]| quality(difference(v[1], v[0]), difference(v[2], v[0]));
        assert!(q(&mesh.vertices) >= q(&before) * (1. - 1e-6));
    }

    #[test]
    fn smoothing_allows_valid_steep_dihedrals_without_pinning_a_safe_patch() {
        let before = vec![
            Vec3(0., -1., 0.),
            Vec3(0., 1., 0.),
            Vec3(-1., 0., 1.),
            Vec3(1., 0., 1.),
            Vec3(3., 0., 0.),
            Vec3(4., 0., 0.),
            Vec3(3., 1., 0.),
        ];
        let mut mesh = Mesh {
            vertices: before.clone(),
            triangles: vec![[0, 1, 2], [1, 0, 3], [4, 5, 6]],
            mask: vec![0.; 7],
            ..Default::default()
        };
        mesh.vertices[2].0 = -0.5;
        mesh.vertices[3].0 = 0.5;
        for p in &mut mesh.vertices[4..] {
            p.2 += 0.1;
        }
        let safe = mesh.vertices[4..].to_vec();
        assert!(mesh.limit_smoothing_deformation(&before));
        assert_eq!(&mesh.vertices[4..], safe);
        // A sharper V has opposing normals but no triangle inversion or
        // intersection. A dihedral sign test must not lock fairing in place.
        assert_eq!(mesh.vertices[2].0, -0.5);
        assert_eq!(mesh.vertices[3].0, 0.5);
        for t in &mesh.triangles {
            assert!(safe_face(
                t.map(|i| before[i]),
                t.map(|i| mesh.vertices[i]),
                None
            ));
        }
    }

    #[test]
    fn directional_stamp_allows_steep_relief_but_rejects_projected_collapse() {
        for scale in [0.001, 1., 1000.] {
            let before = [Vec3(0., 0., 0.), Vec3(1., 0., 1.), Vec3(0., 1., 0.)].map(|p| p * scale);
            let mut after = before;
            after[1].2 = -3. * scale;
            let direction = Some(Vec3(0., 0., 1.));
            assert!(!safe_face(before, after, None));
            assert!(safe_face(before, after, direction));
            // A tangent/back-facing projection cannot override the original guard.
            assert!(!safe_face(before, after, Some(Vec3(0., 1., 0.))));
            assert!(!safe_face(before, after, Some(Vec3(0., 0., -1.))));
            // Correct winding at release does not excuse crossing zero area
            // midway through the motion, even with a stamp projection.
            after[1] = before[1] * -1.;
            after[2] = before[2] * -1.;
            assert!(!safe_face(before, after, direction));
            after = before;
            after[1] = before[0];
            assert!(!safe_face(before, after, direction));
            // Unchanged projected area still needs the triangle quality check.
            after = before;
            after[1].2 = 1000. * scale;
            assert!(!safe_face(before, after, direction));
        }
    }

    #[test]
    fn projection_fold_guard_is_local_and_preserves_existing_sharp_edges() {
        for scale in [0.01, 1., 100.] {
            let before = [
                Vec3(0., -1., 0.),
                Vec3(0., 1., 0.),
                Vec3(-1., 0., 0.),
                Vec3(1., 0., 0.),
                Vec3(3., 0., 0.),
                Vec3(4., 0., 0.),
                Vec3(3., 1., 0.),
            ]
            .map(|p| p * scale);
            let mut mesh = Mesh {
                vertices: before.into(),
                triangles: vec![[0, 1, 2], [1, 0, 3], [4, 5, 6]],
                mask: vec![0.; 7],
                ..Mesh::default()
            };
            mesh.vertices[2] = Vec3(-0.25, 0., 1.) * scale;
            mesh.vertices[3] = Vec3(0.25, 0., 1.) * scale;
            for v in &mut mesh.vertices[4..] {
                v.2 += scale;
            }
            assert!(
                mesh.triangles.iter().all(|&t| safe_face(
                    t.map(|v| before[v]),
                    t.map(|v| mesh.vertices[v]),
                    None
                )),
                "A face-only check misses the fold"
            );
            let independent_patch = mesh.vertices[4..].to_vec();
            assert!(mesh.limit_projection_deformation(&before));
            assert!(
                normal_agreement(
                    face_normal(mesh.triangles[0].map(|v| mesh.vertices[v])),
                    face_normal(mesh.triangles[1].map(|v| mesh.vertices[v])),
                ) >= 0.
            );
            assert_eq!(&mesh.vertices[4..], independent_patch);

            // An already sharp edge is allowed to translate without flattening.
            mesh.vertices[2] = Vec3(-0.25, 0., 1.) * scale;
            mesh.vertices[3] = Vec3(0.25, 0., 1.) * scale;
            let sharp = mesh.vertices.clone();
            for v in &mut mesh.vertices {
                v.2 += scale;
            }
            let translated = mesh.vertices.clone();
            assert!(mesh.limit_projection_deformation(&sharp));
            assert_eq!(mesh.vertices, translated);
        }
    }

    #[test]
    fn oversized_snake_pull_cannot_fold_the_surface() {
        use crate::sculpt::{Brush, Dab, Kind};
        let before = Mesh::sphere();
        let brush = Brush::new(Kind::Snake);
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3(0.2, 0., 0.),
            invert: false,
        };
        let proposed: Vec<_> = before
            .vertices
            .iter()
            .map(|&p| p + brush.snake_motion(dab) * brush.snake_weight(dab, p - dab.center))
            .collect();
        assert!(
            before.triangles.iter().any(|&t| {
                let [a, b, c] = t.map(|v| before.vertices[v]);
                let [x, y, z] = t.map(|v| proposed[v]);
                (b - a).cross(c - a).dot((y - x).cross(z - x)) <= 0.
            }),
            "The unrestricted pull reproduces a folded face"
        );
        let mut guarded = before.clone();
        guarded.stamp(&brush, dab);
        assert!(before.triangles.iter().all(|&t| safe_face(
            t.map(|v| before.vertices[v]),
            t.map(|v| guarded.vertices[v]),
            None,
        )));
        guarded.validate_closed().unwrap();
    }

    #[test]
    fn unsafe_motion_is_reduced_locally_and_cannot_cross_a_collapsed_face() {
        for scale in [0.001, 1., 1000.] {
            let before: Vec<_> = [
                Vec3(0., 0., 0.),
                Vec3(1., 0., 0.),
                Vec3(0., 1., 0.),
                Vec3(3., 0., 0.),
                Vec3(4., 0., 0.),
                Vec3(3., 1., 0.),
            ]
            .map(|p| (p + Vec3(100., 0., 0.)) * scale)
            .into();
            let mut mesh = Mesh {
                vertices: before.clone(),
                triangles: vec![[0, 1, 2], [3, 4, 5]],
                mask: vec![0.; 6],
                quads: Vec::new(),
                creases: Vec::new(),
                vertex_groups: Vec::new(),
                polygroups: Vec::new(),
                hidden_faces: Vec::new(),
            };
            // The endpoint is correctly wound, but reaching it crosses zero area.
            mesh.vertices[1] = before[0] - (before[1] - before[0]);
            mesh.vertices[2] = before[0] - (before[2] - before[0]);
            for (v, &original) in before.iter().enumerate().skip(3) {
                mesh.vertices[v] = original + Vec3(0., 0., scale);
            }
            let safe_patch = mesh.vertices[3..].to_vec();
            assert!(mesh.limit_sculpt_deformation(&before));
            assert_eq!(&mesh.vertices[3..], &safe_patch);
            assert!(mesh.vertices[1].0 > before[0].0);
            assert!(mesh.vertices[2].1 > before[0].1);
            assert!(
                mesh.vertices[1].0 < before[1].0,
                "Keep useful movement instead of cancelling the whole dab"
            );
            assert_eq!(mesh.mask, vec![0.; 6]);
            mesh.validate().unwrap();
        }
    }
}

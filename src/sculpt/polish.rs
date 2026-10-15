//! Level a local surface patch without relaxing its sharp features.
use super::{Brush, Dab, Mesh, Vec3};

impl Mesh {
    pub(super) fn polish_planes(
        &self,
        brush: &Brush,
        dabs: &[Dab],
        normals: &[Vec3],
        sculptable: &mut [bool],
    ) -> Vec<Option<(Vec3, Vec3)>> {
        let visible = sculptable.to_vec();
        let mut centers = vec![Vec3::default(); dabs.len()];
        let mut directions = centers.clone();
        let mut weights = vec![0.; dabs.len()];
        for &[a, b] in &self.creases {
            sculptable[a] = false;
            sculptable[b] = false;
        }
        for &[a, b, c] in &self.triangles {
            let [pa, pb, pc] = [a, b, c].map(|i| self.vertices[i]);
            let cross = (pb - pa).cross(pc - pa);
            let area = cross.len();
            if area <= 1e-12 {
                continue;
            }
            let normal = cross * (1. / area);
            // A face differing sharply from the shared vertex normal indicates
            // a corner. Keep its position instead of rounding it into the patch.
            for v in [a, b, c] {
                if normal.dot(normals[v]) < 0.85 {
                    sculptable[v] = false;
                }
            }
            if ![a, b, c].iter().all(|&i| visible[i]) {
                continue;
            }
            let unmasked = [a, b, c]
                .map(|i| 1. - self.mask[i])
                .into_iter()
                .sum::<f32>()
                / 3.;
            for (i, dab) in dabs.iter().enumerate() {
                if normal.dot(dab.normal) < 0.5 {
                    continue;
                }
                // Accumulate relative to the dab for stability away from origin.
                let offset =
                    ((pa - dab.center) + (pb - dab.center) + (pc - dab.center)) * (1. / 3.);
                if offset.len() > dab.radius * 1.5 {
                    continue;
                }
                let weight = area
                    * unmasked
                    * brush.weight(
                        offset.dot(dab.right) / dab.radius,
                        offset.dot(dab.up) / dab.radius,
                    );
                centers[i] = centers[i] + offset * weight;
                directions[i] = directions[i] + normal * weight;
                weights[i] += weight;
            }
        }
        dabs.iter()
            .enumerate()
            .map(|(i, dab)| {
                (weights[i] > 1e-12 && directions[i].len() > 1e-12).then(|| {
                    (
                        dab.center + centers[i] * (1. / weights[i]),
                        directions[i].norm(),
                    )
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Kind, Primitive, PrimitiveKind};

    fn noisy_face() -> Mesh {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
        primitive.divisions = [16, 16, 2];
        let mut mesh = primitive.mesh().unwrap();
        for p in &mut mesh.vertices {
            if p.2 > 0.9 && p.0.abs() < 0.9 && p.1.abs() < 0.9 {
                p.2 += 0.025 * (p.0 * 18.).cos() * (p.1 * 18.).cos();
            }
        }
        mesh
    }
    fn dab() -> Dab {
        Dab {
            center: Vec3(0., 0., 1.025),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 1.4,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        }
    }
    fn relief(mesh: &Mesh) -> f32 {
        let heights: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|p| p.2 > 0.9 && p.0.abs() < 0.7 && p.1.abs() < 0.7)
            .map(|p| p.2)
            .collect();
        let mean = heights.iter().sum::<f32>() / heights.len() as f32;
        heights.iter().map(|z| (z - mean).powi(2)).sum::<f32>() / heights.len() as f32
    }
    #[test]
    fn polish_levels_relief_without_rounding_corners_or_moving_protected_vertices() {
        let mut mesh = noisy_face();
        let original = mesh.clone();
        let protected = mesh
            .vertices
            .iter()
            .position(|p| p.0 == 0. && p.1 == 0. && p.2 > 0.9)
            .unwrap();
        mesh.mask[protected] = 1.;
        let brush = Brush::new(Kind::Polish);
        for _ in 0..12 {
            mesh.stamp(&brush, dab());
        }
        assert!(relief(&mesh) < relief(&original) * 0.4);
        assert_eq!(mesh.vertices[protected], original.vertices[protected]);
        for (a, b) in original.vertices.iter().zip(&mesh.vertices) {
            if a.0.abs() == 1. || a.1.abs() == 1. || a.2 < 0. {
                assert_eq!(a, b);
            }
        }
        assert_eq!(mesh.triangles, original.triangles);
        assert_eq!(mesh.quads, original.quads);
        mesh.validate_closed().unwrap();
    }
    #[test]
    fn polish_pressure_inversion_crease_and_symmetry_are_consistent() {
        let source = noisy_face();
        let brush = Brush::new(Kind::Polish);
        let mut full = source.clone();
        full.stamp(&brush, dab());
        let mut half = source.clone();
        half.stamp(
            &brush,
            Dab {
                pressure: 0.5,
                ..dab()
            },
        );
        let mut inverse = source.clone();
        inverse.stamp(
            &brush,
            Dab {
                invert: true,
                ..dab()
            },
        );
        let mut copies = source.clone();
        copies.stamp_many(&brush, &[dab(), dab()]);
        for i in 0..source.vertices.len() {
            let delta = full.vertices[i] - source.vertices[i];
            assert!((half.vertices[i] - source.vertices[i] - delta * 0.5).len() < 2e-7);
            assert!((inverse.vertices[i] - source.vertices[i] + delta).len() < 2e-7);
            assert!((copies.vertices[i] - full.vertices[i]).len() < 2e-7);
        }
        let mut creased = source.clone();
        let edge = creased
            .polygon_edges()
            .into_iter()
            .find(|edge| {
                edge.iter().all(|&v| {
                    source.vertices[v].2 > 0.9
                        && source.vertices[v].0.abs() < 0.5
                        && source.vertices[v].1.abs() < 0.5
                })
            })
            .unwrap();
        creased.creases.push(edge);
        creased.stamp(&brush, dab());
        for v in edge {
            assert_eq!(creased.vertices[v], source.vertices[v]);
        }
        let mut hidden = source.clone();
        hidden.hidden_faces = vec![true; hidden.triangles.len()];
        assert!(!hidden.stamp_many(&brush, &[dab()]));
        assert_eq!(hidden.vertices, source.vertices);
    }
}

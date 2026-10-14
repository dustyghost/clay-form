//! Keep the incision direction stable as a Detail stroke crosses its own groove.
use super::{Dab, Mesh, Vec3};

// Four edges per Gaussian core radius; the raised shoulder needs resolution too.
pub(super) const CORE_EDGE_FRACTION: f32 = 0.05;
pub(super) const FEATURE_RADIUS_FRACTION: f32 = 0.6;

impl Mesh {
    /// Use an area-weighted patch normal, rather than following individual
    /// valley-wall normals as adaptive triangles change under the cursor.
    /// The taper avoids abrupt changes as faces enter the brush footprint.
    pub(super) fn detail_dabs(&self, dabs: &[Dab]) -> Vec<Dab> {
        dabs.iter()
            .map(|dab| {
                let mut normal = Vec3::default();
                for (face, &[a, c, d]) in self.triangles.iter().enumerate() {
                    if !self.face_visible(face) {
                        continue;
                    }
                    let [a, c, d] = [a, c, d].map(|i| self.vertices[i]);
                    let cross = (c - a).cross(d - a);
                    let offset = (a + c + d) * (1. / 3.) - dab.center;
                    let r = offset.len() / dab.radius;
                    if r < 1. && cross.dot(dab.normal) > 0. {
                        normal = normal + cross * (1. - r * r).powi(2);
                    }
                }
                Dab {
                    normal: if normal.len() > 1e-8 {
                        normal.norm()
                    } else {
                        dab.normal
                    },
                    ..*dab
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{AdaptiveBudget, AdaptiveSettings, Brush, Kind};

    #[test]
    fn adaptive_detail_regularization_preserves_masked_and_distant_vertices() {
        let mut mesh = Mesh::sphere();
        for (v, mask) in mesh.vertices.iter().zip(&mut mesh.mask) {
            if v.0 < 0. {
                *mask = 1.;
            }
        }
        let before = mesh.clone();
        let brush = Brush::new(Kind::Detail);
        let settings = AdaptiveSettings {
            enabled: true,
            ..Default::default()
        };
        let dab = Dab {
            center: Vec3(0., 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        for _ in 0..4 {
            let mut budget = AdaptiveBudget::default();
            mesh.adapt(&brush, &[dab], &settings, &mut budget, true);
            mesh.stamp(&brush, dab);
            mesh.adapt(&brush, &[dab], &settings, &mut budget, false);
        }
        assert_ne!(mesh, before);
        mesh.validate_closed().unwrap();
        for (i, v) in before.vertices.iter().enumerate() {
            if before.mask[i] == 1. || v.2 < 0. {
                assert!(
                    mesh.vertices
                        .iter()
                        .zip(&mesh.mask)
                        .any(|(p, m)| p == v && *m == before.mask[i]),
                    "protected position or mask changed"
                );
            }
        }
    }
}

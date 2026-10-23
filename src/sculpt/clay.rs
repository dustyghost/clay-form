//! Area-weighted surface reference for Clay and stroke-stable Flatten.

use super::{Dab, Mesh, Vec3};

impl Mesh {
    /// Sample a whole patch instead of following the tiny ridge under the cursor.
    /// Area weighting keeps adaptive triangle density from biasing the plane;
    /// a smooth spatial taper avoids jumps when triangles enter the footprint.
    pub(crate) fn surface_planes(&self, dabs: &[Dab]) -> Vec<(Vec3, Vec3)> {
        dabs.iter()
            .map(|dab| {
                let mut center = Vec3::default();
                let mut direction = Vec3::default();
                let mut total = 0.;
                for (face, &[a, b, c]) in self.triangles.iter().enumerate() {
                    if !self.face_visible(face) {
                        continue;
                    }
                    let [pa, pb, pc] = [a, b, c].map(|i| self.vertices[i]);
                    let offset =
                        ((pa - dab.center) + (pb - dab.center) + (pc - dab.center)) * (1. / 3.);
                    let r2 = offset.dot(offset) / (dab.radius * dab.radius);
                    if r2 >= 1. {
                        continue;
                    }
                    let cross = (pb - pa).cross(pc - pa);
                    let area = cross.len();
                    if area <= 1e-12 || cross.dot(dab.normal) <= area * 0.25 {
                        continue;
                    }
                    let mobility = (3. - self.mask[a] - self.mask[b] - self.mask[c]) / 3.;
                    let weight = (1. - r2).powi(2) * mobility;
                    center = center + offset * (area * weight);
                    direction = direction + cross * weight;
                    total += area * weight;
                }
                if total > 1e-12 && direction.len() > 1e-12 {
                    (dab.center + center * (1. / total), direction.norm())
                } else {
                    // A footprint smaller than a face still needs to lay down clay.
                    (dab.center, dab.normal)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::sculpt::{Brush, Dab, Kind, Mesh, Primitive, PrimitiveKind, Vec3};

    fn plane() -> Mesh {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Plane);
        primitive.divisions = [40, 40, 1];
        primitive.mesh().unwrap()
    }

    fn dab() -> Dab {
        Dab {
            center: Vec3::default(),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.6,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        }
    }

    #[test]
    fn clay_add_and_subtract_build_equivalent_masses_on_a_flat_surface() {
        let mut add = plane();
        let original = add.clone();
        let mut subtract = add.clone();
        let brush = Brush::new(Kind::Clay);
        for _ in 0..12 {
            add.stamp(&brush, dab());
            subtract.stamp(
                &brush,
                Dab {
                    invert: true,
                    ..dab()
                },
            );
        }
        for ((before, added), removed) in original
            .vertices
            .iter()
            .zip(&add.vertices)
            .zip(&subtract.vertices)
        {
            assert!(
                (added.2 + removed.2 - 2. * before.2).abs() < 1e-5,
                "add/subtract diverged: {} vs {}",
                added.2,
                removed.2
            );
        }
        assert!(add.vertices.iter().any(|v| v.2 > 0.05));
    }

    #[test]
    fn clay_uses_the_surface_patch_instead_of_a_single_picked_normal() {
        let mut straight = plane();
        let mut tilted = straight.clone();
        let brush = Brush::new(Kind::Clay);
        straight.stamp(&brush, dab());
        tilted.stamp(
            &brush,
            Dab {
                normal: Vec3(0.35, 0., 1.).norm(),
                ..dab()
            },
        );
        for (a, b) in straight.vertices.iter().zip(&tilted.vertices) {
            assert!(
                (*a - *b).len() < 1e-5,
                "picked normal changed a flat patch: {a:?} vs {b:?}"
            );
        }
    }

    #[test]
    fn clay_fills_low_spots_more_than_peaks_while_standard_keeps_the_relief() {
        let mut original = plane();
        for p in &mut original.vertices {
            p.2 = 0.025 * (p.0 * 30.).sin();
        }
        let low = original
            .vertices
            .iter()
            .position(|p| (p.0 + 0.05).abs() < 1e-6 && p.1.abs() < 1e-6)
            .unwrap();
        let high = original
            .vertices
            .iter()
            .position(|p| (p.0 - 0.05).abs() < 1e-6 && p.1.abs() < 1e-6)
            .unwrap();
        let mut clay = original.clone();
        let mut standard = original.clone();
        clay.stamp(&Brush::new(Kind::Clay), dab());
        standard.stamp(&Brush::new(Kind::Standard), dab());
        let relief = |mesh: &Mesh| mesh.vertices[high].2 - mesh.vertices[low].2;
        assert!(relief(&clay) < relief(&original) * 0.85);
        assert!((relief(&standard) - relief(&original)).abs() < 1e-6);
        assert!(clay.vertices[high].2 > original.vertices[high].2);
        assert!(clay.vertices[low].2 > original.vertices[low].2);
    }

    #[test]
    fn clay_pressure_protection_alpha_and_symmetry_preserve_their_meaning() {
        use crate::sculpt::Alpha;
        let original = plane();
        let brush = Brush::new(Kind::Clay);
        let mut full = original.clone();
        full.stamp(&brush, dab());
        let mut half = original.clone();
        half.stamp(
            &brush,
            Dab {
                pressure: 0.5,
                ..dab()
            },
        );
        let mut duplicate = original.clone();
        duplicate.stamp_many(&brush, &[dab(), dab()]);
        let mut masked = original.clone();
        for (p, mask) in masked.vertices.iter().zip(&mut masked.mask) {
            if p.0 < 0. {
                *mask = 1.;
            }
        }
        masked.stamp(&brush, dab());
        let mut reversed = original.clone();
        reversed.stamp(
            &Brush {
                subtract: true,
                ..brush.clone()
            },
            dab(),
        );
        for (i, p) in original.vertices.iter().enumerate() {
            let delta = full.vertices[i] - *p;
            assert!((half.vertices[i] - *p - delta * 0.5).len() < 1e-6);
            assert!((duplicate.vertices[i] - full.vertices[i]).len() < 1e-6);
            assert!((reversed.vertices[i] - *p + delta).len() < 1e-6);
            if p.0 < 0. {
                assert_eq!(masked.vertices[i], *p);
            }
        }
        for alpha in [
            Alpha::Hard,
            Alpha::Square,
            Alpha::Image {
                width: 2,
                height: 1,
                pixels: vec![0., 1.].into(),
            },
        ] {
            let mut mesh = original.clone();
            mesh.stamp(
                &Brush {
                    alpha,
                    ..brush.clone()
                },
                dab(),
            );
            assert_ne!(mesh.vertices, original.vertices);
            mesh.validate().unwrap();
        }
        let mut hidden = original.clone();
        hidden.hidden_faces = vec![true; hidden.triangles.len()];
        assert!(!hidden.stamp_many(&brush, &[dab()]));
        assert_eq!(hidden.vertices, original.vertices);
    }

    #[test]
    fn clay_drag_stamp_does_not_overshoot_its_target_plane_at_full_strength() {
        use crate::sculpt::{Alpha, StrokeMode};
        let mut mesh = plane();
        let brush = Brush {
            strength: 1.,
            alpha: Alpha::Hard,
            stroke_mode: StrokeMode::DragDot,
            ..Brush::new(Kind::Clay)
        };
        mesh.stamp(&brush, dab());
        let peak = mesh.vertices.iter().map(|p| p.2).fold(0., f32::max);
        assert!(
            peak > 0.05 && peak <= dab().radius * 0.21,
            "stamp overshot its clay layer: {peak}"
        );
    }
}

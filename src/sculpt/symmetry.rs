//! Symmetry transforms operate in object coordinates around a configurable origin.
use super::{Dab, Vec3};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
    Z,
}
impl Axis {
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];
    pub fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3(1., 0., 0.),
            Self::Y => Vec3(0., 1., 0.),
            Self::Z => Vec3(0., 0., 1.),
        }
    }
    pub fn rotate(self, v: Vec3, angle: f32) -> Vec3 {
        v.rotated(self.vector(), angle)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Symmetry {
    pub enabled: bool,
    pub mirror: [bool; 3],
    pub center: Vec3,
    pub radial: bool,
    pub radial_axis: Axis,
    pub radial_count: u32,
}
impl Default for Symmetry {
    fn default() -> Self {
        Self {
            enabled: false,
            mirror: [true, false, false],
            center: Vec3::default(),
            radial: false,
            radial_axis: Axis::Y,
            radial_count: 6,
        }
    }
}
impl Symmetry {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=16).contains(&self.radial_count)
            || [self.center.0, self.center.1, self.center.2]
                .iter()
                .any(|n| !n.is_finite() || n.abs() > 1_000_000.)
        {
            return Err("Invalid symmetry settings".into());
        }
        Ok(())
    }
    pub fn dabs(&self, dab: Dab) -> Vec<Dab> {
        if !self.enabled {
            return vec![dab];
        }
        let mut mirrored = vec![dab];
        for (i, enabled) in self.mirror.iter().enumerate() {
            if !enabled {
                continue;
            }
            let reflect = |v: Vec3| {
                let axis = Axis::ALL[i].vector();
                v - axis * (2. * v.dot(axis))
            };
            let copies: Vec<_> = mirrored
                .iter()
                .map(|d| Dab {
                    center: self.center + reflect(d.center - self.center),
                    normal: reflect(d.normal),
                    right: reflect(d.right),
                    up: reflect(d.up),
                    delta: reflect(d.delta),
                    ..*d
                })
                .collect();
            mirrored.extend(copies);
        }
        if !self.radial {
            return mirrored;
        }
        let mut all = Vec::with_capacity(mirrored.len() * self.radial_count as usize);
        for step in 0..self.radial_count {
            let angle = std::f32::consts::TAU * step as f32 / self.radial_count as f32;
            let rotate = |v| self.radial_axis.rotate(v, angle);
            all.extend(mirrored.iter().map(|d| Dab {
                center: self.center + rotate(d.center - self.center),
                normal: rotate(d.normal),
                right: rotate(d.right),
                up: rotate(d.up),
                delta: rotate(d.delta),
                ..*d
            }));
        }
        all
    }
    /// Mirror guides span a whole plane; radial boundaries span only their ray's half-plane.
    pub fn guides(&self) -> Vec<GuidePlane> {
        if !self.enabled {
            return vec![];
        }
        let mut guides: Vec<_> = self
            .mirror
            .iter()
            .enumerate()
            .filter(|(_, enabled)| **enabled)
            .map(|(i, _)| GuidePlane {
                normal: Axis::ALL[i].vector(),
                side: None,
            })
            .collect();
        if self.radial {
            let base = match self.radial_axis {
                Axis::X => Axis::Y.vector(),
                Axis::Y | Axis::Z => Axis::X.vector(),
            };
            for step in 0..self.radial_count {
                let normal = self.radial_axis.rotate(
                    base,
                    std::f32::consts::TAU * step as f32 / self.radial_count as f32,
                );
                if !guides
                    .iter()
                    .any(|g| g.side.is_none() && g.normal.dot(normal).abs() > 0.99999)
                {
                    guides.push(GuidePlane {
                        normal,
                        side: Some(self.radial_axis.vector().cross(normal)),
                    });
                }
            }
        }
        guides
    }
}

/// `side` clips a radial guide to the half-plane pointing toward one sector boundary.
pub struct GuidePlane {
    pub normal: Vec3,
    pub side: Option<Vec3>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Brush, Kind, Mesh};
    fn dab(center: Vec3) -> Dab {
        Dab {
            center,
            normal: center.norm(),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.25,
            pressure: 1.,
            delta: Vec3(0.1, 0.2, 0.3),
            invert: false,
        }
    }
    fn close(a: Vec3, b: Vec3) {
        assert!((a - b).len() < 1e-5, "{a:?} differs from {b:?}");
    }
    #[test]
    fn combined_mirrors_reflect_points_and_grab_vectors_about_offset_center() {
        let symmetry = Symmetry {
            enabled: true,
            mirror: [true, true, true],
            center: Vec3(1., 2., 3.),
            ..Default::default()
        };
        let copies = symmetry.dabs(dab(Vec3(1.3, 2.4, 3.5)));
        assert_eq!(copies.len(), 8);
        close(copies[7].center, Vec3(0.7, 1.6, 2.5));
        close(copies[7].delta, Vec3(-0.1, -0.2, -0.3));
        close(copies[7].right, Vec3(-1., 0., 0.));
    }
    #[test]
    fn radial_copies_rotate_around_the_selected_offset_axis() {
        let symmetry = Symmetry {
            enabled: true,
            mirror: [false; 3],
            radial: true,
            radial_axis: Axis::Y,
            radial_count: 4,
            center: Vec3(1., 2., 3.),
        };
        let copies = symmetry.dabs(dab(Vec3(2., 2., 3.)));
        for (copy, expected) in copies.iter().zip([
            Vec3(2., 2., 3.),
            Vec3(1., 2., 2.),
            Vec3(0., 2., 3.),
            Vec3(1., 2., 4.),
        ]) {
            close(copy.center, expected);
        }
        close(copies[1].right, Vec3(0., 0., -1.));
        assert_eq!(symmetry.guides().len(), 4);
    }
    #[test]
    fn near_plane_strokes_affect_both_sides_and_overlaps_do_not_double_strength() {
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        let mut mesh = Mesh::sphere();
        let nearest = |point: Vec3| {
            mesh.vertices
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| (**a - point).len().total_cmp(&(**b - point).len()))
                .unwrap()
                .0
        };
        let a = nearest(Vec3(0.065, 0., 0.998));
        let v = mesh.vertices[a];
        let b = nearest(Vec3(-v.0, v.1, v.2));
        let before = mesh.vertices[a];
        let brush = Brush::new(Kind::Standard);
        mesh.stamp_many(&brush, &symmetry.dabs(dab(before)));
        assert!(mesh.vertices[a].2 > before.2);
        close(
            mesh.vertices[a],
            Vec3(-mesh.vertices[b].0, mesh.vertices[b].1, mesh.vertices[b].2),
        );
        let mut single = Mesh::sphere();
        let mut mirrored = single.clone();
        let center_dab = dab(Vec3(0., 0., 1.));
        single.stamp(&brush, center_dab);
        mirrored.stamp_many(&brush, &symmetry.dabs(center_dab));
        for (a, b) in single.vertices.iter().zip(&mirrored.vertices) {
            close(*a, *b);
        }
    }
    #[test]
    fn radial_strokes_on_the_axis_do_not_multiply_strength() {
        let symmetry = Symmetry {
            enabled: true,
            mirror: [false; 3],
            radial: true,
            radial_count: 16,
            ..Default::default()
        };
        let mut single = Mesh::sphere();
        let mut radial = single.clone();
        let brush = Brush::new(Kind::Inflate);
        let mut d = dab(Vec3(0., 1., 0.));
        d.up = Vec3(0., 0., 1.);
        single.stamp(&brush, d);
        radial.stamp_many(&brush, &symmetry.dabs(d));
        for (a, b) in single.vertices.iter().zip(&radial.vertices) {
            close(*a, *b);
        }
    }
    #[test]
    fn invalid_saved_settings_are_rejected() {
        let mut symmetry = Symmetry {
            radial_count: 100,
            ..Default::default()
        };
        assert!(symmetry.validate().is_err());
        symmetry.radial_count = 6;
        symmetry.center.0 = f32::NAN;
        assert!(symmetry.validate().is_err());
    }
}

#[cfg(test)]
mod seam_tests {
    use super::*;
    use crate::sculpt::{Brush, Kind, Mesh};

    #[test]
    fn tilted_standard_dabs_keep_the_mirror_seam_fixed_and_ignore_copy_order() {
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        let original = Mesh::sphere();
        let brush = Brush::new(Kind::Standard);
        let dab = Dab {
            center: Vec3(0.04, 0., 1.),
            normal: Vec3(0.6, 0., 0.8),
            right: Vec3(0.8, 0., -0.6),
            up: Vec3(0., 1., 0.),
            radius: 0.35,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        for invert in [false, true] {
            let copies = symmetry.dabs(Dab { invert, ..dab });
            let mut reverse = copies.clone();
            reverse.reverse();
            let mut forward_mesh = original.clone();
            let mut reverse_mesh = original.clone();
            for _ in 0..12 {
                forward_mesh.stamp_many(&brush, &copies);
                reverse_mesh.stamp_many(&brush, &reverse);
            }
            let mut affected_seam = 0;
            for (i, before) in original.vertices.iter().enumerate() {
                let after = forward_mesh.vertices[i];
                assert!(
                    (after - reverse_mesh.vertices[i]).len() < 1e-6,
                    "copy order changed vertex {i}"
                );
                if before.0.abs() < 1e-6 && before.2 > 0.95 && before.1.abs() < 0.15 {
                    affected_seam += 1;
                    assert!(
                        after.0.abs() < 1e-6,
                        "symmetry seam drifted sideways: {before:?} -> {after:?}"
                    );
                    assert!(
                        (after.2 - before.2) * if invert { -1. } else { 1. } > 0.001,
                        "seam must still receive the brush stroke"
                    );
                }
            }
            assert!(affected_seam > 0);
        }
    }
    #[test]
    fn mirrored_blending_respects_offset_planes_masks_and_single_copy_strength() {
        let symmetry = Symmetry {
            enabled: true,
            center: Vec3(0.3, -0.2, 0.1),
            ..Default::default()
        };
        let mut original = Mesh::sphere();
        for vertex in &mut original.vertices {
            *vertex = *vertex + symmetry.center;
        }
        let dab = Dab {
            center: symmetry.center + Vec3(0.04, 0., 1.),
            normal: Vec3(0.6, 0., 0.8),
            right: Vec3(0.8, 0., -0.6),
            up: Vec3(0., 1., 0.),
            radius: 0.35,
            pressure: 1.,
            delta: Vec3(0.1, 0., 0.),
            invert: false,
        };
        let seam = original
            .vertices
            .iter()
            .position(|v| (v.0 - 0.3).abs() < 1e-6 && (v.1 + 0.2).abs() < 1e-6 && v.2 > 1.)
            .unwrap();
        let brush = Brush::new(Kind::Standard);
        let mut masked = original.clone();
        masked.mask[seam] = 1.;
        let mask = masked.mask.clone();
        masked.stamp_many(&brush, &symmetry.dabs(dab));
        assert_eq!(masked.vertices[seam], original.vertices[seam]);
        assert_eq!(masked.mask, mask);
        let mut mirrored = original.clone();
        mirrored.stamp_many(&brush, &symmetry.dabs(dab));
        assert!((mirrored.vertices[seam].0 - 0.3).abs() < 1e-6);
        assert!(mirrored.vertices[seam].2 > original.vertices[seam].2);
        for (before, after) in original.vertices.iter().zip(&mirrored.vertices) {
            assert!(
                (*after - *before).len() <= dab.radius * 0.15 * brush.strength + 1e-6,
                "overlap multiplied brush strength"
            );
        }
        let mask_brush = Brush::new(Kind::Mask);
        let mut once = original.clone();
        let mut duplicates = original.clone();
        once.stamp(&mask_brush, dab);
        duplicates.stamp_many(&mask_brush, &[dab, dab]);
        assert_eq!(once.mask, duplicates.mask);
    }
}

#[cfg(test)]
mod seam_depth_tests {
    use super::*;
    use crate::sculpt::{Alpha, Brush, Kind, Mesh};

    #[test]
    fn standard_overlap_keeps_full_depth_when_mirrored_normals_diverge() {
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        for (alpha, mask, pressure) in [
            (Alpha::Hard, 0., 1.),
            (Alpha::Soft, 0.4, 0.6),
            (Alpha::Soft, 1., 0.8),
        ] {
            for invert in [false, true] {
                let mut mesh = Mesh::sphere();
                let seam = mesh
                    .vertices
                    .iter()
                    .position(|v| v.0.abs() < 1e-6 && v.1.abs() < 1e-6 && v.2 > 0.99)
                    .unwrap();
                let before = mesh.vertices[seam];
                let mut brush = Brush::new(Kind::Standard);
                brush.alpha = alpha.clone();
                brush.pressure = true;
                mesh.mask[seam] = mask;
                // Two steep mirrored normals have equal brush coverage at the seam.
                // Cancelling their lateral components must not also weaken brush depth.
                let dab = Dab {
                    center: Vec3(0.04, 0., 1.),
                    normal: Vec3(0.8, 0., 0.6),
                    right: Vec3(0.6, 0., -0.8),
                    up: Vec3(0., 1., 0.),
                    radius: 0.3,
                    pressure,
                    delta: Vec3::default(),
                    invert,
                };
                mesh.stamp_many(&brush, &symmetry.dabs(dab));
                let change = mesh.vertices[seam] - before;
                let offset = before - dab.center;
                let expected = dab.radius
                    * 0.15
                    * brush.strength
                    * pressure.powf(brush.pressure_curve)
                    * brush.weight(
                        offset.dot(dab.right) / dab.radius,
                        offset.dot(dab.up) / dab.radius,
                    )
                    * (1. - mask);
                let peak = dab.radius
                    * 0.15
                    * brush.strength
                    * pressure.powf(brush.pressure_curve)
                    * (1. - mask);
                assert!(
                    change.2.abs() + 1e-6 >= expected && change.2.abs() <= peak + 1e-6,
                    "seam depth {} should retain falloff depth {expected} without exceeding peak {peak}",
                    change.2.abs()
                );
                if matches!(alpha, Alpha::Hard) {
                    assert!((change.2.abs() - expected).abs() < 1e-6);
                }
                assert!(change.0.abs() < 1e-6);
                if mask < 1. {
                    assert_eq!(change.2 < 0., invert);
                }
            }
        }
    }
}

#[cfg(test)]
mod soft_join_tests {
    use super::*;
    use crate::sculpt::{Brush, Kind, Mesh};
    #[test]
    fn close_soft_standard_footprints_join_without_a_central_dip() {
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        let original = Mesh::sphere();
        let mut brush = Brush::new(Kind::Standard);
        brush.pressure = false;
        let dab = Dab {
            center: Vec3(0.035, 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        let sample = |x: f32| {
            original
                .vertices
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (**a - Vec3(x, 0., 1.))
                        .len()
                        .total_cmp(&(**b - Vec3(x, 0., 1.)).len())
                })
                .unwrap()
                .0
        };
        let center = sample(0.);
        for invert in [false, true] {
            let mut mesh = original.clone();
            mesh.stamp_many(&brush, &symmetry.dabs(Dab { invert, ..dab }));
            let depth = |i: usize| (mesh.vertices[i].2 - original.vertices[i].2).abs();
            for side in [-1., 1.] {
                assert!(
                    depth(center) + 1e-6 >= depth(sample(side * 0.06)),
                    "centre stroke is weaker than its flanks: {} < {}",
                    depth(center),
                    depth(sample(side * 0.06))
                );
            }
            assert!(depth(center) <= dab.radius * 0.15 * brush.strength + 1e-6);
        }
    }
}

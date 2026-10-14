//! Affine model transforms, shared by world handles and Action Line.
use super::Vec3;
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub pivot: Vec3,
    pub translation: Vec3,
    pub scale: Vec3,
    /// Optional directional stretch (unit direction, positive factor).
    pub stretch: Option<(Vec3, f32)>,
    /// Unit axis and right-handed angle in radians, applied after scaling.
    pub rotation: Option<(Vec3, f32)>,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            pivot: Vec3::default(),
            translation: Vec3::default(),
            scale: Vec3(1., 1., 1.),
            stretch: None,
            rotation: None,
        }
    }
}
impl Transform {
    pub fn is_identity(&self) -> bool {
        self.translation == Vec3::default()
            && self.scale == Vec3(1., 1., 1.)
            && self.stretch.is_none_or(|(_, factor)| factor == 1.)
            && self.rotation.is_none_or(|(_, angle)| angle == 0.)
    }
    pub fn validate(&self) -> Result<(), String> {
        let finite = |v: Vec3| v.0.is_finite() && v.1.is_finite() && v.2.is_finite();
        if !finite(self.pivot)
            || !finite(self.translation)
            || !finite(self.scale)
            || [self.scale.0, self.scale.1, self.scale.2]
                .iter()
                .any(|s| *s < 0.001 || *s > 1000.)
            || self.stretch.is_some_and(|(axis, factor)| {
                !finite(axis)
                    || (axis.len() - 1.).abs() > 0.001
                    || !factor.is_finite()
                    || !(0.001..=1000.).contains(&factor)
            })
            || self.rotation.is_some_and(|(axis, angle)| {
                !finite(axis) || (axis.len() - 1.).abs() > 0.001 || !angle.is_finite()
            })
        {
            return Err("Invalid model transform".into());
        }
        Ok(())
    }
    pub fn point(self, point: Vec3) -> Vec3 {
        self.pivot + self.vector(point - self.pivot) + self.translation
    }
    fn scaled(self, vector: Vec3) -> Vec3 {
        let mut p = Vec3(
            vector.0 * self.scale.0,
            vector.1 * self.scale.1,
            vector.2 * self.scale.2,
        );
        if let Some((axis, factor)) = self.stretch {
            p = p + axis * (p.dot(axis) * (factor - 1.));
        }
        p
    }
    /// Transform a direction without subtracting translated points.
    pub fn vector(self, vector: Vec3) -> Vec3 {
        let p = self.scaled(vector);
        self.rotation
            .map_or(p, |(axis, angle)| p.rotated(axis, angle))
    }
    /// Soft masks scale the angle rather than blending along a chord. Pure
    /// rotations therefore preserve radius and do not depend on event frequency.
    pub fn weighted_point(self, point: Vec3, weight: f32) -> Vec3 {
        if weight == 0. {
            return point;
        }
        let p = point - self.pivot;
        let p = p + (self.scaled(p) - p) * weight;
        let p = self
            .rotation
            .map_or(p, |(axis, angle)| p.rotated(axis, angle * weight));
        self.pivot + p + self.translation * weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn action_line_stretch_uses_pivot_and_keeps_perpendicular_coordinates() {
        let axis = Vec3(1., 1., 0.).norm();
        let transform = Transform {
            pivot: Vec3(2., 3., 4.),
            stretch: Some((axis, 0.5)),
            ..Default::default()
        };
        transform.validate().unwrap();
        let perpendicular = Vec3(-1., 1., 0.);
        let p = transform.pivot + axis * 2. + perpendicular;
        assert!((transform.point(p) - (transform.pivot + axis + perpendicular)).len() < 1e-5);
        assert_eq!(transform.point(transform.pivot), transform.pivot);
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
    #[test]
    fn rotation_uses_pivot_and_right_handed_axes_and_validates_input() {
        let pivot = Vec3(2., 3., 4.);
        for (axis, input, expected) in [
            (Vec3(1., 0., 0.), Vec3(0., 1., 0.), Vec3(0., 0., 1.)),
            (Vec3(0., 1., 0.), Vec3(0., 0., 1.), Vec3(1., 0., 0.)),
            (Vec3(0., 0., 1.), Vec3(1., 0., 0.), Vec3(0., 1., 0.)),
        ] {
            let transform = Transform {
                pivot,
                rotation: Some((axis, FRAC_PI_2)),
                ..Default::default()
            };
            transform.validate().unwrap();
            assert!(!transform.is_identity());
            assert!((transform.point(pivot + input) - pivot - expected).len() < 1e-6);
            assert_eq!(transform.point(pivot), pivot);
            for bad in [Vec3::default(), Vec3(2., 0., 0.), Vec3(f32::NAN, 0., 0.)] {
                assert!(
                    Transform {
                        rotation: Some((bad, 1.)),
                        ..transform
                    }
                    .validate()
                    .is_err()
                );
            }
            assert!(
                Transform {
                    rotation: Some((axis, f32::INFINITY)),
                    ..transform
                }
                .validate()
                .is_err()
            );
        }
    }
    #[test]
    fn masked_rotation_preserves_radius_and_is_independent_of_drag_sampling() {
        let transform = Transform {
            rotation: Some((Vec3(0., 0., 1.), FRAC_PI_2)),
            ..Default::default()
        };
        let p = Vec3(2., 0., 3.);
        assert_eq!(transform.weighted_point(p, 0.), p);
        let half = transform.weighted_point(p, 0.5);
        assert!((half - Vec3(2. * FRAC_PI_4.cos(), 2. * FRAC_PI_4.sin(), 3.)).len() < 1e-6);
        let step = Transform {
            rotation: Some((Vec3(0., 0., 1.), FRAC_PI_2 / 90.)),
            ..Default::default()
        };
        let mut sampled = p;
        for _ in 0..90 {
            sampled = step.weighted_point(sampled, 0.5);
        }
        assert!((sampled - half).len() < 1e-5);
        let reverse = Transform {
            rotation: Some((Vec3(0., 0., 1.), -FRAC_PI_2)),
            ..Default::default()
        };
        assert!((reverse.weighted_point(half, 0.5) - p).len() < 1e-6);
    }
}

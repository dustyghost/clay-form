//! Affine model transforms, shared by world handles and Action Line.
use super::Vec3;
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub pivot: Vec3,
    pub translation: Vec3,
    pub scale: Vec3,
    /// Optional directional stretch (unit direction, positive factor).
    pub stretch: Option<(Vec3, f32)>,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            pivot: Vec3::default(),
            translation: Vec3::default(),
            scale: Vec3(1., 1., 1.),
            stretch: None,
        }
    }
}
impl Transform {
    pub fn is_identity(&self) -> bool {
        self.translation == Vec3::default()
            && self.scale == Vec3(1., 1., 1.)
            && self.stretch.is_none_or(|(_, factor)| factor == 1.)
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
        {
            return Err("Invalid model transform".into());
        }
        Ok(())
    }
    pub fn point(self, point: Vec3) -> Vec3 {
        let p = point - self.pivot;
        let mut p = Vec3(p.0 * self.scale.0, p.1 * self.scale.1, p.2 * self.scale.2);
        if let Some((axis, factor)) = self.stretch {
            p = p + axis * (p.dot(axis) * (factor - 1.));
        }
        self.pivot + p + self.translation
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

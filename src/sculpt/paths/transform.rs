use super::*;

impl SurfacePath {
    /// Release surface attachment without changing the displayed route.
    pub fn detached(&self) -> Self {
        let route = if self.offset != 0. {
            Some(self.dense_curve())
        } else {
            self.attached_points().map(<[PathPoint]>::to_vec)
        };
        let mut result = self.clone();
        for point in &mut result.points {
            point.position = self.display_position(*point);
        }
        result.mode = PathMode::Free;
        result.surface_signature = 0;
        result.offset = 0.;
        result.ring_orientation = None;
        result.attached_route = route.map(|points| AttachedRoute {
            points,
            shape_signature: result.shape_signature(),
        });
        result
    }

    /// A positive uniform transform preserves curve shape, corner proportions,
    /// normals and any retained world-space route. Draft identities may be zero.
    pub fn transformed(
        &self,
        pivot: Vec3,
        translation: Vec3,
        axis: Vec3,
        radians: f32,
        scale: f32,
    ) -> Result<Self, String> {
        if self.mode != PathMode::Free {
            return Err("Detach the surface path before transforming it freely".into());
        }
        if !finite(pivot)
            || !finite(translation)
            || !finite(axis)
            || axis.len() < 1e-6
            || !radians.is_finite()
            || radians.abs() > std::f32::consts::TAU * 100.
            || !scale.is_finite()
            || !(0.001..=1000.).contains(&scale)
        {
            return Err(
                "Choose a finite path transform and positive scale between 0.001 and 1000".into(),
            );
        }
        if translation == Vec3::default() && radians == 0. && scale == 1. {
            return Ok(self.clone());
        }
        let axis = axis.norm();
        let transform = |p: &mut PathPoint| {
            p.position = pivot + (p.position - pivot).rotated(axis, radians) * scale + translation;
            p.normal = p.normal.rotated(axis, radians);
        };
        let mut result = self.clone();
        let route = self.attached_points().map(<[PathPoint]>::to_vec);
        for p in &mut result.points {
            transform(p);
        }
        for radius in &mut result.corner_rounding {
            *radius *= scale;
        }
        result.attached_route = route.map(|mut points| {
            for p in &mut points {
                transform(p);
            }
            AttachedRoute {
                points,
                shape_signature: result.shape_signature(),
            }
        });
        let (id, target) = (result.id, result.target);
        result.id = id.max(1);
        result.target = target.max(1);
        result.validate()?;
        result.id = id;
        result.target = target;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> SurfacePath {
        SurfacePath {
            id: 1,
            target: 1,
            name: "Route".into(),
            mode: PathMode::Free,
            points: [Vec3(-1., 0., 0.), Vec3(0., 1., 0.), Vec3(1., 0., 0.)]
                .map(|position| PathPoint {
                    position,
                    normal: Vec3(0., 0., 1.),
                })
                .to_vec(),
            smoothing: 0.,
            corner_rounding: vec![0., 0.2, 0.],
            closed: false,
            seam: 0,
            surface_signature: 0,
            offset: 0.,
            ring_orientation: None,
            attached_route: None,
        }
    }
    #[test]
    fn whole_path_transform_preserves_shape_rounding_and_normals() {
        let path = path();
        let before = path.clone();
        let delta = Vec3(0.3, -0.2, 0.8);
        let expected = |p: Vec3| Vec3(p.2, p.1, -p.0) * 2. + delta;
        let result = path
            .transformed(
                Vec3::default(),
                delta,
                Vec3(0., 1., 0.),
                std::f32::consts::FRAC_PI_2,
                2.,
            )
            .unwrap();
        assert_eq!(result.corner_rounding, vec![0., 0.4, 0.]);
        for (a, b) in path.dense_curve().iter().zip(result.dense_curve()) {
            assert!((expected(a.position) - b.position).len() < 1e-5);
            assert!((b.normal - Vec3(1., 0., 0.)).len() < 1e-5);
        }
        assert_eq!(path, before);
        for scale in [0., -1., f32::NAN, f32::INFINITY, 1001.] {
            assert!(
                path.transformed(
                    Vec3::default(),
                    Vec3::default(),
                    Vec3(0., 0., 1.),
                    0.,
                    scale
                )
                .is_err()
            );
        }
        let mut draft = path;
        draft.id = 0;
        draft.target = 0;
        let result = draft
            .transformed(Vec3::default(), delta, Vec3(0., 0., 1.), 0., 1.)
            .unwrap();
        assert_eq!((result.id, result.target), (0, 0));
    }
    #[test]
    fn whole_path_detachment_keeps_the_attached_route_and_closed_seam() {
        let mut path = path();
        path.mode = PathMode::Surface;
        path.closed = true;
        path.seam = 1;
        path.corner_rounding.clear();
        let route = vec![
            path.points[1],
            path.points[2],
            PathPoint {
                position: Vec3(0., -0.3, 0.),
                normal: Vec3(0., 0., 1.),
            },
            path.points[0],
            path.points[1],
        ];
        path.attached_route = Some(AttachedRoute {
            points: route.clone(),
            shape_signature: path.shape_signature(),
        });
        assert!(
            path.transformed(Vec3::default(), Vec3::default(), Vec3(0., 0., 1.), 0., 2.)
                .is_err()
        );
        let detached = path.detached();
        assert_eq!(detached.mode, PathMode::Free);
        assert_eq!(detached.dense_curve(), route);
        let transformed = detached
            .transformed(Vec3::default(), Vec3(0., 0., 1.), Vec3(0., 0., 1.), 0., 2.)
            .unwrap();
        assert_eq!(transformed.seam, 1);
        let after = transformed.dense_curve();
        assert_eq!(after.first(), after.last());
        for (a, b) in route.iter().zip(after) {
            assert!((a.position * 2. + Vec3(0., 0., 1.) - b.position).len() < 1e-6);
        }
        transformed.validate().unwrap();
    }
}

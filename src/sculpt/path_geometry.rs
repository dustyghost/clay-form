//! Shared geometry for local corner fillets in surface paths and slice guides.
//! Amounts are distances trimmed along each incident edge, in the caller's units.
use super::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerSample {
    pub position: Vec3,
    /// Convex weights of [previous, corner, next], also suitable for normals.
    pub weights: [f32; 3],
}

fn finite(point: Vec3) -> bool {
    point.0.is_finite() && point.1.is_finite() && point.2.is_finite()
}

/// Neighboring rounded corners each consume at most 45% of their shared edge.
pub fn corner_limit(previous: Vec3, corner: Vec3, next: Vec3) -> f32 {
    if !finite(previous) || !finite(corner) || !finite(next) {
        return 0.;
    }
    let incoming = previous - corner;
    let outgoing = next - corner;
    let in_length = incoming.len();
    let out_length = outgoing.len();
    if !in_length.is_finite() || !out_length.is_finite() || in_length == 0. || out_length == 0. {
        return 0.;
    }
    if (incoming * (1. / in_length))
        .cross(outgoing * (1. / out_length))
        .len()
        <= 1e-6
    {
        return 0.;
    }
    in_length.min(out_length) * 0.45
}

/// Quadratic fillet through the trim endpoints with the sharp corner as its
/// control point. Returns both endpoints, with 2–32 segments. Invalid, zero,
/// straight, and doubling-back corners have no fillet; callers retain the knot.
pub fn rounded_corner(
    previous: Vec3,
    corner: Vec3,
    next: Vec3,
    amount: f32,
    steps: usize,
) -> Option<Vec<CornerSample>> {
    if !amount.is_finite() || amount <= 0. {
        return None;
    }
    let distance = amount.min(corner_limit(previous, corner, next));
    if distance <= 0. {
        return None;
    }
    let incoming = previous - corner;
    let outgoing = next - corner;
    let in_length = incoming.len();
    let out_length = outgoing.len();
    let in_direction = incoming * (1. / in_length);
    let out_direction = outgoing * (1. / out_length);
    let a = corner + in_direction * distance;
    let b = corner + out_direction * distance;
    let in_fraction = distance / in_length;
    let out_fraction = distance / out_length;
    let steps = steps.clamp(2, 32);
    Some(
        (0..=steps)
            .map(|index| {
                let t = index as f32 / steps as f32;
                let before = (1. - t).powi(2);
                let after = t * t;
                let w_previous = before * in_fraction;
                let w_next = after * out_fraction;
                CornerSample {
                    position: a * before + corner * (2. * t * (1. - t)) + b * after,
                    weights: [w_previous, 1. - w_previous - w_next, w_next],
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fillet_endpoints_midpoint_and_weights_match_the_quadratic() {
        let previous = Vec3(-2., 0., 0.);
        let corner = Vec3::default();
        let next = Vec3(0., 4., 0.);
        let samples = rounded_corner(previous, corner, next, 0.5, 4).unwrap();
        assert_eq!(samples.first().unwrap().position, Vec3(-0.5, 0., 0.));
        assert_eq!(samples.last().unwrap().position, Vec3(0., 0.5, 0.));
        assert_eq!(samples[2].position, Vec3(-0.125, 0.125, 0.));
        for sample in samples {
            assert!(
                sample
                    .weights
                    .iter()
                    .all(|&weight| (0. ..=1.).contains(&weight))
            );
            assert!((sample.weights.iter().sum::<f32>() - 1.).abs() < 1e-6);
            let reconstructed = previous * sample.weights[0]
                + corner * sample.weights[1]
                + next * sample.weights[2];
            assert!((sample.position - reconstructed).len() < 1e-6);
        }
    }

    #[test]
    fn units_and_3d_orientation_do_not_change_the_corner_shape() {
        let points = [Vec3(-2., 1., 0.5), Vec3(0.2, 0.3, -0.5), Vec3(1., 2., 2.)];
        let original = rounded_corner(points[0], points[1], points[2], 0.4, 7).unwrap();
        let rotation = Vec3(1., 2., -1.).norm();
        for scale in [0.001, 3., 1000.] {
            let transform = |p: Vec3| p.rotated(rotation, 0.7) * scale;
            let moved = points.map(transform);
            let actual = rounded_corner(moved[0], moved[1], moved[2], 0.4 * scale, 7).unwrap();
            for (before, after) in original.iter().zip(actual) {
                assert!((transform(before.position) - after.position).len() < scale * 1e-5);
                for (a, b) in before.weights.iter().zip(after.weights) {
                    assert!((a - b).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn trimming_and_work_are_bounded_and_bad_corners_remain_sharp() {
        let a = Vec3(-1., 0., 0.);
        let b = Vec3::default();
        let c = Vec3(0., 3., 0.);
        assert_eq!(corner_limit(a, b, c), 0.45);
        let bounded = rounded_corner(a, b, c, 100., usize::MAX).unwrap();
        assert_eq!(bounded.len(), 33);
        assert_eq!(bounded[0].position, Vec3(-0.45, 0., 0.));
        assert_eq!(rounded_corner(a, b, c, 0.1, 0).unwrap().len(), 3);
        for amount in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(rounded_corner(a, b, c, amount, 4).is_none());
        }
        assert!(rounded_corner(b, b, c, 0.1, 4).is_none());
        assert!(rounded_corner(a, b, a, 0.1, 4).is_none());
        assert!(rounded_corner(a, b, a * -1., 0.1, 4).is_none());
        assert_eq!(corner_limit(a, b, a), 0.);
        assert_eq!(corner_limit(a, b, a * -1.), 0.);
        assert_eq!(corner_limit(b, b, c), 0.);
        assert_eq!(corner_limit(a, b, Vec3(f32::NAN, 0., 0.)), 0.);
    }
}

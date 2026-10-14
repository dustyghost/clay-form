//! Join closely overlapping soft Standard footprints without summing their strength.
use super::{Brush, Dab, Vec3};

pub(super) struct SoftOverlap {
    origin: Vec3,
    center_sum: Vec3,
    squared_sum: f32,
    influence_sum: f32,
    strongest: f32,
    radius: f32,
    lower: [f32; 3],
    upper: [f32; 3],
}
impl SoftOverlap {
    pub fn new(origin: Vec3) -> Self {
        Self {
            origin,
            center_sum: Vec3::default(),
            squared_sum: 0.,
            influence_sum: 0.,
            strongest: 0.,
            radius: f32::MAX,
            lower: [f32::INFINITY; 3],
            upper: [f32::NEG_INFINITY; 3],
        }
    }
    pub fn add(&mut self, dab: &Dab, influence: f32) {
        let offset = dab.center - self.origin;
        self.center_sum = self.center_sum + offset * influence;
        self.squared_sum += offset.dot(offset) * influence;
        self.influence_sum += influence;
        self.strongest = self.strongest.max(influence);
        self.radius = self.radius.min(dab.radius);
        for (axis, value) in [offset.0, offset.1, offset.2].into_iter().enumerate() {
            self.lower[axis] = self.lower[axis].min(value);
            self.upper[axis] = self.upper[axis].max(value);
        }
    }
    pub fn depth(
        &self,
        brush: &Brush,
        dabs: &[Dab],
        vertex: Vec3,
        normal: Vec3,
        mobility: f32,
        depth: f32,
    ) -> f32 {
        if self.influence_sum <= 0. || mobility <= 0. {
            return depth;
        }
        let mean = self.center_sum * (1. / self.influence_sum);
        let variance = (self.squared_sum / self.influence_sum - mean.dot(mean)).max(0.);
        // Coincident copies already have the original profile. Distinct footprints
        // only merge when their centres are close relative to the brush radius.
        if variance <= self.radius * self.radius * 1e-12 {
            return depth;
        }
        let blend = near(variance.sqrt() / self.radius, 0.25, 0.5);
        if blend == 0. {
            return depth;
        }
        let center = self.origin + mean;
        let mut joined = depth;
        for dab in dabs {
            let offset = vertex - dab.center;
            if offset.len() > dab.radius * 1.5 || normal.dot(dab.normal) < 0.1 {
                continue;
            }
            let weight = brush.weight(
                offset.dot(dab.right) / dab.radius,
                offset.dot(dab.up) / dab.radius,
            );
            if weight <= 0. {
                continue;
            }
            let strength = brush.strength
                * if brush.pressure {
                    dab.pressure.powf(brush.pressure_curve)
                } else {
                    1.
                };
            let leg = dab.center - center;
            let length_squared = leg.dot(leg);
            if length_squared <= dab.radius * dab.radius * 1e-12 {
                continue;
            }
            let local_blend = near(length_squared.sqrt() / dab.radius, 0.5, 1.);
            if local_blend == 0. {
                continue;
            }
            // Join along lines through the weighted centre, bounded by the
            // contributing centres. Each alpha frame samples both sides of the
            // centre so an oblique projection cannot leave a gap between them.
            let right = leg.dot(dab.right);
            let up = leg.dot(dab.up);
            let projected_length_squared = right * right + up * up;
            if projected_length_squared <= dab.radius * dab.radius * 1e-12 {
                continue;
            }
            let mut low = f32::NEG_INFINITY;
            let mut high = f32::INFINITY;
            for (axis, (origin, direction)) in [(mean.0, leg.0), (mean.1, leg.1), (mean.2, leg.2)]
                .into_iter()
                .enumerate()
            {
                if direction.abs() > dab.radius * 1e-6 {
                    let a = (self.lower[axis] - origin) / direction;
                    let b = (self.upper[axis] - origin) / direction;
                    low = low.max(a.min(b));
                    high = high.min(a.max(b));
                }
            }
            if low > high {
                continue;
            }
            let offset = vertex - center;
            let t = ((offset.dot(dab.right) * right + offset.dot(dab.up) * up)
                / projected_length_squared)
                .clamp(low, high);
            let offset = vertex - (center + leg * t);
            let coverage = brush.weight(
                offset.dot(dab.right) / dab.radius,
                offset.dot(dab.up) / dab.radius,
            );
            let candidate = coverage * strength * dab.radius * 0.15 * mobility * dab.normal.len();
            // A barely contributing copy must not suddenly extend the footprint.
            let participation = (weight * strength / self.strongest).clamp(0., 1.);
            joined = joined
                .max(depth + (candidate - depth).max(0.) * blend * local_blend * participation);
        }
        joined
    }
}
fn near(distance: f32, full: f32, end: f32) -> f32 {
    let t = ((end - distance) / (end - full)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Alpha, Kind, Mesh, Primitive, PrimitiveKind, Symmetry};
    #[test]
    fn soft_join_has_a_level_centre_and_preserves_masks_and_texture_alphas() {
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [100, 4, 1];
        let original = plane.mesh().unwrap();
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        let mut brush = Brush::new(Kind::Standard);
        brush.pressure = false;
        let dab = Dab {
            center: Vec3(0.04, 0., 0.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.4,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        let mut mesh = original.clone();
        mesh.mask.fill(0.5);
        for _ in 0..6 {
            mesh.stamp_many(&brush, &symmetry.dabs(dab));
        }
        let expected = dab.radius * 0.15 * brush.strength * 0.5 * 6.;
        let mut checked = 0;
        for vertex in &mesh.vertices {
            if vertex.0.abs() <= 0.04 && vertex.1.abs() < 1e-6 {
                checked += 1;
                assert!(
                    (vertex.2 - expected).abs() < 1e-5,
                    "join should stay level across repeated strokes: {vertex:?}, expected {expected}"
                );
            }
        }
        assert!(checked >= 3);
        assert!(mesh.mask.iter().all(|m| *m == 0.5));
        for alpha in [
            Alpha::Hard,
            Alpha::Square,
            Alpha::Image {
                width: 2,
                height: 2,
                pixels: vec![0.2, 0.8, 0.8, 0.2].into(),
            },
        ] {
            brush.alpha = alpha;
            let mut mirrored = original.clone();
            mirrored.stamp_many(&brush, &symmetry.dabs(dab));
            let mut a = original.clone();
            a.stamp(&brush, dab);
            let mut b = original.clone();
            b.stamp(&brush, symmetry.dabs(dab)[1]);
            for i in 0..original.vertices.len() {
                let expected = a.vertices[i].2.max(b.vertices[i].2);
                assert!(
                    (mirrored.vertices[i].2 - expected).abs() < 1e-6,
                    "non-soft alpha must keep its original profile"
                );
            }
        }
    }
    #[test]
    fn curved_seam_join_uses_the_brush_projection_at_oblique_view_angles() {
        let mut mesh = Mesh::sphere();
        let seam = mesh
            .vertices
            .iter()
            .position(|v| v.0.abs() < 1e-6 && v.1.abs() < 1e-6 && v.2 > 0.99)
            .unwrap();
        let before = mesh.vertices[seam];
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        let mut brush = Brush::new(Kind::Standard);
        brush.pressure = false;
        let center = Vec3(0.04, 0., (1_f32 - 0.04 * 0.04).sqrt());
        let dab = Dab {
            center,
            normal: center.norm(),
            right: Vec3(0.7, 0., -0.7).norm(),
            up: Vec3(0., 1., 0.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        mesh.stamp_many(&brush, &symmetry.dabs(dab));
        let change = mesh.vertices[seam] - before;
        assert!(change.0.abs() < 1e-6);
        assert!(
            (change.2 - dab.radius * 0.15 * brush.strength).abs() < 1e-6,
            "an angled view must not reintroduce the centre falloff dip"
        );
    }
    #[test]
    fn separated_footprints_and_coincident_copies_keep_the_original_profile() {
        let original = Mesh::sphere();
        let brush = Brush::new(Kind::Standard);
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        for x in [0., 0.7] {
            let dab = Dab {
                center: Vec3(x, 0., (1. - x * x).sqrt()),
                normal: Vec3(0., 0., 1.),
                right: Vec3(1., 0., 0.),
                up: Vec3(0., 1., 0.),
                radius: 0.2,
                pressure: 1.,
                delta: Vec3::default(),
                invert: false,
            };
            let copies = symmetry.dabs(dab);
            let mut mirrored = original.clone();
            mirrored.stamp_many(&brush, &copies);
            let mut a = original.clone();
            a.stamp(&brush, copies[0]);
            let mut b = original.clone();
            b.stamp(&brush, copies[1]);
            for i in 0..original.vertices.len() {
                assert!(
                    (mirrored.vertices[i].2 - a.vertices[i].2.max(b.vertices[i].2)).abs() < 1e-6
                );
            }
        }
    }
}

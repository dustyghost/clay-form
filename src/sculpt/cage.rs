//! A fitted trilinear lattice and two continuous axial deformations.
use super::{Axis, Mesh, Vec3};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Cage {
    pub low: Vec3,
    pub high: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Adjustment {
    Pull(usize, Vec3),
    Bend(Axis, f32),
    Twist(Axis, f32),
}

impl Cage {
    pub fn fit(mesh: &Mesh) -> Option<Self> {
        let &first = mesh.vertices.first()?;
        let (mut low, mut high) = (first, first);
        for &p in &mesh.vertices {
            low = Vec3(low.0.min(p.0), low.1.min(p.1), low.2.min(p.2));
            high = Vec3(high.0.max(p.0), high.1.max(p.1), high.2.max(p.2));
        }
        // Leave a usable frame around planes and lines as well as solids.
        let span = high - low;
        let pad = span.0.max(span.1).max(span.2).max(1e-4) * 0.035;
        let margin = Vec3(pad, pad, pad);
        Some(Self {
            low: low - margin,
            high: high + margin,
        })
    }

    pub fn corner(self, index: usize) -> Vec3 {
        let coord = |axis, lo, hi| {
            if index & (1usize << axis) == 0usize {
                lo
            } else {
                hi
            }
        };
        Vec3(
            coord(0, self.low.0, self.high.0),
            coord(1, self.low.1, self.high.1),
            coord(2, self.low.2, self.high.2),
        )
    }

    pub fn point(self, p: Vec3, edit: Adjustment) -> Vec3 {
        let span = self.high - self.low;
        let local = p - self.low;
        match edit {
            Adjustment::Pull(i, delta) => {
                let t = [local.0 / span.0, local.1 / span.1, local.2 / span.2];
                let weight = t
                    .into_iter()
                    .enumerate()
                    .map(|(axis, t)| {
                        let t = t.clamp(0., 1.);
                        if i & (1 << axis) == 0 { 1. - t } else { t }
                    })
                    .product::<f32>();
                p + delta * weight
            }
            Adjustment::Bend(_, 0.) | Adjustment::Twist(_, 0.) => p,
            Adjustment::Bend(axis, angle) | Adjustment::Twist(axis, angle) => {
                let along = axis.vector();
                let length = span.dot(along);
                let h = local.dot(along);
                let base = (self.low + self.high) * 0.5 - along * (length * 0.5);
                let radial = p - (base + along * h);
                let a = angle * (h / length);
                if matches!(edit, Adjustment::Twist(..)) {
                    p + (radial.rotated(along, a) - radial)
                } else {
                    let toward = match axis {
                        Axis::X => Axis::Y,
                        Axis::Y => Axis::X,
                        Axis::Z => Axis::X,
                    }
                    .vector();
                    let radius = length / angle;
                    // Half-angle form stays accurate for small bends.
                    let offset = toward * (2. * radius * (0.5 * a).sin().powi(2))
                        + along * (radius * a.sin() - h);
                    p + offset + (radial.rotated(along.cross(toward), a) - radial)
                }
            }
        }
    }

    pub fn vertices(
        self,
        mesh: &Mesh,
        edit: Adjustment,
        respect_mask: bool,
    ) -> Result<Vec<Vec3>, String> {
        mesh.vertices
            .iter()
            .zip(&mesh.mask)
            .map(|(&p, &mask)| {
                let mobility = if respect_mask { 1. - mask } else { 1. };
                let next = p + (self.point(p, edit) - p) * mobility;
                if [next.0, next.1, next.2].into_iter().all(f32::is_finite) {
                    Ok(next)
                } else {
                    Err("Frame adjustment exceeds the coordinate range".into())
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cage_axial_deformations_anchor_the_base_and_progress_with_height() {
        let cage = Cage {
            low: Vec3(-1., 0., -1.),
            high: Vec3(1., 2., 1.),
        };
        let angle = std::f32::consts::FRAC_PI_2;
        for height in [0., 1., 2.] {
            let p = Vec3(1., height, 0.);
            let turned = cage.point(p, Adjustment::Twist(Axis::Y, angle));
            let expected = Vec3(
                (angle * height / 2.).cos(),
                height,
                -(angle * height / 2.).sin(),
            );
            assert!((turned - expected).len() < 1e-6);
        }
        let edit = Adjustment::Bend(Axis::Y, angle);
        assert_eq!(cage.point(Vec3(0.5, 0., 0.), edit), Vec3(0.5, 0., 0.));
        let radius = 2. / angle;
        assert!((cage.point(Vec3(0., 2., 0.), edit) - Vec3(radius, radius, 0.)).len() < 1e-5);
        let pull = Adjustment::Pull(7, Vec3(0.5, 0., 0.));
        for i in 0..7 {
            assert_eq!(cage.point(cage.corner(i), pull), cage.corner(i));
        }
        assert_eq!(
            cage.point(cage.corner(7), pull),
            cage.corner(7) + Vec3(0.5, 0., 0.)
        );
    }
}

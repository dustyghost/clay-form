use super::{Brush, Kind, Vec3};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy)]
pub struct Dab {
    pub center: Vec3,
    pub normal: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub radius: f32,
    pub pressure: f32,
    pub delta: Vec3,
    pub invert: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[usize; 3]>,
    pub mask: Vec<f32>,
}
impl Mesh {
    pub fn sphere() -> Self {
        let n = 96;
        let mut v = vec![Vec3(0., 1., 0.)];
        for j in 1..n {
            let t = std::f32::consts::PI * j as f32 / n as f32;
            for i in 0..n * 2 {
                let p = std::f32::consts::TAU * i as f32 / (n * 2) as f32;
                v.push(Vec3(t.sin() * p.cos(), t.cos(), t.sin() * p.sin()));
            }
        }
        let bottom = v.len();
        v.push(Vec3(0., -1., 0.));
        let w = n * 2;
        let mut f = Vec::new();
        for i in 0..w {
            f.push([0, 1 + (i + 1) % w, 1 + i]);
            for j in 0..n - 2 {
                let a = 1 + j * w + i;
                let b = 1 + j * w + (i + 1) % w;
                f.push([a, b, a + w]);
                f.push([b, b + w, a + w]);
            }
            f.push([bottom, 1 + (n - 2) * w + i, 1 + (n - 2) * w + (i + 1) % w]);
        }
        let mask = vec![0.; v.len()];
        Self {
            vertices: v,
            triangles: f,
            mask,
        }
    }
    pub fn normals(&self) -> Vec<Vec3> {
        let mut n = vec![Vec3::default(); self.vertices.len()];
        for &[a, b, c] in &self.triangles {
            let normal =
                (self.vertices[b] - self.vertices[a]).cross(self.vertices[c] - self.vertices[a]);
            for i in [a, b, c] {
                n[i] = n[i] + normal;
            }
        }
        n.into_iter().map(Vec3::norm).collect()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.vertices.len() > 500_000
            || self.vertices.is_empty()
            || self.mask.len() != self.vertices.len()
            || self
                .vertices
                .iter()
                .any(|v| !v.0.is_finite() || !v.1.is_finite() || !v.2.is_finite())
            || self
                .mask
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || self
                .triangles
                .iter()
                .flatten()
                .any(|&i| i >= self.vertices.len())
        {
            return Err("Invalid mesh data".into());
        }
        Ok(())
    }
    pub fn stamp(&mut self, b: &Brush, dab: Dab) {
        self.stamp_many(b, &[dab]);
    }
    /// Evaluate all symmetry copies against the same geometry, taking the strongest
    /// influence per vertex so overlapping copies cannot multiply brush strength.
    pub fn stamp_many(&mut self, b: &Brush, dabs: &[Dab]) {
        let old = self.vertices.clone();
        let normals = self.normals();
        let mut avg = vec![Vec3::default(); old.len()];
        let mut count = vec![0f32; old.len()];
        if b.kind == Kind::Smooth {
            for &[a, c, d] in &self.triangles {
                for (i, j, k) in [(a, c, d), (c, a, d), (d, a, c)] {
                    avg[i] = avg[i] + old[j] + old[k];
                    count[i] += 2.;
                }
            }
        }
        for i in 0..old.len() {
            let mut strongest = 0.;
            let mut selected = None;
            for dab in dabs {
                let offset = old[i] - dab.center;
                if offset.len() > dab.radius * 1.5 || normals[i].dot(dab.normal) < 0.1 {
                    continue;
                }
                let weight = b.weight(
                    offset.dot(dab.right) / dab.radius,
                    offset.dot(dab.up) / dab.radius,
                );
                let strength = b.strength
                    * if b.pressure {
                        dab.pressure.powf(b.pressure_curve)
                    } else {
                        1.
                    };
                let influence = weight * strength;
                if influence > strongest {
                    strongest = influence;
                    selected = Some((*dab, offset));
                }
            }
            let Some((
                Dab {
                    center,
                    normal,
                    radius,
                    delta,
                    invert,
                    ..
                },
                offset,
            )) = selected
            else {
                continue;
            };
            let sign = if b.subtract ^ invert { -1. } else { 1. };
            let w = strongest;
            if b.kind == Kind::Mask {
                self.mask[i] = (self.mask[i] + w * sign).clamp(0., 1.);
                continue;
            }
            let w = w * (1. - self.mask[i]);
            let displacement = match b.kind {
                Kind::Standard => normal * (radius * 0.15 * w * sign),
                Kind::Clay => normal * ((radius * 0.1 - offset.dot(normal)).max(0.) * w * sign),
                Kind::Inflate => normals[i] * (radius * 0.15 * w * sign),
                Kind::Smooth => (avg[i] * (1. / count[i].max(1.)) - old[i]) * (w * sign),
                Kind::Flatten => normal * (-offset.dot(normal) * w * sign),
                Kind::Pinch => {
                    (center - old[i] - normal * (-offset.dot(normal))) * (w * 0.2 * sign)
                }
                Kind::Crease => {
                    (center - old[i]) * (w * 0.12 * sign) - normal * (radius * w * 0.08 * sign)
                }
                Kind::Grab => delta * (w * sign),
                Kind::Mask => Vec3::default(),
            };
            self.vertices[i] = old[i] + displacement;
        }
    }
}

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};
#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct V(pub f32, pub f32, pub f32);
impl Add for V {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self(self.0 + b.0, self.1 + b.1, self.2 + b.2)
    }
}
impl Sub for V {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self(self.0 - b.0, self.1 - b.1, self.2 - b.2)
    }
}
impl Mul<f32> for V {
    type Output = Self;
    fn mul(self, b: f32) -> Self {
        Self(self.0 * b, self.1 * b, self.2 * b)
    }
}
impl V {
    pub fn dot(self, b: Self) -> f32 {
        self.0 * b.0 + self.1 * b.1 + self.2 * b.2
    }
    pub fn cross(self, b: Self) -> Self {
        Self(
            self.1 * b.2 - self.2 * b.1,
            self.2 * b.0 - self.0 * b.2,
            self.0 * b.1 - self.1 * b.0,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> Self {
        self * (1. / self.len().max(1e-8))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Standard,
    Clay,
    Inflate,
    Smooth,
    Flatten,
    Pinch,
    Crease,
    Grab,
    Mask,
}
impl Kind {
    pub const ALL: [Self; 9] = [
        Self::Standard,
        Self::Clay,
        Self::Inflate,
        Self::Smooth,
        Self::Flatten,
        Self::Pinch,
        Self::Crease,
        Self::Grab,
        Self::Mask,
    ];
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Alpha {
    Soft,
    Hard,
    Square,
    Image {
        width: usize,
        height: usize,
        pixels: Vec<f32>,
    },
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Brush {
    pub name: String,
    pub kind: Kind,
    pub alpha: Alpha,
    pub radius: f32,
    pub strength: f32,
    pub falloff: f32,
    pub spacing: f32,
    pub rotation: f32,
    pub subtract: bool,
    pub pressure: bool,
    pub pressure_radius: bool,
    pub pressure_curve: f32,
}
impl Brush {
    pub fn new(kind: Kind) -> Self {
        Self {
            name: format!("{kind:?}"),
            kind,
            alpha: Alpha::Soft,
            radius: 60.,
            strength: 0.25,
            falloff: 1.5,
            spacing: 0.15,
            rotation: 0.,
            subtract: false,
            pressure: true,
            pressure_radius: false,
            pressure_curve: 1.,
        }
    }
    pub fn weight(&self, x: f32, y: f32) -> f32 {
        let (s, c) = self.rotation.to_radians().sin_cos();
        let (x, y) = (x * c - y * s, x * s + y * c);
        let r = (x * x + y * y).sqrt();
        if r >= 1. {
            return 0.;
        }
        match &self.alpha {
            Alpha::Soft => (1. - r).powf(self.falloff),
            Alpha::Hard => 1.,
            Alpha::Square => {
                if x.abs().max(y.abs()) < 0.7 {
                    1.
                } else {
                    0.
                }
            }
            Alpha::Image {
                width,
                height,
                pixels,
            } => {
                let ix = (((x + 1.) * 0.5 * (*width - 1) as f32) as usize).min(*width - 1);
                let iy = (((1. - y) * 0.5 * (*height - 1) as f32) as usize).min(*height - 1);
                pixels[iy * width + ix] * (1. - r).powf(self.falloff)
            }
        }
    }
}
#[derive(Clone, Copy)]
pub struct Dab {
    pub center: V,
    pub normal: V,
    pub right: V,
    pub up: V,
    pub radius: f32,
    pub pressure: f32,
    pub delta: V,
    pub invert: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<V>,
    pub triangles: Vec<[usize; 3]>,
    pub mask: Vec<f32>,
}
impl Mesh {
    pub fn sphere() -> Self {
        let n = 96;
        let mut v = vec![V(0., 1., 0.)];
        for j in 1..n {
            let t = std::f32::consts::PI * j as f32 / n as f32;
            for i in 0..n * 2 {
                let p = std::f32::consts::TAU * i as f32 / (n * 2) as f32;
                v.push(V(t.sin() * p.cos(), t.cos(), t.sin() * p.sin()));
            }
        }
        let bottom = v.len();
        v.push(V(0., -1., 0.));
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
    pub fn normals(&self) -> Vec<V> {
        let mut n = vec![V::default(); self.vertices.len()];
        for &[a, b, c] in &self.triangles {
            let normal =
                (self.vertices[b] - self.vertices[a]).cross(self.vertices[c] - self.vertices[a]);
            for i in [a, b, c] {
                n[i] = n[i] + normal;
            }
        }
        n.into_iter().map(V::norm).collect()
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
        let Dab {
            center,
            normal,
            right,
            up,
            radius,
            pressure,
            delta,
            invert,
        } = dab;
        let old = self.vertices.clone();
        let normals = self.normals();
        let sign = if b.subtract ^ invert { -1. } else { 1. };
        let strength = b.strength
            * if b.pressure {
                pressure.powf(b.pressure_curve)
            } else {
                1.
            };
        let mut avg = vec![V::default(); old.len()];
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
            let offset = old[i] - center;
            if offset.len() > radius * 1.5 || normals[i].dot(normal) < 0.1 {
                continue;
            }
            let weight = b.weight(offset.dot(right) / radius, offset.dot(up) / radius);
            let w = weight * strength;
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
                Kind::Grab => delta * (weight * strength * (1. - self.mask[i]) * sign),
                Kind::Mask => V::default(),
            };
            self.vertices[i] = old[i] + displacement;
        }
    }
}
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub mesh: Mesh,
    pub brushes: Vec<Brush>,
}
pub fn validate_brush(b: &Brush) -> Result<(), String> {
    if b.name.len() > 256
        || !b.radius.is_finite()
        || !(1. ..=500.).contains(&b.radius)
        || !b.strength.is_finite()
        || !(0. ..=1.).contains(&b.strength)
        || !b.falloff.is_finite()
        || !(0.1..=8.).contains(&b.falloff)
        || !b.spacing.is_finite()
        || !(0.01..=1.).contains(&b.spacing)
        || !b.rotation.is_finite()
        || !b.pressure_curve.is_finite()
        || !(0.1..=4.).contains(&b.pressure_curve)
    {
        return Err("Invalid brush settings".into());
    }
    if let Alpha::Image {
        width,
        height,
        pixels,
    } = &b.alpha
        && (*width == 0
            || *height == 0
            || *width > 2048
            || *height > 2048
            || pixels.len() != width * height
            || pixels
                .iter()
                .any(|x| !x.is_finite() || !(0. ..=1.).contains(x)))
    {
        return Err("Invalid alpha".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sphere_is_closed_and_outward() {
        let m = Mesh::sphere();
        m.validate().unwrap();
        let n = m.normals();
        assert!(m.vertices.iter().zip(n).all(|(v, n)| v.dot(n) > 0.99));
        let mut edges = std::collections::HashMap::new();
        for t in m.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        assert!(edges.values().all(|&n| n == 2));
    }
    #[test]
    fn add_subtract_and_mask() {
        let mut m = Mesh::sphere();
        let original = m.vertices[0];
        let b = Brush::new(Kind::Standard);
        m.stamp(
            &b,
            Dab {
                center: original,
                normal: original,
                right: V(1., 0., 0.),
                up: V(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: V::default(),
                invert: false,
            },
        );
        assert!(m.vertices[0].1 > original.1);
        let mut m = Mesh::sphere();
        m.stamp(
            &b,
            Dab {
                center: original,
                normal: original,
                right: V(1., 0., 0.),
                up: V(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: V::default(),
                invert: true,
            },
        );
        assert!(m.vertices[0].1 < original.1);
        m.mask[0] = 1.;
        let before = m.vertices[0];
        m.stamp(
            &b,
            Dab {
                center: before,
                normal: original,
                right: V(1., 0., 0.),
                up: V(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: V::default(),
                invert: false,
            },
        );
        assert_eq!(m.vertices[0].1, before.1);
    }
    #[test]
    fn black_alpha_protects_from_every_brush() {
        for kind in Kind::ALL {
            let mut mesh = Mesh::sphere();
            let old = mesh.vertices.clone();
            let mut brush = Brush::new(kind);
            brush.alpha = Alpha::Image {
                width: 1,
                height: 1,
                pixels: vec![0.],
            };
            mesh.stamp(
                &brush,
                Dab {
                    center: V(0., 1., 0.),
                    normal: V(0., 1., 0.),
                    right: V(1., 0., 0.),
                    up: V(0., 0., 1.),
                    radius: 0.3,
                    pressure: 1.,
                    delta: V(0.1, 0., 0.),
                    invert: false,
                },
            );
            assert!(
                mesh.vertices
                    .iter()
                    .zip(old)
                    .all(|(a, b)| (a.0, a.1, a.2) == (b.0, b.1, b.2))
            );
            assert!(mesh.mask.iter().all(|&m| m == 0.));
        }
    }

    #[test]
    fn project_roundtrip() {
        let p = Project {
            version: 1,
            mesh: Mesh::sphere(),
            brushes: Kind::ALL.into_iter().map(Brush::new).collect(),
        };
        let bytes = serde_json::to_vec(&p).unwrap();
        let q: Project = serde_json::from_slice(&bytes).unwrap();
        q.mesh.validate().unwrap();
        for b in q.brushes {
            validate_brush(&b).unwrap();
        }
    }
}

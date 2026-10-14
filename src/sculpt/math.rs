use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};
#[derive(Clone, Copy, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec3(pub f32, pub f32, pub f32);
impl Add for Vec3 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self(self.0 + b.0, self.1 + b.1, self.2 + b.2)
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self(self.0 - b.0, self.1 - b.1, self.2 - b.2)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, b: f32) -> Self {
        Self(self.0 * b, self.1 * b, self.2 * b)
    }
}
impl Vec3 {
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
    /// Right-handed rotation in radians about a unit axis.
    pub fn rotated(self, axis: Self, angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        self * cos + axis.cross(self) * sin + axis * (axis.dot(self) * (1. - cos))
    }
}

// Closest point via projected barycentric coordinates, then boundary segments.
pub(crate) fn closest_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d00 = ab.dot(ab);
    let d01 = ab.dot(ac);
    let d11 = ac.dot(ac);
    let d20 = ap.dot(ab);
    let d21 = ap.dot(ac);
    let denominator = d00 * d11 - d01 * d01;
    if denominator > 1e-20 {
        let v = (d11 * d20 - d01 * d21) / denominator;
        let w = (d00 * d21 - d01 * d20) / denominator;
        if v >= 0. && w >= 0. && v + w <= 1. {
            return a + ab * v + ac * w;
        }
    }
    [(a, b), (b, c), (c, a)]
        .into_iter()
        .map(|(x, y)| {
            let d = y - x;
            x + d * ((p - x).dot(d) / d.dot(d).max(1e-20)).clamp(0., 1.)
        })
        .min_by(|x, y| (*x - p).len().total_cmp(&(*y - p).len()))
        .unwrap()
}

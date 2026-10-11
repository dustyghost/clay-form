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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[usize; 3]>,
    /// Polygon topology; triangles remain the surface used for picking and rendering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quads: Vec<[usize; 4]>,
    pub mask: Vec<f32>,
}
impl Mesh {
    /// Copy fully masked faces and compact their vertices; the boundary stays open.
    pub fn extract_masked(&self) -> Result<Self, String> {
        let mut result = Self {
            quads: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
            mask: Vec::new(),
        };
        let mut indices = vec![usize::MAX; self.vertices.len()];
        for face in &self.triangles {
            if !face.iter().all(|&i| self.mask[i] >= 0.5) {
                continue;
            }
            let mut triangle = [0; 3];
            for (corner, &i) in face.iter().enumerate() {
                if indices[i] == usize::MAX {
                    indices[i] = result.vertices.len();
                    result.vertices.push(self.vertices[i]);
                    result.mask.push(0.);
                }
                triangle[corner] = indices[i];
            }
            result.triangles.push(triangle);
        }
        for q in &self.quads {
            if q.iter().all(|&i| self.mask[i] >= 0.5) {
                result.quads.push(q.map(|i| indices[i]));
            }
        }
        if result.triangles.is_empty() {
            return Err("Mask a region first: extraction needs faces with all three vertices at least 50% masked".into());
        }
        result.validate()?;
        Ok(result)
    }
    pub fn sphere() -> Self {
        let n = 48;
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
        let mut quads = Vec::new();
        for i in 0..w {
            f.push([0, 1 + (i + 1) % w, 1 + i]);
            for j in 0..n - 2 {
                let a = 1 + j * w + i;
                let b = 1 + j * w + (i + 1) % w;
                quads.push([a, b, b + w, a + w]);
                f.push([a, b, a + w]);
                f.push([b, b + w, a + w]);
            }
            f.push([bottom, 1 + (n - 2) * w + i, 1 + (n - 2) * w + (i + 1) % w]);
        }
        let mask = vec![0.; v.len()];
        Self {
            quads,
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
        if self.vertices.len() > super::subdivision::MAX_VERTICES
            || self.triangles.len() > super::subdivision::MAX_TRIANGLES
            || self.vertices.is_empty()
            || self
                .quads
                .iter()
                .flatten()
                .any(|&i| i >= self.vertices.len())
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
        if !self.quads.is_empty() {
            let key = |mut t: [usize; 3]| {
                t.sort_unstable();
                t
            };
            let faces: std::collections::HashSet<_> =
                self.triangles.iter().copied().map(key).collect();
            let mut used = std::collections::HashSet::new();
            for &[a, b, c, d] in &self.quads {
                if [a, b, c, d]
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != 4
                {
                    return Err("Invalid quad face".into());
                }
                let pair = if faces.contains(&key([a, b, c])) && faces.contains(&key([a, c, d])) {
                    [key([a, b, c]), key([a, c, d])]
                } else if faces.contains(&key([a, b, d])) && faces.contains(&key([b, c, d])) {
                    [key([a, b, d]), key([b, c, d])]
                } else {
                    return Err("Quad does not match surface triangulation".into());
                };
                if !used.insert(pair[0]) || !used.insert(pair[1]) {
                    return Err("Overlapping quad faces".into());
                }
            }
        }
        Ok(())
    }
    pub fn stamp(&mut self, b: &Brush, dab: Dab) {
        self.stamp_many(b, &[dab]);
    }
    /// Evaluate all symmetry copies against the same geometry, taking the strongest
    /// influence per vertex so overlapping copies cannot multiply brush strength.
    pub fn stamp_many(&mut self, b: &Brush, dabs: &[Dab]) -> bool {
        let mut deformed = false;
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
                if offset.len() > dab.radius * 1.5
                    || (b.kind != Kind::Snake && normals[i].dot(dab.normal) < 0.1)
                {
                    continue;
                }
                let weight = if b.kind == Kind::Snake {
                    b.snake_weight(*dab, offset)
                } else {
                    b.weight(
                        offset.dot(dab.right) / dab.radius,
                        offset.dot(dab.up) / dab.radius,
                    )
                };
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
                Kind::Detail => {
                    // A narrow incision with raised shoulders, plus tangent-only
                    // pinching. Pinching stays inward when the depth is inverted.
                    let tangent = offset - normal * offset.dot(normal);
                    let r = tangent.len() / radius;
                    let core = (-(r / 0.20).powi(2)).exp();
                    let shoulder = (-((r - 0.38) / 0.13).powi(2)).exp();
                    let depth = radius * (-0.18 * core + 0.045 * shoulder);
                    normal * (depth * w * sign) - tangent * (0.12 * (-(r / 0.45).powi(2)).exp() * w)
                }
                Kind::Grab => delta * (w * sign),
                Kind::Snake => (delta + normal * (delta.len() * 0.12)) * (w * sign),
                Kind::Mask => Vec3::default(),
            };
            self.vertices[i] = old[i] + displacement;
            deformed |= self.vertices[i] != old[i];
        }
        deformed
    }
}

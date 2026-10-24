//! Portable, opaque metallic/roughness surface values, independent of groups and masks.
use super::{Brush, Dab, Mesh};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    /// Scene-linear sRGB primaries, not display-encoded RGB.
    pub base_color: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
}
impl Default for Material {
    fn default() -> Self {
        Self::from_srgb([190., 155., 125.].map(|v| v / 255.), 0.85, 0.)
    }
}
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    }
}
impl Material {
    pub fn from_srgb(rgb: [f32; 3], roughness: f32, metallic: f32) -> Self {
        Self {
            base_color: rgb.map(srgb_to_linear),
            roughness,
            metallic,
        }
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Matches the viewport's fixed studio light, also used by the CPU fallback.
    pub(crate) fn studio_color(
        self,
        normal: super::Vec3,
        eye: super::Vec3,
        light: super::Vec3,
    ) -> [f32; 3] {
        let h = (light + eye).norm();
        let nl = normal.dot(light).max(0.);
        let nv = normal.dot(eye).max(0.001);
        let nh = normal.dot(h).max(0.);
        let vh = eye.dot(h).max(0.);
        let a2 = self.roughness.max(0.045).powi(4);
        let d = a2 / (std::f32::consts::PI * (nh * nh * (a2 - 1.) + 1.).powi(2));
        let gv = 2. * nv / (nv + (a2 + (1. - a2) * nv * nv).sqrt());
        let gl = 2. * nl / (nl + (a2 + (1. - a2) * nl * nl).sqrt()).max(0.00001);
        self.base_color.map(|base| {
            let f0 = 0.04 * (1. - self.metallic) + base * self.metallic;
            let f = f0 + (1. - f0) * (1. - vh).powi(5);
            let spec = d * gv * gl * f / (4. * nv * nl.max(0.001)).max(0.00001);
            let diffuse = (1. - f) * (1. - self.metallic) * base / std::f32::consts::PI;
            let ambient = base * (0.10 * (1. - self.metallic) + 0.16 * self.metallic);
            linear_to_srgb((ambient + (diffuse + spec) * nl * 2.5).max(0.))
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if self
            .values()
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Material channels must be finite values from 0 to 1".into());
        }
        Ok(())
    }
    fn values(self) -> [f32; 5] {
        [
            self.base_color[0],
            self.base_color[1],
            self.base_color[2],
            self.roughness,
            self.metallic,
        ]
    }
    fn from_values(v: [f32; 5]) -> Self {
        Self {
            base_color: [v[0], v[1], v[2]],
            roughness: v[3],
            metallic: v[4],
        }
    }
    pub(crate) fn blend(samples: impl IntoIterator<Item = (Self, f32)>) -> Self {
        let mut v = [0.; 5];
        for (material, weight) in samples {
            for (out, x) in v.iter_mut().zip(material.values()) {
                *out += x * weight;
            }
        }
        Self::from_values(v.map(|x| x.clamp(0., 1.)))
    }
    pub(crate) fn delta(self, new: Self, old: Self) -> Self {
        let (a, b, c) = (self.values(), new.values(), old.values());
        Self::from_values(std::array::from_fn(|i| {
            (a[i] + (b[i] - c[i])).clamp(0., 1.)
        }))
    }
    pub fn examples() -> [(&'static str, Self); 6] {
        [
            ("Matte clay", Self::default()),
            (
                "Glazed ceramic",
                Self::from_srgb([0.12, 0.55, 0.65], 0.18, 0.),
            ),
            ("Rubber", Self::from_srgb([0.075, 0.08, 0.09], 0.92, 0.)),
            (
                "Satin aluminium",
                Self::from_srgb([0.91, 0.92, 0.93], 0.38, 1.),
            ),
            ("Copper", Self::from_srgb([0.95, 0.64, 0.46], 0.24, 1.)),
            (
                "Red plastic",
                Self::from_srgb([0.72, 0.045, 0.025], 0.3, 0.),
            ),
        ]
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PaintChannels {
    #[default]
    Material,
    Color,
    Roughness,
    Metallic,
}
impl PaintChannels {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub const ALL: [Self; 4] = [Self::Material, Self::Color, Self::Roughness, Self::Metallic];
    pub fn label(self) -> &'static str {
        match self {
            Self::Material => "Whole material",
            Self::Color => "Colour only",
            Self::Roughness => "Roughness only",
            Self::Metallic => "Metallic only",
        }
    }
    fn apply(self, old: Material, target: Material, weight: f32) -> Material {
        if old == target || weight <= 0. {
            return old;
        }
        let (a, b) = (old.values(), target.values());
        let mut result = if weight >= 1. {
            target
        } else {
            Material::from_values(std::array::from_fn(|i| {
                (a[i] + (b[i] - a[i]) * weight).clamp(0., 1.)
            }))
        };
        if !matches!(self, Self::Material | Self::Color) {
            result.base_color = old.base_color;
        }
        if !matches!(self, Self::Material | Self::Roughness) {
            result.roughness = old.roughness;
        }
        if !matches!(self, Self::Material | Self::Metallic) {
            result.metallic = old.metallic;
        }
        result
    }
}
impl Mesh {
    pub(crate) fn fill_all_material(&mut self, brush: &Brush) {
        for i in 0..self.vertices.len() {
            let old = self.material(i);
            let next = brush.paint_channels.apply(old, brush.material, 1.);
            if next != old {
                if self.materials.is_empty() {
                    self.materials
                        .resize(self.vertices.len(), Material::default());
                }
                self.materials[i] = next;
            }
        }
    }
    pub(crate) fn fill_material(&mut self, brush: &Brush) {
        let eligible = self.sculptable_vertices();
        for (i, allowed) in eligible.into_iter().enumerate() {
            if !allowed {
                continue;
            }
            let old = self.material(i);
            let target = if brush.subtract {
                Material::default()
            } else {
                brush.material
            };
            let next =
                brush
                    .paint_channels
                    .apply(old, target, brush.strength * (1. - self.mask[i]));
            if next != old {
                if self.materials.is_empty() {
                    self.materials
                        .resize(self.vertices.len(), Material::default());
                }
                self.materials[i] = next;
            }
        }
    }
    /// Nearest surface interpolation for voxel remeshing. Bound work and fail atomically.
    pub(crate) fn transfer_materials_to(
        &self,
        dest: &mut Self,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        if self.materials.is_empty() {
            return Ok(());
        }
        let surface = super::projection::Surface::new(self, cancel)?;
        let mut budget = 30_000_000;
        let mut materials = Vec::with_capacity(dest.vertices.len());
        for (index, &p) in dest.vertices.iter().enumerate() {
            if index % 256 == 0 && cancel() {
                return Err("Material transfer cancelled".into());
            }
            let Some((face, q, _)) = surface.nearest(p, f32::MAX.sqrt(), None, &mut budget)? else {
                return Err("Could not transfer painted materials to the remeshed surface".into());
            };
            let ids = self.triangles[face];
            let [a, b, c] = ids.map(|i| self.vertices[i]);
            let (u, v, w) = (b - a, c - a, q - a);
            let (uu, uv, vv, wu, wv) = (u.dot(u), u.dot(v), v.dot(v), w.dot(u), w.dot(v));
            let denominator = uu * vv - uv * uv;
            let weights = if denominator > 1e-20 {
                let y = ((vv * wu - uv * wv) / denominator).clamp(0., 1.);
                let z = ((uu * wv - uv * wu) / denominator).clamp(0., 1.);
                let x = (1. - y - z).max(0.);
                let total = x + y + z;
                [x / total, y / total, z / total]
            } else {
                [1., 0., 0.]
            };
            materials.push(Material::blend(
                ids.into_iter()
                    .zip(weights)
                    .map(|(i, w)| (self.material(i), w)),
            ));
        }
        dest.materials = materials;
        Ok(())
    }
    pub fn material(&self, i: usize) -> Material {
        self.materials.get(i).copied().unwrap_or_default()
    }
    pub(crate) fn validate_materials(&self) -> Result<(), String> {
        if !self.materials.is_empty() && self.materials.len() != self.vertices.len() {
            return Err("Painted material count must match mesh vertices".into());
        }
        for m in &self.materials {
            m.validate()?;
        }
        Ok(())
    }
    pub(crate) fn append_material(&mut self, source: &Self, weights: &[(usize, f32)]) {
        if !source.materials.is_empty() {
            self.materials.push(Material::blend(
                weights.iter().map(|&(i, w)| (source.material(i), w)),
            ));
        }
    }
    pub(crate) fn reindex_materials(
        &self,
        indices: impl IntoIterator<Item = usize>,
    ) -> Vec<Material> {
        if self.materials.is_empty() {
            Vec::new()
        } else {
            indices.into_iter().map(|i| self.material(i)).collect()
        }
    }
    pub(crate) fn paint_material(
        &mut self,
        brush: &Brush,
        dabs: &[Dab],
        stencils: &[super::alpha::Projection],
    ) -> bool {
        let normals = self.normals();
        let eligible = self.sculptable_vertices();
        let mut changed = false;
        for i in 0..self.vertices.len() {
            if !eligible[i] || self.mask[i] >= 1. {
                continue;
            }
            let mut strongest = 0.;
            let mut erase = false;
            for (j, dab) in dabs.iter().enumerate() {
                let offset = self.vertices[i] - dab.center;
                if dab.radius <= 0.
                    || offset.len() > dab.radius * 1.5
                    || normals[i].dot(dab.normal) < 0.1
                {
                    continue;
                }
                let weight = brush.weight(
                    offset.dot(dab.right) / dab.radius,
                    offset.dot(dab.up) / dab.radius,
                ) * brush.strength
                    * (1. - self.mask[i])
                    * if brush.pressure {
                        dab.pressure.powf(brush.pressure_curve)
                    } else {
                        1.
                    }
                    * stencils.get(j).map_or(1., |s| s.weight(self.vertices[i]));
                if weight > strongest {
                    strongest = weight;
                    erase = brush.subtract ^ dab.invert;
                }
            }
            if strongest <= 0. {
                continue;
            }
            let old = self.material(i);
            let next = brush.paint_channels.apply(
                old,
                if erase {
                    Material::default()
                } else {
                    brush.material
                },
                strongest.clamp(0., 1.),
            );
            if next != old {
                if self.materials.is_empty() {
                    self.materials
                        .resize(self.vertices.len(), Material::default());
                }
                self.materials[i] = next;
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests;

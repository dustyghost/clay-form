//! Portable, opaque metallic/roughness surface values, independent of groups and masks.
use super::{Brush, Dab, Mesh};
use serde::{Deserialize, Serialize};
pub mod layers;
use layers::{Influence, Layer, VALUE_COUNT, group, limits};
fn links_empty(v: &std::sync::Arc<Vec<Influence>>) -> bool {
    v.is_empty()
}
pub(super) fn zero(v: &f32) -> bool {
    *v == 0.
}
pub(super) fn black(v: &[f32; 3]) -> bool {
    *v == [0.; 3]
}
pub(super) fn is_ior(v: &f32) -> bool {
    *v == 1.5
}
pub(super) fn is_specular(v: &f32) -> bool {
    *v == 0.5
}
pub(super) fn default_radius() -> [f32; 3] {
    [1., 0.2, 0.1]
}
pub(super) fn default_scale() -> f32 {
    0.05
}
pub(super) fn is_radius(v: &[f32; 3]) -> bool {
    *v == default_radius()
}
pub(super) fn is_scale(v: &f32) -> bool {
    *v == default_scale()
}
fn default_ior() -> f32 {
    1.5
}
fn default_specular() -> f32 {
    0.5
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    /// Scene-linear sRGB primaries, not display-encoded RGB.
    pub base_color: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    #[serde(default = "default_ior", skip_serializing_if = "is_ior")]
    pub ior: f32,
    #[serde(default = "default_specular", skip_serializing_if = "is_specular")]
    pub specular: f32,
    #[serde(default, skip_serializing_if = "black")]
    pub emission: [f32; 3],
    #[serde(default, skip_serializing_if = "zero")]
    pub emission_strength: f32,
    #[serde(default, skip_serializing_if = "zero")]
    pub subsurface: f32,
    #[serde(default = "default_radius", skip_serializing_if = "is_radius")]
    pub subsurface_radius: [f32; 3],
    #[serde(default = "default_scale", skip_serializing_if = "is_scale")]
    pub subsurface_scale: f32,
    #[serde(default, skip_serializing_if = "layers::layers_default")]
    pub layers: layers::LayerStack,
    #[serde(default, skip_serializing_if = "links_empty")]
    pub links: std::sync::Arc<Vec<Influence>>,
}
impl Default for Material {
    fn default() -> Self {
        Self {
            // Scene-linear values from the clay-session-defaults sculpt.
            base_color: [0.09664174, 0.43852815, 0.44494006],
            ..Self::from_srgb([0.; 3], 0.53, 0.)
        }
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
            ior: 1.5,
            specular: 0.5,
            emission: [0.; 3],
            emission_strength: 0.,
            subsurface: 0.,
            subsurface_radius: default_radius(),
            subsurface_scale: default_scale(),
            layers: Default::default(),
            links: Default::default(),
        }
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Fixed illumination for material-library swatches.
    pub(crate) fn studio_color(
        &self,
        normal: super::Vec3,
        eye: super::Vec3,
        light: super::Vec3,
    ) -> [f32; 3] {
        self.lit_color(normal, eye, light, [2.5; 3], 1.)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.values().iter().enumerate().any(|(i, v)| {
            let (lo, hi) = limits(i);
            !v.is_finite() || *v < lo || *v > hi
        }) {
            return Err("Invalid material surface values".into());
        }
        if self.links.len() > 256 {
            return Err("Too many material influences".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut sums = [0.; 4];
        for link in self.links.iter() {
            if link.id == 0
                || !ids.insert(link.id)
                || link
                    .weights
                    .iter()
                    .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            {
                return Err("Invalid material influence".into());
            }
            for (sum, w) in sums.iter_mut().zip(link.weights) {
                *sum += w;
            }
        }
        if sums.iter().any(|v| *v > 1.0001) {
            return Err("Material influence weights exceed one".into());
        }
        Ok(())
    }
    pub(crate) fn needs_extended_shading(&self) -> bool {
        self.ior != 1.5
            || self.specular != 0.5
            || self.emission_strength != 0.
            || self.subsurface != 0.
            || self.layers.iter().any(|l| l.weight != 0.)
    }
    pub fn values(&self) -> [f32; VALUE_COUNT] {
        let mut v = [0.; VALUE_COUNT];
        v[..16].copy_from_slice(&self.surface().values());
        for (i, layer) in self.layers.iter().enumerate() {
            let start = 16 + i * 18;
            v[start..start + 16].copy_from_slice(&layer.surface.values());
            v[start + 16] = layer.weight;
            v[start + 17] = layer.add;
        }
        v
    }
    pub fn from_values(v: [f32; VALUE_COUNT]) -> Self {
        let v = std::array::from_fn::<_, VALUE_COUNT, _>(|i| {
            let (lo, hi) = limits(i);
            v[i].clamp(lo, hi)
        });
        let mut m = Self::from_srgb([0.; 3], v[3], v[4]);
        m.base_color = [v[0], v[1], v[2]];
        m.ior = v[5];
        m.specular = v[6];
        m.emission = [v[7], v[8], v[9]];
        m.emission_strength = v[10];
        m.subsurface = v[11];
        m.subsurface_radius = [v[12], v[13], v[14]];
        m.subsurface_scale = v[15];
        m.layers = std::array::from_fn(|i| {
            let start = 16 + i * 18;
            Layer {
                surface: layers::Surface::from_values(&v[start..start + 16]),
                weight: v[start + 16],
                add: v[start + 17],
            }
        })
        .into();
        m
    }
    pub fn detach_asset(&mut self, id: u64) {
        if self.links.iter().any(|link| link.id == id) {
            std::sync::Arc::make_mut(&mut self.links).retain(|link| link.id != id);
        }
    }
    pub fn unlinked(&self) -> Self {
        let mut m = self.clone();
        m.links = Default::default();
        m
    }
    pub fn with_link(&self, id: u64) -> Self {
        let mut m = self.unlinked();
        m.links = std::sync::Arc::new(vec![Influence {
            id,
            weights: [1.; 4],
        }]);
        m
    }
    pub fn primary_asset(&self) -> Option<u64> {
        if self.links.len() == 1 && self.links[0].weights == [1.; 4] {
            Some(self.links[0].id)
        } else {
            None
        }
    }
    pub fn rebase_asset(&mut self, id: u64, old: &Self, new: &Self) {
        if let Some(link) = self.links.iter().find(|link| link.id == id) {
            if link.weights == [1.; 4] && self.values() == old.values() {
                let links = self.links.clone();
                *self = new.unlinked();
                self.links = links;
                return;
            }
            let (a, b, c) = (self.values(), old.values(), new.values());
            let mut result = Self::from_values(std::array::from_fn(|i| {
                a[i] + (c[i] - b[i]) * link.weights[group(i)]
            }));
            result.links = self.links.clone();
            *self = result;
        }
    }
    fn combine_links(
        samples: impl IntoIterator<Item = (Self, [f32; 4])>,
    ) -> std::sync::Arc<Vec<Influence>> {
        let mut links = std::collections::BTreeMap::<u64, [f32; 4]>::new();
        for (m, weights) in samples {
            for link in m.links.iter() {
                let entry = links.entry(link.id).or_default();
                for i in 0..4 {
                    entry[i] += link.weights[i] * weights[i];
                }
            }
        }
        let mut sums = [0_f32; 4];
        for v in links.values_mut() {
            for i in 0..4 {
                v[i] = v[i].clamp(0., 1.);
                sums[i] += v[i];
            }
        }
        links
            .into_iter()
            .filter_map(|(id, mut weights)| {
                for i in 0..4 {
                    weights[i] /= sums[i].max(1.);
                }
                weights
                    .iter()
                    .any(|w| *w > 0.000001)
                    .then_some(Influence { id, weights })
            })
            .collect::<Vec<_>>()
            .into()
    }
    pub(crate) fn blend(samples: impl IntoIterator<Item = (Self, f32)>) -> Self {
        let samples: Vec<_> = samples.into_iter().collect();
        let mut v = [0.; VALUE_COUNT];
        for (m, w) in &samples {
            for (out, x) in v.iter_mut().zip(m.values()) {
                *out += x * w;
            }
        }
        let mut m = Self::from_values(v);
        m.links = Self::combine_links(samples.into_iter().map(|(m, w)| (m, [w; 4])));
        m
    }
    pub(crate) fn delta(self, new: Self, old: Self) -> Self {
        let (a, b, c) = (self.values(), new.values(), old.values());
        let mut m = Self::from_values(std::array::from_fn(|i| a[i] + (b[i] - c[i])));
        m.links = Self::combine_links([(self, [1.; 4]), (new, [1.; 4]), (old, [-1.; 4])]);
        m
    }
    fn organic(
        rgb: [f32; 3],
        roughness: f32,
        subsurface: f32,
        radius: [f32; 3],
        scale: f32,
    ) -> Self {
        let mut m = Self::from_srgb(rgb, roughness, 0.);
        m.ior = 1.45;
        m.specular = 0.4;
        m.subsurface = subsurface;
        m.subsurface_radius = radius;
        m.subsurface_scale = scale;
        m.layers[0] = Layer {
            surface: layers::Surface {
                roughness: 0.25,
                ior: 1.45,
                specular: 0.3,
                ..Default::default()
            },
            weight: 0.12,
            add: 1.,
        };
        m
    }
    pub fn examples() -> [(&'static str, Self); 11] {
        [
            (
                "Matte clay",
                Self::from_srgb([190., 155., 125.].map(|v| v / 255.), 0.85, 0.),
            ),
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
            ("Cool clay", Self::from_srgb([0.48, 0.53, 0.56], 0.9, 0.)),
            (
                "Beeswax",
                Self::organic([0.82, 0.55, 0.18], 0.38, 0.7, [1., 0.6, 0.25], 0.09),
            ),
            (
                "Ivory wax",
                Self::organic([0.88, 0.83, 0.69], 0.32, 0.8, [1., 0.8, 0.5], 0.12),
            ),
            (
                "Warm skin",
                Self::organic([0.69, 0.41, 0.30], 0.62, 0.45, [1., 0.35, 0.2], 0.035),
            ),
            (
                "Deep skin",
                Self::organic([0.28, 0.14, 0.085], 0.58, 0.5, [1., 0.35, 0.2], 0.045),
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
        if matches!(self, Self::Material) && weight >= 1. {
            return target;
        }
        if self != Self::Material {
            let w = weight.clamp(0., 1.);
            let mut result = old.clone();
            let mix = |a: f32, b: f32| if w >= 1. { b } else { a + (b - a) * w };
            match self {
                Self::Color => {
                    result.base_color =
                        std::array::from_fn(|c| mix(old.base_color[c], target.base_color[c]))
                }
                Self::Roughness => result.roughness = mix(old.roughness, target.roughness),
                Self::Metallic => result.metallic = mix(old.metallic, target.metallic),
                Self::Material => unreachable!(),
            }
            let channel = match self {
                Self::Color => 0,
                Self::Roughness => 1,
                _ => 2,
            };
            let weights = std::array::from_fn(|i| if i == channel { w } else { 0. });
            result.links =
                Material::combine_links([(old, weights.map(|w| 1. - w)), (target, weights)]);
            return result;
        }
        let (a, b) = (old.values(), target.values());
        let weights: [f32; 4] = std::array::from_fn(|g| {
            if matches!(self, Self::Material)
                || matches!(
                    (self, g),
                    (Self::Color, 0) | (Self::Roughness, 1) | (Self::Metallic, 2)
                )
            {
                weight.clamp(0., 1.)
            } else {
                0.
            }
        });
        let mut result = Material::from_values(std::array::from_fn(|i| match weights[group(i)] {
            0. => a[i],
            1. => b[i],
            w => a[i] + (b[i] - a[i]) * w,
        }));
        result.links = Material::combine_links([(old, weights.map(|w| 1. - w)), (target, weights)]);
        result
    }
}
impl Mesh {
    pub(crate) fn fill_all_material(&mut self, brush: &Brush) {
        for i in 0..self.vertices.len() {
            let old = self.material(i);
            let next = brush
                .paint_channels
                .apply(old.clone(), brush.material.clone(), 1.);
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
                brush.material.clone()
            };
            let next = brush.paint_channels.apply(
                old.clone(),
                target,
                brush.strength * (1. - self.mask[i]),
            );
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
        self.materials.get(i).cloned().unwrap_or_default()
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
        self.paint_material_with_scratch(
            brush,
            dabs,
            stencils,
            &[],
            &mut super::texture::PaintScratch::default(),
        )
    }
    pub(crate) fn paint_material_with_scratch(
        &mut self,
        brush: &Brush,
        dabs: &[Dab],
        stencils: &[super::alpha::Projection],
        occluders: &[&Mesh],
        scratch: &mut super::texture::PaintScratch,
    ) -> bool {
        if scratch.normals.len() != self.vertices.len() {
            scratch.normals = self.normals();
            scratch.eligible = self.sculptable_vertices();
        }
        for i in scratch.touched.drain(..) {
            scratch.weights[i] = (0., false);
        }
        scratch.weights.resize(self.vertices.len(), (0., false));
        for (j, dab) in dabs.iter().enumerate() {
            if dab.radius <= 0. {
                continue;
            }
            scratch.depth.prepare(self, dab, occluders);
            for &i in &scratch.depth.active_vertices {
                let point = self.vertices[i];
                let offset = point - dab.center;
                if !scratch.eligible[i]
                    || self.mask[i] >= 1.
                    || offset.len() > dab.radius * 1.5
                    || scratch.normals[i].dot(dab.normal) < 0.1
                {
                    continue;
                }
                let local = brush.paint_weight(
                    offset.dot(dab.right) / dab.radius,
                    offset.dot(dab.up) / dab.radius,
                ) * (1. - self.mask[i])
                    * if brush.pressure {
                        dab.pressure.powf(brush.pressure_curve)
                    } else {
                        1.
                    }
                    * stencils.get(j).map_or(1., |s| s.weight(point));
                let weight = brush.paint_opacity(local);
                if weight > scratch.weights[i].0 && scratch.depth.visible(point, dab, None) {
                    if scratch.weights[i].0 == 0. {
                        scratch.touched.push(i);
                    }
                    scratch.weights[i] = (weight, brush.subtract ^ dab.invert);
                }
            }
        }
        let mut changed = false;
        for &i in &scratch.touched {
            let (strongest, erase) = scratch.weights[i];
            if strongest <= 0. {
                continue;
            }
            let old = self.material(i);
            let next = brush.paint_channels.apply(
                old.clone(),
                if erase {
                    Material::default()
                } else {
                    brush.material.clone()
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

#[cfg(test)]
mod paint_regressions {
    use super::*;
    #[test]
    fn vertex_paint_rejects_occluded_sheets_and_other_models() {
        let mut mesh = super::super::texture::tests::sheets();
        let mut brush = Brush::new(super::super::Kind::Paint);
        brush.alpha = super::super::Alpha::Hard;
        brush.strength = 1.;
        brush.pressure = false;
        brush.material = Material::from_srgb([1., 0., 0.], 0.5, 0.);
        let dab = super::super::texture::tests::dab();
        let old = mesh.material(0);
        assert!(mesh.paint_material(&brush, &[dab], &[]));
        assert!(mesh.materials[..4].iter().all(|m| *m == brush.material));
        assert!(mesh.materials[4..].iter().all(|m| *m == old));
        let mut barrier = mesh.clone();
        for p in &mut barrier.vertices {
            p.2 += 0.01;
        }
        let mut covered = super::super::texture::tests::sheets();
        assert!(!covered.paint_material_with_scratch(
            &brush,
            &[dab],
            &[],
            &[&barrier],
            &mut Default::default()
        ));
        covered.hidden_faces = vec![true, true, false, false];
        assert!(covered.paint_material(&brush, &[dab], &[]));
        assert!(covered.materials[..4].iter().all(|m| *m == old));
        assert!(covered.materials[4..].iter().all(|m| *m == brush.material));
    }
    #[test]
    fn channel_edits_retain_layer_storage_and_unselected_values() {
        let mut old = Material::default();
        old.layers[0].weight = 0.5;
        let id = old.layers.storage_id();
        let target = Material::from_srgb([1., 0., 0.], 0.1, 1.);
        for channel in [
            PaintChannels::Color,
            PaintChannels::Roughness,
            PaintChannels::Metallic,
        ] {
            let changed = channel.apply(old.clone(), target.clone(), 0.4);
            assert_eq!(changed.layers.storage_id(), id);
            let group = match channel {
                PaintChannels::Color => 0,
                PaintChannels::Roughness => 1,
                _ => 2,
            };
            for (i, (a, b)) in old.values().into_iter().zip(changed.values()).enumerate() {
                if layers::group(i) != group {
                    assert_eq!(a, b);
                }
            }
        }
    }
}

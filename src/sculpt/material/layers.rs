use super::{Material, linear_to_srgb};
use crate::sculpt::Vec3;
use serde::{Deserialize, Serialize};

pub const VALUE_COUNT: usize = 70;
pub const VERTEX_STRIDE: usize = 77;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Surface {
    #[serde(skip_serializing_if = "super::black")]
    pub base_color: [f32; 3],
    pub roughness: f32,
    #[serde(skip_serializing_if = "super::zero")]
    pub metallic: f32,
    #[serde(skip_serializing_if = "super::is_ior")]
    pub ior: f32,
    #[serde(skip_serializing_if = "super::is_specular")]
    pub specular: f32,
    #[serde(skip_serializing_if = "super::black")]
    pub emission: [f32; 3],
    #[serde(skip_serializing_if = "super::zero")]
    pub emission_strength: f32,
    #[serde(skip_serializing_if = "super::zero")]
    pub subsurface: f32,
    #[serde(skip_serializing_if = "super::is_radius")]
    pub subsurface_radius: [f32; 3],
    #[serde(skip_serializing_if = "super::is_scale")]
    pub subsurface_scale: f32,
}
impl Default for Surface {
    fn default() -> Self {
        Self {
            base_color: [0.; 3],
            roughness: 0.2,
            metallic: 0.,
            ior: 1.5,
            specular: 0.5,
            emission: [0.; 3],
            emission_strength: 0.,
            subsurface: 0.,
            subsurface_radius: super::default_radius(),
            subsurface_scale: super::default_scale(),
        }
    }
}
impl Surface {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn values(self) -> [f32; 16] {
        [
            self.base_color[0],
            self.base_color[1],
            self.base_color[2],
            self.roughness,
            self.metallic,
            self.ior,
            self.specular,
            self.emission[0],
            self.emission[1],
            self.emission[2],
            self.emission_strength,
            self.subsurface,
            self.subsurface_radius[0],
            self.subsurface_radius[1],
            self.subsurface_radius[2],
            self.subsurface_scale,
        ]
    }
    pub fn from_values(v: &[f32]) -> Self {
        Self {
            base_color: [v[0], v[1], v[2]],
            roughness: v[3],
            metallic: v[4],
            ior: v[5],
            specular: v[6],
            emission: [v[7], v[8], v[9]],
            emission_strength: v[10],
            subsurface: v[11],
            subsurface_radius: [v[12], v[13], v[14]],
            subsurface_scale: v[15],
        }
    }
    pub fn linear_color(
        self,
        normal: Vec3,
        eye: Vec3,
        light: Vec3,
        radiance: [f32; 3],
        fill: f32,
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
        std::array::from_fn(|channel| {
            let base = self.base_color[channel];
            let dielectric = ((self.ior - 1.) / (self.ior + 1.)).powi(2) * self.specular * 2.;
            let f0 = dielectric * (1. - self.metallic) + base * self.metallic;
            let f = f0 + (1. - f0) * (1. - vh).powi(5);
            let spec = d * gv * gl * f / (4. * nv * nl.max(0.001)).max(0.00001);
            let diffuse = (1. - f) * (1. - self.metallic) * base / std::f32::consts::PI;
            let ambient = base * (0.10 * (1. - self.metallic) + 0.16 * self.metallic);
            // Fast local approximation; exchange uses Principled random-walk SSS.
            let wrap =
                (self.subsurface_radius[channel] * self.subsurface_scale * 10.).clamp(0., 1.);
            let scattered = ((normal.dot(light) + wrap) / (1. + wrap)).max(0.) / (1. + wrap);
            let diffuse_light = nl * (1. - self.subsurface) + scattered * self.subsurface;
            ambient * fill
                + (diffuse * diffuse_light + spec * nl) * radiance[channel]
                + self.emission[channel] * self.emission_strength
        })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layer {
    #[serde(skip_serializing_if = "Surface::is_default")]
    pub surface: Surface,
    #[serde(skip_serializing_if = "super::zero")]
    pub weight: f32,
    /// 0 = Mix Shader, 1 = Add Shader. Intermediate values preserve painted interpolation.
    #[serde(skip_serializing_if = "super::zero")]
    pub add: f32,
}
/// Copy-on-write layers keep ordinary vertex paint compact. Identical unedited
/// stacks share storage across vertices, display snapshots and history entries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerStack(std::sync::Arc<[Layer; 3]>);
impl Default for LayerStack {
    fn default() -> Self {
        static EMPTY: std::sync::OnceLock<std::sync::Arc<[Layer; 3]>> = std::sync::OnceLock::new();
        Self(
            EMPTY
                .get_or_init(|| std::sync::Arc::new([Layer::default(); 3]))
                .clone(),
        )
    }
}
impl From<[Layer; 3]> for LayerStack {
    fn from(layers: [Layer; 3]) -> Self {
        if layers == [Layer::default(); 3] {
            Self::default()
        } else {
            Self(std::sync::Arc::new(layers))
        }
    }
}
impl std::ops::Deref for LayerStack {
    type Target = [Layer; 3];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for LayerStack {
    fn deref_mut(&mut self) -> &mut Self::Target {
        std::sync::Arc::make_mut(&mut self.0)
    }
}
impl LayerStack {
    pub(crate) fn storage_id(&self) -> usize {
        std::sync::Arc::as_ptr(&self.0) as usize
    }
}
pub fn layers_default(layers: &LayerStack) -> bool {
    **layers == [Layer::default(); 3]
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Influence {
    pub id: u64,
    /// Independent binding weights for colour, roughness, metallic and remaining shader controls.
    pub weights: [f32; 4],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: u64,
    pub name: String,
    pub material: Material,
}
pub fn fresh_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(7);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(7, |t| t.as_nanos() as u64);
    NEXT.fetch_max(now, Ordering::Relaxed);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
pub fn limits(i: usize) -> (f32, f32) {
    let property = if i < 16 { i } else { (i - 16) % 18 };
    match property {
        5 => (1., 4.),
        10 => (0., 100.),
        12..=15 => (0., 10.),
        _ => (0., 1.),
    }
}
pub fn group(i: usize) -> usize {
    match i {
        0..=2 => 0,
        3 => 1,
        4 => 2,
        _ => 3,
    }
}
impl Material {
    pub fn surface(&self) -> Surface {
        Surface {
            base_color: self.base_color,
            roughness: self.roughness,
            metallic: self.metallic,
            ior: self.ior,
            specular: self.specular,
            emission: self.emission,
            emission_strength: self.emission_strength,
            subsurface: self.subsurface,
            subsurface_radius: self.subsurface_radius,
            subsurface_scale: self.subsurface_scale,
        }
    }
    pub(crate) fn lit_color(
        &self,
        normal: Vec3,
        eye: Vec3,
        light: Vec3,
        radiance: [f32; 3],
        fill: f32,
    ) -> [f32; 3] {
        let mut result = self
            .surface()
            .linear_color(normal, eye, light, radiance, fill);
        for layer in self.layers.iter() {
            if layer.weight <= 0. {
                continue;
            }
            let colour = layer
                .surface
                .linear_color(normal, eye, light, radiance, fill);
            for i in 0..3 {
                result[i] =
                    result[i] * (1. - layer.weight * (1. - layer.add)) + colour[i] * layer.weight;
            }
        }
        result.map(|v| linear_to_srgb(v.max(0.)))
    }
}

pub fn validate_assets(assets: &[Asset]) -> Result<(), String> {
    if assets.len() > 256 {
        return Err("Too many named materials".into());
    }
    let mut ids = std::collections::HashSet::new();
    for asset in assets {
        if asset.id == 0
            || !ids.insert(asset.id)
            || asset.name.trim().is_empty()
            || asset.name.len() > 128
            || !asset.material.links.is_empty()
        {
            return Err("Invalid named material".into());
        }
        asset.material.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn material_subsurface_preview_softens_backlit_diffuse_by_rgb_radius() {
        let mut m = Material::from_srgb([0.5; 3], 0.6, 0.);
        let n = Vec3(0., 0., 1.);
        let light = Vec3(-1., 0., -0.2).norm();
        assert_eq!(m.lit_color(n, n, light, [2.5; 3], 0.), [0.; 3]);
        m.subsurface = 1.;
        m.subsurface_scale = 0.1;
        m.subsurface_radius = [1., 0.1, 0.05];
        let colour = m.lit_color(n, n, light, [2.5; 3], 0.);
        assert!(colour[0] > 0.1);
        assert_eq!(colour[1], 0.);
        assert_eq!(colour[2], 0.);
        let restored: Material = serde_json::from_slice(&serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(m, restored);
    }
    #[test]
    fn material_layer_mix_add_legacy_defaults_and_channel_bindings() {
        let old: Material =
            serde_json::from_str(r#"{"base_color":[0.2,0.3,0.4],"roughness":0.5,"metallic":0.1}"#)
                .unwrap();
        assert_eq!(old.ior, 1.5);
        assert!(old.layers.iter().all(|l| l.weight == 0.));
        old.validate().unwrap();
        let mut m = old.clone();
        m.layers[0] = Layer {
            surface: Surface {
                emission: [0.2, 0.4, 0.8],
                emission_strength: 2.,
                ..Default::default()
            },
            weight: 0.25,
            add: 0.,
        };
        let direction = Vec3(0., 0., 1.);
        let base = old
            .surface()
            .linear_color(direction, direction, direction, [0.; 3], 0.);
        for add in [0., 0.3, 1.] {
            m.layers[0].add = add;
            let actual = m.lit_color(direction, direction, direction, [0.; 3], 0.);
            for c in 0..3 {
                let expected =
                    linear_to_srgb(base[c] * (1. - 0.25 * (1. - add)) + [0.4, 0.8, 1.6][c] * 0.25);
                assert!((actual[c] - expected).abs() < 1e-6);
            }
        }
        let blended = Material::blend([(m.with_link(5), 0.25), (old.with_link(7), 0.75)]);
        blended.validate().unwrap();
        assert_eq!(blended.links[0].weights, [0.25; 4]);
        assert_eq!(blended.links[1].weights, [0.75; 4]);
        let mut invalid = m;
        invalid.layers[1].surface.ior = 0.;
        assert!(invalid.validate().is_err());
    }
}

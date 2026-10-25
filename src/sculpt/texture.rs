//! Portable, copy-on-write image assets. Colour is sRGB; scalar maps are linear data.
use super::{Brush, Dab, Mesh, Vec3};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
pub const MAX_SIZE: u32 = 4096;
pub const MAX_PIXELS: usize = 3 * 4096 * 4096;
mod pixel_storage;
mod pixels;
pub use pixels::{Pixels, Precision};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    #[default]
    Color,
    Roughness,
    Metallic,
}
impl Channel {
    pub const ALL: [Self; 3] = [Self::Color, Self::Roughness, Self::Metallic];
    pub fn label(self) -> &'static str {
        match self {
            Self::Color => "Base colour",
            Self::Roughness => "Roughness (linear)",
            Self::Metallic => "Metallic (linear)",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Color => 0,
            Self::Roughness => 1,
            Self::Metallic => 2,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Texture {
    #[serde(default)]
    pub profile: Vec<u8>,
    #[serde(default)]
    pub exchange: Option<Exchange>,
    pub id: u64,
    pub name: String,
    pub channel: Channel,
    pub width: u32,
    pub height: u32,
    /// RGBA16, straight alpha. Colour pixels are display encoded; data pixels are not.
    #[serde(with = "pixel_storage")]
    pub pixels: Pixels,
}
impl Texture {
    pub fn new(channel: Channel, size: u32, material: &super::Material) -> Result<Self, String> {
        Self::new_with_precision(channel, size, material, Precision::U16)
    }
    pub fn new_id() -> u64 {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64)
            .wrapping_add(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
            .max(1)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.id == 0
            || self.name.len() > 256
            || self.profile.len() > 1024 * 1024
            || self.exchange.as_ref().is_some_and(|e| e.path.len() > 4096)
            || self.width == 0
            || self.height == 0
            || self.width > MAX_SIZE
            || self.height > MAX_SIZE
            || self.pixels.len() != self.width as usize * self.height as usize * 4
        {
            return Err("Invalid texture asset (maximum 4096 × 4096 RGBA)".into());
        }
        if let Pixels::Float32(pixels) = &self.pixels {
            if pixels.iter().any(|v| !v.is_finite())
                || pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| !(0. ..=1.).contains(&p[3]))
            {
                return Err("Float textures require finite values and alpha inside 0–1".into());
            }
            if !self.profile.is_empty() {
                return Err("Float EXR colour uses linear Rec.709, not an ICC profile".into());
            }
        }
        Ok(())
    }
    pub fn new_with_precision(
        channel: Channel,
        size: u32,
        material: &super::Material,
        precision: Precision,
    ) -> Result<Self, String> {
        let value = match channel {
            Channel::Color if precision == Precision::U16 => {
                material.base_color.map(super::material::linear_to_srgb)
            }
            Channel::Color => material.base_color,
            Channel::Roughness => [material.roughness; 3],
            Channel::Metallic => [material.metallic; 3],
        };
        if !(16..=MAX_SIZE).contains(&size) {
            return Err("Texture size must be 16–4096 pixels".into());
        }
        let pixel = [value[0], value[1], value[2], 1.];
        let pixels = match precision {
            Precision::U16 => Pixels::U16(Arc::new(
                pixel
                    .map(|v| (v.clamp(0., 1.) * 65535.).round() as u16)
                    .repeat((size * size) as usize),
            )),
            Precision::Float32 => Pixels::Float32(Arc::new(pixel.repeat((size * size) as usize))),
        };
        Ok(Self {
            profile: Vec::new(),
            exchange: None,
            id: Self::new_id(),
            name: channel.label().into(),
            channel,
            width: size,
            height: size,
            pixels,
        })
    }

    pub fn linear_pixel(&self, index: usize) -> [f32; 4] {
        let mut p = self.pixels.pixel(index);
        if self.channel == Channel::Color && self.pixels.precision() == Precision::U16 {
            for v in &mut p[..3] {
                *v = super::material::srgb_to_linear(*v);
            }
        }
        p
    }
    pub fn packed_pixels(&self) -> Box<dyn Iterator<Item = [u32; 2]> + '_> {
        match &self.pixels {
            Pixels::U16(p) => Box::new(p.as_chunks::<4>().0.iter().map(|p| {
                [
                    u32::from(p[0]) | (u32::from(p[1]) << 16),
                    u32::from(p[2]) | (u32::from(p[3]) << 16),
                ]
            })),
            Pixels::Float32(p) => {
                Box::new(p.as_chunks::<2>().0.iter().map(|p| p.map(f32::to_bits)))
            }
        }
    }
}
/// Three 4096² images or an equivalent mix, uploaded as bounded texture-array pages.
pub(crate) fn validate_budget<'a>(
    meshes: impl IntoIterator<Item = &'a Mesh>,
) -> Result<(), String> {
    let mut assets = std::collections::HashMap::new();
    for mesh in meshes {
        for texture in &mesh.uv.textures {
            assets.insert(texture.id, texture.width as usize * texture.height as usize);
        }
    }
    if assets.values().sum::<usize>() > MAX_PIXELS {
        Err("Texture budget exceeded: use at most three 4K images (48 million unique pixels) across this project, or smaller images".into())
    } else {
        Ok(())
    }
}
pub fn barycentric(q: [f32; 2], t: [[f32; 2]; 3]) -> Option<[f32; 3]> {
    let a = super::uv::area(t);
    if a.abs() < 1e-15 {
        return None;
    }
    let w = [
        super::uv::area([q, t[1], t[2]]) / a,
        super::uv::area([t[0], q, t[2]]) / a,
        super::uv::area([t[0], t[1], q]) / a,
    ];
    w.iter().all(|v| *v >= -1e-5).then_some(w)
}
impl Mesh {
    /// Extend edge pixels into unused atlas gutters without touching another island.
    pub(crate) fn pad_texture(&mut self, channel: Channel) {
        if !self.uv.current(self) {
            return;
        }
        let Some(i) = self.uv.textures.iter().position(|t| t.channel == channel) else {
            return;
        };
        let texture = &self.uv.textures[i];
        let (width, height) = (texture.width as usize, texture.height as usize);
        let mut occupied = vec![false; width * height];
        for (f, t) in self.uv.corners.iter().enumerate() {
            if !self.uv.mapped(f) {
                continue;
            }
            raster(t.map(|q| [q[0], 1. - q[1]]), width, height, |x, y, _| {
                occupied[y * width + x] = true
            });
        }
        pad_pixels(&mut self.uv.textures[i], occupied);
    }
    /// Rasterize visible surface triangles into UV space. The dab's frontmost depth
    /// field excludes occluded sheets, while sampling either side of seams in 3D.
    pub(crate) fn paint_texture(
        &mut self,
        channel: Channel,
        brush: &Brush,
        dabs: &[Dab],
        stencils: &[super::alpha::Projection],
        occluders: &[&Mesh],
    ) -> bool {
        if !self.uv.current(self) {
            return false;
        }
        let Some(index) = self.uv.textures.iter().position(|t| t.channel == channel) else {
            return false;
        };
        let texture = &self.uv.textures[index];
        let width = texture.width as usize;
        let height = texture.height as usize;
        let mut weights = std::collections::HashMap::<usize, (f32, bool)>::new();
        for (di, dab) in dabs.iter().enumerate() {
            if dab.radius <= 0. {
                continue;
            }
            // Projected triangle bins accelerate exact frontmost-surface queries.
            const DEPTH: usize = 128;
            let mut depth = vec![Vec::<usize>::new(); DEPTH * DEPTH];
            let projected: Vec<_> = self
                .vertices
                .iter()
                .map(|p| {
                    let q = *p - dab.center;
                    [
                        q.dot(dab.right) / dab.radius,
                        q.dot(dab.up) / dab.radius,
                        q.dot(dab.normal),
                    ]
                })
                .collect();
            let mut depth_faces: Vec<_> = self
                .triangles
                .iter()
                .map(|t| t.map(|i| projected[i]))
                .collect();
            let mut visible: Vec<_> = (0..self.triangles.len())
                .map(|f| self.face_visible(f))
                .collect();
            for mesh in occluders {
                for (f, t) in mesh.triangles.iter().enumerate() {
                    depth_faces.push(t.map(|i| {
                        let q = mesh.vertices[i] - dab.center;
                        [
                            q.dot(dab.right) / dab.radius,
                            q.dot(dab.up) / dab.radius,
                            q.dot(dab.normal),
                        ]
                    }));
                    visible.push(mesh.face_visible(f));
                }
            }
            for (f, q) in depth_faces.iter().enumerate() {
                if !visible[f] {
                    continue;
                }
                let xy = q.map(|p| [(p[0] + 1.) * 0.5, (1. - p[1]) * 0.5]);
                if (0..2).any(|k| xy.iter().all(|p| p[k] < 0.) || xy.iter().all(|p| p[k] > 1.)) {
                    continue;
                }
                let lo: [usize; 2] = std::array::from_fn(|k| {
                    (xy.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min) * DEPTH as f32)
                        .floor()
                        .clamp(0., (DEPTH - 1) as f32) as usize
                });
                let hi: [usize; 2] = std::array::from_fn(|k| {
                    (xy.iter().map(|p| p[k]).fold(f32::NEG_INFINITY, f32::max) * DEPTH as f32)
                        .floor()
                        .clamp(0., (DEPTH - 1) as f32) as usize
                });
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        depth[y * DEPTH + x].push(f);
                    }
                }
            }
            for (f, t) in self.triangles.iter().enumerate() {
                if !self.face_visible(f) {
                    continue;
                }
                let p = t.map(|i| self.vertices[i]);
                let normal = (p[1] - p[0]).cross(p[2] - p[0]).norm();
                if normal.dot(dab.normal) < 0.1 {
                    continue;
                }
                let projected_face = t.map(|i| projected[i]);
                if (0..2).any(|k| {
                    projected_face.iter().all(|p| p[k] < -1.)
                        || projected_face.iter().all(|p| p[k] > 1.)
                }) {
                    continue;
                }
                if !self.uv.mapped(f) {
                    continue;
                }
                let uv = self.uv.corners[f].map(|q| [q[0], 1. - q[1]]);
                raster(uv, width, height, |x, y, w| {
                    let point = (0..3).fold(Vec3::default(), |sum, c| sum + p[c] * w[c]);
                    let offset = point - dab.center;
                    if offset.len() > dab.radius * 1.5 {
                        return;
                    }
                    let q = [
                        offset.dot(dab.right) / dab.radius,
                        offset.dot(dab.up) / dab.radius,
                    ];
                    if q[0].abs() > 1. || q[1].abs() > 1. {
                        return;
                    }
                    let dx = (((q[0] + 1.) * 0.5 * DEPTH as f32) as usize).min(DEPTH - 1);
                    let dy = (((1. - q[1]) * 0.5 * DEPTH as f32) as usize).min(DEPTH - 1);
                    let query = [(q[0] + 1.) * 0.5, (1. - q[1]) * 0.5];
                    let occluded = depth[dy * DEPTH + dx].iter().any(|&other| {
                        if other == f {
                            return false;
                        }
                        let face = depth_faces[other];
                        let xy = face.map(|p| [(p[0] + 1.) * 0.5, (1. - p[1]) * 0.5]);
                        barycentric(query, xy).is_some_and(|w| {
                            (0..3).map(|c| face[c][2] * w[c]).sum::<f32>()
                                > offset.dot(dab.normal) + dab.radius * 1e-5
                        })
                    });
                    if occluded {
                        return;
                    }
                    let mask = (0..3).map(|c| self.mask[t[c]] * w[c]).sum::<f32>();
                    let weight = brush.paint_weight(q[0], q[1])
                        * brush.strength
                        * (1. - mask)
                        * if brush.pressure {
                            dab.pressure.powf(brush.pressure_curve)
                        } else {
                            1.
                        }
                        * stencils.get(di).map_or(1., |s| s.weight(point));
                    let entry = weights.entry(y * width + x).or_default();
                    if weight > entry.0 {
                        *entry = (weight.clamp(0., 1.), brush.subtract ^ dab.invert);
                    }
                });
            }
        }
        let value = match channel {
            Channel::Color => brush.material.base_color,
            Channel::Roughness => [brush.material.roughness; 3],
            Channel::Metallic => [brush.material.metallic; 3],
        };
        let mut changed = false;
        let texture = &mut self.uv.textures[index];
        for (pixel, (weight, erase)) in weights {
            if weight <= 0. {
                continue;
            }
            let old = texture.linear_pixel(pixel);
            let mut next = old;
            if erase {
                next[3] = old[3] * (1. - weight);
            } else {
                let alpha = old[3];
                let out_alpha = weight + alpha * (1. - weight);
                for c in 0..3 {
                    next[c] =
                        (value[c] * weight + old[c] * alpha * (1. - weight)) / out_alpha.max(1e-10);
                }
                next[3] = out_alpha;
            }
            if channel == Channel::Color && texture.pixels.precision() == Precision::U16 {
                for v in &mut next[..3] {
                    *v = super::material::linear_to_srgb(*v);
                }
            }
            changed |= texture.pixels.set_pixel(pixel, next);
        }
        changed
    }
}
pub(crate) fn pad_pixels(texture: &mut Texture, mut occupied: Vec<bool>) {
    let width = texture.width as usize;
    let height = texture.height as usize;
    let mut frontier = Vec::new();
    for i in 0..occupied.len() {
        if !occupied[i] {
            continue;
        }
        let (x, y) = (i % width, i / width);
        if x == 0
            || y == 0
            || x + 1 == width
            || y + 1 == height
            || !occupied[i - 1]
            || !occupied[i + 1]
            || !occupied[i - width]
            || !occupied[i + width]
        {
            frontier.push(i);
        }
    }
    let mut pixels = None;
    for _ in 0..4 {
        let mut next = Vec::new();
        for i in frontier {
            let (x, y) = (i % width, i / width);
            for j in [
                x.checked_sub(1).map(|x| y * width + x),
                (x + 1 < width).then_some(i + 1),
                y.checked_sub(1).map(|y| y * width + x),
                (y + 1 < height).then_some(i + width),
            ]
            .into_iter()
            .flatten()
            {
                if occupied[j] {
                    continue;
                }
                occupied[j] = true;
                next.push(j);
                let values = pixels.get_or_insert_with(|| texture.pixels.clone());
                let sample = values.pixel(i);
                values.set_pixel(j, sample);
            }
        }
        frontier = next;
    }
    if let Some(pixels) = pixels {
        texture.pixels = pixels;
    }
}
pub(crate) fn raster(
    t: [[f32; 2]; 3],
    width: usize,
    height: usize,
    mut visit: impl FnMut(usize, usize, [f32; 3]),
) {
    let size = [width as f32, height as f32];
    let mut lo = [0; 2];
    let mut hi = [0; 2];
    for k in 0..2 {
        lo[k] = (t.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min) * size[k] - 0.5)
            .floor()
            .max(0.) as usize;
        hi[k] = (t.iter().map(|p| p[k]).fold(f32::NEG_INFINITY, f32::max) * size[k] - 0.5)
            .ceil()
            .clamp(0., size[k] - 1.) as usize;
    }
    for y in lo[1]..=hi[1] {
        for x in lo[0]..=hi[0] {
            if let Some(w) =
                barycentric([(x as f32 + 0.5) / size[0], (y as f32 + 0.5) / size[1]], t)
            {
                visit(x, y, w);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exchange {
    pub path: String,
    pub baseline: u64,
}
impl Texture {
    pub fn fingerprint(&self) -> u64 {
        self.packed_pixels()
            .flatten()
            .fold((self.width as u64) << 32 | self.height as u64, |h, v| {
                (h ^ u64::from(v)).wrapping_mul(1099511628211)
            })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    #[test]
    fn spray_respects_zero_pressure_masks_and_occluding_surfaces() {
        let mut brush = Brush::new(super::super::Kind::Paint);
        brush.paint_effect.mode = super::super::PaintMode::Spray;
        brush.paint_effect.density = 1.;
        brush.paint_effect.dot_size = 0.08;
        brush.material = super::super::Material::from_srgb([1., 0., 0.], 0.5, 0.);
        brush.strength = 1.;
        let original = sheets();
        let mut mesh = original.clone();
        assert!(!mesh.paint_texture(
            Channel::Color,
            &brush,
            &[Dab {
                pressure: 0.,
                ..dab()
            }],
            &[],
            &[]
        ));
        mesh.mask.fill(1.);
        assert!(!mesh.paint_texture(Channel::Color, &brush, &[dab()], &[], &[]));
        assert_eq!(mesh.uv.textures[0].pixels, original.uv.textures[0].pixels);
        mesh.mask.fill(0.);
        assert!(mesh.paint_texture(Channel::Color, &brush, &[dab()], &[], &[]));
        for y in 0..64 {
            for x in 32..64 {
                assert_eq!(
                    mesh.uv.textures[0].pixels.pixel(y * 64 + x),
                    original.uv.textures[0].pixels.pixel(y * 64 + x),
                    "spray cannot reach the covered back sheet"
                );
            }
        }
    }
    #[test]
    fn float_texture_paint_preserves_hdr_outside_the_stroke() {
        let mut mesh = sheets();
        mesh.uv.textures[0].pixels =
            Pixels::Float32(Arc::new([-0.25, 4., 16., 1.].repeat(64 * 64)));
        let before = mesh.uv.textures[0].pixels.clone();
        let mut brush = Brush::new(super::super::Kind::Paint);
        brush.material = super::super::Material::from_srgb([1., 0., 0.], 0.5, 0.);
        brush.strength = 1.;
        assert!(mesh.paint_texture(Channel::Color, &brush, &[dab()], &[], &[]));
        assert_eq!(
            mesh.uv.textures[0].pixels.pixel(32 * 64 + 48),
            before.pixel(32 * 64 + 48)
        );
        assert_ne!(
            mesh.uv.textures[0].pixels.pixel(32 * 64 + 16),
            before.pixel(32 * 64 + 16)
        );
        assert!(
            mesh.uv.textures[0].pixels.pixel(32 * 64 + 16)[2] > 1.,
            "painting blends HDR values without clipping"
        );
        mesh.pad_texture(Channel::Color);
        assert_eq!(mesh.uv.textures[0].pixels.precision(), Precision::Float32);
    }
    pub(crate) fn sheets() -> Mesh {
        let mut mesh = Mesh::default();
        for z in [0., -0.001] {
            mesh.vertices.extend([
                Vec3(-1., -1., z),
                Vec3(1., -1., z),
                Vec3(1., 1., z),
                Vec3(-1., 1., z),
            ]);
        }
        mesh.triangles = vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]];
        mesh.mask = vec![0.; 8];
        mesh.uv.topology = mesh.triangles.clone();
        mesh.uv.corners = vec![
            [[0.05, 0.05], [0.45, 0.05], [0.45, 0.95]],
            [[0.05, 0.05], [0.45, 0.95], [0.05, 0.95]],
            [[0.55, 0.05], [0.95, 0.05], [0.95, 0.95]],
            [[0.55, 0.05], [0.95, 0.95], [0.55, 0.95]],
        ];
        mesh.uv.textures.push(
            Texture::new(
                Channel::Color,
                64,
                &super::super::Material::from_srgb([0.; 3], 0.5, 0.),
            )
            .unwrap(),
        );
        mesh
    }
    pub(crate) fn dab() -> Dab {
        Dab {
            center: Vec3::default(),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 2.,
            pressure: 0.5,
            delta: Vec3::default(),
            invert: false,
        }
    }
    #[test]
    fn removed_uv_faces_do_not_paint_but_still_occlude_other_faces() {
        let mut mesh = sheets();
        mesh.uv.remove_islands(&[0, 1]).unwrap();
        let before = mesh.uv.textures[0].pixels.clone();
        let mut brush = Brush::new(super::super::Kind::Paint);
        brush.material = super::super::Material::from_srgb([1., 0., 0.], 0.5, 0.);
        brush.strength = 1.;
        assert!(!mesh.paint_texture(Channel::Color, &brush, &[dab()], &[], &[]));
        assert_eq!(mesh.uv.textures[0].pixels, before);
        mesh.hidden_faces = vec![true, true, false, false];
        assert!(mesh.paint_texture(Channel::Color, &brush, &[dab()], &[], &[]));
    }
    #[test]
    fn texture_stroke_respects_occlusion_masks_pressure_and_hidden_faces() {
        let mut mesh = sheets();
        let before = mesh.uv.textures[0].pixels.clone();
        let mut brush = Brush::new(super::super::Kind::Paint);
        brush.material = super::super::Material::from_srgb([1., 0., 0.], 0.5, 0.);
        brush.strength = 1.;
        brush.pressure = true;
        assert!(mesh.paint_texture(Channel::Color, &brush, &[dab()], &[], &[]));
        let pixels = mesh.uv.textures[0].pixels.u16();
        let front = (32 * 64 + 16) * 4;
        let back = (32 * 64 + 48) * 4;
        assert!(pixels[front] > 10000 && pixels[front] < 65535);
        assert_eq!(&pixels[back..back + 4], &before.u16()[back..back + 4]);
        let mut barrier = sheets();
        for p in &mut barrier.vertices {
            p.2 += 0.01;
        }
        let mut blocked = sheets();
        assert!(!blocked.paint_texture(Channel::Color, &brush, &[dab()], &[], &[&barrier]));
        let mut protected = sheets();
        protected.mask.fill(1.);
        assert!(!protected.paint_texture(Channel::Color, &brush, &[dab()], &[], &[]));
        assert_eq!(protected.uv.textures[0].pixels, before);
        let mut hidden = sheets();
        hidden.hidden_faces = vec![true, true, false, false];
        assert!(hidden.paint_texture(Channel::Color, &brush, &[dab()], &[], &[]));
        assert_eq!(hidden.uv.textures[0].pixels.u16()[front], 0);
        assert!(hidden.uv.textures[0].pixels.u16()[back] > 0);
        mesh.pad_texture(Channel::Color);
        assert!(
            mesh.uv.textures[0].pixels.u16()[(32 * 64 + 2) * 4] > 0,
            "gutter extends painted edge"
        );
        assert_eq!(
            &mesh.uv.textures[0].pixels.u16()[back..back + 4],
            &before.u16()[back..back + 4]
        );
    }
}

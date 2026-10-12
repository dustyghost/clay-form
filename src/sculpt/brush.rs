use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
    Snake,
    Detail,
}
impl Kind {
    pub const ALL: [Self; 11] = [
        Self::Standard,
        Self::Clay,
        Self::Inflate,
        Self::Smooth,
        Self::Flatten,
        Self::Pinch,
        Self::Crease,
        Self::Grab,
        Self::Mask,
        Self::Snake,
        Self::Detail,
    ];
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Alpha {
    Soft,
    Hard,
    Square,
    Image {
        width: usize,
        height: usize,
        pixels: Arc<[f32]>,
    },
}
impl PartialEq for Alpha {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Soft, Self::Soft) | (Self::Hard, Self::Hard) | (Self::Square, Self::Square) => {
                true
            }
            (
                Self::Image {
                    width: a,
                    height: b,
                    pixels: c,
                },
                Self::Image {
                    width: x,
                    height: y,
                    pixels: z,
                },
            ) => a == x && b == y && (Arc::ptr_eq(c, z) || c.as_ref() == z.as_ref()),
            _ => false,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct BrushIcon {
    pub size: usize,
    pub rgba: Arc<[u8]>,
}
impl BrushIcon {
    pub fn validate(&self) -> Result<(), String> {
        if self.size == 0 || self.size > 256 || self.rgba.len() != self.size * self.size * 4 {
            return Err("Invalid brush icon (maximum 256 × 256)".into());
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct IconFile {
    size: usize,
    png: Vec<u8>,
}
impl Serialize for BrushIcon {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use image::ImageEncoder;
        self.validate().map_err(serde::ser::Error::custom)?;
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new_with_quality(
            &mut png,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        )
        .write_image(
            &self.rgba,
            self.size as u32,
            self.size as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(serde::ser::Error::custom)?;
        IconFile {
            size: self.size,
            png,
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for BrushIcon {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let file = IconFile::deserialize(deserializer)?;
        if file.size == 0 || file.size > 256 || file.png.len() > 1024 * 1024 {
            return Err(serde::de::Error::custom(
                "Invalid brush icon dimensions or data",
            ));
        }
        let mut reader = image::ImageReader::with_format(
            std::io::Cursor::new(file.png),
            image::ImageFormat::Png,
        );
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(256);
        limits.max_image_height = Some(256);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(serde::de::Error::custom)?
            .to_rgba8();
        if image.width() != file.size as u32 || image.height() != file.size as u32 {
            return Err(serde::de::Error::custom(
                "Brush icon size does not match image",
            ));
        }
        Ok(Self {
            size: file.size,
            rgba: image.into_raw().into(),
        })
    }
}
fn guid(bytes: [u8; 16]) -> String {
    let h = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}
fn content_guid(data: &[u8]) -> String {
    let mut a = 0xcbf29ce484222325u64;
    let mut b = 0x84222325cbf29ce4u64;
    for &v in data {
        a = (a ^ v as u64).wrapping_mul(0x100000001b3);
        b = (b ^ (v as u64 + 1)).wrapping_mul(0x100000001b3);
    }
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&a.to_le_bytes());
    bytes[8..].copy_from_slice(&b.to_le_bytes());
    bytes[6] = (bytes[6] & 15) | 0x80;
    bytes[8] = (bytes[8] & 63) | 0x80;
    guid(bytes)
}
pub(crate) fn fresh_brush_id() -> String {
    use std::{
        hash::{BuildHasher, Hasher},
        io::Read,
        sync::atomic::{AtomicU64, Ordering},
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut bytes = [0; 16];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_err()
    {
        for chunk in bytes.chunks_mut(8) {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            );
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }
    }
    bytes[6] = (bytes[6] & 15) | 0x40;
    bytes[8] = (bytes[8] & 63) | 0x80;
    guid(bytes)
}
fn zero_radius(value: &f32) -> bool {
    *value == 0.
}
fn disabled_cursor(value: &bool) -> bool {
    !*value
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Brush {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<BrushIcon>,
    pub name: String,
    pub kind: Kind,
    pub alpha: Alpha,
    pub radius: f32,
    pub strength: f32,
    pub falloff: f32,
    #[serde(default, skip_serializing_if = "zero_radius")]
    pub inner_radius: f32,
    #[serde(default, skip_serializing_if = "disabled_cursor")]
    pub surface_cursor: bool,
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
            id: Some(content_guid(
                format!("clay-form/default-brush/{kind:?}").as_bytes(),
            )),
            icon: None,
            name: format!("{kind:?}"),
            kind,
            alpha: Alpha::Soft,
            radius: 60.,
            strength: match kind {
                Kind::Snake => 1.,
                Kind::Detail => 0.35,
                _ => 0.25,
            },
            falloff: 1.5,
            inner_radius: 0.,
            surface_cursor: false,
            spacing: if kind == Kind::Detail { 0.08 } else { 0.15 },
            rotation: 0.,
            subtract: false,
            pressure: true,
            pressure_radius: false,
            pressure_curve: 1.,
        }
    }
    pub fn identity(&self) -> String {
        if let Some(id) = &self.id {
            return id.to_ascii_lowercase();
        }
        let mut default = Self::new(self.kind);
        let id = default.id.take().unwrap();
        if *self == default {
            return id;
        }
        content_guid(&serde_json::to_vec(self).expect("brush serialization"))
    }
    pub fn ensure_identity(&mut self) {
        self.id = Some(self.identity());
    }
    pub(crate) fn renew_identity(&mut self) {
        self.id = Some(fresh_brush_id());
    }
    pub(crate) fn snake_weight(&self, dab: super::Dab, offset: super::Vec3) -> f32 {
        let r = offset.len() / dab.radius;
        // Keep a rigid core: repeatedly attenuating every off-center vertex
        // pinches a long drag down to a single needle, even after remeshing.
        let t = ((1. - r) / (0.65 * (1. - self.inner_radius)).max(0.0001)).clamp(0., 1.);
        let volume = t * t * (3. - 2. * t);
        match &self.alpha {
            Alpha::Soft => volume.powf(self.falloff),
            _ => {
                volume
                    * self.weight(
                        offset.dot(dab.right) / dab.radius,
                        offset.dot(dab.up) / dab.radius,
                    )
            }
        }
    }
    pub fn snake_motion(&self, dab: super::Dab) -> super::Vec3 {
        let gain = self.strength
            * if self.pressure {
                dab.pressure.powf(self.pressure_curve)
            } else {
                1.
            }
            * self.weight(0., 0.);
        let sign = if self.subtract ^ dab.invert { -1. } else { 1. };
        (dab.delta + dab.normal * (dab.delta.len() * 0.12)) * (gain * sign)
    }
    pub fn weight(&self, x: f32, y: f32) -> f32 {
        let (s, c) = self.rotation.to_radians().sin_cos();
        let (x, y) = (x * c - y * s, x * s + y * c);
        let r = (x * x + y * y).sqrt();
        if r >= 1. {
            return 0.;
        }
        let fade = ((1. - r) / (1. - self.inner_radius).max(0.0001))
            .clamp(0., 1.)
            .powf(self.falloff);
        match &self.alpha {
            Alpha::Soft => fade,
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
                let px = (x + 1.) * 0.5 * (*width - 1) as f32;
                let py = (1. - y) * 0.5 * (*height - 1) as f32;
                let ix = px.floor() as usize;
                let iy = py.floor() as usize;
                let nx = (ix + 1).min(*width - 1);
                let ny = (iy + 1).min(*height - 1);
                let tx = px - ix as f32;
                let ty = py - iy as f32;
                let top = pixels[iy * width + ix] * (1. - tx) + pixels[iy * width + nx] * tx;
                let bottom = pixels[ny * width + ix] * (1. - tx) + pixels[ny * width + nx] * tx;
                (top * (1. - ty) + bottom * ty) * fade
            }
        }
    }
}
pub fn validate_brush(b: &Brush) -> Result<(), String> {
    if b.name.len() > 256
        || !b.radius.is_finite()
        || !(1. ..=500.).contains(&b.radius)
        || !b.strength.is_finite()
        || !(0. ..=1.).contains(&b.strength)
        || !b.inner_radius.is_finite()
        || !(0. ..=1.).contains(&b.inner_radius)
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
    if let Some(id) = &b.id
        && (id.len() != 36
            || id.bytes().enumerate().any(|(i, c)| {
                if [8, 13, 18, 23].contains(&i) {
                    c != b'-'
                } else {
                    !c.is_ascii_hexdigit()
                }
            }))
    {
        return Err("Invalid brush identity".into());
    }
    if let Some(icon) = &b.icon {
        icon.validate()?;
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
mod footprint_tests {
    use super::*;
    #[test]
    fn inner_radius_preserves_alpha_and_fades_outside_core() {
        let mut brush = Brush::new(Kind::Standard);
        let old = brush.weight(0.25, 0.);
        brush.inner_radius = 0.5;
        assert_eq!(brush.weight(0.25, 0.), 1.);
        assert!(brush.weight(0.75, 0.) < 1.);
        assert_eq!(brush.weight(1., 0.), 0.);
        brush.alpha = Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![0.2; 4].into(),
        };
        assert!((brush.weight(0.25, 0.) - 0.2).abs() < 1e-6);
        brush.inner_radius = 0.;
        brush.alpha = Alpha::Soft;
        assert_eq!(brush.weight(0.25, 0.), old);
        let mut json = serde_json::to_value(&brush).unwrap();
        json.as_object_mut().unwrap().remove("inner_radius");
        json.as_object_mut().unwrap().remove("surface_cursor");
        let legacy: Brush = serde_json::from_value(json).unwrap();
        assert_eq!(legacy.inner_radius, 0.);
        assert!(!legacy.surface_cursor);
        brush.inner_radius = f32::NAN;
        assert!(validate_brush(&brush).is_err());
    }
}

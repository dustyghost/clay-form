//! Project-owned screen references. Image bytes are shared and embedded, never linked.
use serde::{Deserialize, Serialize};
use std::{io::Read, sync::Arc};

pub const MAX_REFERENCES: usize = 8;
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_PIXELS: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ReferencePixels {
    pub(crate) size: [usize; 2],
    pub(crate) rgba: Arc<[u8]>,
    png: Arc<[u8]>,
}
impl PartialEq for ReferencePixels {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size && (Arc::ptr_eq(&self.rgba, &other.rgba) || self.rgba == other.rgba)
    }
}
impl Serialize for ReferencePixels {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.png.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for ReferencePixels {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        Self::decode(&bytes).map_err(serde::de::Error::custom)
    }
}
impl ReferencePixels {
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        use image::{ImageDecoder, ImageEncoder};
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Reference image exceeds 16 MB".into());
        }
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        if !matches!(
            reader.format(),
            Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ) {
            return Err("Choose a PNG or JPEG reference image".into());
        }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(2048);
        limits.max_image_height = Some(2048);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let mut decoder = reader
            .into_decoder()
            .map_err(|e| format!("Could not load reference (maximum 2048 × 2048): {e}"))?;
        let orientation = decoder.orientation().map_err(|e| e.to_string())?;
        let mut image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
        image.apply_orientation(orientation);
        let image = image.to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
        if png.len() > MAX_FILE_BYTES {
            return Err("Embedded reference exceeds 16 MB".into());
        }
        Ok(Self {
            size,
            rgba: image.into_raw().into(),
            png: png.into(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    /// Fractions of the viewport, independent of camera zoom and pan.
    pub center: [f32; 2],
    pub width: f32,
    pub opacity: f32,
    pub visible: bool,
    pub locked: bool,
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            center: [0.2, 0.35],
            width: 0.3,
            opacity: 1.,
            visible: true,
            locked: false,
        }
    }
}
impl Placement {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self
            .center
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || !self.width.is_finite()
            || !(0.05..=0.9).contains(&self.width)
            || !self.opacity.is_finite()
            || !(0.05..=1.).contains(&self.opacity)
        {
            return Err("Invalid reference image placement or opacity".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceImage {
    pub name: String,
    pub pixels: ReferencePixels,
    pub placement: Placement,
}
impl ReferenceImage {
    pub(crate) fn from_bytes(name: &str, bytes: &[u8]) -> Result<Self, String> {
        let name: String = name.chars().filter(|c| !c.is_control()).take(32).collect();
        Ok(Self {
            name: if name.trim().is_empty() {
                "Reference".into()
            } else {
                name
            },
            pixels: ReferencePixels::decode(bytes)?,
            placement: Placement::default(),
        })
    }
    pub(crate) fn load(path: &std::path::Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Choose a regular PNG or JPEG file".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::from_bytes(
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Reference"),
            &bytes,
        )
    }
}
pub(crate) fn validate(images: &[ReferenceImage]) -> Result<(), String> {
    if images.len() > MAX_REFERENCES {
        return Err("Keep up to 8 reference images per project".into());
    }
    let mut pixels = 0;
    let mut bytes = 0;
    for image in images {
        image.placement.validate()?;
        if image.name.trim().is_empty() || image.name.len() > 128 {
            return Err("Invalid reference image name".into());
        }
        pixels += image.pixels.size[0] * image.pixels.size[1];
        bytes += image.pixels.png.len();
        if pixels > MAX_PIXELS || bytes > MAX_FILE_BYTES {
            return Err(
                "Reference images exceed the project budget (8 megapixels / 16 MB embedded)".into(),
            );
        }
    }
    Ok(())
}
pub(crate) fn deserialize_list<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<ReferenceImage>, D::Error> {
    struct Images;
    impl<'de> serde::de::Visitor<'de> for Images {
        type Value = Vec<ReferenceImage>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("up to 8 reference images")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut images = Vec::new();
            while let Some(image) = seq.next_element()? {
                images.push(image);
                validate(&images).map_err(serde::de::Error::custom)?;
            }
            Ok(images)
        }
    }
    d.deserialize_seq(Images)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn image() -> ReferenceImage {
        let image = image::RgbaImage::from_fn(120, 160, |x, y| {
            let dx = x as f32 - 60.;
            let dy = y as f32 - 75.;
            if dx * dx / 1600. + dy * dy / 3600. < 1. {
                image::Rgba([75, 135, 165, 255])
            } else {
                image::Rgba([230, 190, 120, 255])
            }
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        ReferenceImage::from_bytes("Front study", bytes.get_ref()).unwrap()
    }
    #[test]
    fn reference_codec_handles_png_jpeg_transparency_and_invalid_data() {
        let rgba = image::RgbaImage::from_pixel(2, 3, image::Rgba([30, 80, 160, 90]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        rgba.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let image = ReferenceImage::from_bytes("透明", bytes.get_ref()).unwrap();
        assert_eq!(image.pixels.size, [2, 3]);
        assert_eq!(&image.pixels.rgba[..4], &[30, 80, 160, 90]);
        let restored: ReferenceImage =
            serde_json::from_slice(&serde_json::to_vec(&image).unwrap()).unwrap();
        assert_eq!(restored, image);
        let rgb = image::RgbImage::from_pixel(4, 2, image::Rgb([70, 90, 180]));
        let mut jpeg = std::io::Cursor::new(Vec::new());
        rgb.write_to(&mut jpeg, image::ImageFormat::Jpeg).unwrap();
        // Little-endian TIFF: one Orientation=6 (rotate 90 degrees) entry.
        use image::ImageEncoder;
        let exif = vec![
            73, 73, 42, 0, 8, 0, 0, 0, 1, 0, 18, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut rotated = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut rotated);
        encoder.set_exif_metadata(exif).unwrap();
        encoder
            .write_image(rgb.as_raw(), 4, 2, image::ExtendedColorType::Rgb8)
            .unwrap();
        let oriented = ReferenceImage::from_bytes("phone photo", &rotated).unwrap();
        assert_eq!(oriented.pixels.size, [2, 4]);
        let roundtrip: ReferenceImage =
            serde_json::from_slice(&serde_json::to_vec(&oriented).unwrap()).unwrap();
        assert_eq!(
            roundtrip, oriented,
            "embedded pixels must not rotate twice on reopen"
        );
        assert_eq!(
            ReferenceImage::from_bytes("jpeg", jpeg.get_ref())
                .unwrap()
                .pixels
                .size,
            [4, 2]
        );
        assert!(ReferencePixels::decode(b"broken PNG").is_err());
        assert!(ReferencePixels::decode(&vec![0; MAX_FILE_BYTES + 1]).is_err());
        let big = image::RgbaImage::new(2049, 1);
        let mut bytes = std::io::Cursor::new(Vec::new());
        big.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        assert!(ReferencePixels::decode(bytes.get_ref()).is_err());
    }
    #[test]
    fn reference_project_validation_and_undo_preserve_independent_images() {
        let mut doc = crate::document::Document::default();
        doc.duplicate_model(false).unwrap();
        doc.undo();
        let history = doc.history_counts();
        let before = doc.geometry_snapshot();
        doc.mark_saved("original.clay");
        doc.add_reference(image()).unwrap();
        assert!(doc.is_dirty());
        assert_eq!(doc.geometry_snapshot(), before);
        assert_eq!(doc.history_counts(), history);
        doc.redo();
        assert_eq!(doc.references().len(), 1);
        let project = doc.project();
        let loaded = serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
        doc.replace_project(loaded, "loaded.clay").unwrap();
        assert!(doc.project() == project);
        assert!(!doc.is_dirty());
        let mut invalid = doc.project();
        invalid.references[0].placement.width = f32::NAN;
        assert!(doc.replace_project(invalid, "invalid.clay").is_err());
        assert!(doc.project() == project);
        assert_eq!(doc.current_path(), "loaded.clay");
        for _ in 1..MAX_REFERENCES {
            doc.add_reference(image()).unwrap();
        }
        assert!(doc.add_reference(image()).is_err());
        let mut oversized = doc.project();
        oversized.references.push(image());
        assert!(
            serde_json::from_slice::<crate::sculpt::Project>(
                &serde_json::to_vec(&oversized).unwrap()
            )
            .is_err()
        );
        doc.new_sphere();
        assert!(doc.references().is_empty());
        assert!(!doc.is_dirty());
    }
}

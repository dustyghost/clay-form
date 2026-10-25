//! Bounded PNG/JPEG/EXR decoding and precision-preserving image exchange.
use crate::sculpt::texture::{Channel, Pixels, Precision, Texture};
use image::{ImageDecoder, ImageEncoder};
use std::{io::Read, path::Path, sync::Arc};
pub fn load(path: &str, channel: Channel) -> Result<Texture, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open texture: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Choose a regular texture image".into());
    }
    let mut bytes = Vec::new();
    file.take(320 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 320 * 1024 * 1024 {
        return Err("Texture image exceeds 320 MB".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::OpenExr)
    ) {
        return Err("Choose PNG, JPEG or a single-layer RGB/RGBA EXR image".into());
    }
    let float = reader.format() == Some(image::ImageFormat::OpenExr);
    if float {
        validate_exr(&bytes, channel)?;
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(crate::sculpt::texture::MAX_SIZE);
    limits.max_image_height = Some(crate::sculpt::texture::MAX_SIZE);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let profile = decoder
        .icc_profile()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    let (width, height) = (image.width(), image.height());
    let pixels = if float {
        let mut pixels = image.to_rgba32f().into_raw();
        // EXR RGB(A) uses associated alpha, including numeric maps with coverage.
        {
            for p in pixels.as_chunks_mut::<4>().0 {
                if p[3] > 0. {
                    for c in 0..3 {
                        p[c] /= p[3];
                    }
                }
            }
        }
        Pixels::Float32(Arc::new(pixels))
    } else {
        let mut pixels = image.to_rgba16().into_raw();
        if channel == Channel::Color && !profile.is_empty() {
            pixels = convert_profile(&pixels, &profile, true)?;
        }
        Pixels::U16(Arc::new(pixels))
    };
    let asset = Texture {
        id: Texture::new_id(),
        name: Path::new(path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(60)
            .collect(),
        channel,
        width,
        height,
        pixels,
        profile,
        exchange: None,
    };
    asset.validate()?;
    Ok(asset)
}
fn validate_exr(bytes: &[u8], channel: Channel) -> Result<(), String> {
    let metadata =
        exr::meta::MetaData::read_from_unbuffered(bytes, false).map_err(|e| e.to_string())?;
    if metadata.headers.len() != 1 {
        return Err("Flatten EXR to one RGB/RGBA layer before importing".into());
    }
    let header = &metadata.headers[0];
    if header
        .channels
        .list
        .iter()
        .any(|c| c.sample_type == exr::meta::attribute::SampleType::U32)
    {
        return Err(
            "Use half- or full-float EXR samples; integer EXR channels are unsupported".into(),
        );
    }
    if header.layer_size != header.shared_attributes.display_window.size
        || header.own_attributes.layer_position != header.shared_attributes.display_window.position
    {
        return Err("Flatten the EXR data window to its display window before importing".into());
    }

    if header.deep
        || header
            .channels
            .list
            .iter()
            .any(|c| !["R", "G", "B", "A"].contains(&c.name.to_string().as_str()))
    {
        return Err(
            "Import a non-deep RGB/RGBA EXR; arbitrary AOV channels are not supported".into(),
        );
    }
    if channel == Channel::Color
        && let Some(c) = header.shared_attributes.chromaticities
    {
        let actual = [
            c.red.0, c.red.1, c.green.0, c.green.1, c.blue.0, c.blue.1, c.white.0, c.white.1,
        ];
        let expected = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];
        if actual
            .iter()
            .zip(expected)
            .any(|(a, b)| (a - b).abs() > 0.0001)
        {
            return Err("Convert this EXR to linear Rec.709/sRGB primaries before importing; ACEScg and other primaries are not converted automatically".into());
        }
    }
    Ok(())
}
fn convert_profile(pixels: &[u16], icc: &[u8], to_srgb: bool) -> Result<Vec<u16>, String> {
    let profile = moxcms::ColorProfile::new_from_slice(icc)
        .map_err(|e| format!("Unsupported colour profile: {e}"))?;
    let srgb = moxcms::ColorProfile::new_srgb();
    let (source, target) = if to_srgb {
        (&profile, &srgb)
    } else {
        (&srgb, &profile)
    };
    let transform = source
        .create_transform_16bit(
            moxcms::Layout::Rgba,
            target,
            moxcms::Layout::Rgba,
            Default::default(),
        )
        .map_err(|e| format!("Unsupported RGB colour profile: {e}"))?;
    let mut output = vec![0; pixels.len()];
    transform
        .transform(pixels, &mut output)
        .map_err(|e| e.to_string())?;
    Ok(output)
}
pub fn png(texture: &Texture) -> Result<Vec<u8>, String> {
    texture.validate()?;
    if texture.pixels.precision() == Precision::Float32 {
        return Err("Export float textures as .exr to retain HDR and negative values".into());
    }
    let pixels = if texture.channel == Channel::Color && !texture.profile.is_empty() {
        convert_profile(texture.pixels.u16(), &texture.profile, false)?
    } else {
        texture.pixels.u16().to_vec()
    };
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    if texture.channel == Channel::Color && !texture.profile.is_empty() {
        encoder
            .set_icc_profile(texture.profile.clone())
            .map_err(|e| e.to_string())?;
    }
    encoder
        .write_image(
            bytemuck::cast_slice(&pixels),
            texture.width,
            texture.height,
            image::ExtendedColorType::Rgba16,
        )
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}
pub fn save(path: &Path, texture: &Texture) -> Result<(), String> {
    save_impl(path, texture, false)
}
/// GIMP's native EXR loader reads straight RGB, while its GEGL exporter writes
/// associated RGB. Keep this adaptation confined to outgoing GIMP exchanges.
pub(crate) fn save_gimp(path: &Path, texture: &Texture) -> Result<(), String> {
    save_impl(path, texture, true)
}
fn save_impl(path: &Path, texture: &Texture, gimp_straight: bool) -> Result<(), String> {
    let is_exr = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exr"));
    if !is_exr {
        return super::write_bytes(path, &png(texture)?);
    }
    texture.validate()?;
    let mut pixels = Vec::with_capacity(texture.pixels.len());
    for i in 0..texture.pixels.len() / 4 {
        let mut p = texture.linear_pixel(i);
        if !gimp_straight && p[3] > 0. {
            for c in 0..3 {
                p[c] *= p[3];
            }
        }
        pixels.extend_from_slice(&p);
    }
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::codecs::openexr::OpenExrEncoder::new(&mut bytes)
        .write_image(
            bytemuck::cast_slice(&pixels),
            texture.width,
            texture.height,
            image::ExtendedColorType::Rgba32F,
        )
        .map_err(|e| e.to_string())?;
    super::write_bytes(path, bytes.get_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_exr_preserves_hdr_negative_values_alpha_and_half_input() {
        let mut texture = Texture::new_with_precision(
            Channel::Color,
            16,
            &Default::default(),
            Precision::Float32,
        )
        .unwrap();
        texture.pixels = Pixels::Float32(Arc::new([-0.25, 4., 16.25, 0.5].repeat(256)));
        let path = std::env::temp_dir().join(format!("clay-float-{}.exr", texture.id));
        save(&path, &texture).unwrap();
        let loaded = load(path.to_str().unwrap(), Channel::Color).unwrap();
        assert_eq!(loaded.pixels, texture.pixels);
        assert_eq!(loaded.pixels.precision(), Precision::Float32);
        save_gimp(&path, &texture).unwrap();
        let outgoing = image::open(&path).unwrap().to_rgba32f();
        assert_eq!(
            &outgoing.as_raw()[..4],
            &texture.pixels.pixel(0),
            "GIMP receives straight RGB with the original alpha"
        );
        assert!(
            png(&texture).is_err(),
            "PNG export must not silently clip float pixels"
        );
        let bytes = serde_json::to_vec(&texture).unwrap();
        let restored: Texture = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.pixels, texture.pixels);
        let mut data = texture.clone();
        data.channel = Channel::Roughness;
        data.pixels = Pixels::Float32(Arc::new([-0.5, 65536., 0.000123, 0.25].repeat(256)));
        save(&path, &data).unwrap();
        assert_eq!(
            load(path.to_str().unwrap(), Channel::Roughness)
                .unwrap()
                .pixels,
            data.pixels
        );
        use exr::prelude::{f16, write_rgba_file};
        write_rgba_file(&path, 16, 16, |_, _| {
            (
                f16::from_f32(-0.125),
                f16::from_f32(2.),
                f16::from_f32(8.125),
                f16::from_f32(0.5),
            )
        })
        .unwrap();
        assert_eq!(
            load(path.to_str().unwrap(), Channel::Color).unwrap().pixels,
            texture.pixels
        );
        texture.pixels.set_pixel(0, [f32::NAN, 0., 0., 1.]);
        assert!(texture.validate().is_err());
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn rgba16_data_map_and_alpha_survive_png_exchange() {
        let mut texture = Texture::new(Channel::Roughness, 16, &Default::default()).unwrap();
        texture.pixels.u16_mut()[..8]
            .copy_from_slice(&[12345, 23456, 34567, 45678, 1, 65534, 101, 0]);
        let path = std::env::temp_dir().join(format!("clay-texture-{}.png", texture.id));
        save(&path, &texture).unwrap();
        let loaded = load(path.to_str().unwrap(), Channel::Roughness).unwrap();
        assert_eq!(loaded.pixels, texture.pixels);
        assert_eq!((loaded.width, loaded.height), (16, 16));
        let _ = std::fs::remove_file(path);
    }
    #[test]
    #[ignore = "manual GIMP interoperability fixture; run scripts/validate-textures-gimp.py in GIMP first"]
    fn gimp_texture_colour_alpha_and_data_precision_review() {
        let data = load("/tmp/clay-gimp-data.png", Channel::Roughness).unwrap();
        assert_eq!((data.width, data.height), (32, 32));
        for (actual, expected) in data.pixels.u16()[..4]
            .iter()
            .zip([12345_u16, 23456, 34567, 45678])
        {
            assert!(
                actual.abs_diff(expected) <= 1,
                "data-map precision: {actual} != {expected}"
            );
        }
        let original_colour = load("/tmp/clay-gimp-data.png", Channel::Color).unwrap();
        assert_eq!(original_colour.pixels.u16()[3], 45678);
        let colour = load("/tmp/clay-gimp-edited.png", Channel::Color).unwrap();
        for (actual, expected) in colour.pixels.u16()[..4].iter().zip([
            crate::sculpt::material::linear_to_srgb(0.8),
            crate::sculpt::material::linear_to_srgb(0.1),
            crate::sculpt::material::linear_to_srgb(0.2),
            1.,
        ]) {
            assert!(
                (*actual as f32 / 65535. - expected).abs() < 0.005,
                "colour/alpha: {actual} != {expected}"
            );
        }
        assert!(
            !colour.profile.is_empty(),
            "the GIMP fixture exercises an embedded ICC profile"
        );
        let path = std::env::temp_dir().join("clay-gimp-profile-return.png");
        save(&path, &colour).unwrap();
        let returned = load(path.to_str().unwrap(), Channel::Color).unwrap();
        assert_eq!(returned.profile, colour.profile);
        assert!(
            returned
                .pixels
                .u16()
                .iter()
                .zip(colour.pixels.u16().iter())
                .all(|(a, b)| a.abs_diff(*b) <= 4)
        );
    }

    #[test]
    #[ignore = "manual GIMP EXR fixture; run scripts/validate-textures-gimp-exr.py in GIMP first"]
    fn gimp_texture_float_exr_review() {
        let image = load("/tmp/clay-gimp-float-return.exr", Channel::Color).unwrap();
        assert_eq!(image.pixels.precision(), Precision::Float32);
        for (actual, expected) in image
            .pixels
            .pixel(0)
            .into_iter()
            .zip([-0.25, 4.5, 0.12345678, 0.5])
        {
            assert!(
                (actual - expected).abs() < 0.000001,
                "GIMP float return: {actual} != {expected}"
            );
        }
        let metadata =
            exr::meta::MetaData::read_from_file("/tmp/clay-gimp-float-return.exr", false).unwrap();
        assert!(
            metadata.headers[0]
                .channels
                .list
                .iter()
                .all(|c| c.sample_type == exr::meta::attribute::SampleType::F32)
        );
    }
}

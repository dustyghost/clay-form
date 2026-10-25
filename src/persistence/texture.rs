//! Bounded image decoding and 16-bit PNG exchange; image updates commit after validation.
use crate::sculpt::texture::{Channel, Texture};
use image::{ImageDecoder, ImageEncoder};
use std::{io::Read, path::Path, sync::Arc};
pub fn load(path: &str, channel: Channel) -> Result<Texture, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open texture: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Choose a regular texture image".into());
    }
    let mut bytes = Vec::new();
    file.take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Texture image exceeds 32 MB".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(
            "Choose a PNG or JPEG image; export layered GIMP documents to PNG first".into(),
        );
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let profile = decoder
        .icc_profile()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    let image = image.to_rgba16();
    let (width, height) = image.dimensions();
    let mut pixels = image.into_raw();
    if channel == Channel::Color && !profile.is_empty() {
        pixels = convert_profile(&pixels, &profile, true)?;
    }
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
        pixels: Arc::new(pixels),
        profile,
        exchange: None,
    };
    asset.validate()?;
    Ok(asset)
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
    let pixels = if texture.channel == Channel::Color && !texture.profile.is_empty() {
        convert_profile(&texture.pixels, &texture.profile, false)?
    } else {
        texture.pixels.as_ref().clone()
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
    super::write_bytes(path, &png(texture)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgba16_data_map_and_alpha_survive_png_exchange() {
        let mut texture = Texture::new(Channel::Roughness, 16, &Default::default()).unwrap();
        Arc::make_mut(&mut texture.pixels)[..8]
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
        for (actual, expected) in data.pixels[..4]
            .iter()
            .zip([12345_u16, 23456, 34567, 45678])
        {
            assert!(
                actual.abs_diff(expected) <= 1,
                "data-map precision: {actual} != {expected}"
            );
        }
        let original_colour = load("/tmp/clay-gimp-data.png", Channel::Color).unwrap();
        assert_eq!(original_colour.pixels[3], 45678);
        let colour = load("/tmp/clay-gimp-edited.png", Channel::Color).unwrap();
        for (actual, expected) in colour.pixels[..4].iter().zip([
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
                .iter()
                .zip(colour.pixels.iter())
                .all(|(a, b)| a.abs_diff(*b) <= 4)
        );
    }
}

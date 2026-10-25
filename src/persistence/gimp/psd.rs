//! Minimal layered RGB PSD writer (16-bit integer / 32-bit float).
//! Layout follows Adobe's Photoshop File Formats Specification.
use crate::sculpt::texture::{Pixels, Texture};
use std::io::Write;
pub(super) const GUIDE_NAME: &str = "Clay UV guide - excluded on reload";

fn u16be(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_be_bytes());
}
fn u32be(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_be_bytes());
}
fn block(out: &mut Vec<u8>, data: &[u8]) {
    u32be(out, data.len() as u32);
    out.extend(data);
}
fn tag(out: &mut Vec<u8>, key: &[u8; 4], data: &[u8]) {
    out.extend(b"8BIM");
    out.extend(key);
    block(out, data);
    if !data.len().is_multiple_of(2) {
        out.push(0);
    }
}

pub(super) fn encode(
    texture: &Texture,
    guide: &image::RgbaImage,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u8>, String> {
    texture.validate()?;
    let float = matches!(texture.pixels, Pixels::Float32(_));
    let depth = if float { 32 } else { 16 };
    let count = (texture.width * texture.height) as usize;
    let mut channels = Vec::new();
    for is_guide in [false, true] {
        for c in [3, 0, 1, 2] {
            let mut bytes = Vec::with_capacity(count * depth as usize / 8 + 2);
            u16be(&mut bytes, 0); // raw planar samples
            for i in 0..count {
                if i.is_multiple_of(65536) && cancel() {
                    return Err("External paint exchange cancelled".into());
                }
                if is_guide {
                    let v = guide.as_raw()[i * 4 + c] as f32 / 255.;
                    if float {
                        let v = if c == 3 {
                            v
                        } else {
                            crate::sculpt::material::srgb_to_linear(v)
                        };
                        bytes.extend(v.to_be_bytes());
                    } else {
                        bytes.extend(((v * 65535.).round() as u16).to_be_bytes());
                    }
                } else {
                    match &texture.pixels {
                        Pixels::U16(p) => bytes.extend(p[i * 4 + c].to_be_bytes()),
                        Pixels::Float32(p) => bytes.extend(p[i * 4 + c].to_be_bytes()),
                    }
                }
            }
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(&bytes[2..]).map_err(|e| e.to_string())?;
            let compressed = encoder.finish().map_err(|e| e.to_string())?;
            let mut encoded = vec![0, 2];
            encoded.extend(compressed);
            channels.push(encoded);
        }
    }
    let mut info = Vec::new();
    u16be(&mut info, (-2i16) as u16); // merged alpha is transparency
    for (layer, name) in ["Texture paint", GUIDE_NAME].into_iter().enumerate() {
        u32be(&mut info, 0);
        u32be(&mut info, 0);
        u32be(&mut info, texture.height);
        u32be(&mut info, texture.width);
        u16be(&mut info, 4);
        for (channel, id) in [-1i16, 0, 1, 2].into_iter().enumerate() {
            u16be(&mut info, id as u16);
            u32be(&mut info, channels[layer * 4 + channel].len() as u32);
        }
        info.extend(b"8BIMnorm");
        info.extend([255, 0, 0, 0]);
        let mut extra = vec![0; 8]; // no mask or blending ranges
        extra.push(name.len() as u8);
        extra.extend(name.as_bytes());
        while !extra.len().is_multiple_of(4) {
            extra.push(0);
        }
        if layer == 1 {
            tag(&mut extra, b"lspf", &7u32.to_be_bytes());
        } // lock guide
        block(&mut info, &extra);
    }
    for data in channels {
        info.extend(data);
    }
    if !info.len().is_multiple_of(2) {
        info.push(0);
    }
    let mut layers = Vec::new();
    block(&mut layers, &info);
    drop(info);
    u32be(&mut layers, 0);
    let mut out = Vec::new();
    out.extend(b"8BPS");
    u16be(&mut out, 1);
    out.extend([0; 6]);
    u16be(&mut out, 4);
    u32be(&mut out, texture.height);
    u32be(&mut out, texture.width);
    u16be(&mut out, depth);
    u16be(&mut out, 3);
    u32be(&mut out, 0);
    // Integer source pixels are stored in sRGB, irrespective of import profile.
    // 32-bit PSD RGB samples are linear by convention.
    let mut resources = Vec::new();
    resources.extend(b"8BIM");
    u16be(&mut resources, 1024);
    resources.extend([0, 0]);
    block(&mut resources, &0u16.to_be_bytes()); // active paint layer
    let mut profile = moxcms::ColorProfile::new_srgb();
    if float {
        profile.red_trc = Some(moxcms::ToneReprCurve::Lut(vec![]));
        profile.green_trc = profile.red_trc.clone();
        profile.blue_trc = profile.red_trc.clone();
        profile.cicp = None;
        profile.description = None;
    }
    let icc = profile.encode().map_err(|e| e.to_string())?;
    resources.extend(b"8BIM");
    u16be(&mut resources, 1039);
    resources.extend([0, 0]);
    block(&mut resources, &icc);
    if !icc.len().is_multiple_of(2) {
        resources.push(0);
    }
    block(&mut out, &resources);
    block(&mut out, &layers);
    drop(layers);
    u16be(&mut out, 0);
    // Compatible merged preview, composited with the visible guide.
    for c in [0, 1, 2, 3] {
        for i in 0..count {
            let p = texture.pixels.pixel(i);
            let g = &guide.as_raw()[i * 4..i * 4 + 4];
            let alpha = g[3] as f32 / 255.;
            let out_alpha = alpha + p[3] * (1. - alpha);
            let value = if c == 3 {
                out_alpha
            } else {
                let color = g[c] as f32 / 255.;
                let color = if float {
                    crate::sculpt::material::srgb_to_linear(color)
                } else {
                    color
                };
                (color * alpha + p[c] * p[3] * (1. - alpha)) / out_alpha.max(1e-20)
            };
            if float {
                out.extend(value.to_be_bytes());
            } else {
                out.extend(((value.clamp(0., 1.) * 65535.).round() as u16).to_be_bytes());
            }
        }
    }
    Ok(out)
}

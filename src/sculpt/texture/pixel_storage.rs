//! Lossless compact native pixels with an explicit integer/float tag.
use super::Pixels;
use serde::{Deserialize, Deserializer, Serializer, de::Error, ser::Error as _};
use std::{
    io::{Read, Write},
    sync::Arc,
};
const MAX_SAMPLES: usize = 4096 * 4096 * 4;
pub fn serialize<S: Serializer>(pixels: &Pixels, serializer: S) -> Result<S::Ok, S::Error> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    let prefix = match pixels {
        Pixels::U16(p) => {
            for values in p.chunks(4096) {
                let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
                encoder.write_all(&bytes).map_err(S::Error::custom)?;
            }
            "zlib16le:"
        }
        Pixels::Float32(p) => {
            for values in p.chunks(4096) {
                let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
                encoder.write_all(&bytes).map_err(S::Error::custom)?;
            }
            "zlib32le:"
        }
    };
    let bytes = encoder.finish().map_err(S::Error::custom)?;
    const HEX: &[u8] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2 + 9);
    encoded.push_str(prefix);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 15) as usize] as char);
    }
    serializer.serialize_str(&encoded)
}
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Pixels, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Compressed(String),
        Legacy(Vec<u16>),
    }
    let (encoded, float) = match Stored::deserialize(deserializer)? {
        Stored::Legacy(p) => {
            if p.len() > MAX_SAMPLES {
                return Err(D::Error::custom("Texture exceeds 4K pixel limit"));
            }
            return Ok(Pixels::U16(Arc::new(p)));
        }
        Stored::Compressed(encoded) => {
            let float = encoded.starts_with("zlib32le:");
            if !float && !encoded.starts_with("zlib16le:") {
                return Err(D::Error::custom("Unknown texture pixel encoding"));
            }
            (encoded, float)
        }
    };
    let hex = &encoded[9..];
    let max_bytes = MAX_SAMPLES * if float { 4 } else { 2 };
    if hex.len() > max_bytes * 2 + 2 * 1024 * 1024 || !hex.len().is_multiple_of(2) {
        return Err(D::Error::custom("Invalid compressed texture length"));
    }
    let digit = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    };
    let bytes: Result<Vec<_>, _> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| {
            digit(p[0])
                .zip(digit(p[1]))
                .map(|(a, b)| (a << 4) | b)
                .ok_or_else(|| D::Error::custom("Invalid texture encoding"))
        })
        .collect();
    let bytes = bytes?;
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(bytes.as_slice())
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(D::Error::custom)?;
    if raw.len() > max_bytes || !raw.len().is_multiple_of(if float { 4 } else { 2 }) {
        return Err(D::Error::custom("Decoded texture exceeds 4K pixel limit"));
    }
    Ok(if float {
        Pixels::Float32(Arc::new(
            raw.as_chunks::<4>()
                .0
                .iter()
                .map(|p| f32::from_le_bytes(*p))
                .collect(),
        ))
    } else {
        Pixels::U16(Arc::new(
            raw.as_chunks::<2>()
                .0
                .iter()
                .map(|p| u16::from_le_bytes(*p))
                .collect(),
        ))
    })
}

//! Lossless texture words, shared in small row-aligned chunks across display frames.
use crate::sculpt::texture::{Pixels, Texture};
use std::{ops::Range, sync::Arc};
pub(super) const CHUNK: usize = 16 * 1024;
#[derive(Clone, Default)]
pub(super) struct Atlas {
    pub(super) chunks: Vec<Arc<Vec<[u32; 2]>>>,
    len: usize,
}
impl Atlas {
    pub(super) fn len(&self) -> usize {
        self.len
    }
    pub(super) fn get(&self, index: usize) -> [u32; 2] {
        self.chunks[index / CHUNK][index % CHUNK]
    }
    pub(super) fn append(&mut self, texture: &Texture) {
        let stride = if matches!(texture.pixels, Pixels::Float32(_)) {
            2
        } else {
            1
        };
        let len = texture.width as usize * texture.height as usize * stride;
        let start = self.len;
        self.len += len;
        while self.chunks.len() * CHUNK < self.len {
            self.chunks.push(Arc::new(Vec::new()));
        }
        for (i, chunk) in self.chunks.iter_mut().enumerate().skip(start / CHUNK) {
            Arc::make_mut(chunk).resize((self.len - i * CHUNK).min(CHUNK), [0; 2]);
        }
        self.update(texture, start, 0..len / stride);
    }
    pub(super) fn update(&mut self, texture: &Texture, offset: usize, pixels: Range<usize>) {
        let stride = if matches!(texture.pixels, Pixels::Float32(_)) {
            2
        } else {
            1
        };
        let range = pixels.start * stride..pixels.end * stride;
        if range.is_empty() {
            return;
        }
        // Bulk copying avoids decoding/re-encoding float values, preserving every bit.
        #[cfg(target_endian = "little")]
        {
            let words = match &texture.pixels {
                Pixels::U16(p) => bytemuck::try_cast_slice::<u16, [u32; 2]>(p),
                Pixels::Float32(p) => bytemuck::try_cast_slice::<f32, [u32; 2]>(p),
            };
            if let Ok(words) = words {
                let mut start = range.start;
                while start < range.end {
                    let target = offset + start;
                    let count = (CHUNK - target % CHUNK).min(range.end - start);
                    Arc::make_mut(&mut self.chunks[target / CHUNK])
                        [target % CHUNK..target % CHUNK + count]
                        .copy_from_slice(&words[start..start + count]);
                    start += count;
                }
                return;
            }
        }
        for (i, word) in texture
            .packed_pixels()
            .enumerate()
            .skip(range.start)
            .take(range.len())
        {
            let target = offset + i;
            Arc::make_mut(&mut self.chunks[target / CHUNK])[target % CHUNK] = word;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{
        Material,
        texture::{Channel, Precision},
    };
    #[test]
    fn atlas_float_dab_copies_only_intersecting_chunks() {
        let mut texture = Texture::new_with_precision(
            Channel::Color,
            256,
            &Material::default(),
            Precision::Float32,
        )
        .unwrap();
        let mut old = Atlas::default();
        old.append(&texture);
        let mut next = old.clone();
        texture.pixels.set_pixel(9000, [2., 0.1, 0.2, 0.5]);
        next.update(&texture, 0, 9000..9001);
        for i in 0..old.chunks.len() {
            assert_eq!(
                Arc::ptr_eq(&old.chunks[i], &next.chunks[i]),
                i != 18000 / CHUNK
            );
        }
        assert_eq!(next.get(18000)[0], 2f32.to_bits());
        assert_ne!(old.get(18000), next.get(18000));
    }
}

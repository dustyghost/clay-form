use super::*;
use std::hash::{Hash, Hasher};

/// UV-to-gutter correspondence survives colour strokes; only changed edge samples are copied.
#[derive(Default)]
pub(crate) struct GutterCache {
    key: Option<u64>,
    copies: Vec<(usize, usize)>,
    // Last image for which all correspondence destinations were initialized.
    initialized: Option<(u64, u64, usize)>,
}
impl GutterCache {
    fn prepare(&mut self, mesh: &Mesh, width: usize, height: usize) {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (width, height).hash(&mut hash);
        for (f, t) in mesh.uv.corners.iter().enumerate() {
            mesh.uv.mapped(f).hash(&mut hash);
            for q in t {
                for v in q {
                    v.to_bits().hash(&mut hash);
                }
            }
        }
        let key = hash.finish();
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.initialized = None;
        self.copies.clear();
        let mut occupied = vec![false; width * height];
        for (f, t) in mesh.uv.corners.iter().enumerate() {
            if mesh.uv.mapped(f) {
                raster(t.map(|p| [p[0], 1. - p[1]]), width, height, |x, y, _| {
                    occupied[y * width + x] = true
                });
            }
        }
        let neighbours = |i: usize| {
            let (x, y) = (i % width, i / width);
            [
                x.checked_sub(1).map(|x| y * width + x),
                (x + 1 < width).then_some(i + 1),
                y.checked_sub(1).map(|y| y * width + x),
                (y + 1 < height).then_some(i + width),
            ]
        };
        let mut frontier: Vec<_> = (0..occupied.len())
            .filter(|&i| occupied[i] && neighbours(i).into_iter().flatten().any(|j| !occupied[j]))
            .map(|i| (i, i))
            .collect();
        for _ in 0..4 {
            let mut next = Vec::new();
            for (i, source) in frontier {
                for j in neighbours(i).into_iter().flatten() {
                    if occupied[j] {
                        continue;
                    }
                    occupied[j] = true;
                    self.copies.push((source, j));
                    next.push((j, source));
                }
            }
            frontier = next;
        }
    }
}
impl Mesh {
    pub(crate) fn pad_texture_cached(
        &mut self,
        channel: Channel,
        before: Option<&Texture>,
        cache: &mut GutterCache,
    ) {
        if !self.uv.current(self) {
            return;
        }
        let Some(index) = self.uv.textures.iter().position(|t| t.channel == channel) else {
            return;
        };
        let texture = &self.uv.textures[index];
        cache.prepare(self, texture.width as usize, texture.height as usize);
        let texture = &mut self.uv.textures[index];
        let before = before.filter(|old| {
            old.width == texture.width && old.height == texture.height && old.id == texture.id
        });
        let before = before.filter(|old| {
            cache.initialized == Some((old.id, old.upload.revision(), old.pixels.storage_id()))
        });
        let mut dirty = usize::MAX..0;
        for &(source, dest) in &cache.copies {
            let sample = texture.pixels.pixel(source);
            if before.is_some_and(|old| old.pixels.pixel(source) == sample) {
                continue;
            }
            if texture.pixels.set_pixel(dest, sample) {
                dirty.start = dirty.start.min(dest);
                dirty.end = dirty.end.max(dest + 1);
            }
        }
        if dirty.start < dirty.end {
            texture.upload.record(dirty);
        }
        cache.initialized = Some((
            texture.id,
            texture.upload.revision(),
            texture.pixels.storage_id(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_gutter_updates_match_full_padding_without_copying_image() {
        let mut mesh = super::super::tests::sheets();
        let mut cache = GutterCache::default();
        mesh.pad_texture_cached(Channel::Color, None, &mut cache);
        let before = mesh.uv.textures[0].clone();
        mesh.uv.textures[0]
            .pixels
            .set_pixel(32 * 64 + 3, [1., 0., 0., 1.]);
        let mut expected = mesh.clone();
        expected.pad_texture(Channel::Color);
        // Drop the comparison copy's shared ownership before testing local mutation.
        let expected_pixels = expected.uv.textures[0].pixels.clone();
        drop(expected);
        let storage = mesh.uv.textures[0].pixels.storage_id();
        mesh.pad_texture_cached(Channel::Color, Some(&before), &mut cache);
        assert_eq!(mesh.uv.textures[0].pixels, expected_pixels);
        assert_eq!(mesh.uv.textures[0].pixels.storage_id(), storage);
        let previous_key = cache.key;
        mesh.uv.corners[0][0][0] += 0.01;
        mesh.pad_texture_cached(Channel::Color, None, &mut cache);
        assert_ne!(
            cache.key, previous_key,
            "moving UVs rebuilds gutter correspondence"
        );
    }
}

#[cfg(test)]
mod initialization_tests {
    use super::*;
    fn gradient(mesh: &mut Mesh, flip: bool) {
        let values = (0..64 * 64)
            .flat_map(|i| {
                let x = (i % 64) as f32 / 64.;
                [if flip { 1. - x } else { x }, 0., 0., 1.]
            })
            .collect();
        mesh.uv.textures[0].pixels = Pixels::Float32(std::sync::Arc::new(values));
    }
    #[test]
    fn cached_gutters_initialize_after_uv_rearrangement_and_same_id_image_import() {
        let mut mesh = super::super::tests::sheets();
        gradient(&mut mesh, false);
        let mut cache = GutterCache::default();
        mesh.pad_texture_cached(Channel::Color, None, &mut cache);
        for corners in &mut mesh.uv.corners {
            for uv in corners {
                uv[0] += 0.01;
            }
        }
        for imported in [false, true] {
            if imported {
                gradient(&mut mesh, true);
            }
            let before = mesh.uv.textures[0].clone();
            let mut expected = mesh.clone();
            expected.pad_texture(Channel::Color);
            assert_ne!(
                expected.uv.textures[0].pixels, before.pixels,
                "fixture requires new gutter samples"
            );
            mesh.pad_texture_cached(Channel::Color, Some(&before), &mut cache);
            assert_eq!(mesh.uv.textures[0].pixels, expected.uv.textures[0].pixels);
        }
    }
}

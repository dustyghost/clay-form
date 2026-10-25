use super::*;

#[derive(Clone, Copy)]
pub(crate) struct BakeOptions {
    pub size: u32,
    pub precision: Precision,
    pub preserve_existing: bool,
    pub composite: bool,
}

impl Mesh {
    /// Bake unlit base PBR values, optionally compositing existing painted maps.
    #[cfg(test)]
    pub(crate) fn bake_material_textures(
        &self,
        all: bool,
        size: u32,
        precision: Precision,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<Texture>, String> {
        self.bake_texture_channels(
            if all {
                &Channel::ALL
            } else {
                &[Channel::Color]
            },
            BakeOptions {
                size,
                precision,
                preserve_existing: false,
                composite: all,
            },
            cancel,
            &|_| {},
        )
    }

    pub(crate) fn bake_texture_channels(
        &self,
        channels: &[Channel],
        options: BakeOptions,
        cancel: &dyn Fn() -> bool,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<Texture>, String> {
        if !(16..=MAX_SIZE).contains(&options.size) {
            return Err("Texture size must be 16–4096 pixels".into());
        }
        if !self.uv.current(self) || !(0..self.triangles.len()).any(|f| self.uv.mapped(f)) {
            return Err("Unwrap the model before baking paint or material textures".into());
        }
        if self
            .uv
            .corners
            .iter()
            .enumerate()
            .filter(|(f, _)| self.uv.mapped(*f))
            .any(|(_, t)| {
                super::super::uv::area(*t).abs() < 1e-12
                    || t.iter().flatten().any(|v| !(0. ..=1.).contains(v))
            })
        {
            return Err(
                "Repair degenerate UVs and arrange islands inside 0–1 before baking".into(),
            );
        }
        let mut result = Vec::new();
        for (channel_index, &channel) in channels.iter().enumerate() {
            let old = self.uv.textures.iter().find(|t| t.channel == channel);
            let source = if options.composite { old } else { None };
            let preserved = old.filter(|_| options.preserve_existing);
            let width = preserved.map_or(options.size, |t| t.width);
            let height = preserved.map_or(options.size, |t| t.height);
            let precision = preserved.map_or(options.precision, |t| t.pixels.precision());
            if source.is_some_and(|t| t.pixels.precision() == Precision::Float32)
                && precision != Precision::Float32
            {
                return Err("Choose 32-bit float to bake existing HDR texture paint without losing precision".into());
            }
            let mut texture =
                Texture::new_with_precision(channel, 16, &self.material(0), precision)?;
            texture.width = width;
            texture.height = height;
            texture.pixels = match &texture.pixels {
                Pixels::U16(p) => Pixels::U16(Arc::new(p[..4].repeat((width * height) as usize))),
                Pixels::Float32(p) => {
                    Pixels::Float32(Arc::new(p[..4].repeat((width * height) as usize)))
                }
            };
            if let Some(old) = old {
                texture.id = old.id;
            }
            let mut occupied = vec![false; (width * height) as usize];
            for (f, face) in self.triangles.iter().enumerate() {
                if cancel() {
                    return Err("Texture bake cancelled".into());
                }
                if f % 256 == 0 {
                    progress(
                        (channel_index as f32 + f as f32 / self.triangles.len().max(1) as f32)
                            / channels.len() as f32,
                    );
                }
                if !self.uv.mapped(f) {
                    continue;
                }
                let values = face.map(|i| {
                    let material = self.material(i);
                    match channel {
                        Channel::Color => material.base_color,
                        Channel::Roughness => [material.roughness; 3],
                        Channel::Metallic => [material.metallic; 3],
                    }
                });
                raster_cancellable(
                    self.uv.corners[f].map(|p| [p[0], 1. - p[1]]),
                    width as usize,
                    height as usize,
                    cancel,
                    |x, y, w| {
                        let index = y * width as usize + x;
                        occupied[index] = true;
                        let painted = source.map(|t| resample(t, x, y, width, height));
                        let mut p = [0., 0., 0., 1.];
                        for c in 0..3 {
                            p[c] = (0..3).map(|k| w[k] * values[k][c]).sum();
                            if let Some(paint) = painted {
                                p[c] = paint[c] * paint[3] + p[c] * (1. - paint[3]);
                            }
                            if channel == Channel::Color && precision == Precision::U16 {
                                p[c] = super::super::material::linear_to_srgb(p[c]);
                            }
                        }
                        texture.pixels.set_pixel(index, p);
                    },
                )?;
            }
            if cancel() {
                return Err("Texture bake cancelled".into());
            }
            pad_pixels(&mut texture, occupied);
            result.push(texture);
        }
        progress(1.);
        Ok(result)
    }
}

fn raster_cancellable(
    t: [[f32; 2]; 3],
    width: usize,
    height: usize,
    cancel: &dyn Fn() -> bool,
    mut visit: impl FnMut(usize, usize, [f32; 3]),
) -> Result<(), String> {
    let size = [width as f32, height as f32];
    let lo: [usize; 2] = std::array::from_fn(|k| {
        ((t.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min) * size[k] - 0.5)
            .floor()
            .max(0.)) as usize
    });
    let hi: [usize; 2] = std::array::from_fn(|k| {
        ((t.iter().map(|p| p[k]).fold(f32::NEG_INFINITY, f32::max) * size[k] - 0.5)
            .ceil()
            .clamp(0., size[k] - 1.)) as usize
    });
    for y in lo[1]..=hi[1] {
        if cancel() {
            return Err("Texture bake cancelled".into());
        }
        for x in lo[0]..=hi[0] {
            if let Some(w) =
                barycentric([(x as f32 + 0.5) / size[0], (y as f32 + 0.5) / size[1]], t)
            {
                visit(x, y, w);
            }
        }
    }
    Ok(())
}

// Area filtering when shrinking, bilinear when enlarging. Accumulate premultiplied
// linear samples so transparent RGB cannot bleed into the baked surface colour.
fn resample(texture: &Texture, x: usize, y: usize, width: u32, height: u32) -> [f32; 4] {
    fn footprint(index: usize, source: u32, target: u32) -> (i32, i32, f32, f32, bool) {
        let scale = source as f32 / target as f32;
        if scale > 1. {
            let lo = index as f32 * scale;
            let hi = (index + 1) as f32 * scale;
            (lo.floor() as i32, hi.ceil() as i32 - 1, lo, hi, true)
        } else {
            let center = (index as f32 + 0.5) * scale - 0.5;
            (
                center.floor() as i32,
                center.floor() as i32 + 1,
                center,
                0.,
                false,
            )
        }
    }
    fn weight(index: i32, f: (i32, i32, f32, f32, bool)) -> f32 {
        if f.4 {
            ((index + 1) as f32).min(f.3) - (index as f32).max(f.2)
        } else {
            (1. - (index as f32 - f.2).abs()).max(0.)
        }
    }
    let fx = footprint(x, texture.width, width);
    let fy = footprint(y, texture.height, height);
    let mut result = [0.; 4];
    let mut total = 0.;
    for sy in fy.0..=fy.1 {
        for sx in fx.0..=fx.1 {
            let w = weight(sx, fx) * weight(sy, fy);
            if w <= 0. {
                continue;
            }
            let i = sy.clamp(0, texture.height as i32 - 1) as usize * texture.width as usize
                + sx.clamp(0, texture.width as i32 - 1) as usize;
            let p = texture.linear_pixel(i);
            for c in 0..3 {
                result[c] += p[c] * p[3] * w;
            }
            result[3] += p[3] * w;
            total += w;
        }
    }
    if result[3] > 0. {
        for c in 0..3 {
            result[c] /= result[3];
        }
    }
    result[3] /= total.max(f32::MIN_POSITIVE);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bake_preserves_rectangular_float_maps_and_filters_coverage() {
        let mut mesh = super::super::tests::sheets();
        let source = &mut mesh.uv.textures[0];
        source.width = 64;
        source.height = 32;
        source.pixels = Pixels::Float32(Arc::new([2., -0.25, 0.5, 1.].repeat(64 * 32)));
        let maps = mesh
            .bake_texture_channels(
                &Channel::ALL,
                BakeOptions {
                    size: 16,
                    precision: Precision::U16,
                    preserve_existing: true,
                    composite: true,
                },
                &|| false,
                &|_| {},
            )
            .unwrap();
        assert_eq!(
            (maps[0].width, maps[0].height, maps[0].pixels.precision()),
            (64, 32, Precision::Float32)
        );
        assert_eq!(
            (maps[1].width, maps[1].height, maps[1].pixels.precision()),
            (16, 16, Precision::U16)
        );
        assert!((maps[0].linear_pixel(16 * 64 + 16)[0] - 2.).abs() < 1e-6);
        let mut t = Texture::new_with_precision(
            Channel::Color,
            16,
            &Default::default(),
            Precision::Float32,
        )
        .unwrap();
        t.pixels = Pixels::Float32(Arc::new([1., 0., 0., 1., 0., 50., 0., 0.].repeat(128)));
        let p = resample(&t, 0, 0, 8, 8);
        assert_eq!(
            p,
            [1., 0., 0., 0.5],
            "area filter must ignore transparent RGB"
        );
        t.pixels = Pixels::Float32(Arc::new([1., 0., 0., 1., 0., 0., 1., 1.].repeat(128)));
        let p = resample(&t, 1, 0, 32, 32);
        assert_eq!(
            p,
            [0.75, 0., 0.25, 1.],
            "upsampling interpolates between texel centres"
        );
    }

    #[test]
    fn bake_captures_vertex_colour_and_composites_pbr_maps_without_lighting() {
        let mut mesh = super::super::tests::sheets();
        let material = super::super::super::Material::from_srgb([0.8, 0.2, 0.1], 0.3, 0.7);
        mesh.materials = vec![material.clone(); mesh.vertices.len()];
        let captured = mesh
            .bake_material_textures(false, 64, Precision::Float32, &|| false)
            .unwrap();
        let i = 32 * 64 + 16;
        assert_eq!(captured.len(), 1);
        for c in 0..3 {
            assert!((captured[0].linear_pixel(i)[c] - material.base_color[c]).abs() < 1e-6);
        }
        let old_id = mesh.uv.textures[0].id;
        mesh.uv.textures[0].pixels = Pixels::Float32(Arc::new([2., 0., 0., 0.25].repeat(64 * 64)));
        let maps = mesh
            .bake_material_textures(true, 64, Precision::Float32, &|| false)
            .unwrap();
        assert_eq!(maps.len(), 3);
        assert_eq!(maps[0].id, old_id);
        assert!((maps[0].linear_pixel(i)[0] - (0.5 + material.base_color[0] * 0.75)).abs() < 1e-6);
        assert!((maps[1].linear_pixel(i)[0] - 0.3).abs() < 1e-6);
        assert!((maps[2].linear_pixel(i)[0] - 0.7).abs() < 1e-6);
        assert!(
            mesh.bake_material_textures(true, 64, Precision::U16, &|| false)
                .is_err()
        );
        assert!(
            mesh.bake_material_textures(true, 64, Precision::Float32, &|| true)
                .is_err()
        );
        for (i, p) in mesh.vertices.iter().enumerate() {
            mesh.materials[i].base_color = [(p.0 + 1.) / 2., (p.1 + 1.) / 2., 0.];
        }
        let gradient = mesh
            .bake_material_textures(false, 64, Precision::Float32, &|| false)
            .unwrap();
        let pixel = gradient[0].linear_pixel(i);
        assert!((pixel[0] - 0.51953125).abs() < 1e-6);
        assert!((pixel[1] - 0.49131945).abs() < 1e-6);
    }
}

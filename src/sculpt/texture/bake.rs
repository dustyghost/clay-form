use super::*;

impl Mesh {
    /// Bake unlit base PBR values, optionally compositing existing painted maps.
    pub(crate) fn bake_material_textures(
        &self,
        all: bool,
        size: u32,
        precision: Precision,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<Texture>, String> {
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
        let channels: &[Channel] = if all {
            &Channel::ALL
        } else {
            &[Channel::Color]
        };
        let mut result = Vec::new();
        for &channel in channels {
            let old = self.uv.textures.iter().find(|t| t.channel == channel);
            let source = if all { old } else { None };
            if source.is_some_and(|t| t.pixels.precision() == Precision::Float32)
                && precision != Precision::Float32
            {
                return Err("Choose 32-bit float to bake existing HDR texture paint without losing precision".into());
            }
            let mut texture =
                Texture::new_with_precision(channel, size, &self.material(0), precision)?;
            if let Some(old) = old {
                texture.id = old.id;
            }
            let mut occupied = vec![false; (size * size) as usize];
            for (f, face) in self.triangles.iter().enumerate() {
                if cancel() {
                    return Err("Texture bake cancelled".into());
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
                raster(
                    self.uv.corners[f].map(|p| [p[0], 1. - p[1]]),
                    size as usize,
                    size as usize,
                    |x, y, w| {
                        let index = y * size as usize + x;
                        occupied[index] = true;
                        let painted = source.map(|t| {
                            let sx = x * t.width as usize / size as usize;
                            let sy = y * t.height as usize / size as usize;
                            t.linear_pixel(sy * t.width as usize + sx)
                        });
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
                );
            }
            pad_pixels(&mut texture, occupied);
            result.push(texture);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

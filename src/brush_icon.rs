//! Clean, square brush thumbnails, independent of viewport overlays.
use crate::{
    camera::Camera,
    sculpt::{BrushIcon, Kind, Mesh, Vec3},
};
const SIZE: usize = 256;
pub(crate) fn capture(mesh: &Mesh, camera: &Camera) -> Result<BrushIcon, String> {
    render(mesh, camera, false)
}

fn render(mesh: &Mesh, camera: &Camera, wireframe: bool) -> Result<BrushIcon, String> {
    mesh.validate()?;
    if mesh.triangles.is_empty() {
        return Err("The active model has no surface to capture".into());
    }
    let (right, up, view) = camera.basis();
    let positions: Vec<_> = mesh
        .vertices
        .iter()
        .map(|p| Vec3(p.dot(right), p.dot(up), p.dot(view)))
        .collect();
    let mut min = Vec3(f32::INFINITY, f32::INFINITY, 0.);
    let mut max = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, 0.);
    for p in &positions {
        min.0 = min.0.min(p.0);
        min.1 = min.1.min(p.1);
        max.0 = max.0.max(p.0);
        max.1 = max.1.max(p.1);
    }
    let scale = SIZE as f32 * 0.86 / (max.0 - min.0).max(max.1 - min.1).max(0.001);
    let cx = (max.0 + min.0) * 0.5;
    let cy = (max.1 + min.1) * 0.5;
    let projected: Vec<_> = positions
        .iter()
        .map(|p| {
            Vec3(
                SIZE as f32 * 0.5 + (p.0 - cx) * scale,
                SIZE as f32 * 0.5 - (p.1 - cy) * scale,
                p.2,
            )
        })
        .collect();
    let light = (view + up * 0.7 - right * 0.5).norm();
    let shading = mesh.surface_shading();
    let shades: Vec<_> = shading
        .vertices
        .iter()
        .map(|(_, n)| 0.25 + 0.75 * n.dot(light).max(0.))
        .collect();
    let mut rgba = [23, 26, 31, 255].repeat(SIZE * SIZE);
    let mut depth = vec![f32::NEG_INFINITY; SIZE * SIZE];
    for (fi, face) in mesh.triangles.iter().enumerate() {
        let shade_indices = shading.triangles[fi];
        let [a, b, c] = face.map(|i| projected[i]);
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area >= -1e-7 {
            continue;
        }
        let altitudes = [(b, c), (c, a), (a, b)]
            .map(|(p, q)| area.abs() / (p.0 - q.0).hypot(p.1 - q.1).max(1e-7));
        let x0 = a.0.min(b.0).min(c.0).floor().max(0.) as usize;
        let x1 = a.0.max(b.0).max(c.0).ceil().min((SIZE - 1) as f32) as usize;
        let y0 = a.1.min(b.1).min(c.1).floor().max(0.) as usize;
        let y1 = a.1.max(b.1).max(c.1).ceil().min((SIZE - 1) as f32) as usize;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let wb = ((px - a.0) * (c.1 - a.1) - (py - a.1) * (c.0 - a.0)) / area;
                let wc = ((b.0 - a.0) * (py - a.1) - (b.1 - a.1) * (px - a.0)) / area;
                let wa = 1. - wb - wc;
                if wa < -1e-5 || wb < -1e-5 || wc < -1e-5 {
                    continue;
                }
                let z = wa * a.2 + wb * b.2 + wc * c.2;
                let i = y * SIZE + x;
                if z <= depth[i] {
                    continue;
                }
                depth[i] = z;
                let shade = wa * shades[shade_indices[0]]
                    + wb * shades[shade_indices[1]]
                    + wc * shades[shade_indices[2]];
                let wire = if wireframe {
                    let distance = (wa * altitudes[0])
                        .min(wb * altitudes[1])
                        .min(wc * altitudes[2]);
                    (3. - distance).clamp(0., 1.)
                } else {
                    0.
                };
                for (channel, base) in [190., 155., 125.].into_iter().enumerate() {
                    let line = [32., 38., 44.][channel];
                    rgba[i * 4 + channel] =
                        (base * shade * (1. - wire) + line * wire).clamp(0., 255.) as u8;
                }
            }
        }
    }
    Ok(BrushIcon {
        size: SIZE,
        rgba: rgba.into(),
    })
}
pub(crate) fn default_icon(kind: Kind) -> BrushIcon {
    if kind == Kind::Decimate {
        // A sparse, rounded triangle mesh shows the topology Decimate produces.
        let mesh = crate::sculpt::Primitive {
            kind: crate::sculpt::PrimitiveKind::Icosphere,
            subdivisions: 2,
            ..Default::default()
        }
        .mesh()
        .expect("valid Decimate icon geometry");
        let camera = Camera {
            yaw: 0.25,
            pitch: -0.2,
            ..Default::default()
        };
        let icon = render(&mesh, &camera, true).expect("valid Decimate icon surface");
        let image = image::RgbaImage::from_raw(SIZE as u32, SIZE as u32, icon.rgba.to_vec())
            .expect("complete icon pixels");
        return BrushIcon {
            size: 96,
            rgba: image::imageops::resize(&image, 96, 96, image::imageops::FilterType::Lanczos3)
                .into_raw()
                .into(),
        };
    }
    let n = 96;
    let mut rgba = Vec::with_capacity(n * n * 4);
    let height = |x: f32, y: f32| {
        let r = x * x + y * y;
        let stroke = (-((x / 0.65).powi(6) + (y / 0.20).powi(2))).exp();
        match kind {
            Kind::Decimate => unreachable!("Decimate uses a wireframe mesh thumbnail"),
            Kind::Standard => 0.24 * stroke,
            Kind::Clay => 0.20 * (-((x / 0.65).powi(6) + (y / 0.28).powi(6))).exp(),
            Kind::Inflate => 0.40 * (-r / 0.20).exp(),
            Kind::Smooth => 0.10 * (-r / 0.48).exp(),
            Kind::Flatten => 0.28 * (1. - (r / 0.45).min(1.)).min(0.5),
            Kind::Polish => {
                let plateau = (0.18 - x * 0.08).min(0.30 * (-r / 0.40).exp());
                plateau.max(0.)
            }
            Kind::Pinch => 0.30 * (-((x / 0.65).powi(6) + (y / 0.07).powi(2))).exp(),
            Kind::Crease => -0.24 * (-((x / 0.65).powi(6) + (y / 0.075).powi(2))).exp(),
            Kind::Detail => {
                let span = (-(x / 0.65).powi(6)).exp();
                span * (-0.30 * (-(y / 0.055).powi(2)).exp()
                    + 0.09 * (-((y.abs() - 0.12) / 0.05).powi(2)).exp())
            }
            Kind::Grab => 0.35 * (-((x - 0.20) / 0.28).powi(2) - (y / 0.35).powi(2)).exp(),
            Kind::Mask => 0.10 * stroke,
            Kind::Snake => {
                0.35 * (-((y - 0.25 * (x * 5.).sin()) / 0.13).powi(2) - (x / 0.65).powi(6)).exp()
            }
        }
    };
    for y in 0..n {
        for x in 0..n {
            let u = x as f32 / (n - 1) as f32 * 2. - 1.;
            let v = 1. - y as f32 / (n - 1) as f32 * 2.;
            let r = u * u + v * v;
            if r > 0.85 {
                rgba.extend([23, 26, 31, 255]);
                continue;
            }
            let dx = (height(u + 0.01, v) - height(u - 0.01, v)) / 0.02;
            let dy = (height(u, v + 0.01) - height(u, v - 0.01)) / 0.02;
            let normal = Vec3(-dx, -dy, 1.).norm();
            let shade = 0.25 + 0.75 * normal.dot(Vec3(-0.4, 0.5, 0.75).norm()).max(0.);
            let mask = if kind == Kind::Mask && height(u, v) > 0.02 {
                0.25
            } else {
                1.
            };
            let edge = ((0.85 - r) / 0.1).clamp(0., 1.);
            for base in [190., 155., 125.] {
                rgba.push((base * shade * mask * edge + 24. * (1. - edge)) as u8);
            }
            rgba.push(255);
        }
    }
    BrushIcon {
        size: n,
        rgba: rgba.into(),
    }
}

/// Project the effective alpha onto the thumbnail, retaining the captured base.
pub(crate) fn projected(brush: &crate::sculpt::Brush) -> BrushIcon {
    let base = brush
        .icon
        .clone()
        .unwrap_or_else(|| default_icon(brush.kind));
    let mut rgba = base.rgba.to_vec();
    for y in 0..base.size {
        for x in 0..base.size {
            let u = x as f32 / (base.size - 1) as f32 * 2. - 1.;
            let v = 1. - y as f32 / (base.size - 1) as f32 * 2.;
            let weight = brush.weight(u, v);
            let i = (y * base.size + x) * 4;
            for (c, tint) in [125., 210., 245.].into_iter().enumerate() {
                rgba[i + c] =
                    (rgba[i + c] as f32 * (1. - 0.55 * weight) + tint * 0.55 * weight) as u8;
            }
        }
    }
    BrushIcon {
        size: base.size,
        rgba: rgba.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brush_capture_fits_surface_and_preserves_model() {
        let document = crate::document::Document::default();
        let original = document.mesh().clone();
        let mut camera = Camera::default();
        let icon = capture(&original, &camera).unwrap();
        icon.validate().unwrap();
        assert_eq!(icon.size, 256);
        assert_eq!(&icon.rgba[..4], &[23, 26, 31, 255]);
        assert!(
            icon.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| **p != [23, 26, 31, 255])
                .count()
                > 10000
        );
        assert!(icon.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
        camera.zoom = 8.;
        camera.pan = eframe::egui::vec2(300., -200.);
        assert!(capture(&original, &camera).unwrap() == icon);
        assert_eq!(*document.mesh(), original);
        let mut empty = original.clone();
        empty.triangles.clear();
        assert!(capture(&empty, &camera).is_err());
        if let Ok(dir) = std::env::var("CLAY_ICON_PREVIEW") {
            image::save_buffer_with_format(
                format!("{dir}/capture.png"),
                &icon.rgba,
                256,
                256,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .unwrap();
        }
    }
    #[test]
    fn projected_thumbnail_tracks_alpha_rotation_and_preserves_capture() {
        let mut brush = crate::sculpt::Brush::new(Kind::Clay);
        brush.alpha = crate::sculpt::Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![1., 0., 0., 0.].into(),
        };
        brush.icon = Some(default_icon(Kind::Clay));
        let original = brush.icon.clone();
        let first = projected(&brush);
        brush.rotation = 90.;
        assert!(first != projected(&brush));
        assert!(brush.icon == original);
        brush.rotation = 0.;
        brush.inner_radius = 0.8;
        assert!(first != projected(&brush));
        projected(&brush).validate().unwrap();
    }
    #[test]
    fn brush_default_icons_are_valid_and_distinct() {
        let mut icons = Vec::new();
        for kind in Kind::ALL {
            let icon = default_icon(kind);
            icon.validate().unwrap();
            assert!(icons.iter().all(|old| old != &icon));
            if let Ok(dir) = std::env::var("CLAY_ICON_PREVIEW") {
                image::save_buffer_with_format(
                    format!("{dir}/{kind:?}.png"),
                    &icon.rgba,
                    icon.size as u32,
                    icon.size as u32,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .unwrap();
            }
            icons.push(icon);
        }
    }
}

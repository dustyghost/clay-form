use super::ui_capture::{Headless, UiCapture};
use super::*;
use crate::{
    camera::Camera,
    sculpt::{Geometry, Primitive, PrimitiveKind, Transform},
};

#[test]
fn display_cache_inputs_keep_quad_edges_and_model_ownership() {
    let mut document = Document::default();
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [1; 3];
    document
        .replace_geometry(Geometry::new(cube.mesh().unwrap()))
        .unwrap();
    let display = DisplayMesh::new(&document);
    assert_eq!(display.edges(false).len() / 24, 12);
    assert_eq!(display.edges(true).len() / 24, 18);
    assert_eq!(display.active_vertices, 8);
    document.duplicate_model(false).unwrap();
    let display = DisplayMesh::new(&document);
    assert_eq!(display.mesh.vertices.len(), 16);
    assert!(display.owners.contains(&0) && display.owners.contains(&1));
}

#[test]
fn similarly_named_groups_display_distinct_muted_colors_and_renames_keep_them() {
    let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
    plane.divisions = [2, 1, 1];
    let mut mesh = plane.mesh().unwrap();
    mesh.create_vertex_group("Group 1".into(), 0.5, true)
        .unwrap();
    mesh.mask = mesh
        .vertices
        .iter()
        .map(|p| if p.0 >= 0. { 1. } else { 0. })
        .collect();
    mesh.create_vertex_group("Group 2".into(), 0.5, false)
        .unwrap();
    let mut document = Document::default();
    document.replace_geometry(Geometry::new(mesh)).unwrap();
    let before = DisplayMesh::new(&document).poly_colors;
    let distinct: std::collections::HashSet<_> = before.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        2,
        "separate groups need separate face colors"
    );
    let mut colors = distinct.into_iter();
    let a = colors.next().unwrap();
    let b = colors.next().unwrap();
    assert!(
        a[..3]
            .iter()
            .zip(&b[..3])
            .map(|(&a, &b)| (a as f32 - b as f32).powi(2))
            .sum::<f32>()
            .sqrt()
            > 60.
    );
    document.rename_vertex_group(0, "Renamed".into()).unwrap();
    assert_eq!(DisplayMesh::new(&document).poly_colors, before);
}

/// Real shader compilation and framebuffer readback, without opening a window.
/// Uses the installed Vulkan implementation; run explicitly on Linux graphics hosts.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn gpu_offscreen_depth_wireframe_and_cache() {
    {
        let gl = Headless::new();

        let mut renderer = Renderer::new(&gl.state).unwrap();
        let mut document = Document::default();
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [1; 3];
        document
            .replace_geometry(Geometry::new(plane.mesh().unwrap()))
            .unwrap();
        let first = renderer.geometry(&document);
        assert!(Arc::ptr_eq(&first, &renderer.geometry(&document)));
        let mut brush = document.brushes()[0].clone();
        brush.rotation = 45.;
        document.update_brush(0, brush).unwrap();
        assert!(
            Arc::ptr_eq(&first, &renderer.geometry(&document)),
            "brush edits must not upload mesh buffers"
        );
        document.duplicate_model(false).unwrap();
        document
            .apply_transform(
                Transform {
                    translation: Vec3(0., 0., 0.4),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        document.end_stroke();
        document.select_submodel(1).unwrap();
        let display = renderer.geometry(&document);
        assert!(!Arc::ptr_eq(&first, &display));
        let camera = Camera::default();
        let (right, up, view) = camera.basis();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.));
        let mut params = Params::new(
            rect,
            right,
            up,
            view,
            89.6,
            rect.center(),
            display.depth_extent,
        );
        let handle = renderer.gpu.clone();
        let mut gpu = handle.lock().unwrap();
        gpu.upload(&display);
        let read = |gpu: &Gpu| gpu.read();
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let solid = read(&gpu);
        let center = (128 * 256 + 128) * 4;
        assert!(
            solid[center + 2] > solid[center],
            "inactive foreground must occlude clay behind it"
        );
        assert_eq!(&solid[..4], &[23, 26, 31, 255]);
        params.background = [190, 197, 205];
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let light_background = read(&gpu);
        assert_eq!(&light_background[..4], &[190, 197, 205, 255]);
        assert_eq!(
            &light_background[center..center + 4],
            &solid[center..center + 4],
            "workspace background must not change mesh shading"
        );
        params.background = [23, 26, 31];
        let uploaded = gpu.uploaded.clone().unwrap();
        gpu.upload(&display);
        assert!(Arc::ptr_eq(&uploaded, gpu.uploaded.as_ref().unwrap()));
        gpu.paint(
            &params,
            [256, 256],
            true,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let visible = read(&gpu);
        gpu.paint(
            &params,
            [256, 256],
            true,
            WireMode::Front,
            false,
            &Preview::default(),
        );
        let front = read(&gpu);
        assert!(
            visible != front,
            "visible mode rejects the occluded rear edges; front mode draws them"
        );
        gpu.paint(
            &params,
            [256, 256],
            true,
            WireMode::Transparent,
            false,
            &Preview::default(),
        );
        let transparent = read(&gpu);
        assert_eq!(
            &transparent[center..center + 4],
            &[23, 26, 31, 255],
            "transparent wireframe has no surface fill"
        );
        assert!(
            transparent
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| *p != [23, 26, 31, 255])
        );
        let make_preview = |alpha: Alpha| Preview {
            show_polygroups: false,
            backface_opacity: 0.,
            brush: Some(BrushPreview {
                center: Vec3(0., 0., 0.),
                normal: Vec3(0., 0., 1.),
                radius: 0.5,
                brush: {
                    let mut b = Brush::new(crate::sculpt::Kind::Standard);
                    b.alpha = alpha;
                    b
                },
                color: [0.2, 0.8, 1.],
            }),
            symmetry: None,
        };
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &make_preview(Alpha::Hard),
        );
        assert!(
            read(&gpu) == solid,
            "alpha must not paint inactive foreground models"
        );
        document.select_submodel(0).unwrap();
        let active = renderer.geometry(&document);
        gpu.upload(&active);
        let mut preview = make_preview(Alpha::Hard);
        preview.brush.as_mut().unwrap().center.2 = 0.4;
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let active_solid = read(&gpu);
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &preview,
        );
        assert!(
            read(&gpu)[center + 2] > active_solid[center + 2],
            "hard alpha changes active surface shading"
        );
        preview.brush.as_mut().unwrap().brush.alpha = Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![0.; 4].into(),
        };
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &preview,
        );
        assert!(
            read(&gpu) == active_solid,
            "black image alpha must preserve shading"
        );
        preview.brush.as_mut().unwrap().brush.alpha = Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![1.; 4].into(),
        };
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &preview,
        );
        assert!(
            read(&gpu)[center + 2] > active_solid[center + 2],
            "replacement alpha texture must upload"
        );
        // Compare shader alpha against CPU sculpt weights at an actual pixel.
        for alpha in [
            Alpha::Soft,
            Alpha::Hard,
            Alpha::Square,
            Alpha::Image {
                width: 2,
                height: 2,
                pixels: vec![1., 0., 0., 0.].into(),
            },
        ] {
            let mut preview = make_preview(alpha);
            let b = preview.brush.as_mut().unwrap();
            b.center.2 = 0.4;
            b.brush.inner_radius = 0.35;
            b.brush.rotation = 35.;
            let q = 0.5 / 89.6 / b.radius;
            let weight = b.brush.weight(q, q);
            let tint = b.color;
            gpu.paint(
                &params,
                [256, 256],
                false,
                WireMode::Visible,
                false,
                &preview,
            );
            let rgba = read(&gpu);
            for channel in 0..3 {
                let expected = active_solid[center + channel] as f32 * (1. - weight * 90. / 255.)
                    + tint[channel] * weight * 90.;
                assert!(
                    (rgba[center + channel] as f32 - expected).abs() < 2.,
                    "GPU alpha must match sculpting alpha"
                );
            }
        }
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        let preview = Preview {
            show_polygroups: false,
            backface_opacity: 0.,
            brush: None,
            symmetry: Some((symmetry, 1.)),
        };
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &preview,
        );
        assert!(
            read(&gpu)[center] > active_solid[center],
            "mirror guide appears on the surface"
        );
        document
            .create_vertex_group("GPU color".into(), true)
            .unwrap();
        let grouped = renderer.geometry(&document);
        gpu.upload(&grouped);
        let colored = Preview {
            show_polygroups: true,
            ..Default::default()
        };
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &colored,
        );
        assert_ne!(
            &read(&gpu)[center..center + 3],
            &active_solid[center..center + 3],
            "polygroup color must reach the framebuffer"
        );
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        assert_eq!(
            read(&gpu),
            active_solid,
            "hiding colors restores original shading"
        );
        document.set_vertex_group_visible(0, false).unwrap();
        let hidden = renderer.geometry(&document);
        assert_eq!(
            hidden.mesh.triangles.len(),
            grouped.mesh.triangles.len() - 2
        );
        gpu.upload(&hidden);
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        assert_ne!(
            &read(&gpu)[center..center + 3],
            &active_solid[center..center + 3],
            "hiding the foreground group reveals the surface behind it"
        );
        document.set_vertex_group_visible(0, true).unwrap();
        gpu.upload(&renderer.geometry(&document));
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        assert_eq!(read(&gpu), active_solid);
        gpu.paint(
            &params,
            [128, 128],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        assert_eq!(gpu.target.as_ref().unwrap().size, [128, 128]);
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [2, 2, 1];
        let mut geometry = Geometry::new(plane.mesh().unwrap());
        geometry.settings.primitive = Some(plane);
        document.replace_geometry(geometry).unwrap();
        gpu.upload(&renderer.geometry(&document));
        gpu.paint(
            &params,
            [256, 256],
            true,
            WireMode::Transparent,
            false,
            &Preview::default(),
        );
        let wire = read(&gpu);
        for axis in [0, 1] {
            let color = super::super::controls::AXES[axis];
            assert!(
                wire.as_chunks::<4>().0.iter().any(|p| p[..3]
                    .iter()
                    .zip([color.r(), color.g(), color.b()])
                    .all(|(&a, b)| a.abs_diff(b) <= 1)),
                "axis color must reach the wireframe framebuffer"
            );
        }

        let mut tilted = Primitive::for_kind(PrimitiveKind::Plane);
        tilted.divisions = [2, 2, 1];
        tilted.basis[1] = Vec3(0., 0.8, 0.6);
        let mut geometry = Geometry::new(tilted.mesh().unwrap());
        geometry.settings.primitive = Some(tilted);
        document.replace_geometry(geometry).unwrap();
        gpu.upload(&renderer.geometry(&document));
        gpu.paint(
            &params,
            [256, 256],
            true,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let wire = read(&gpu);
        let green = super::super::controls::AXES[1];
        let covered = (44..212)
            .filter(|&x| {
                (126..130).any(|y| {
                    let pixel = &wire[(y * 256 + x) * 4..(y * 256 + x) * 4 + 3];
                    pixel
                        .iter()
                        .zip([green.r(), green.g(), green.b()])
                        .all(|(&a, b)| a.abs_diff(b) <= 1)
                })
            })
            .count();
        assert!(
            covered >= 160,
            "tilted-face cut lines must remain continuous, got {covered} columns"
        );
        gpu.clear();
        gpu.clear(); // shutdown is safe even when a callback retains the handle
        drop(gpu);
        drop(renderer);
        let mut capture = UiCapture::new([256, 256]);
        let mut renderer = Renderer::new(&capture.state).unwrap();
        let document = Document::default();
        let mut camera = Camera::default();
        camera.orbit(egui::vec2(50., 20.));
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                let model_rect = rect.shrink(24.);
                let scene = Scene::paint_with_renderer(
                    &painter,
                    model_rect,
                    &document,
                    &camera,
                    false,
                    WireMode::Visible,
                    false,
                    Some(&mut renderer),
                );
                let brush = Brush::new(crate::sculpt::Kind::Standard);
                renderer.set_brush(
                    &scene,
                    egui::pos2(155., 128.),
                    45.,
                    &brush,
                    egui::Color32::from_rgb(125, 210, 245),
                );
                renderer.set_guides(
                    &Symmetry {
                        enabled: true,
                        ..Default::default()
                    },
                    1.,
                );
                painter.rect_filled(
                    egui::Rect::from_min_max(egui::pos2(30., 30.), egui::pos2(42., 42.)),
                    0.,
                    egui::Color32::RED,
                );
                painter.text(
                    egui::pos2(30., 216.),
                    egui::Align2::LEFT_TOP,
                    "GPU viewport",
                    egui::FontId::proportional(14.),
                    egui::Color32::WHITE,
                );
            },
        );
        let captured = capture.render(&ctx, output);
        let rgba = captured;
        let overlay = ((256 - 36) * 256 + 36) * 4;
        assert_eq!(
            &rgba[overlay..overlay + 4],
            &[255, 0, 0, 255],
            "egui overlays must survive the callback"
        );
        assert!(rgba[center] > 40, "model callback must render inside egui");
        if let Ok(path) = std::env::var("CLAY_GPU_PREVIEW") {
            let mut flipped = Vec::with_capacity(rgba.len());
            for row in rgba.as_chunks::<{ 256 * 4 }>().0.iter().rev() {
                flipped.extend_from_slice(row);
            }
            image::save_buffer(path, &flipped, 256, 256, image::ColorType::Rgba8).unwrap();
        }
        // Face-less primitive outlines are egui overlays and must be painted
        // after the GPU callback's background, not erased by it.
        let mut circle = Primitive::for_kind(PrimitiveKind::Circle);
        circle.filled = false;
        let mut document = Document::default();
        document
            .replace_geometry(Geometry::new(circle.mesh().unwrap()))
            .unwrap();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                Scene::paint_with_renderer(
                    &painter,
                    rect,
                    &document,
                    &Camera::default(),
                    false,
                    WireMode::Visible,
                    false,
                    Some(&mut renderer),
                );
            },
        );
        let captured = capture.render(&ctx, output);
        let rgba = captured;
        assert!(
            rgba.as_chunks::<4>().0.iter().filter(|p| p[0] > 80).count() > 100,
            "unfilled circle outline must remain visible"
        );
        let cache = renderer.geometry(&document);
        document.new_sphere();
        assert!(
            !Arc::ptr_eq(&cache, &renderer.geometry(&document)),
            "New must invalidate cached geometry"
        );
        let cache = renderer.geometry(&document);
        document.initialize_primitive().unwrap();
        document.undo();
        assert!(
            !Arc::ptr_eq(&cache, &renderer.geometry(&document)),
            "undo must invalidate cached geometry"
        );
        assert_eq!(
            renderer.geometry(&document).mesh.vertices,
            document.mesh().vertices
        );
        document.invert_mask().unwrap();
        document.delete_masked().unwrap();
        let empty = renderer.geometry(&document);
        assert!(empty.mesh.vertices.is_empty());
        {
            let mut gpu = renderer.gpu.lock().unwrap();
            gpu.upload(&empty);
            for (wire, mode) in [
                (false, WireMode::Visible),
                (true, WireMode::Visible),
                (true, WireMode::Transparent),
            ] {
                gpu.paint(&params, [256, 256], wire, mode, false, &Preview::default());
            }
            let rgba = gpu.read();
            assert!(
                rgba.as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [23, 26, 31, 255]),
                "an empty model must render a clean background"
            );
        }
        // Temporary shells are display-only; picking keeps the original mesh.
        let plane = Primitive::for_kind(PrimitiveKind::Plane);
        document
            .replace_geometry(Geometry::new(plane.mesh().unwrap()))
            .unwrap();
        let original = document.mesh().clone();
        document.set_temporary_thickness(0.2).unwrap();
        let base = renderer.geometry(&document);
        let shell = renderer.render_geometry.as_ref().unwrap().clone();
        assert_eq!(base.mesh, original);
        assert_eq!(shell.mesh, original.solidify(0.2).unwrap());
        shell.mesh.validate_closed().unwrap();
        {
            let mut gpu = renderer.gpu.lock().unwrap();
            gpu.upload(&shell);
            gpu.paint(
                &params,
                [256, 256],
                false,
                WireMode::Visible,
                false,
                &Preview::default(),
            );
        }
        if let Ok(path) = std::env::var("CLAY_THICKNESS_PREVIEW") {
            let camera = Camera {
                yaw: 0.65,
                pitch: 0.4,
                ..Default::default()
            };
            let (right, up, view) = camera.basis();
            let params = Params::new(
                rect,
                right,
                up,
                view,
                89.6,
                rect.center(),
                shell.depth_extent,
            );
            let mut gpu = renderer.gpu.lock().unwrap();
            gpu.paint(
                &params,
                [256, 256],
                false,
                WireMode::Visible,
                false,
                &Preview::default(),
            );
            let mut pixels = vec![0; 256 * 256 * 4];
            pixels = gpu.read();
            let mut flipped = Vec::with_capacity(pixels.len());
            for row in pixels.as_chunks::<{ 256 * 4 }>().0.iter().rev() {
                flipped.extend_from_slice(row);
            }
            image::save_buffer(path, &flipped, 256, 256, image::ColorType::Rgba8).unwrap();
        }
        document.apply_thickness().unwrap();
        let baked = renderer.geometry(&document);
        assert_eq!(
            shell.mesh, baked.mesh,
            "Apply must match the preview exactly"
        );
        assert!(
            Arc::ptr_eq(&baked, renderer.render_geometry.as_ref().unwrap()),
            "baked geometry no longer needs a second display mesh"
        );
        let handle = renderer.gpu.clone();
        let mut viewport = super::super::Viewport {
            renderer: Some(renderer),
            ..Default::default()
        };
        viewport.cancel(&mut document);
        assert!(
            Arc::ptr_eq(&handle, &viewport.renderer.as_ref().unwrap().gpu),
            "gesture cancellation must keep wgpu resources"
        );
        let renderer = viewport.renderer.take().unwrap();
        renderer.destroy();
        drop(renderer);
        drop(gl);
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn gpu_batches_preserve_face_colors_and_depth() {
    let headless = Headless::new();
    let document = Document::default();
    let display = Arc::new(DisplayMesh::new(&document));
    let mut gpu = Gpu::new(&headless.state.device, &headless.state.queue, COLOR);
    gpu.upload(&display);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(253., 189.));
    let (right, up, view) = Camera::default().basis();
    let params = Params::new(
        rect,
        right,
        up,
        view,
        70.,
        rect.center(),
        display.depth_extent,
    );
    let preview = Preview {
        show_polygroups: true,
        backface_opacity: 0.3,
        ..Default::default()
    };
    gpu.paint(
        &params,
        [253, 189],
        false,
        WireMode::Visible,
        false,
        &preview,
    );
    let original = gpu.read(); // Exercises padded readback rows as well.
    gpu.upload_meshes(&display, 16_384);
    assert!(gpu.meshes.len() > 1);
    assert!(gpu.meshes.iter().all(|m| m.vertices.size() <= 16_384));
    gpu.paint(
        &params,
        [253, 189],
        false,
        WireMode::Visible,
        false,
        &preview,
    );
    assert_eq!(
        original,
        gpu.read(),
        "batch boundaries must not change pixels or depth"
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn gpu_composite_preserves_dpi_clipping_and_srgb() {
    let mut samples = Vec::new();
    for format in [COLOR, wgpu::TextureFormat::Rgba8UnormSrgb] {
        let mut capture = UiCapture::with_format([381, 285], format);
        let mut renderer = Renderer::new(&capture.state).unwrap();
        let document = Document::default();
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.5);
        let rect = egui::Rect::from_min_max(egui::pos2(10.3, 12.7), egui::pos2(243.7, 177.3));
        let clip = egui::Rect::from_min_max(egui::pos2(70., 40.), egui::pos2(190., 150.));
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(254., 190.),
                )),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                painter.rect_filled(
                    egui::Rect::EVERYTHING,
                    0.,
                    egui::Color32::from_rgb(23, 26, 31),
                );
                let painter = painter.with_clip_rect(clip);
                Scene::paint_with_renderer(
                    &painter,
                    rect,
                    &document,
                    &Camera::default(),
                    false,
                    WireMode::Visible,
                    false,
                    Some(&mut renderer),
                );
            },
        );
        assert_eq!(output.pixels_per_point, 1.5);
        let rgba = capture.render(&ctx, output);
        let sample = |x: usize, y: usize| {
            let i = ((284 - y) * 381 + x) * 4;
            <[u8; 4]>::try_from(&rgba[i..i + 4]).unwrap()
        };
        assert_eq!(
            sample(90, 140),
            [23, 26, 31, 255],
            "outside the scissor must keep the background"
        );
        let center = sample(190, 142);
        assert!(
            center[0] > 40,
            "fractional-DPI viewport must paint inside the scissor"
        );
        let target = renderer.gpu.lock().unwrap().target.as_ref().unwrap().size;
        let viewport = egui::PaintCallbackInfo {
            viewport: rect,
            clip_rect: clip,
            pixels_per_point: 1.5,
            screen_size_px: [381, 285],
        }
        .viewport_in_pixels();
        assert_eq!(
            target,
            [viewport.width_px as u32, viewport.height_px as u32]
        );
        samples.push(center);
    }
    assert!(
        samples[0]
            .iter()
            .zip(samples[1])
            .all(|(&a, b)| a.abs_diff(b) <= 1),
        "sRGB output must preserve display RGB shading: {samples:?}"
    );
}

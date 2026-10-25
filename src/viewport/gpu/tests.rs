#[cfg(target_os = "linux")]
use super::ui_capture::{Headless, UiCapture};
use super::*;
use crate::{
    camera::Camera,
    sculpt::{Geometry, Primitive, PrimitiveKind, Transform},
};

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Vulkan; verifies image pixels, checker and UV buffer batching"]
fn gpu_uv_texture_pixels_and_checker_survive_batching() {
    use crate::sculpt::{
        Material,
        texture::{Channel, Texture},
    };
    let headless = Headless::new();
    let renderer = Renderer::new(&headless.state).unwrap();
    let mut gpu = renderer.gpu.lock().unwrap();
    let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
    mesh.uv = mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
    let red = Material::from_srgb([0.8, 0.02, 0.01], 0.85, 0.);
    mesh.uv
        .textures
        .push(Texture::new(Channel::Color, 16, &red).unwrap());
    let mut doc = Document::default();
    doc.replace_geometry(Geometry::new(mesh)).unwrap();
    let display = Arc::new(DisplayMesh::new(&doc));
    gpu.upload(&display);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.));
    let (right, up, view) = (Vec3(1., 0., 0.), Vec3(0., 1., 0.), Vec3(0., 0., 1.));
    let params = Params::new(rect, right, up, view, 90., rect.center(), 3.);
    gpu.paint(
        &params,
        [256, 256],
        false,
        WireMode::Visible,
        false,
        &Preview::default(),
    );
    let rgba = gpu.read();
    let center = (128 * 256 + 128) * 4;
    assert!(
        rgba[center] > rgba[center + 1] * 2,
        "texture must colour actual surface fragments"
    );
    gpu.upload_meshes(&display, 1024);
    gpu.paint(
        &params,
        [256, 256],
        false,
        WireMode::Visible,
        false,
        &Preview::default(),
    );
    assert_eq!(gpu.read(), rgba);
    let preview = Preview {
        uv_checker: true,
        ..Default::default()
    };
    gpu.paint(
        &params,
        [256, 256],
        false,
        WireMode::Visible,
        false,
        &preview,
    );
    let checker = gpu.read();
    assert_ne!(checker, rgba);
    assert_eq!(
        checker[center],
        checker[center + 1],
        "checker overrides painted colour"
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Vulkan; checks material pixels, interpolation, masks and buffer batching"]
fn gpu_material_channels_match_linear_reference_and_survive_batching() {
    use crate::sculpt::{Material, Vec3};
    let headless = Headless::new();
    let renderer = Renderer::new(&headless.state).unwrap();
    let handle = renderer.gpu.clone();
    let mut gpu = handle.lock().unwrap();
    let mut document = Document::default();
    let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
    plane.divisions = [4, 4, 1];
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.));
    let (right, up, view) = (Vec3(1., 0., 0.), Vec3(0., 1., 0.), Vec3(0., 0., 1.));
    let params = Params::new(rect, right, up, view, 90., rect.center(), 3.);
    let center = (128 * 256 + 128) * 4;
    let mut samples = Vec::new();
    let mut layered = Material::from_srgb([0.1, 0.2, 0.3], 0.6, 0.2);
    layered.ior = 1.8;
    layered.specular = 0.8;
    layered.emission = [0.1, 0.02, 0.03];
    layered.emission_strength = 0.3;
    layered.layers[0].weight = 0.35;
    layered.layers[0].surface = Material::examples()[4].1.surface();
    layered.layers[1].weight = 0.2;
    layered.layers[1].add = 1.;
    layered.layers[1].surface.emission = [0.1, 0.4, 0.2];
    layered.layers[1].surface.emission_strength = 0.6;
    for (name, material) in Material::examples()
        .into_iter()
        .chain(std::iter::once(("Layered surface", layered)))
    {
        let mut mesh = plane.mesh().unwrap();
        mesh.materials = vec![material.clone(); mesh.vertices.len()];
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        let display = Arc::new(DisplayMesh::new(&document));
        gpu.upload(&display);
        gpu.paint(
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let rgba = gpu.read();
        let expected = material.studio_color(view, view, (view + up * 0.7 - right * 0.5).norm());
        for (i, expected) in expected.into_iter().enumerate() {
            assert!(
                (rgba[center + i] as f32 - expected.clamp(0., 1.) * 255.).abs() <= 2.,
                "{name} channel {i}: {} vs {expected}",
                rgba[center + i]
            );
        }
        if material.needs_extended_shading() {
            assert_eq!(gpu.vertex_stride, 77);
            gpu.upload_meshes(&display, 1024);
            gpu.paint(
                &params,
                [256, 256],
                false,
                WireMode::Visible,
                false,
                &Preview::default(),
            );
            assert_eq!(
                gpu.read(),
                rgba,
                "Layered channels survive batch boundaries"
            );
        } else {
            assert_eq!(gpu.vertex_stride, 12);
        }
        samples.push(rgba[center..center + 3].to_vec());
    }
    assert!(samples.windows(2).all(|s| s[0] != s[1]));
    let mut mesh = plane.mesh().unwrap();
    mesh.materials = mesh
        .vertices
        .iter()
        .map(|p| Material {
            base_color: [(p.0 + 1.) * 0.5, 0.1, 0.2],
            roughness: 0.45,
            metallic: 0.3,
            ..Default::default()
        })
        .collect();
    mesh.mask.fill(0.5);
    document.replace_geometry(Geometry::new(mesh)).unwrap();
    let display = Arc::new(DisplayMesh::new(&document));
    gpu.upload(&display);
    gpu.paint(
        &params,
        [256, 256],
        false,
        WireMode::Visible,
        false,
        &Preview::default(),
    );
    let single = gpu.read();
    let material = Material {
        base_color: [0.5, 0.1, 0.2],
        roughness: 0.45,
        metallic: 0.3,
        ..Default::default()
    };
    for (i, expected) in material
        .studio_color(view, view, (view + up * 0.7 - right * 0.5).norm())
        .into_iter()
        .enumerate()
    {
        assert!(
            (single[center + i] as f32 - expected * 255. * 0.625).abs() <= 2.,
            "interpolated colour and mask channel {i}"
        );
    }
    gpu.upload_meshes(&display, 1024);
    assert!(gpu.meshes.len() > 1);
    gpu.paint(
        &params,
        [256, 256],
        false,
        WireMode::Visible,
        false,
        &Preview::default(),
    );
    assert_eq!(
        gpu.read(),
        single,
        "material packing must survive buffer batch boundaries"
    );
}

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
        let headless = Headless::new();

        let mut renderer = Renderer::new(&headless.state).unwrap();
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
            uv_checker: false,
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
        // Compare shader alpha against CPU sculpt weights at an actual pixel,
        // including the display-only contrast boost for image alpha ghosts.
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
            let opacity = if matches!(b.brush.alpha, Alpha::Image { .. }) {
                weight.clamp(0., 1.).sqrt() * 0.75
            } else {
                weight * (90. / 255.)
            };
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
                let expected = active_solid[center + channel] as f32 * (1. - opacity)
                    + tint[channel] * opacity * 255.;
                assert!(
                    (rgba[center + channel] as f32 - expected).abs() < 2.,
                    "GPU ghost must match sculpt weights with preview opacity"
                );
            }
        }
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        let preview = Preview {
            uv_checker: false,
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
            uv_checker: false,
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
        drop(headless);
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
        uv_checker: false,
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

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Vulkan; validates movable directional/point lighting against the CPU model"]
fn gpu_lighting_types_colour_and_distance_match_cpu() {
    use crate::{
        lighting::{Light, LightKind, Lighting},
        sculpt::{Material, Vec3},
    };
    let headless = Headless::new();
    let renderer = Renderer::new(&headless.state).unwrap();
    let handle = renderer.gpu.clone();
    let mut gpu = handle.lock().unwrap();
    let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
    plane.divisions = [2, 2, 1];
    let mut mesh = plane.mesh().unwrap();
    let plain = Material::from_srgb([0.6, 0.5, 0.4], 0.6, 0.2);
    let mut layered = plain.clone();
    layered.emission = [0.02, 0.01, 0.03];
    layered.emission_strength = 1.;
    layered.subsurface = 0.3;
    layered.layers[0].weight = 0.3;
    layered.layers[0].surface.base_color = [0.2, 0.1, 0.1];
    layered.layers[0].surface.emission = [0.01, 0.02, 0.01];
    layered.layers[0].surface.emission_strength = 0.5;
    for material in [plain, layered] {
        mesh.materials = vec![material.clone(); mesh.vertices.len()];
        let mut doc = Document::default();
        doc.replace_geometry(Geometry::new(mesh.clone())).unwrap();
        gpu.upload(&Arc::new(DisplayMesh::new(&doc)));
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.));
        let (right, up, view) = (Vec3(1., 0., 0.), Vec3(0., 1., 0.), Vec3(0., 0., 1.));
        let mut params = Params::new(rect, right, up, view, 90., rect.center(), 3.);
        let configurations = [
            Lighting::default(),
            Lighting {
                lights: vec![Light {
                    azimuth: 75.,
                    elevation: 15.,
                    ..Default::default()
                }],
                fill: 0.3,
                ..Default::default()
            },
            Lighting {
                lights: vec![Light {
                    kind: LightKind::Point,
                    distance: 3.,
                    colour: [0.2, 0.5, 1.],
                    ..Default::default()
                }],
                ..Default::default()
            },
            Lighting {
                lights: vec![Light {
                    kind: LightKind::Point,
                    distance: 6.,
                    colour: [0.2, 0.5, 1.],
                    ..Default::default()
                }],
                ..Default::default()
            },
            Lighting {
                lights: vec![
                    Light {
                        intensity: 0.6,
                        colour: [1., 0.1, 0.2],
                        azimuth: -65.,
                        ..Default::default()
                    },
                    Light {
                        kind: LightKind::Point,
                        distance: 4.,
                        intensity: 0.7,
                        colour: [0.1, 0.5, 1.],
                        azimuth: 45.,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            Lighting {
                lights: vec![],
                ..Default::default()
            },
            Lighting {
                lights: vec![
                    Light {
                        intensity: 0.,
                        ..Default::default()
                    };
                    2
                ],
                ..Default::default()
            },
            Lighting {
                lights: vec![],
                fill: 0.,
                ..Default::default()
            },
            Lighting {
                lights: (0..8)
                    .map(|i| Light {
                        azimuth: i as f32 * 40. - 160.,
                        intensity: 0.2,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
        ];
        let mut samples = Vec::new();
        for lighting in configurations {
            params.lighting = lighting.clone();
            gpu.paint(
                &params,
                [256, 256],
                false,
                WireMode::Visible,
                false,
                &Preview::default(),
            );
            let rgba = gpu.read();
            let center = (128 * 256 + 128) * 4;
            let expected = material.lit_color_many(
                view,
                view,
                lighting.samples(Vec3::default(), right, up, view),
                lighting.fill,
            );
            for (i, c) in expected.into_iter().enumerate() {
                assert!(
                    (rgba[center + i] as f32 - c.clamp(0., 1.) * 255.).abs() < 3.,
                    "lighting {lighting:?}: channel {i}"
                );
            }
            samples.push(rgba[center..center + 3].to_vec());
        }
        assert_eq!(
            samples[5], samples[6],
            "Adding lights must not multiply ambient fill or emission"
        );
        assert_ne!(samples[4], samples[5], "Both light types contribute");
        assert!(
            samples[2][2] > samples[3][2],
            "Point illumination falls off with distance"
        );
    }
}

#[test]
fn dense_stroke_display_keeps_full_resolution_and_rejects_incompatible_topology() {
    use crate::sculpt::{Brush, Dab, Kind, Mesh, Vec3};
    let mut geometry = Geometry::new(Mesh::starting_sphere());
    for _ in 0..2 {
        geometry.subdivide().unwrap();
    }
    let mut doc = Document::default();
    doc.replace_geometry(geometry).unwrap();
    let mut previous = DisplayMesh::new(&doc);
    let dab = Dab {
        center: Vec3(0., 0., 1.),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 0.3,
        pressure: 1.,
        delta: Vec3::default(),
        invert: false,
    };
    for kind in [Kind::Paint, Kind::Standard, Kind::Clay] {
        let before = doc.geometry_snapshot();
        assert!(doc.apply_dab(&Brush::new(kind), dab));
        let preview = previous
            .during_stroke(&doc)
            .expect("dense fixed-topology stroke has a fast display path");
        assert_eq!(preview.mesh.vertices, doc.mesh().vertices);
        assert_eq!(preview.mesh.materials, doc.mesh().materials);
        assert_eq!(preview.mesh.mask, doc.mesh().mask);
        assert_eq!(preview.shading.triangles, previous.shading.triangles);
        assert!(
            preview
                .shading
                .vertices
                .iter()
                .all(|(_, n)| n.0.is_finite() && n.1.is_finite() && n.2.is_finite())
        );
        if kind == Kind::Paint {
            assert_eq!(preview.shading.vertices, previous.shading.vertices);
        }
        doc.end_stroke();
        assert!(preview.during_stroke(&doc).is_none());
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        previous = DisplayMesh::new(&doc);
    }
    doc.begin_stroke();
    let mut changed = doc.geometry_snapshot();
    changed.mesh.triangles.swap(0, 1);
    changed.settings.lower.clear();
    changed.settings.upper.clear();
    doc.replace_geometry(changed).unwrap();
    doc.begin_stroke();
    assert!(previous.during_stroke(&doc).is_none());
    doc.end_stroke();
    doc.select_face_region(|p| p.0 > 0., true).unwrap();
    doc.begin_stroke();
    assert!(previous.during_stroke(&doc).is_none());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Vulkan; restores full-detail shading when a dense stroke finishes"]
fn gpu_dense_stroke_display_finishes_with_exact_shading() {
    use crate::sculpt::{Brush, Dab, Kind, Mesh, Vec3};
    let headless = Headless::new();
    let mut renderer = Renderer::new(&headless.state).unwrap();
    let mut geometry = Geometry::new(Mesh::starting_sphere());
    for _ in 0..2 {
        geometry.subdivide().unwrap();
    }
    let mut doc = Document::default();
    doc.replace_geometry(geometry).unwrap();
    let initial = renderer.geometry(&doc);
    doc.apply_dab(
        &Brush::new(Kind::Clay),
        Dab {
            center: Vec3(0., 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        },
    );
    let preview = renderer.geometry(&doc);
    assert!(renderer.live_display);
    assert!(!Arc::ptr_eq(&initial, &preview));
    let revision = doc.display_revision();
    doc.end_stroke();
    assert_eq!(revision, doc.display_revision());
    let final_display = renderer.geometry(&doc);
    assert!(!renderer.live_display);
    assert!(!Arc::ptr_eq(&preview, &final_display));
    let expected = DisplayMesh::new(&doc);
    assert_eq!(final_display.shading.vertices, expected.shading.vertices);
    assert_eq!(final_display.shading.triangles, expected.shading.triangles);
    assert!(Arc::ptr_eq(&final_display, &renderer.geometry(&doc)));
}

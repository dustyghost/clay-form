use super::{ui_capture::Headless, *};
use crate::camera::Camera;

#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn gpu_visibility_hide_split_and_undo() {
    {
        let headless = Headless::new();
        let mut document = Document::default();
        let mut renderer = Renderer::new(&headless.state).unwrap();
        let mut frames = Vec::new();
        for phase in 0..4 {
            match phase {
                1 => document.select_face_region(|p| p.0 > 0., true).unwrap(),
                2 => document.split_hidden().unwrap(),
                3 => document.undo(),
                _ => (),
            }
            let display = renderer.geometry(&document);
            let (right, up, view) = Camera::default().basis();
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(512.));
            let params = Params::new(
                rect,
                right,
                up,
                view,
                180.,
                rect.center(),
                display.depth_extent,
            );
            let mut gpu = renderer.gpu.lock().unwrap();
            gpu.upload(&display);
            gpu.paint(
                &params,
                [512, 512],
                false,
                WireMode::Visible,
                false,
                &Preview::default(),
            );
            let mut pixels = vec![0; 512 * 512 * 4];
            pixels = gpu.read();
            let pixel = |x: usize| &pixels[(256 * 512 + x) * 4..(256 * 512 + x) * 4 + 4];
            let background = &[23, 26, 31, 255];
            assert_ne!(pixel(180), background, "visible half must render");
            if phase == 1 || phase == 3 {
                assert_eq!(pixel(330), background, "hidden half must be absent");
            } else {
                assert_ne!(pixel(330), background, "split restores both surfaces");
            }
            frames.push(pixels);
        }
        assert_eq!(frames[1], frames[3], "undo must restore the hidden display");
        if let Ok(path) = std::env::var("CLAY_VISIBILITY_PREVIEW") {
            let mut pixels = Vec::with_capacity(512 * 512 * 4 * 4);
            for y in (0..512).rev() {
                for frame in &frames {
                    pixels.extend_from_slice(&frame[y * 512 * 4..(y + 1) * 512 * 4]);
                }
            }
            image::save_buffer(path, &pixels, 2048, 512, image::ColorType::Rgba8).unwrap();
        }
        renderer.destroy();
    }
}

#[test]
#[ignore = "requires Vulkan; checks dense hidden-surface stroke pixels, picking and history"]
fn gpu_hidden_stroke_cache_pixels_picking_and_history() {
    check_stroke_cache_pixels_picking_and_history(true);
}

#[test]
#[ignore = "requires Vulkan; checks dense visible-surface stroke pixels, picking and history"]
fn gpu_visible_stroke_cache_pixels_picking_and_history() {
    check_stroke_cache_pixels_picking_and_history(false);
}

fn check_stroke_cache_pixels_picking_and_history(hidden: bool) {
    use crate::sculpt::{Brush, Dab, Geometry, Kind, Material, Mesh};
    fn draw(renderer: &Renderer, display: &Arc<DisplayMesh>, wire: bool) -> Vec<u8> {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.));
        let (right, up, view) = Camera::default().basis();
        let params = Params::new(
            rect,
            right,
            up,
            view,
            89.6,
            rect.center(),
            display.depth_extent,
        );
        let mut gpu = renderer.gpu.lock().unwrap();
        gpu.upload(display);
        gpu.paint(
            &params,
            [256, 256],
            wire,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        gpu.read()
    }
    let headless = Headless::new();
    let mut rows = Vec::new();
    for kind in [Kind::Standard, Kind::Paint] {
        let mut geometry = Geometry::new(Mesh::starting_sphere());
        for _ in 0..2 {
            geometry.subdivide().unwrap();
        }
        let mesh = &mut geometry.mesh;
        mesh.hidden_faces = mesh
            .triangles
            .iter()
            .map(|t| hidden && t.iter().all(|&i| mesh.vertices[i].0 > 0.1))
            .collect();
        mesh.mask = mesh
            .vertices
            .iter()
            .map(|p| if p.0 < -0.5 { 1. } else { 0. })
            .collect();
        mesh.materials = vec![Material::from_srgb([0.2, 0.4, 0.6], 0.65, 0.); mesh.vertices.len()];
        let mut doc = Document::default();
        doc.replace_geometry(geometry).unwrap();
        let original = doc.mesh().clone();
        let mut renderer = Renderer::new(&headless.state).unwrap();
        let initial = renderer.geometry(&doc);
        let before = draw(&renderer, &initial, false);
        let mut brush = Brush::new(kind);
        brush.strength = 0.15;
        brush.set_paint_colour(false, [0.9, 0.1, 0.2]);
        let mut live = initial;
        for step in 0..3 {
            assert!(doc.apply_dab(
                &brush,
                Dab {
                    center: Vec3(-0.2 + step as f32 * 0.03, 0., 0.98),
                    normal: Vec3(0., 0., 1.),
                    right: Vec3(1., 0., 0.),
                    up: Vec3(0., 1., 0.),
                    radius: 0.4,
                    pressure: 0.8,
                    delta: Vec3::default(),
                    invert: false,
                }
            ));
            live = renderer.geometry(&doc);
            assert!(
                renderer.live_display,
                "dense geometry must take the cached path"
            );
        }
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.));
        let scene = Scene::from_geometry(rect, &Camera::default(), live.clone(), false);
        assert!(scene.pick(egui::pos2(100., 128.)).is_some());
        assert_eq!(
            scene.pick(egui::pos2(190., 128.)).is_none(),
            hidden,
            "picking must follow face visibility"
        );
        assert_eq!(scene.pick_polygroup(egui::pos2(100., 128.), &doc), Some(1));
        let canonical = Arc::new(DisplayMesh::new(&doc));
        let background = [23, 26, 31, 255];
        let mut live_pixels = Vec::new();
        for wire in [false, true] {
            let actual = draw(&renderer, &live, wire);
            let expected = draw(&renderer, &canonical, wire);
            for (a, b) in actual
                .as_chunks::<4>()
                .0
                .iter()
                .zip(expected.as_chunks::<4>().0)
            {
                if *b == background {
                    assert_eq!(a, b, "hidden/background pixels must stay absent");
                }
            }
            if kind == Kind::Paint {
                assert_eq!(
                    actual, expected,
                    "Paint preserves exact canonical normals and pixels"
                );
            } else {
                let differences: Vec<_> = actual
                    .iter()
                    .zip(&expected)
                    .map(|(&a, &b)| a.abs_diff(b))
                    .collect();
                assert!(
                    *differences.iter().max().unwrap() <= 12,
                    "temporary normals must remain close to canonical shading"
                );
                assert!(
                    differences.iter().map(|&v| v as usize).sum::<usize>() as f64
                        / (differences.len() as f64)
                        < 0.5
                );
            }
            if !wire {
                live_pixels = actual;
            }
        }
        assert_ne!(before, live_pixels, "the stroke must be visible");
        doc.end_stroke();
        let finished = renderer.geometry(&doc);
        assert!(!renderer.live_display);
        if kind == Kind::Paint {
            assert!(
                Arc::ptr_eq(&live, &finished),
                "Paint release must reuse exact geometry"
            );
        }
        let after = draw(&renderer, &finished, false);
        assert_eq!(
            after,
            draw(&renderer, &canonical, false),
            "release must restore exact canonical shading"
        );
        doc.undo();
        assert_eq!(doc.mesh(), &original);
        let restored = renderer.geometry(&doc);
        let undone = draw(&renderer, &restored, false);
        assert_eq!(undone, before);
        doc.redo();
        let redone = renderer.geometry(&doc);
        assert_eq!(draw(&renderer, &redone, false), after);
        if hidden {
            doc.show_all_faces().unwrap();
            let all = renderer.geometry(&doc);
            assert_eq!(all.mesh.triangles.len(), doc.mesh().triangles.len());
            let scene = Scene::from_geometry(rect, &Camera::default(), all.clone(), false);
            assert!(scene.pick(egui::pos2(190., 128.)).is_some());
            let shown = draw(&renderer, &all, false);
            assert_ne!(
                &shown[(128 * 256 + 190) * 4..(128 * 256 + 190) * 4 + 4],
                &background
            );
            doc.undo();
            let hidden = renderer.geometry(&doc);
            assert_eq!(draw(&renderer, &hidden, false), after);
        }
        rows.push([before, live_pixels, after, undone]);
        renderer.destroy();
    }
    if let Ok(path) = std::env::var(if hidden {
        "CLAY_HIDDEN_STROKE_PREVIEW"
    } else {
        "CLAY_VISIBLE_STROKE_PREVIEW"
    }) {
        let mut pixels = Vec::new();
        for row in rows {
            for y in (0..256).rev() {
                for frame in &row {
                    pixels.extend_from_slice(&frame[y * 256 * 4..(y + 1) * 256 * 4]);
                }
            }
        }
        image::save_buffer(path, &pixels, 1024, 512, image::ColorType::Rgba8).unwrap();
    }
}

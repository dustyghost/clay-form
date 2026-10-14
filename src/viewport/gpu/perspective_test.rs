//! Compare actual perspective framebuffer coverage and alpha against CPU picks.
use super::ui_capture::UiCapture;
use super::*;
use crate::{
    camera::Camera,
    sculpt::{Geometry, Mesh, Primitive, PrimitiveKind},
};

fn render(
    capture: &mut UiCapture,
    ctx: &egui::Context,
    renderer: &mut Renderer,
    document: &Document,
    camera: &Camera,
    wire: bool,
) -> (Scene, Vec<u8>) {
    let mut scene = None;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::splat(512.),
            )),
            ..Default::default()
        },
        |ctx| {
            scene = Some(Scene::paint_with_renderer(
                &ctx.layer_painter(egui::LayerId::background()),
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(512.)),
                document,
                camera,
                wire,
                WireMode::Visible,
                false,
                Some(renderer),
            ));
        },
    );
    (scene.unwrap(), capture.render(ctx, output))
}
fn pixel(rgba: &[u8], x: usize, y: usize) -> &[u8] {
    let i = ((511 - y) * 512 + x) * 4;
    &rgba[i..i + 3]
}
fn coverage(scene: &Scene, rgba: &[u8]) {
    let background = &[23, 26, 31];
    let mut hits = 0;
    for y in (12..500).step_by(11) {
        for x in (12..500).step_by(11) {
            let hit = scene
                .pick_model(egui::pos2(x as f32 + 0.5, y as f32 + 0.5))
                .is_some();
            if hit {
                hits += 1;
            }
            assert_eq!(
                hit,
                pixel(rgba, x, y) != background,
                "GPU/pick disagreement at {x},{y}"
            );
        }
    }
    assert!(hits > 50);
}
#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_perspective_picking_alpha_wireframe_and_clipping() {
    let mut capture = UiCapture::new([512, 512]);
    let ctx = egui::Context::default();
    let mut renderer = Renderer::new(&capture.gl).unwrap();
    let mut document = Document::default();
    let cube = Primitive::for_kind(PrimitiveKind::Cube);
    document
        .replace_geometry(Geometry::new(cube.mesh().unwrap()))
        .unwrap();
    let mut camera = Camera {
        perspective: true,
        field_of_view: 65.,
        zoom: 0.6,
        yaw: 0.5,
        pitch: 0.25,
        pan: egui::vec2(17., -12.),
        ..Default::default()
    };
    let cached = renderer.geometry(&document);
    let (scene, baseline) = render(&mut capture, &ctx, &mut renderer, &document, &camera, false);
    coverage(&scene, &baseline);
    let (_, wire) = render(&mut capture, &ctx, &mut renderer, &document, &camera, true);
    assert_ne!(wire, baseline);
    assert!(Arc::ptr_eq(&cached, &renderer.geometry(&document)));
    let point = scene.project(Vec3(0.2, 0.1, 0.5));
    let (center, normal) = scene.pick(point).unwrap();
    let brush = Brush::new(crate::sculpt::Kind::Standard);
    renderer.set_brush(
        &scene,
        point,
        60.,
        &brush,
        egui::Color32::from_rgb(125, 210, 245),
    );
    let (_, alpha) = render(&mut capture, &ctx, &mut renderer, &document, &camera, false);
    let radius = 60. / scene.scale_at(center);
    let mut affected = 0;
    for y in (12..500).step_by(7) {
        for x in (12..500).step_by(7) {
            if let Some((p, n)) = scene.pick(egui::pos2(x as f32 + 0.5, y as f32 + 0.5)) {
                let offset = p - center;
                let weight = brush.weight(
                    offset.dot(scene.right) / radius,
                    offset.dot(scene.up) / radius,
                );
                if offset.len() < radius * 1.5 && n.dot(normal) > 0.1 && weight > 0.15 {
                    assert_ne!(pixel(&baseline, x, y), pixel(&alpha, x, y));
                    affected += 1;
                } else if offset.len() > radius * 1.5 || weight == 0. {
                    assert_eq!(pixel(&baseline, x, y), pixel(&alpha, x, y));
                }
            }
        }
    }
    assert!(affected > 30);
    renderer.clear_preview();
    let symmetry = Symmetry {
        enabled: true,
        ..Default::default()
    };
    renderer.set_guides(&symmetry, 1.);
    let (_, guides) = render(&mut capture, &ctx, &mut renderer, &document, &camera, false);
    assert_ne!(guides, baseline);
    capture.save("/tmp/clay-perspective-cube.png", &guides);
    renderer.clear_preview();
    // Camera-only changes must use the same GPU mesh buffers.
    camera.zoom = 8.;
    camera.field_of_view = 75.;
    let (close, pixels) = render(&mut capture, &ctx, &mut renderer, &document, &camera, false);
    assert!(close.pick(close.center).is_some());
    assert_ne!(pixel(&pixels, 256, 256), &[23, 26, 31]);
    assert!(Arc::ptr_eq(&cached, &renderer.geometry(&document)));
    // Cross the near/eye planes, then exercise a distant surface with dynamic far clipping.
    camera = Camera {
        perspective: true,
        ..Default::default()
    };
    for vertices in [
        vec![
            Vec3(-1., -1., 0.),
            Vec3(1., -1., 0.),
            Vec3(0., 1., camera.eye_distance() + 0.5),
        ],
        vec![
            Vec3(-500., -500., -1000.),
            Vec3(500., -500., -1000.),
            Vec3(0., 500., -1000.),
        ],
    ] {
        let mesh = Mesh {
            vertices,
            triangles: vec![[0, 1, 2]],
            quads: vec![],
            mask: vec![0.; 3],
            vertex_groups: vec![],
            polygroups: vec![],
            hidden_faces: vec![],
        };
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        let (scene, rgba) = render(&mut capture, &ctx, &mut renderer, &document, &camera, false);
        coverage(&scene, &rgba);
    }
    renderer.destroy(&capture.gl);
}

//! Actual framebuffer checks for the shared rotation UI and primitive placement.
use super::ui_capture::UiCapture;
use crate::{
    App,
    command::Command,
    sculpt::{Primitive, PrimitiveKind, Vec3},
    viewport::{TransformMode, TransformOperation},
};
use eframe::egui;

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_rotation_controls_and_primitive_regeneration() {
    let mut capture = UiCapture::new([1000, 800]);
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.viewport.initialize_renderer(&capture.gl).unwrap();
    app.document.initialize_primitive().unwrap();
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.size = Vec3(1.4, 0.8, 1.);
    primitive.divisions = [2; 3];
    app.document.update_primitive(primitive).unwrap();
    app.document.end_primitive_edit();
    app.camera.yaw = 0.45;
    app.camera.pitch = 0.3;
    app.dispatch(Command::TransformMode(TransformMode::World), &ctx);
    app.dispatch(
        Command::TransformOperation(TransformOperation::Rotate),
        &ctx,
    );
    let count = app.document.history_counts().0;
    let mut rect = egui::Rect::NOTHING;
    let mut before = Vec::new();
    for step in 0..7 {
        let center = rect.center();
        let ring_point = |angle: f32| {
            let (right, up, _) = app.camera.basis();
            let p = Vec3(angle.cos(), angle.sin(), 0.);
            center + egui::vec2(p.dot(right), -p.dot(up)) * 85.
        };
        let a = ring_point(0.7);
        let b = ring_point(1.5);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let events = match step {
            2 => vec![egui::Event::PointerMoved(a), button(a, true)],
            3 => vec![egui::Event::PointerMoved(b)],
            5 => vec![button(b, false)],
            _ => vec![],
        };
        if step == 6 {
            let mut p = app.document.primitive().unwrap().clone();
            p.size.1 *= 1.5;
            let placement = (p.center, p.basis);
            app.document.update_primitive(p).unwrap();
            app.document.end_primitive_edit();
            let p = app.document.primitive().unwrap();
            assert_eq!((p.center, p.basis), placement);
        }
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    rect = ui.available_rect_before_wrap();
                    app.show_viewport(ui);
                });
            },
        );
        if step == 4 {
            assert!(output.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().ends_with('°'))
            ));
            assert_eq!(app.document.history_counts().0, count + 1);
        }
        let rgba = capture.render(&ctx, output);
        if step == 1 {
            let mut colors = [0; 3];
            for y in 285..515 {
                for x in 385..615 {
                    let p = &rgba[(y * 1000 + x) * 4..][..3];
                    for i in 0..3 {
                        if p[i] > 100
                            && p[i] as f32 > p[(i + 1) % 3] as f32 * 1.2
                            && p[i] as f32 > p[(i + 2) % 3] as f32 * 1.2
                        {
                            colors[i] += 1;
                        }
                    }
                }
            }
            assert!(
                colors.iter().all(|&n| n > 20),
                "axis rings must render in all three colors: {colors:?}"
            );
            before = rgba.clone();
        }
        if step == 4 || step == 6 {
            assert_ne!(rgba, before);
        }
        if [1, 4, 6].contains(&step)
            && let Ok(path) = std::env::var("CLAY_ROTATION_PREVIEW")
        {
            capture.save(format!("{path}-{step}.png"), &rgba);
        }
    }
    app.viewport.destroy_renderer(&capture.gl);
}

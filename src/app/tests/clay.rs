use super::*;
use crate::sculpt::{AdaptiveSettings, Geometry, Kind, Primitive, PrimitiveKind};

fn stroke(app: &mut App, ctx: &egui::Context, offset: f32) {
    let rect = navigation_frame(app, ctx, vec![]);
    let start = rect.center() + Vec2::new(offset, -130.);
    navigation_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    let mut end = start;
    for step in 1..=40 {
        let t = step as f32 / 40.;
        end = start + Vec2::new(20. * (t * std::f32::consts::PI).sin(), 260. * t);
        navigation_frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
    }
    navigation_frame(
        app,
        ctx,
        vec![primary_event(end, false), egui::Event::PointerGone],
    );
}

fn scene(kind: Kind, flat: bool, adaptive: bool) -> App {
    let mut app = App::default();
    if flat {
        let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
        cube.divisions = [40, 40, 2];
        app.document
            .replace_geometry(Geometry::new(cube.mesh().unwrap()))
            .unwrap();
    }
    app.ui.selected = app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == kind)
        .unwrap();
    app.document
        .set_adaptive(AdaptiveSettings {
            enabled: adaptive,
            ..Default::default()
        })
        .unwrap();
    app
}

#[test]
fn clay_viewport_stroke_restores_geometry_and_topology_with_one_undo() {
    for adaptive in [false, true] {
        let mut app = scene(Kind::Clay, false, adaptive);
        let ctx = egui::Context::default();
        let before = app.document.geometry_snapshot();
        let history = app.document.history_counts();
        stroke(&mut app, &ctx, 0.);
        let after = app.document.geometry_snapshot();
        assert_ne!(before, after);
        after.mesh.validate_closed().unwrap();
        assert_eq!(app.document.history_counts().0, history.0 + 1);
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Redo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), after);
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn clay_buildup_offscreen_preview() {
    let mut capture = crate::viewport::UiCapture::new([1000, 800]);
    for (kind, flat, adaptive) in [
        (Kind::Standard, true, false),
        (Kind::Clay, true, false),
        (Kind::Standard, false, false),
        (Kind::Clay, false, false),
        (Kind::Clay, false, true),
    ] {
        let mut app = scene(kind, flat, adaptive);
        let ctx = egui::Context::default();
        for offset in [-110., 40., 40., 40.] {
            stroke(&mut app, &ctx, offset);
        }
        app.document.mesh().validate_closed().unwrap();
        app.viewport.initialize_renderer(&capture.state).unwrap();
        app.camera.yaw = 0.3;
        app.camera.pitch = -0.15;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000., 800.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
        let rgba = capture.render(&ctx, output);
        if let Ok(prefix) = std::env::var("CLAY_BUILDUP_PREVIEW") {
            capture.save(
                format!(
                    "{prefix}-{kind:?}-{}-{}.png",
                    if flat { "flat" } else { "sphere" },
                    if adaptive { "adaptive" } else { "fixed" }
                ),
                &rgba,
            );
        }
        app.viewport.destroy_renderer();
    }
}

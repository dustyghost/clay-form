use super::*;
use crate::sculpt::{AdaptiveSettings, Kind};

#[test]
fn polish_can_be_added_to_old_projects_and_roundtrips_with_one_stroke_undo() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut project = app.document.project();
    project.brushes.retain(|b| b.kind != Kind::Polish);
    app.document.replace_project(project, "old.clay").unwrap();
    app.dispatch(Command::Action(Action::NewBrush("Polish".into())), &ctx);
    let brush = app.document.brushes()[app.ui.selected].clone();
    assert_eq!(brush.kind, Kind::Polish);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    for _ in 0..4 {
        app.document.apply_dab(
            &brush,
            crate::sculpt::Dab {
                center: crate::sculpt::Vec3(0., 0., 1.),
                normal: crate::sculpt::Vec3(0., 0., 1.),
                right: crate::sculpt::Vec3(1., 0., 0.),
                up: crate::sculpt::Vec3(0., 1., 0.),
                radius: 0.5,
                pressure: 1.,
                delta: Default::default(),
                invert: false,
            },
        );
    }
    app.document.end_stroke();
    assert_ne!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    let serialized = serde_json::to_string(&app.document.project()).unwrap();
    let loaded: crate::sculpt::Project = serde_json::from_str(&serialized).unwrap();
    loaded.validate().unwrap();
    assert!(loaded.brushes.iter().any(|b| b.kind == Kind::Polish));
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}

fn detail_groove(adaptive: AdaptiveSettings, stabilize: bool) -> (App, egui::Context) {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.selected = app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == Kind::Detail)
        .unwrap();
    let mut brush = app.document.brushes()[app.ui.selected].clone();
    brush.stabilize = stabilize;
    app.document.update_brush(app.ui.selected, brush).unwrap();
    app.document.set_adaptive(adaptive).unwrap();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let start = rect.center() + Vec2::new(-20., -140.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    for i in 1..=60 {
        let t = i as f32 / 60.;
        let point = start + Vec2::new(25. * (t * 3.).sin(), t * 280.);
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
    }
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            primary_event(start + Vec2::new(0., 280.), false),
            egui::Event::PointerGone,
        ],
    );
    (app, ctx)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn detail_groove_offscreen_preview() {
    let mut capture = crate::viewport::UiCapture::new([1000, 800]);
    let (mut app, ctx) = detail_groove(
        AdaptiveSettings {
            enabled: true,
            ..Default::default()
        },
        false,
    );
    app.viewport.initialize_renderer(&capture.state).unwrap();
    app.camera.set_zoom(2.);
    app.document.mesh().validate_closed().unwrap();
    let mesh = app.document.mesh();
    eprintln!(
        "Detail preview: {} split normals",
        mesh.surface_shading()
            .vertices
            .len()
            .saturating_sub(mesh.vertices.len())
    );
    for wire in [false, true] {
        app.ui.wire = wire;
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
        if let Ok(path) = std::env::var("CLAY_DETAIL_PREVIEW") {
            capture.save(
                format!("{path}-{}.png", if wire { "wire" } else { "solid" }),
                &rgba,
            );
        }
    }
    app.viewport.destroy_renderer();
}

#[test]
fn reset_brush_settings_restores_behavior_and_keeps_preset_identity() {
    use crate::sculpt::{Alpha, Brush, BrushIcon, StrokeMode};
    for kind in Kind::ALL {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.dispatch(Command::AddBrush(kind), &ctx);
        let index = app.ui.selected;
        let mut edited = app.document.brushes()[index].clone();
        edited.name = "My variation".into();
        edited.icon = Some(BrushIcon {
            size: 1,
            rgba: vec![10, 20, 30, 255].into(),
        });
        edited.alpha = Alpha::Square;
        edited.radius = 123.;
        edited.strength = 0.8;
        edited.falloff = 3.;
        edited.inner_radius = 0.5;
        edited.surface_cursor = true;
        edited.stroke_mode = StrokeMode::DragDot;
        edited.stabilize = true;
        edited.stabilizer_distance = 90.;
        edited.spacing = 0.3;
        edited.rotation = 27.;
        edited.subtract = true;
        edited.pressure = false;
        edited.pressure_radius = true;
        edited.pressure_curve = 2.;
        app.document.update_brush(index, edited.clone()).unwrap();
        let original = app.document.geometry_snapshot();
        let untouched = app.document.brushes()[0].clone();
        app.dispatch(Command::Action(Action::ResetBrushSettings), &ctx);
        let mut expected = Brush::new(kind);
        expected.id = edited.id;
        expected.name = edited.name;
        expected.icon = edited.icon;
        assert!(app.document.brushes()[index] == expected);
        assert!(app.document.brushes()[0] == untouched);
        assert_eq!(app.document.geometry_snapshot(), original);
        assert!(!app.document.can_undo());
        app.document.mark_saved("reset.clay");
        app.dispatch(Command::Action(Action::ResetBrushSettings), &ctx);
        assert!(
            !app.document.is_dirty(),
            "resetting defaults again is a no-op"
        );
    }
}

#[test]
fn reset_brush_settings_targets_temporary_smooth_and_respects_modal_guards() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let smooth = app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == Kind::Smooth)
        .unwrap();
    let mut brush = app.document.brushes()[smooth].clone();
    brush.radius = 200.;
    app.document.update_brush(smooth, brush).unwrap();
    let other = app.document.brushes()[0].clone();
    app.ui.selected = smooth;
    app.ui.pending = Some(crate::command::PendingAction::New);
    app.dispatch(Command::Action(Action::ResetBrushSettings), &ctx);
    assert_eq!(app.document.brushes()[smooth].radius, 200.);
    app.ui.pending = None;
    app.ui.selected = 0;
    let _ = ctx.run(
        egui::RawInput {
            modifiers: egui::Modifiers::SHIFT,
            ..Default::default()
        },
        |ctx| {
            app.dispatch(Command::Action(Action::ResetBrushSettings), ctx);
        },
    );
    assert_eq!(app.document.brushes()[smooth].radius, 60.);
    assert!(app.document.brushes()[0] == other);
    assert_eq!(app.ui.selected, 0);
}

#[test]
fn adaptive_detail_groove_does_not_fold_with_or_without_stabilization() {
    for stabilize in [false, true] {
        let (mut app, ctx) = detail_groove(
            AdaptiveSettings {
                enabled: true,
                ..Default::default()
            },
            stabilize,
        );
        let mesh = app.document.mesh();
        mesh.validate_closed().unwrap();
        let splits = mesh
            .surface_shading()
            .vertices
            .len()
            .saturating_sub(mesh.vertices.len());
        assert_eq!(
            splits, 0,
            "the smooth valley must not produce isolated hard-normal fans (stabilize={stabilize})"
        );
        let inward = mesh
            .triangles
            .iter()
            .filter(|&&[a, b, c]| {
                let normal = (mesh.vertices[b] - mesh.vertices[a])
                    .cross(mesh.vertices[c] - mesh.vertices[a]);
                normal.dot(mesh.vertices[a] + mesh.vertices[b] + mesh.vertices[c]) <= 0.
            })
            .count();
        assert_eq!(
            inward, 0,
            "Detail groove folded back through the sphere surface (stabilize={stabilize})"
        );
        assert!(
            mesh.vertices.iter().any(|v| v.2 > 0. && v.len() < 0.98),
            "the fix must preserve the incision"
        );
        assert!(
            mesh.vertices.iter().any(|v| v.2 > 0. && v.len() > 1.001),
            "the fix must preserve raised shoulders"
        );
        let original = crate::sculpt::Mesh::starting_sphere();
        for v in original.vertices.iter().filter(|v| v.2 < 0.) {
            assert!(
                mesh.vertices.contains(v),
                "the back of the sphere must not change"
            );
        }
        let after = mesh.clone();
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.mesh(), &original);
        app.dispatch(Command::Redo, &ctx);
        assert_eq!(app.document.mesh(), &after);
    }
}

#[test]
fn alpha_drag_default_and_add_menu_preserve_standard_and_roundtrip() {
    use crate::sculpt::{Alpha, Brush, Project, StrokeMode};
    let mut app = App::default();
    let standard = app.document.brushes()[0].clone();
    let preset = app
        .document
        .brushes()
        .iter()
        .find(|b| b.name == "Alpha Drag")
        .unwrap()
        .clone();
    assert_eq!(preset.kind, Kind::Standard);
    assert_eq!(preset.stroke_mode, StrokeMode::DragRect);
    assert!(matches!(preset.alpha, Alpha::Image { .. }));
    assert_eq!(
        preset.alpha,
        crate::sculpt::alpha::builtins()
            .iter()
            .find(|a| a.name == "Wood grain")
            .unwrap()
            .image
    );
    assert_ne!(preset.identity(), standard.identity());
    assert_eq!(preset.strength, standard.strength);
    assert_eq!(standard.stroke_mode, StrokeMode::Freehand);
    let mut project = app.document.project();
    project.brushes.retain(|b| b.name != "Alpha Drag");
    app.document.replace_project(project, "old.clay").unwrap();
    let ctx = egui::Context::default();
    app.dispatch(Command::Action(Action::NewBrush("Alpha Drag".into())), &ctx);
    assert!(app.document.brushes()[app.ui.selected] == preset);
    app.dispatch(Command::Action(Action::NewBrush("Alpha Drag".into())), &ctx);
    assert_ne!(
        app.document.brushes()[app.ui.selected].identity(),
        preset.identity()
    );
    assert!(app.document.brushes()[0] == standard);
    let reopened: Project =
        serde_json::from_slice(&serde_json::to_vec(&app.document.project()).unwrap()).unwrap();
    reopened.validate().unwrap();
    assert_eq!(
        reopened.brushes.last().unwrap().alpha,
        Brush::alpha_drag().alpha
    );
    assert_eq!(
        reopened.brushes.last().unwrap().stroke_mode,
        StrokeMode::DragRect
    );
}

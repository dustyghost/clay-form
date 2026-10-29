use super::*;
use crate::preferences::{
    ViewportControl, ViewportControlIcon as Icon, ViewportControlKind as Kind,
    ViewportControlStyle as Style,
};

fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            app.ui.viewport_controls_editor(ctx, &app.document);
        },
    )
}

fn center(output: &egui::FullOutput, label: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing {label}"))
}

fn control_rect(ctx: &egui::Context, index: usize) -> egui::Rect {
    ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("viewport_control_response", index))))
        .unwrap()
}

fn click(app: &mut App, ctx: &egui::Context, position: Pos2) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(position),
            primary_event(position, true),
        ],
    );
    frame(app, ctx, vec![primary_event(position, false)]);
}

fn settle(app: &mut App, ctx: &egui::Context) -> egui::FullOutput {
    for _ in 0..3 {
        frame(app, ctx, vec![]);
    }
    frame(app, ctx, vec![])
}

#[test]
fn viewport_controls_drag_camera_without_editing_clay_even_after_leaving_button() {
    for kind in [Kind::Pan, Kind::Zoom, Kind::Orbit, Kind::Roll] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.camera.zoom = 5.;
        app.ui.preferences.viewport_controls = vec![ViewportControl {
            kind: kind.clone(),
            label: "Navigation".into(),
            position: [0.5, 0.5],
            offset: [0.; 2],
            style: Style::Text,
            icon: Icon::Auto,
        }];
        let before = app.document.geometry_snapshot();
        let output = settle(&mut app, &ctx);
        let start = center(&output, "Navigation");
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        let end = start + Vec2::new(75., -60.);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![primary_event(end, false)]);
        match kind {
            Kind::Pan => assert_eq!(app.camera.pan, Vec2::new(75., -60.)),
            Kind::Zoom => assert!(app.camera.zoom > 5.),
            Kind::Orbit => assert!(app.camera.yaw.abs() > 0.1 && app.camera.pitch.abs() > 0.1),
            Kind::Roll => {
                assert!((app.camera.roll - 0.6).abs() < 1e-6);
                assert_eq!(app.camera.yaw, 0.);
                assert_eq!(app.camera.pitch, 0.);
            }
            _ => unreachable!(),
        }
        assert_eq!(app.document.geometry_snapshot(), before);
        assert!(!app.document.can_undo());
        assert!(app.viewport.session.gesture.is_none());
    }
}

#[test]
fn viewport_controls_toggles_reset_and_layout_drag_use_real_buttons() {
    let mut app = App::default();
    for control in &mut app.ui.preferences.viewport_controls {
        control.style = Style::Text;
    }
    let ctx = egui::Context::default();
    let output = settle(&mut app, &ctx);
    let wire = center(&output, "Toggle wireframe");
    let was_wire = app.ui.wire;
    click(&mut app, &ctx, wire);
    assert_ne!(app.ui.wire, was_wire);
    let groups = center(&settle(&mut app, &ctx), "Vertex groups");
    let was_groups = app.ui.show_polygroups;
    click(&mut app, &ctx, groups);
    assert_ne!(app.ui.show_polygroups, was_groups);
    app.camera.pan = Vec2::new(80., -25.);
    app.camera.zoom = 3.;
    app.camera.yaw = 1.;
    app.camera.roll = 0.7;
    let reset = center(&settle(&mut app, &ctx), "Reset view");
    click(&mut app, &ctx, reset);
    assert_eq!(app.camera.pan, Vec2::ZERO);
    assert_eq!(app.camera.zoom, 1.);
    assert_eq!(app.camera.yaw, 0.);
    assert_eq!(app.camera.roll, 0.);

    let before = app.ui.preferences.viewport_controls[0].position;
    app.ui.viewport_controls.arranging = true;
    let start = center(&settle(&mut app, &ctx), "Pan view");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    let end = start - Vec2::new(180., 100.);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_ne!(app.ui.preferences.viewport_controls[0].position, before);
    assert_eq!(app.camera.pan, Vec2::ZERO);
    assert!(!app.document.can_undo());
}

#[test]
fn viewport_controls_support_brush_values_and_block_navigation_behind_editor() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.preferences.viewport_controls = vec![ViewportControl {
        kind: Kind::Action(Action::BrushStrength),
        label: String::new(),
        position: [0.5, 0.5],
        offset: [0.; 2],
        style: Style::Text,
        icon: Icon::Auto,
    }];
    let output = settle(&mut app, &ctx);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Strength")));
    app.ui.viewport_controls.editor_open = true;
    let before = app.camera;
    let position = Pos2::new(850., 700.);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(position),
            primary_event(position, true),
        ],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(position + Vec2::new(40., 40.))],
    );
    frame(&mut app, &ctx, vec![primary_event(position, false)]);
    assert_eq!(app.camera.pan, before.pan);
    assert_eq!(app.camera.yaw, before.yaw);
    assert!(!app.document.can_undo());
}

#[test]
fn viewport_controls_editor_adds_places_and_removes_controls() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.preferences.viewport_controls.clear();
    let launcher = center(&settle(&mut app, &ctx), "Viewport controls…");
    click(&mut app, &ctx, launcher);
    assert!(app.ui.viewport_controls.editor_open);
    let add = center(&settle(&mut app, &ctx), "Add");
    click(&mut app, &ctx, add);
    assert_eq!(app.ui.preferences.viewport_controls.len(), 1);
    assert_eq!(app.ui.preferences.viewport_controls[0].kind, Kind::Pan);
    let place = center(&settle(&mut app, &ctx), "Place controls");
    click(&mut app, &ctx, place);
    assert!(!app.ui.viewport_controls.editor_open);
    assert!(app.ui.viewport_controls.arranging);
    let done = center(&settle(&mut app, &ctx), "Done placing controls");
    click(&mut app, &ctx, done);
    assert!(!app.ui.viewport_controls.arranging);

    settle(&mut app, &ctx);
    let pan = control_rect(&ctx, 0).center();
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(pan),
                egui::Event::PointerButton {
                    pos: pan,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    let remove = center(&settle(&mut app, &ctx), "Remove from viewport");
    click(&mut app, &ctx, remove);
    assert!(app.ui.preferences.viewport_controls.is_empty());
    assert!(!app.document.can_undo());
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "manual viewport control visual review; requires Vulkan"]
fn viewport_controls_offscreen_preview() {
    for (name, size) in [
        ("normal", [1440, 1000]),
        ("editor", [1440, 1000]),
        ("small", [1024, 768]),
        ("horizontal", [1024, 768]),
        ("labelled", [1440, 1000]),
    ] {
        let mut capture = crate::viewport::UiCapture::new(size);
        let ctx = egui::Context::default();
        let mut app = App::default();
        app.viewport.initialize_renderer(&capture.state).unwrap();
        app.ui.viewport_controls.editor_open = name == "editor";
        if name == "horizontal" {
            app.ui.align_viewport_controls(&ctx, false);
        }
        if name == "labelled" {
            for control in &mut app.ui.preferences.viewport_controls {
                control.style = Style::IconAndText;
            }
            app.ui.align_viewport_controls(&ctx, true);
        }
        let mut rgba = Vec::new();
        for _ in 0..4 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(size[0] as f32, size[1] as f32),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    app.ui.toolbar(ctx, &app.document, &app.camera);
                    app.ui.custom_surfaces(ctx, &app.document);
                    app.ui
                        .brush_panel(ctx, &app.document, "Visual review", 1., &app.camera);
                    app.ui.viewport_controls_editor(ctx, &app.document);
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
            rgba = capture.render(&ctx, output);
        }
        if let Ok(path) = std::env::var("CLAY_VIEWPORT_CONTROLS_PREVIEW") {
            capture.save(format!("{path}-{name}.png"), &rgba);
        }
    }
}

#[test]
fn viewport_controls_icons_drag_toggle_and_open_live_values() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    settle(&mut app, &ctx);
    let pan = control_rect(&ctx, 0).center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(pan), primary_event(pan, true)],
    );
    let end = pan + Vec2::new(-60., 30.);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(app.camera.pan, Vec2::new(-60., 30.));
    let was_wire = app.ui.wire;
    click(&mut app, &ctx, control_rect(&ctx, 5).center());
    assert_ne!(app.ui.wire, was_wire);
    assert!(!app.document.can_undo());

    app.ui.preferences.viewport_controls[0].kind = Kind::Action(Action::BrushStrength);
    settle(&mut app, &ctx);
    click(&mut app, &ctx, control_rect(&ctx, 0).center());
    let output = settle(&mut app, &ctx);
    center(&output, "Strength");
    let before = app.document.brushes()[0].strength;
    let value = center(&output, &format!("{:.0}%", before * 100.));
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(value), primary_event(value, true)],
    );
    let end = value + Vec2::new(35., 0.);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(app.document.brushes()[0].strength > before);
}

#[test]
fn viewport_controls_alignment_keeps_even_gaps_after_resize_and_style_changes() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    for size in [Vec2::new(1000., 800.), Vec2::new(650., 550.)] {
        for _ in 0..4 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
        }
        for i in 1..app.ui.preferences.viewport_controls.len() {
            let previous = control_rect(&ctx, i - 1);
            let next = control_rect(&ctx, i);
            assert!((previous.right() - next.right()).abs() < 1.);
            assert!((next.top() - previous.bottom() - 8.).abs() < 1.);
        }
    }
    // The editor can change styles and align before widgets have new measurements.
    for control in &mut app.ui.preferences.viewport_controls {
        control.style = Style::IconAndText;
    }
    app.ui.align_viewport_controls(&ctx, false);
    settle(&mut app, &ctx);
    for i in 1..app.ui.preferences.viewport_controls.len() {
        let previous = control_rect(&ctx, i - 1);
        let next = control_rect(&ctx, i);
        assert!((previous.bottom() - next.bottom()).abs() < 1.);
        assert!(
            (next.left() - previous.right() - 8.).abs() < 1.,
            "{previous:?}, {next:?}"
        );
    }
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "manual screen-space roll visual review; requires Vulkan"]
fn roll_view_offscreen_preview() {
    let mut capture = crate::viewport::UiCapture::new([1000, 800]);
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.viewport.initialize_renderer(&capture.state).unwrap();
    let mut primitive = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
    primitive.size = crate::sculpt::Vec3(1.4, 0.6, 0.7);
    app.document
        .replace_geometry(crate::sculpt::Geometry::new(primitive.mesh().unwrap()))
        .unwrap();
    app.camera.yaw = 0.4;
    app.camera.pitch = 0.25;
    app.ui.preferences.viewport_controls = vec![ViewportControl {
        kind: Kind::Roll,
        label: String::new(),
        position: [0.15, 0.85],
        offset: [0.; 2],
        style: Style::IconAndText,
        icon: Icon::Auto,
    }];
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let mut start = Pos2::ZERO;
    let mut initial = Vec::new();
    for step in 0..6 {
        let end = start + Vec2::new(100., 0.);
        let events = match step {
            2 => vec![egui::Event::PointerMoved(start), primary_event(start, true)],
            3 => vec![egui::Event::PointerMoved(end)],
            4 => vec![primary_event(end, false)],
            _ => vec![],
        };
        let output = frame(&mut app, &ctx, events);
        let rgba = capture.render(&ctx, output);
        if step == 1 {
            start = control_rect(&ctx, 0).center();
            capture.save("/tmp/clay-roll-before.png", &rgba);
            initial = rgba.clone();
        }
        if step == 5 {
            capture.save("/tmp/clay-roll-after.png", &rgba);
            assert_ne!(rgba, initial);
        }
    }
    assert!((app.camera.roll - 0.8).abs() < 1e-6);
    assert_eq!(app.camera.yaw, 0.4);
    assert_eq!(app.camera.pitch, 0.25);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
}

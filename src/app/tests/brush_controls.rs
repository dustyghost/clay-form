//! Interaction coverage for editable brush shelves and the S popup.
use super::*;
use crate::preferences::{Dock, Placement, Shortcut};

fn setup() -> (App, egui::Context) {
    let mut app = App::default();
    for panel in &mut app.ui.preferences.panels {
        panel.dock = Dock::Hidden;
    }
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.);
    (app, ctx)
}
fn frame(
    app: &mut App,
    ctx: &egui::Context,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            for command in app.ui.toolbar(ctx, &app.document, &app.camera) {
                app.dispatch(command, ctx);
            }
            for command in app.ui.custom_surfaces(ctx, &app.document) {
                app.dispatch(command, ctx);
            }
            for command in app
                .ui
                .brush_panel(ctx, &app.document, "Test", 1., &app.camera)
            {
                app.dispatch(command, ctx);
            }
            app.shortcuts(ctx);
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    )
}
fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn point(output: &egui::FullOutput, matches: impl Fn(&str) -> bool) -> Pos2 {
    output
        .shapes
        .iter()
        .rev()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if matches(t.galley.text()) => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
        .expect("control text is visible")
}
fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn size_and_inner_radius_drags_keep_both_outlines_at_last_brush_position() {
    for popup in [false, true] {
        let (mut app, ctx) = setup();
        let size = Vec2::new(1100., 750.);
        let mut brush = app.document.brushes()[0].clone();
        brush.inner_radius = 0.4;
        app.document.update_brush(0, brush).unwrap();
        frame(&mut app, &ctx, size, vec![]);
        frame(&mut app, &ctx, size, vec![]);
        let anchor = app.ui.viewport_menu.viewport_rect.unwrap().center();
        frame(
            &mut app,
            &ctx,
            size,
            vec![egui::Event::PointerMoved(anchor)],
        );
        if popup {
            frame(&mut app, &ctx, size, vec![key(egui::Key::S)]);
            frame(&mut app, &ctx, size, vec![]);
        }
        let geometry = app.document.geometry_snapshot();
        let mut output = frame(&mut app, &ctx, size, vec![]);
        for inner in [false, true] {
            let pos = point(&output, |s| s == if inner { "40%" } else { "60 px" });
            let before = app.document.brushes()[0].clone();
            frame(
                &mut app,
                &ctx,
                size,
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
            );
            for delta in [10., 20., 35.] {
                output = frame(
                    &mut app,
                    &ctx,
                    size,
                    vec![egui::Event::PointerMoved(pos + Vec2::new(delta, 0.))],
                );
                let brush = &app.document.brushes()[0];
                for radius in [brush.radius, brush.radius * brush.inner_radius] {
                    assert!(output.shapes.iter().any(|s| matches!(&s.shape,
                        egui::Shape::Circle(c) if c.center.distance(anchor) < 0.01 && (c.radius - radius).abs() < 0.01
                            && c.stroke.color == egui::Color32::from_rgb(125, 210, 245))),
                        "missing radius {radius} while adjusting inner={inner}, popup={popup}");
                }
            }
            if inner {
                assert!(app.document.brushes()[0].inner_radius > before.inner_radius);
                assert_eq!(app.document.brushes()[0].radius, before.radius);
            } else {
                assert!(app.document.brushes()[0].radius > before.radius);
            }
            frame(
                &mut app,
                &ctx,
                size,
                vec![button(pos + Vec2::new(35., 0.), false)],
            );
            output = frame(&mut app, &ctx, size, vec![]);
        }
        assert_eq!(app.document.geometry_snapshot(), geometry);
    }
}

#[test]
fn brush_controls_stay_visible_on_narrow_and_compact_workspaces() {
    for width in [640., 1280.] {
        for compact in [false, true] {
            let (mut app, ctx) = setup();
            app.ui.preferences.compact_toolbar = compact;
            let size = Vec2::new(width, 720.);
            frame(&mut app, &ctx, size, vec![]);
            frame(&mut app, &ctx, size, vec![]);
            let output = frame(&mut app, &ctx, size, vec![]);
            for label in [
                "Tool controls",
                "Material: Default clay",
                "Primary",
                "Secondary",
                "Swap colours",
                "Size",
                "Strength",
                "Inner radius",
                "Surface footprint and normal",
                "Stroke mode",
                "Stroke stabilisation",
            ] {
                let shape = output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.text() == label => Some((
                            s.clip_rect,
                            egui::Rect::from_min_size(t.pos, t.galley.size()),
                        )),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Missing {label}"));
                assert!(
                    shape.0.contains_rect(shape.1),
                    "{label} is clipped at width {width}"
                );
                assert!(shape.1.right() <= width, "{label} extends beyond window");
            }
        }
    }
}
#[test]
fn brush_controls_popup_drag_edits_preset_without_closing_or_sculpting() {
    let (mut app, ctx) = setup();
    let size = Vec2::new(1100., 750.);
    frame(&mut app, &ctx, size, vec![]);
    let output = frame(&mut app, &ctx, size, vec![]);
    point(&output, |s| s == "Size");
    point(&output, |s| s == "Stroke stabilisation");
    let id = app.ui.preferences.menus[0].id;
    assert_eq!(app.ui.preferences.menus[0].placement, Placement::LeftPanel);
    assert_eq!(
        app.ui.preferences.shortcut(&Action::OpenMenu(id)),
        Some(&Shortcut::new(egui::Key::S, egui::Modifiers::NONE))
    );
    let geometry = app.document.geometry_snapshot();
    frame(
        &mut app,
        &ctx,
        size,
        vec![
            egui::Event::PointerMoved(Pos2::new(420., 260.)),
            key(egui::Key::S),
        ],
    );
    frame(&mut app, &ctx, size, vec![]);
    let mut output = frame(&mut app, &ctx, size, vec![]);
    for strength in [false, true] {
        let before = app.document.brushes()[0].clone();
        let pos = if strength {
            point(&output, |s| s.ends_with('%'))
        } else {
            point(&output, |s| s.ends_with(" px") && !s.starts_with("30"))
        };
        frame(
            &mut app,
            &ctx,
            size,
            vec![egui::Event::PointerMoved(pos), button(pos, true)],
        );
        for distance in [10., 20., 35.] {
            frame(
                &mut app,
                &ctx,
                size,
                vec![egui::Event::PointerMoved(pos + Vec2::new(distance, 0.))],
            );
            assert_eq!(app.ui.customization.popup_menu, Some(id));
        }
        frame(
            &mut app,
            &ctx,
            size,
            vec![button(pos + Vec2::new(35., 0.), false)],
        );
        output = frame(&mut app, &ctx, size, vec![]);
        if strength {
            assert!(app.document.brushes()[0].strength > before.strength);
        } else {
            assert!(app.document.brushes()[0].radius > before.radius);
        }
    }
    let input = app.ui.paint_input_index(&app.document).unwrap();
    let primary = app.document.brushes()[input].material.base_color;
    let secondary = app.document.brushes()[input].secondary_colour;
    let pos = point(&output, |s| s == "Swap colours");
    frame(
        &mut app,
        &ctx,
        size,
        vec![egui::Event::PointerMoved(pos), button(pos, true)],
    );
    frame(&mut app, &ctx, size, vec![button(pos, false)]);
    output = frame(&mut app, &ctx, size, vec![]);
    assert_eq!(app.document.brushes()[input].material.base_color, secondary);
    assert_eq!(app.document.brushes()[input].secondary_colour, primary);
    assert_eq!(app.ui.customization.popup_menu, Some(id));
    let footprint = app.document.brushes()[0].surface_cursor;
    let pos = point(&output, |s| s == "Surface footprint and normal");
    frame(
        &mut app,
        &ctx,
        size,
        vec![egui::Event::PointerMoved(pos), button(pos, true)],
    );
    frame(&mut app, &ctx, size, vec![button(pos, false)]);
    assert_eq!(app.document.brushes()[0].surface_cursor, !footprint);
    assert_eq!(app.ui.customization.popup_menu, Some(id));
    assert_eq!(app.document.geometry_snapshot(), geometry);
    frame(&mut app, &ctx, size, vec![key(egui::Key::Escape)]);
    assert!(app.ui.customization.popup_menu.is_none());
}

#[test]
#[ignore = "visual brush control review; requires Vulkan and CLAY_BRUSH_CONTROLS_PREVIEW"]
fn brush_controls_offscreen_review() {
    use crate::viewport::UiCapture;
    let directory = std::path::PathBuf::from(std::env::var("CLAY_BRUSH_CONTROLS_PREVIEW").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    for width in [1280, 640] {
        let size = [width, 720];
        let mut capture = UiCapture::new(size);
        let (mut app, ctx) = setup();
        let mut brush = app.document.brushes()[0].clone();
        brush.inner_radius = 0.4;
        app.document.update_brush(0, brush).unwrap();
        app.viewport.initialize_renderer(&capture.state).unwrap();
        for popup in [false, true] {
            for i in 0..4 {
                let events = if popup && i == 0 {
                    vec![
                        egui::Event::PointerMoved(Pos2::new(width as f32 - 20., 500.)),
                        key(egui::Key::S),
                    ]
                } else {
                    vec![]
                };
                let output = frame(&mut app, &ctx, Vec2::new(width as f32, 720.), events);
                let rgba = capture.render(&ctx, output);
                if i == 3 {
                    capture.save(
                        directory.join(format!("brush-controls-{width}-{popup}.png")),
                        &rgba,
                    );
                }
            }
        }
        let screen = Vec2::new(width as f32, 720.);
        let output = frame(&mut app, &ctx, screen, vec![key(egui::Key::Escape)]);
        capture.render(&ctx, output);
        let anchor = app.ui.viewport_menu.viewport_rect.unwrap().center();
        let output = frame(
            &mut app,
            &ctx,
            screen,
            vec![egui::Event::PointerMoved(anchor)],
        );
        capture.render(&ctx, output);
        for inner in [false, true] {
            let output = frame(&mut app, &ctx, screen, vec![]);
            let pos = point(&output, |s| s == if inner { "40%" } else { "60 px" });
            capture.render(&ctx, output);
            for (step, events) in [
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
                vec![egui::Event::PointerMoved(pos + Vec2::new(15., 0.))],
                vec![egui::Event::PointerMoved(pos + Vec2::new(40., 0.))],
            ]
            .into_iter()
            .enumerate()
            {
                let output = frame(&mut app, &ctx, screen, events);
                let rgba = capture.render(&ctx, output);
                if step == 2 {
                    capture.save(
                        directory.join(format!("brush-outline-{width}-inner-{inner}.png")),
                        &rgba,
                    );
                }
            }
            let output = frame(
                &mut app,
                &ctx,
                screen,
                vec![button(pos + Vec2::new(40., 0.), false)],
            );
            capture.render(&ctx, output);
        }
        app.viewport.destroy_renderer();
    }
}

#[test]
fn appearance_controls_are_not_in_the_main_toolbar() {
    let (mut app, ctx) = setup();
    for _ in 0..2 {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1280., 720.),
                )),
                ..Default::default()
            },
            |ctx| {
                app.ui.toolbar(ctx, &app.document, &app.camera);
            },
        );
        assert!(!output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().starts_with("Material:") || t.galley.text() == "Swap colours")));
    }
}

#[test]
fn startup_defaults_and_z_swap_colours_once_per_press() {
    let (mut app, ctx) = setup();
    let input = app.ui.paint_input_index(&app.document).unwrap();
    let original = app.document.brushes()[input].clone();
    assert_eq!(
        original.material,
        crate::sculpt::Material::default().with_link(crate::sculpt::Material::DEFAULT_PRESET_ID)
    );
    assert_eq!(original.secondary_colour, [1.; 3]);
    assert_eq!(original.texture_colour, None);
    assert_eq!(original.secondary_texture_colour, None);
    let geometry = app.document.geometry_snapshot();
    let mut press = |repeat: bool| {
        if !repeat {
            let mut release = key(egui::Key::Z);
            if let egui::Event::Key { pressed, .. } = &mut release {
                *pressed = false;
            }
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![release],
                    ..Default::default()
                },
                |ctx| app.shortcuts(ctx),
            );
        }
        let mut event = key(egui::Key::Z);
        if let egui::Event::Key { repeat: value, .. } = &mut event {
            *value = repeat;
        }
        let _ = ctx.run(
            egui::RawInput {
                events: vec![event],
                ..Default::default()
            },
            |ctx| app.shortcuts(ctx),
        );
        app.document.brushes()[input].clone()
    };
    let swapped = press(false);
    assert_eq!(swapped.material.base_color, original.secondary_colour);
    assert_eq!(swapped.secondary_colour, original.material.base_color);
    assert!(
        press(true) == swapped,
        "holding Z must not flicker between colours"
    );
    let restored = press(false);
    assert_eq!(restored.material.base_color, original.material.base_color);
    assert_eq!(restored.secondary_colour, original.secondary_colour);
    assert_eq!(app.document.geometry_snapshot(), geometry);
}

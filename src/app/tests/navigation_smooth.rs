use super::*;
use crate::sculpt::{Alpha, Brush, Kind, StrokeMode};

#[test]
fn navigation_frame_is_empty_space_even_when_clay_fills_the_view() {
    for mode in 0..6 {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.camera.set_zoom(8.);
        match mode {
            1 => app.ui.selection_tool = Some(crate::viewport::SelectionTool::MaskLasso),
            2 => app.ui.slice_active = true,
            3 => app.ui.transform_mode = crate::viewport::TransformMode::ActionLine,
            5 => app.ui.arrange_references = true,
            _ => {}
        }
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let start = Pos2::new(rect.left() + 10., rect.center().y);
        let end = start + Vec2::new(90., 75.);
        let before = app.document.geometry_snapshot();
        let yaw = app.camera.yaw;
        let pan = app.camera.pan;
        let modifiers = if mode == 4 {
            egui::Modifiers::ALT
        } else {
            egui::Modifiers::NONE
        };
        visibility_frame(
            &mut app,
            &ctx,
            modifiers,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        assert!(app.viewport.gesture == Some(Gesture::Orbit));
        visibility_frame(
            &mut app,
            &ctx,
            modifiers,
            vec![egui::Event::PointerMoved(end)],
        );
        if mode == 4 {
            assert_ne!(app.camera.pan, pan);
            assert_eq!(app.camera.yaw, yaw);
        } else {
            assert_ne!(app.camera.yaw, yaw);
        }
        visibility_frame(
            &mut app,
            &ctx,
            egui::Modifiers::CTRL,
            vec![egui::Event::PointerMoved(end + Vec2::new(0., 80.))],
        );
        assert!(app.camera.zoom < 8.);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert!(!app.document.can_undo());
    }
}

#[test]
fn navigation_frame_fast_taps_do_not_stamp_select_or_cut() {
    for mode in 0..3 {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.camera.set_zoom(8.);
        let mut brush = app.document.brushes()[0].clone();
        brush.stroke_mode = StrokeMode::DragDot;
        app.document.update_brush(0, brush).unwrap();
        if mode == 1 {
            app.ui.slice_active = true;
            app.ui.slice_path_mode = true;
        }
        if mode == 2 {
            app.ui.selection_tool = Some(crate::viewport::SelectionTool::Rectangle);
        }
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let before = app.document.geometry_snapshot();
        for pos in [
            rect.left_center() + Vec2::new(10., 0.),
            rect.center_top() + Vec2::new(0., 10.),
            rect.right_center() - Vec2::new(10., 0.),
            rect.center_bottom() - Vec2::new(0., 10.),
        ] {
            navigation_frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    primary_event(pos, true),
                    primary_event(pos, false),
                ],
            );
            assert_eq!(app.document.geometry_snapshot(), before);
            assert!(app.viewport.gesture.is_none());
        }
        assert!(!app.document.can_undo());
    }
}

#[test]
fn temporary_smooth_matches_selecting_the_saved_preset_for_every_stroke_mode() {
    for mode in StrokeMode::ALL {
        let run = |temporary: bool| {
            let mut app = App::default();
            let ctx = egui::Context::default();
            let smooth = app
                .document
                .brushes()
                .iter()
                .position(|b| b.kind == Kind::Smooth)
                .unwrap();
            let mut preset = app.document.brushes()[smooth].clone();
            preset.radius = 85.;
            preset.strength = 0.8;
            preset.alpha = Alpha::Square;
            preset.rotation = 27.;
            preset.inner_radius = 0.4;
            preset.falloff = 0.8;
            preset.spacing = 0.08;
            preset.pressure = true;
            preset.pressure_radius = true;
            preset.pressure_curve = 1.8;
            preset.stabilize = true;
            preset.stabilizer_distance = 12.;
            preset.stroke_mode = mode;
            preset.subtract = true;
            app.document.update_brush(smooth, preset).unwrap();
            let mut other = Brush::new(Kind::Mask);
            other.radius = 5.;
            other.strength = 0.;
            other.stroke_mode = StrokeMode::DragRect;
            app.document.update_brush(0, other).unwrap();
            if !temporary {
                app.ui.select_brush(smooth, &app.document);
            }
            app.pressure = 0.7;
            let center = navigation_frame(&mut app, &ctx, vec![]).center();
            let before = app.document.geometry_snapshot();
            let modifiers = if temporary {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            };
            visibility_frame(
                &mut app,
                &ctx,
                modifiers,
                vec![
                    egui::Event::PointerMoved(center),
                    primary_event(center, true),
                ],
            );
            for offset in [20., 40., 60.] {
                visibility_frame(
                    &mut app,
                    &ctx,
                    modifiers,
                    vec![egui::Event::PointerMoved(center + Vec2::new(offset, 20.))],
                );
            }
            visibility_frame(
                &mut app,
                &ctx,
                modifiers,
                vec![primary_event(center + Vec2::new(60., 20.), false)],
            );
            let after = app.document.geometry_snapshot();
            assert_ne!(after, before);
            assert_eq!(app.document.history_counts().0, 1);
            if temporary {
                assert_eq!(app.ui.selected, 0);
                assert_eq!(
                    app.ui.active_brush_index(&app.document, modifiers),
                    Some(smooth)
                );
                assert_eq!(
                    app.ui
                        .active_brush_index(&app.document, egui::Modifiers::NONE),
                    Some(0)
                );
            }
            after
        };
        assert_eq!(
            run(true),
            run(false),
            "{mode:?}: temporary Smooth must use the complete preset"
        );
    }
}

#[test]
fn smooth_variation_is_remembered_and_size_actions_edit_it_while_held() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::AddBrush(Kind::Smooth), &ctx);
    let smooth = app.ui.selected;
    let id = app.document.brushes()[smooth].identity();
    app.dispatch(
        Command::Action(Action::SelectBrush(app.document.brushes()[0].identity())),
        &ctx,
    );
    let original = app.document.brushes()[0].clone();
    let radius = app.document.brushes()[smooth].radius;
    let _ = ctx.run(
        egui::RawInput {
            modifiers: egui::Modifiers::SHIFT,
            ..Default::default()
        },
        |ctx| {
            assert_eq!(
                app.ui
                    .active_brush_index(&app.document, ctx.input(|i| i.modifiers)),
                Some(smooth)
            );
            app.dispatch(Command::Action(Action::Larger), ctx);
        },
    );
    assert!(app.document.brushes()[0] == original);
    assert!(app.document.brushes()[smooth].radius > radius);
    assert_eq!(app.document.brushes()[smooth].identity(), id);
    assert_eq!(app.ui.selected, 0);
}

#[test]
fn switching_to_smooth_discards_pending_stamp_and_keeps_one_undo_step() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut brush = app.document.brushes()[0].clone();
    brush.kind = Kind::Mask;
    brush.stroke_mode = StrokeMode::DragRect;
    app.document.update_brush(0, brush).unwrap();
    let before = app.document.geometry_snapshot();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::new(50., 0.))],
    );
    assert_eq!(
        app.document.geometry_snapshot(),
        before,
        "unreleased stamp is only a preview"
    );
    visibility_frame(&mut app, &ctx, egui::Modifiers::SHIFT, vec![]);
    assert_ne!(
        app.document.geometry_snapshot(),
        before,
        "Smooth uses its freehand mode immediately"
    );
    assert_eq!(
        app.document.mesh().mask,
        before.mesh.mask,
        "the pending Mask stamp was not applied"
    );
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::SHIFT,
        vec![primary_event(center + Vec2::new(50., 0.), false)],
    );
    assert_eq!(app.document.history_counts().0, 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn adaptive_shift_smooth_keeps_topology_and_decimate_reports_reduction() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document
        .set_adaptive(crate::sculpt::AdaptiveSettings {
            enabled: true,
            detail: 32.,
            ..Default::default()
        })
        .unwrap();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let before = app.document.geometry_snapshot();
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::SHIFT,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    for offset in [15., 30., 45.] {
        visibility_frame(
            &mut app,
            &ctx,
            egui::Modifiers::SHIFT,
            vec![egui::Event::PointerMoved(center + Vec2::new(offset, 0.))],
        );
    }
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::SHIFT,
        vec![primary_event(center + Vec2::new(45., 0.), false)],
    );
    assert_eq!(app.document.mesh().triangles, before.mesh.triangles);
    assert_ne!(app.document.mesh().vertices, before.mesh.vertices);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    let index = app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == Kind::Decimate)
        .unwrap();
    app.ui.select_brush(index, &app.document);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    for offset in [15., 30., 45.] {
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(center + Vec2::new(offset, 0.))],
        );
    }
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(center + Vec2::new(45., 0.), false)],
    );
    let removed = before.mesh.vertices.len() - app.document.mesh().vertices.len();
    assert!(removed > 0);
    assert!(
        app.ui
            .status
            .contains(&format!("removed {removed} vertices")),
        "{}",
        app.ui.status
    );
    app.document.mesh().validate_closed().unwrap();
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn navigation_and_smooth_offscreen_preview() {
    use crate::preferences::{Dock, Tool};
    let size = [1200, 850];
    let mut capture = crate::viewport::UiCapture::new(size);
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.viewport.initialize_renderer(&capture.state).unwrap();
    app.camera.set_zoom(8.);
    for panel in &mut app.ui.preferences.panels {
        panel.dock = match panel.tool {
            Tool::Library => Dock::Left,
            Tool::Brush | Tool::Mesh => Dock::Right,
            _ => Dock::Hidden,
        };
    }
    let smooth = app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == Kind::Smooth)
        .unwrap();
    let mut brush = app.document.brushes()[smooth].clone();
    brush.radius = 125.;
    app.document.update_brush(smooth, brush).unwrap();
    let before = app.document.geometry_snapshot();
    for (preview, (held, zoom, name)) in [
        (false, 8., "standard"),
        (true, 8., "smooth"),
        (false, 1., "brush-settings"),
    ]
    .into_iter()
    .enumerate()
    {
        app.camera.set_zoom(zoom);
        for step in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(size[0] as f32, size[1] as f32),
                    )),
                    modifiers: if held {
                        egui::Modifiers::SHIFT
                    } else {
                        egui::Modifiers::NONE
                    },
                    time: Some((step + preview * 3) as f64),
                    ..Default::default()
                },
                |ctx| {
                    let _ = app.ui.toolbar(ctx, &app.document, &app.camera);
                    let _ = app
                        .ui
                        .brush_panel(ctx, &app.document, "Preview", 1., &app.camera);
                    app.ui.status_bar(ctx);
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
            let rgba = capture.render(&ctx, output);
            if step == 2
                && let Ok(path) = std::env::var("CLAY_NAVIGATION_PREVIEW")
            {
                capture.save(format!("{path}-{name}.png"), &rgba);
            }
            assert_eq!(
                app.ui
                    .active_brush_index(&app.document, ctx.input(|i| i.modifiers)),
                Some(if held { smooth } else { 0 })
            );
        }
    }
    assert_eq!(app.document.geometry_snapshot(), before);
    app.viewport.destroy_renderer();
}

#[test]
fn missing_smooth_preset_uses_independent_defaults_without_changing_the_library() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut project = app.document.project();
    project.brushes.truncate(1);
    project.brushes[0].strength = 0.;
    project.brushes[0].radius = 1.;
    app.document
        .replace_project(project, "legacy.clay")
        .unwrap();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let before = app.document.geometry_snapshot();
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::SHIFT,
        vec![egui::Event::PointerMoved(center)],
    );
    assert!(!app.document.is_dirty());
    assert_eq!(
        app.ui
            .active_brush_index(&app.document, egui::Modifiers::SHIFT),
        None
    );
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::SHIFT,
        vec![primary_event(center, true)],
    );
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::SHIFT,
        vec![primary_event(center, false)],
    );
    assert_ne!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.brushes().len(), 1);
    assert_eq!(app.document.brushes()[0].strength, 0.);
}

#[test]
fn upside_down_orbit_keeps_horizontal_direction_in_sculpt_and_modeling() {
    for modeling in [false, true] {
        for secondary in [false, true] {
            let mut app = App::default();
            let ctx = egui::Context::default();
            app.dispatch(Command::ModelingMode(modeling), &ctx);
            app.camera.pitch = std::f32::consts::PI;
            let before = app.document.geometry_snapshot();
            let history = app.document.history_counts();
            let rect = navigation_frame(&mut app, &ctx, vec![]);
            let start = if secondary {
                rect.center()
            } else {
                Pos2::new(rect.left() + 10., rect.center().y)
            };
            let button = if secondary {
                egui::PointerButton::Secondary
            } else {
                egui::PointerButton::Primary
            };
            let event = |pos, pressed| egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), event(start, true)],
            );
            let end = start + Vec2::new(25., 0.);
            navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            navigation_frame(&mut app, &ctx, vec![event(end, false)]);
            assert!(
                app.camera.yaw > 0.,
                "modeling {modeling}, secondary {secondary}"
            );
            assert_eq!(app.document.geometry_snapshot(), before);
            assert_eq!(app.document.history_counts(), history);
        }
    }
}

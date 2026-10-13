use super::*;
use crate::viewport::Gesture;
use eframe::egui::{Pos2, Vec2};
fn navigation_frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::Rect {
    let mut rect = egui::Rect::NOTHING;
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
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
    rect
}
fn primary_event(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    }
}
#[test]
fn empty_space_drag_keeps_orbiting_when_crossing_clay() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    navigation_frame(&mut app, &ctx, vec![]);
    let empty = Pos2::new(100., 100.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(empty), primary_event(empty, true)],
    );
    assert!(app.viewport.gesture == Some(Gesture::Orbit));
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(Pos2::new(500., 400.))],
    );
    assert!(app.camera.yaw < -0.1);
    assert!(app.camera.pitch < -0.1);
    assert!(!app.document.is_dirty());
    assert!(!app.document.can_undo());
}
#[test]
fn gizmo_axis_click_snaps_and_drag_rotates_without_sculpting() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let center = Pos2::new(rect.right() - 79., rect.top() + 79.);
    let x = center + Vec2::new(42., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(x), primary_event(x, true)],
    );
    assert!(app.viewport.gesture == Some(Gesture::Gizmo));
    navigation_frame(&mut app, &ctx, vec![primary_event(x, false)]);
    assert!((app.camera.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
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
        vec![egui::Event::PointerMoved(center + Vec2::new(-60., 20.))],
    );
    assert!(app.camera.yaw > std::f32::consts::FRAC_PI_2 + 0.1);
    assert!(app.camera.pitch < -0.1);
    assert!(!app.document.is_dirty());
    assert!(!app.document.can_undo());
}

#[test]
fn viewport_pen_drag_sculpts_and_records_undo() {
    let mut app = App::default();
    let original = app.document.mesh().vertices.clone();
    let ctx = egui::Context::default();
    let _ = ctx.run(
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
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(1000., 800.),
        )),
        events: vec![
            egui::Event::PointerMoved(Pos2::new(500., 400.)),
            egui::Event::PointerButton {
                pos: Pos2::new(500., 400.),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
        ],
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
    });
    assert!(app.document.is_dirty());
    assert!(app.document.can_undo());
    assert!(
        app.document
            .mesh()
            .vertices
            .iter()
            .zip(&original)
            .any(|(a, b)| (a.0, a.1, a.2) != (b.0, b.1, b.2))
    );
    app.document.undo();
    assert!(!app.document.can_undo());
    assert!(
        app.document
            .mesh()
            .vertices
            .iter()
            .zip(original)
            .all(|(a, b)| (a.0, a.1, a.2) == (b.0, b.1, b.2))
    );
}

#[test]
fn saving_before_open_cannot_overwrite_the_pending_target() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.clear_mask();
    app.dispatch(
        Command::Request(PendingAction::Open("./sculpt.clay".into())),
        &ctx,
    );
    app.ui.path = "some-other-project.clay".into();
    app.dispatch(Command::SaveAndContinue, &ctx);
    assert!(app.document.is_dirty());
    assert!(app.ui.pending.is_some());
    assert!(app.ui.status.contains("avoid overwriting"));
}
#[test]
fn failed_open_after_discard_keeps_existing_unsaved_content() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.clear_mask();
    let before = serde_json::to_value(app.document.project()).unwrap();
    let missing = std::env::temp_dir().join(format!(
        "clay-missing-{}-{}.clay",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.dispatch(
        Command::Request(PendingAction::Open(missing.to_string_lossy().into())),
        &ctx,
    );
    app.dispatch(Command::DiscardAndContinue, &ctx);
    assert!(app.document.is_dirty());
    assert_eq!(
        serde_json::to_value(app.document.project()).unwrap(),
        before
    );
    assert!(app.document.can_undo());
}

#[test]
fn custom_shortcut_changes_view_and_pauses_during_settings_and_text_entry() {
    use crate::preferences::{Action, Shortcut};
    let mut app = App::default();
    app.ui
        .preferences
        .bind(
            Action::Wireframe,
            Shortcut::new(egui::Key::F2, egui::Modifiers::NONE),
        )
        .unwrap();
    let ctx = egui::Context::default();
    for (settings, typing, expected) in [
        (false, false, true),
        (true, false, true),
        (false, true, true),
    ] {
        app.ui.settings_open = settings;
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::F2,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| {
                if typing {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.text_edit_singleline(&mut app.ui.path).request_focus();
                    });
                }
                app.shortcuts(ctx);
                assert_eq!(app.ui.wire, expected);
            },
        );
    }
}

#[test]
fn customized_tool_groups_render_in_all_docks_and_settings() {
    use crate::preferences::Dock;
    let mut app = App::default();
    app.ui.preferences.panels[0].dock = Dock::Right;
    app.ui.preferences.panels[2].dock = Dock::Floating;
    app.ui.preferences.panels[3].dock = Dock::Hidden;
    app.ui.settings_open = true;
    app.ui.layout_open = true;
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 850.),
                )),
                ..Default::default()
            },
            |ctx| {
                assert!(
                    app.ui
                        .toolbar(ctx, &app.document, app.camera.zoom)
                        .is_empty()
                );
                assert!(
                    app.ui
                        .brush_panel(ctx, &app.document, "Test tablet", 1.)
                        .is_empty()
                );
                app.ui.settings(ctx);
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
    }
    app.ui.preferences.validate().unwrap();
    assert!(!app.document.is_dirty());
}

fn finish_mesh_job(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.mesh_job.is_some() {
        app.poll_mesh_job();
        assert!(std::time::Instant::now() < deadline, "mesh job timed out");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
#[test]
fn background_mesh_jobs_commit_once_and_cancellation_or_failure_keeps_original() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let original = app.document.geometry_snapshot();
    app.dispatch(Command::Subdivide, &ctx);
    assert!(app.mesh_job.is_some());
    app.dispatch(Command::Undo, &ctx); // Mesh edits are locked while the snapshot is in flight.
    assert_eq!(app.document.geometry_snapshot(), original);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.mesh_settings().level(), 2);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), original);
    app.dispatch(Command::Subdivide, &ctx);
    app.dispatch(Command::CancelMeshJob, &ctx);
    app.poll_mesh_job();
    assert_eq!(app.document.geometry_snapshot(), original);
    app.dispatch(Command::SetDynamicMesh(true), &ctx);
    app.dispatch(Command::SetRemeshSettings(16, false), &ctx);
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::Remesh, &ctx);
    finish_mesh_job(&mut app);
    app.document.mesh().validate_closed().unwrap();
    assert_ne!(
        app.document.mesh().triangles.len(),
        before.mesh.triangles.len()
    );
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::SetDynamicMesh(false), &ctx);
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::Remesh, &ctx);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(app.ui.status.contains("Switch to dynamic"));
}

#[test]
fn automatic_remeshing_waits_for_stroke_release_and_is_undoable() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::SetDynamicMesh(true), &ctx);
    app.dispatch(Command::SetRemeshSettings(16, true), &ctx);
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let center = rect.center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    assert!(app.mesh_job.is_none());
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + egui::vec2(20., 0.))],
    );
    let sculpted = app.document.mesh().clone();
    assert!(app.mesh_job.is_none());
    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    assert!(app.mesh_job.is_some());
    finish_mesh_job(&mut app);
    assert_ne!(
        app.document.mesh().triangles.len(),
        sculpted.triangles.len()
    );
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.mesh(), &sculpted);
    // A pen held down during an explicit mesh operation must be lifted before
    // sculpting can resume against the new topology.
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    app.dispatch(Command::Remesh, &ctx);
    finish_mesh_job(&mut app);
    let remeshed = app.document.mesh().clone();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + egui::vec2(30., 0.))],
    );
    assert_eq!(app.document.mesh(), &remeshed);
    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    assert!(app.mesh_job.is_none());
}

#[test]
fn undo_redo_shortcuts_restore_geometry_and_report_the_action() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    app.document.clear_mask();
    assert_eq!(app.document.undo_label(), Some("Clear mask"));
    let press = |modifiers| egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    let _ = ctx.run(press(egui::Modifiers::CTRL), |ctx| app.shortcuts(ctx));
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), (0, 1));
    assert_eq!(app.ui.status, "Undid: Clear mask");
    let _ = ctx.run(
        press(egui::Modifiers::CTRL | egui::Modifiers::SHIFT),
        |ctx| app.shortcuts(ctx),
    );
    assert_eq!(app.document.history_counts(), (1, 0));
    assert_eq!(app.ui.status, "Redid: Clear mask");
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.ui.status, "Nothing to redo");
}

#[test]
fn undo_during_a_pen_drag_restores_the_stroke_and_waits_for_release() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.mesh().clone();
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
        vec![egui::Event::PointerMoved(center + egui::vec2(20., 0.))],
    );
    let sculpted = app.document.mesh().clone();
    assert_ne!(sculpted, before);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.mesh(), &before);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + egui::vec2(40., 0.))],
    );
    assert_eq!(app.document.mesh(), &before);
    assert!(app.document.can_redo());
    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.mesh(), &sculpted);
}

#[test]
fn zoom_buttons_and_rebound_keys_change_only_the_camera_and_respect_limits() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    app.camera.yaw = 0.4;
    app.camera.pan = egui::vec2(20., 10.);
    app.dispatch(Command::ZoomIn, &ctx);
    assert!(app.camera.zoom > 1.);
    app.dispatch(Command::ZoomOut, &ctx);
    assert!((app.camera.zoom - 1.).abs() < 1e-6);
    for _ in 0..100 {
        app.dispatch(Command::ZoomOut, &ctx);
    }
    assert_eq!(app.camera.zoom, 0.2);
    for _ in 0..100 {
        app.dispatch(Command::ZoomIn, &ctx);
    }
    assert_eq!(app.camera.zoom, 8.);
    app.dispatch(Command::SetZoom(1.), &ctx);
    assert_eq!(app.camera.yaw, 0.4);
    assert_eq!(app.camera.pan, egui::vec2(20., 10.));
    app.ui
        .preferences
        .bind(
            crate::preferences::Action::ZoomIn,
            crate::preferences::Shortcut::new(egui::Key::F3, egui::Modifiers::NONE),
        )
        .unwrap();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::F3,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| app.shortcuts(ctx),
    );
    assert!(app.camera.zoom > 1.);
    app.dispatch(Command::SetZoom(f32::NAN), &ctx);
    assert!(app.camera.zoom.is_finite());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.is_dirty());
    assert!(!app.document.can_undo());
}

#[test]
fn held_zoom_modifier_replaces_orbit_and_can_toggle_mid_drag() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let empty = rect.min + egui::vec2(40., 40.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(empty), primary_event(empty, true)],
    );
    let yaw = app.camera.yaw;
    let pitch = app.camera.pitch;
    let frame = |app: &mut App, pos: Pos2, modifiers: egui::Modifiers| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                modifiers,
                events: vec![egui::Event::PointerMoved(pos)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
    };
    frame(
        &mut app,
        empty + egui::vec2(10., -20.),
        egui::Modifiers::CTRL,
    );
    assert!(app.camera.zoom > 1.);
    assert_eq!(app.camera.yaw, yaw);
    assert_eq!(app.camera.pitch, pitch);
    let zoom = app.camera.zoom;
    frame(&mut app, empty + egui::vec2(20., 0.), egui::Modifiers::CTRL);
    assert!(app.camera.zoom < zoom);
    let zoom = app.camera.zoom;
    frame(
        &mut app,
        empty + egui::vec2(40., 20.),
        egui::Modifiers::NONE,
    );
    assert_eq!(app.camera.zoom, zoom);
    assert_ne!(app.camera.yaw, yaw);
    assert!(!app.document.is_dirty());
    assert!(!app.document.can_undo());
}

#[test]
fn holding_zoom_when_pen_drag_starts_over_clay_avoids_sculpting() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let original = app.document.mesh().clone();
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1000., 800.),
            )),
            modifiers: egui::Modifiers::CTRL,
            events: vec![
                egui::Event::PointerMoved(center),
                egui::Event::PointerButton {
                    pos: center,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::CTRL,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
    assert!(app.viewport.gesture == Some(Gesture::Orbit));
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1000., 800.),
            )),
            modifiers: egui::Modifiers::CTRL,
            events: vec![egui::Event::PointerMoved(center - egui::vec2(0., 40.))],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
    assert!(app.camera.zoom > 1.);
    assert_eq!(app.camera.yaw, 0.);
    assert_eq!(app.document.mesh(), &original);
    assert!(!app.document.is_dirty());
    assert!(!app.document.can_undo());
}

#[test]
fn custom_zoom_modifier_works_on_right_drag_and_gizmo() {
    for (button, gizmo) in [
        (egui::PointerButton::Secondary, false),
        (egui::PointerButton::Primary, true),
    ] {
        let mut app = App::default();
        app.ui.preferences.zoom_hold = crate::preferences::Hold::Alt;
        let ctx = egui::Context::default();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let start = if gizmo {
            Pos2::new(rect.right() - 79., rect.top() + 79.)
        } else {
            rect.center()
        };
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                modifiers: egui::Modifiers::ALT,
                events: vec![
                    egui::Event::PointerMoved(start),
                    egui::Event::PointerButton {
                        pos: start,
                        button,
                        pressed: true,
                        modifiers: egui::Modifiers::ALT,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                modifiers: egui::Modifiers::ALT,
                events: vec![egui::Event::PointerMoved(start - egui::vec2(0., 40.))],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
        assert!(app.camera.zoom > 1.);
        assert_eq!(app.camera.yaw, 0.);
        assert_eq!(app.camera.pitch, 0.);
        assert!(!app.document.is_dirty());
        assert!(!app.document.can_undo());
    }
}

#[test]
fn snake_drag_extends_beyond_model_with_adaptive_topology_and_one_undo_step() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(
        Command::SetAdaptive(crate::sculpt::AdaptiveSettings {
            enabled: true,
            detail: 5.,
            simplify: false,
            ..Default::default()
        }),
        &ctx,
    );
    app.dispatch(Command::AddBrush(crate::sculpt::Kind::Snake), &ctx);
    assert_eq!(
        app.document.brushes()[app.ui.selected].kind,
        crate::sculpt::Kind::Snake
    );
    let original = app.document.mesh().clone();
    let count = app.document.history_counts().0;
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let start = center + egui::vec2(150., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    for step in 1..=14 {
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(
                start + egui::vec2(step as f32 * 20., 0.),
            )],
        );
    }
    let end = start + egui::vec2(280., 0.);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    let sculpted = app.document.mesh().clone();
    assert!(sculpted.vertices.len() > original.vertices.len());
    assert!(
        sculpted.vertices.iter().any(|v| v.0 > 1.3),
        "snake did not extend beyond the original sphere"
    );
    sculpted.validate_closed().unwrap();
    assert_eq!(app.document.history_counts().0, count + 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.mesh(), &original);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.mesh(), &sculpted);
}

#[test]
fn multiple_adaptive_dabs_with_symmetry_guides_use_stable_picking() {
    let mut app = App::default();
    let initial_vertices = app.document.mesh().vertices.len();
    let ctx = egui::Context::default();
    app.dispatch(
        Command::SetAdaptive(crate::sculpt::AdaptiveSettings {
            enabled: true,
            detail: 3.,
            ..Default::default()
        }),
        &ctx,
    );
    let mut symmetry = app.document.symmetry().clone();
    symmetry.enabled = true;
    app.dispatch(Command::SetSymmetry(symmetry), &ctx);
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    for step in 1..=4 {
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(
                center + egui::vec2(step as f32 * 40., 10.),
            )],
        );
    }
    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    app.document.mesh().validate_closed().unwrap();
    assert!(app.document.mesh().vertices.len() > initial_vertices);
}

#[test]
fn world_handles_move_scale_and_undo_without_sculpting_or_camera_motion() {
    use crate::viewport::{TransformMode, TransformOperation};
    let mut app = App::default();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::TransformMode(TransformMode::World), &ctx);
    let start = rect.center() + Vec2::new(85., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    let end = start + Vec2::new(60., 0.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    let delta = app.document.mesh().vertices[0] - before.mesh.vertices[0];
    assert!(delta.0 > 0.1 && delta.1.abs() < 1e-5 && delta.2.abs() < 1e-5);
    assert_eq!(app.document.undo_label(), Some("Transform submodel"));
    assert_eq!(app.document.history_counts(), (1, 0));
    assert_eq!(app.camera.yaw, 0.);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::TransformOperation(TransformOperation::Scale), &ctx);
    let start = rect.center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    let end = start + Vec2::new(-60., 0.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(app.document.mesh().vertices[0].1 < 0.8);
    assert_eq!(app.document.history_counts(), (1, 0));
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}
#[test]
fn action_line_placement_moves_and_stretches_with_one_undo_per_drag() {
    use crate::viewport::{TransformMode, TransformOperation};
    let mut app = App::default();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::TransformMode(TransformMode::ActionLine), &ctx);
    let start = rect.center() + Vec2::new(-60., 0.);
    let end = rect.center() + Vec2::new(60., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_undo());
    app.dispatch(Command::TransformOperation(TransformOperation::Move), &ctx);
    let start = rect.center();
    let end = start + Vec2::new(40., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(app.document.mesh().vertices[0].0 > 0.1);
    assert_eq!(app.document.history_counts(), (1, 0));
    let moved = app.document.geometry_snapshot();
    app.dispatch(Command::TransformOperation(TransformOperation::Scale), &ctx);
    let start = rect.center() + Vec2::new(100., 0.);
    let end = start + Vec2::new(-40., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(app.document.history_counts(), (2, 0));
    assert_eq!(app.document.mesh().vertices[0].1, moved.mesh.vertices[0].1);
    let width = |mesh: &crate::sculpt::Mesh| {
        mesh.vertices
            .iter()
            .map(|v| v.0)
            .fold(f32::NEG_INFINITY, f32::max)
            - mesh
                .vertices
                .iter()
                .map(|v| v.0)
                .fold(f32::INFINITY, f32::min)
    };
    assert!(width(app.document.mesh()) < width(&moved.mesh) * 0.8);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), moved);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}
#[test]
fn click_inactive_surface_selects_it_without_deforming_until_release() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.duplicate_model(false).unwrap();
    app.document
        .apply_transform(
            crate::sculpt::Transform {
                translation: crate::sculpt::Vec3(0., 0., 2.),
                ..Default::default()
            },
            false,
        )
        .unwrap();
    app.document.end_stroke();
    app.document.select_submodel(1).unwrap();
    let before = app.document.mesh().clone();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let point = rect.center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    assert_eq!(app.document.active_model_index(), 0);
    let selected = app.document.mesh().clone();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point + Vec2::new(10., 0.))],
    );
    assert_eq!(app.document.mesh(), &selected);
    assert_eq!(app.document.model(1).unwrap().mesh, &before);
    navigation_frame(&mut app, &ctx, vec![primary_event(point, false)]);
}

#[test]
fn floating_transform_toolbar_switches_operation_without_editing_the_model() {
    use crate::viewport::{TransformMode, TransformOperation};
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
            },
        )
    }
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::TransformMode(TransformMode::ActionLine), &ctx);
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Scale" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("Scale is directly visible in the floating toolbar");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    frame(&mut app, &ctx, vec![primary_event(point, false)]);
    assert_eq!(app.ui.transform_operation, TransformOperation::Scale);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_undo());
    assert!(app.viewport.gesture.is_none());
    app.dispatch(Command::TransformMode(TransformMode::Sculpt), &ctx);
    let (commands, rect) = app.ui.transform_toolbar(&ctx, egui::Rect::EVERYTHING, true);
    assert!(commands.is_empty() && rect.is_none());
}

#[test]
fn transform_button_activates_floating_controls_with_one_click() {
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1400., 800.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                for command in app.ui.toolbar(ctx, &app.document, app.camera.zoom) {
                    app.dispatch(command, ctx);
                }
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        )
    }
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    frame(&mut app, &ctx, vec![]); // Let the taller toolbar settle its panel height.
    let output = frame(&mut app, &ctx, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Transform" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    frame(&mut app, &ctx, vec![primary_event(point, false)]);
    assert_eq!(app.ui.transform_mode, crate::viewport::TransformMode::World);
    let output = frame(&mut app, &ctx, vec![]);
    assert!(output.shapes.iter().any(
        |shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()=="Scale")
    ));
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_undo());
}

#[test]
fn primitive_button_requires_explicit_replacement_and_cancel_keeps_the_scene() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::RequestPrimitive, &ctx);
    assert!(app.ui.primitive_warning.is_some());
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::SelectSubmodel(0), &ctx);
    assert!(app.ui.primitive_warning.is_some());
    navigation_frame(&mut app, &ctx, vec![]);
    app.dispatch(Command::CancelPrimitive, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_undo());
    app.dispatch(Command::RequestPrimitive, &ctx);
    app.dispatch(Command::ConfirmPrimitive, &ctx);
    assert!(app.document.primitive().is_some());
    assert!(app.ui.primitive_controls_requested);
    navigation_frame(&mut app, &ctx, vec![]);
    let count = app.document.history_counts();
    app.dispatch(Command::RequestPrimitive, &ctx);
    assert!(app.ui.primitive_warning.is_none());
    assert_eq!(app.document.history_counts(), count);
    app.dispatch(Command::ConvertPrimitive, &ctx);
    assert!(app.document.primitive().is_none());
    assert!(!app.ui.primitive_controls_requested);
}
#[test]
fn primitive_shape_handle_drag_edits_recipe_without_sculpting_and_undoes_once() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.initialize_primitive().unwrap();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let scale = rect.width().min(rect.height()) * 0.35;
    let start = rect.center() - Vec2::new(scale, 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    assert!(app.viewport.gesture == Some(Gesture::Primitive));
    for distance in [10., 20., 30.] {
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start - Vec2::new(distance, 0.))],
        );
    }
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(start - Vec2::new(30., 0.), false)],
    );
    assert!(app.document.primitive().unwrap().size.0 > 2.);
    assert_eq!(app.document.history_counts().0, count.0 + 1);
    assert_eq!(app.camera.yaw, 0.);
    app.document
        .mesh_settings()
        .validate(app.document.mesh())
        .unwrap();
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn requesting_primitive_controls_reveals_the_submodels_section() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.initialize_primitive().unwrap();
    let panel = app
        .ui
        .preferences
        .panels
        .iter_mut()
        .find(|p| p.tool == crate::preferences::Tool::Submodels)
        .unwrap();
    panel.open = false;
    panel.dock = crate::preferences::Dock::Hidden;
    app.dispatch(Command::RequestPrimitive, &ctx);
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1200., 1000.),
            )),
            ..Default::default()
        },
        |ctx| {
            app.ui.brush_panel(ctx, &app.document, "", 1.);
        },
    );
    let panel = app
        .ui
        .preferences
        .panels
        .iter()
        .find(|p| p.tool == crate::preferences::Tool::Submodels)
        .unwrap();
    assert!(panel.open);
    assert_eq!(panel.dock, crate::preferences::Dock::Right);
    assert!(!app.ui.primitive_controls_requested);
}

#[test]
fn primitive_sidebar_changes_segments_without_dragging_model_and_hides_after_conversion() {
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000., 1800.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                let mut commands = Vec::new();
                egui::SidePanel::right("submodel_test_panel")
                    .default_width(330.)
                    .show(ctx, |ui| {
                        app.ui.primitive_controls(ui, &app.document, &mut commands);
                    });
                for command in commands {
                    app.dispatch(command, ctx);
                }
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        )
    }
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.initialize_primitive().unwrap();
    app.document
        .update_primitive(crate::sculpt::Primitive::for_kind(
            crate::sculpt::PrimitiveKind::UvSphere,
        ))
        .unwrap();
    app.document.end_primitive_edit();
    let count = app.document.history_counts().0;
    let before = app.document.geometry_snapshot();
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    let texts: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((t.galley.text(), t.pos + t.galley.size() * 0.5)),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts.iter().filter(|(text, _)| *text == "Segments").count(),
        1
    );
    assert!(
        !texts
            .iter()
            .any(|(text, _)| *text == "Primitive shape" || text.starts_with("Resolution ·"))
    );
    let row = texts
        .iter()
        .find(|(text, _)| *text == "Segments")
        .unwrap()
        .1;
    let plus = texts
        .iter()
        .find(|(text, pos)| *text == "+" && (pos.y - row.y).abs() < 5.)
        .unwrap()
        .1;
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(plus), primary_event(plus, true)],
    );
    frame(&mut app, &ctx, vec![primary_event(plus, false)]);
    assert_eq!(app.document.primitive().unwrap().segments, 49);
    assert_eq!(app.document.history_counts().0, count + 1);
    assert!(!app.ui.primitive_controls_requested);
    assert!(app.viewport.gesture.is_none());
    assert_eq!(app.camera.yaw, 0.);
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::ConvertPrimitive, &ctx);
    let output = frame(&mut app, &ctx, vec![]);
    assert!(!output.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text() == "Segments" || t.galley.text().starts_with("Primitive ·"))));
}

#[test]
fn primitive_sidebar_opens_the_jaw_with_one_click_and_undo_restores_it() {
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000., 1800.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                let mut commands = Vec::new();
                egui::SidePanel::right("submodel_test_panel")
                    .default_width(330.)
                    .show(ctx, |ui| {
                        app.ui.primitive_controls(ui, &app.document, &mut commands);
                    });
                for command in commands {
                    app.dispatch(command, ctx);
                }
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        )
    }
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.initialize_primitive().unwrap();
    app.document
        .update_primitive(crate::sculpt::Primitive::for_kind(
            crate::sculpt::PrimitiveKind::HumanHead,
        ))
        .unwrap();
    app.document.end_primitive_edit();
    let before = app.document.geometry_snapshot();
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    let texts: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((t.galley.text(), t.pos + t.galley.size() * 0.5)),
            _ => None,
        })
        .collect();
    let close = texts
        .iter()
        .find(|(text, _)| *text == "Close mouth")
        .unwrap()
        .1;
    let point = texts
        .iter()
        .find(|(text, pos)| *text == "Open mouth" && (pos.y - close.y).abs() < 5.)
        .unwrap()
        .1;
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    frame(&mut app, &ctx, vec![primary_event(point, false)]);
    assert_eq!(app.document.primitive().unwrap().head.mouth_open, 1.);
    assert!(!app.ui.primitive_controls_requested);
    assert!(app.viewport.gesture.is_none());
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn brush_icon_capture_preview_cancel_and_assignment() {
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
                if let Some(command) = app.ui.icon_capture_popup(ctx, &app.document) {
                    app.dispatch(command, ctx);
                }
            },
        )
    }
    fn button(output: egui::FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("capture action is visible")
    }
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    app.document.mark_saved("test.clay");
    app.dispatch(Command::CaptureBrushIcon(0), &ctx);
    assert!(app.ui.icon_capture_open());
    frame(&mut app, &ctx, vec![]);
    let cancel = button(frame(&mut app, &ctx, vec![]), "Cancel");
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(cancel),
            primary_event(cancel, true),
        ],
    );
    frame(&mut app, &ctx, vec![primary_event(cancel, false)]);
    assert!(!app.ui.icon_capture_open());
    assert!(!app.document.is_dirty());
    assert!(app.document.brushes()[0].icon.is_none());
    app.dispatch(Command::CaptureBrushIcon(0), &ctx);
    frame(&mut app, &ctx, vec![]);
    let assign = button(frame(&mut app, &ctx, vec![]), "Use as brush icon");
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(assign),
            primary_event(assign, true),
        ],
    );
    frame(&mut app, &ctx, vec![primary_event(assign, false)]);
    assert!(!app.ui.icon_capture_open());
    assert!(app.document.brushes()[0].icon.is_some());
    assert!(app.document.is_dirty());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_undo());
}

#[test]
fn mask_commands_cut_invert_and_undo_and_empty_models_render_safely() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let original = app.document.geometry_snapshot();
    app.dispatch(Command::InvertMask, &ctx);
    assert!(app.document.mesh().mask.iter().all(|m| *m == 1.));
    app.dispatch(Command::DeleteMasked, &ctx);
    assert!(app.document.mesh().vertices.is_empty());
    navigation_frame(&mut app, &ctx, vec![]);
    app.ui.transform_mode = crate::viewport::TransformMode::World;
    navigation_frame(&mut app, &ctx, vec![]);
    app.ui.transform_mode = crate::viewport::TransformMode::ActionLine;
    navigation_frame(&mut app, &ctx, vec![]);
    app.dispatch(Command::Undo, &ctx);
    assert!(!app.document.mesh().vertices.is_empty());
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), original);
}

fn patterned_mask_app(mode: crate::sculpt::StrokeMode) -> App {
    let mut app = App::default();
    let mut plane = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
    plane.divisions = [20, 20, 1];
    app.document
        .replace_geometry(crate::sculpt::Geometry::new(plane.mesh().unwrap()))
        .unwrap();
    let mut brush = crate::sculpt::Brush::new(crate::sculpt::Kind::Mask);
    brush.stroke_mode = mode;
    brush.radius = 18.;
    brush.spacing = 0.2;
    brush.strength = 1.;
    brush.pressure = false;
    brush.alpha = crate::sculpt::Alpha::Hard;
    app.document.update_brush(0, brush).unwrap();
    app.document.mark_saved("pattern-test.clay");
    app
}
fn mask_near(app: &App, x: f32, y: f32) -> f32 {
    let index = app
        .document
        .mesh()
        .vertices
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            ((a.0 - x).powi(2) + (a.1 - y).powi(2))
                .total_cmp(&((b.0 - x).powi(2) + (b.1 - y).powi(2)))
        })
        .unwrap()
        .0;
    app.document.mesh().mask[index]
}
#[test]
fn patterned_line_previews_then_applies_a_straight_path_as_one_undo_step() {
    let mut app = patterned_mask_app(crate::sculpt::StrokeMode::Line);
    let ctx = egui::Context::default();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    let start = center - egui::vec2(80., 0.);
    let end = center + egui::vec2(80., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start + egui::vec2(0., 80.))],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.document.geometry_snapshot() == before);
    assert_eq!(app.document.history_counts().0, count);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(mask_near(&app, 0., 0.) > 0.9);
    assert_eq!(mask_near(&app, -0.3, -0.3), 0.);
    assert_eq!(app.document.history_counts().0, count + 1);
    app.document.undo();
    assert!(app.document.geometry_snapshot() == before);
}
#[test]
fn patterned_drag_rect_sets_radius_and_alpha_angle_without_changing_the_preset() {
    let mut app = patterned_mask_app(crate::sculpt::StrokeMode::DragRect);
    let mut brush = app.document.brushes()[0].clone();
    brush.alpha = crate::sculpt::Alpha::Image {
        width: 2,
        height: 2,
        pixels: vec![0., 1., 0., 1.].into(),
    };
    app.document.update_brush(0, brush.clone()).unwrap();
    let ctx = egui::Context::default();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    let end = center + egui::vec2(0., 80.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.document.geometry_snapshot() == before);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(
        mask_near(&app, 0., -0.1) > mask_near(&app, 0., 0.1) + 0.1,
        "dragging down rotates the image alpha down"
    );
    assert!(
        mask_near(&app, 0., -0.2) > 0.,
        "drag distance sets stamp size"
    );
    assert!(app.document.brushes()[0] == brush);
    assert_eq!(app.document.history_counts().0, count + 1);
    app.document.undo();
    assert!(app.document.geometry_snapshot() == before);
}
#[test]
fn patterned_drag_dot_only_stamps_the_final_position_and_can_cancel() {
    let mut app = patterned_mask_app(crate::sculpt::StrokeMode::DragDot);
    let ctx = egui::Context::default();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let before = app.document.geometry_snapshot();
    let start = center - egui::vec2(80., 0.);
    let end = center + egui::vec2(80., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.document.geometry_snapshot() == before);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(mask_near(&app, 0.3, 0.) > 0.9);
    assert_eq!(mask_near(&app, -0.3, 0.), 0.);
    app.document.undo();
    let count = app.document.history_counts();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(app.document.geometry_snapshot() == before);
    assert_eq!(app.document.history_counts(), count);
}
#[test]
fn patterned_stamps_keep_last_pen_pressure_when_release_reports_zero() {
    let mut app = patterned_mask_app(crate::sculpt::StrokeMode::DragDot);
    let mut brush = app.document.brushes()[0].clone();
    brush.pressure = true;
    app.document.update_brush(0, brush).unwrap();
    app.pressure = 0.5;
    let ctx = egui::Context::default();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    app.pressure = 0.;
    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    assert!((mask_near(&app, 0., 0.) - 0.5).abs() < 1e-5);
}
#[test]
fn patterned_drag_dot_accepts_a_press_and_release_in_one_frame() {
    let mut app = patterned_mask_app(crate::sculpt::StrokeMode::DragDot);
    let ctx = egui::Context::default();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
            primary_event(center, false),
        ],
    );
    assert!(mask_near(&app, 0., 0.) > 0.9);
}

fn finish_slice_job(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.mesh_job.is_some() && std::time::Instant::now() < deadline {
        app.poll_mesh_job();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(app.mesh_job.is_none(), "slice worker timed out");
}

#[test]
fn slice_drag_previews_without_edits_and_commits_each_mode_as_one_undo() {
    use crate::sculpt::{SliceMode, SliceOptions};
    for mode in SliceMode::ALL {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.dispatch(Command::SliceTool(true), &ctx);
        app.ui.slice_options = SliceOptions {
            mode,
            ..Default::default()
        };
        let before = app.document.geometry_snapshot();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let start = rect.center() + Vec2::new(33., -320.);
        let end = rect.center() + Vec2::new(33., 320.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        assert!(app.viewport.gesture == Some(Gesture::Slice));
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), (0, 0));
        assert_eq!(app.camera.yaw, 0.);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert!(app.mesh_job.is_some());
        finish_slice_job(&mut app);
        assert_ne!(
            app.document.geometry_snapshot(),
            before,
            "{}",
            app.ui.status
        );
        assert_eq!(app.document.history_counts(), (1, 0));
        assert_eq!(app.document.undo_label(), Some(mode.history_label()));
        assert_eq!(
            app.document.model_count(),
            if mode == SliceMode::Separate { 2 } else { 1 }
        );
        app.document.mesh().validate_closed().unwrap();
        let after = app.document.geometry_snapshot();
        let project: crate::sculpt::Project =
            serde_json::from_slice(&serde_json::to_vec(&app.document.project()).unwrap()).unwrap();
        project.validate().unwrap();
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Redo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), after);
    }
}

#[test]
fn slice_escape_short_click_miss_and_job_cancel_leave_geometry_and_history_unchanged() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::SliceTool(true), &ctx);
    let before = app.document.geometry_snapshot();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let start = rect.center() + Vec2::new(20., -300.);
    let end = rect.center() + Vec2::new(20., 300.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            },
        ],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(app.mesh_job.is_none());
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert!(app.mesh_job.is_none());
    let plane = crate::sculpt::SlicePlane {
        origin: crate::sculpt::Vec3(3., 0., 0.),
        normal: crate::sculpt::Vec3(1., 0., 0.),
    };
    app.start_mesh_job(Operation::Slice(plane, Default::default()))
        .unwrap();
    finish_slice_job(&mut app);
    assert!(app.ui.status.contains("both sides"));
    let plane = crate::sculpt::SlicePlane {
        origin: crate::sculpt::Vec3::default(),
        ..plane
    };
    app.start_mesh_job(Operation::Slice(plane, Default::default()))
        .unwrap();
    app.cancel_mesh_job();
    app.poll_mesh_job();
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), (0, 0));
    assert!(!app.document.is_dirty());
}

#[test]
fn slice_separate_resets_only_active_stack_and_keeps_scene_metadata() {
    let mut app = App::default();
    app.document.duplicate_model(false).unwrap();
    app.document.set_mesh_level(1).unwrap();
    let mut geometry = app.document.geometry_snapshot();
    geometry.subdivide().unwrap();
    geometry.scene.groups.push("Parts".into());
    geometry.scene.active_metadata.group = "Parts".into();
    geometry.scene.active_metadata.mask_threshold = 0.7;
    app.document.replace_geometry(geometry.clone()).unwrap();
    let count = app.document.history_counts().0;
    app.start_mesh_job(Operation::Slice(
        crate::sculpt::SlicePlane {
            origin: crate::sculpt::Vec3(0.1, 0., 0.),
            normal: crate::sculpt::Vec3(1., 0., 0.),
        },
        crate::sculpt::SliceOptions {
            mode: crate::sculpt::SliceMode::Separate,
            ..Default::default()
        },
    ))
    .unwrap();
    finish_slice_job(&mut app);
    assert_eq!(app.document.model_count(), 3, "{}", app.ui.status);
    assert_eq!(app.document.mesh_settings().levels(), 1);
    assert_eq!(app.document.model(1).unwrap().metadata.group, "Parts");
    assert_eq!(app.document.model(1).unwrap().metadata.mask_threshold, 0.7);
    assert_eq!(
        app.document.model(2).unwrap().mesh,
        &geometry.submodels[0].mesh
    );
    assert_eq!(app.document.history_counts().0, count + 1);
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), geometry);
}

#[test]
fn slice_camera_or_option_changes_cancel_a_pending_cut() {
    for change_camera in [false, true] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.dispatch(Command::SliceTool(true), &ctx);
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let start = rect.center() + Vec2::new(0., -250.);
        let end = rect.center() + Vec2::new(0., 250.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        if change_camera {
            app.camera.zoom_step(true);
        } else {
            app.ui.slice_options.mode = crate::sculpt::SliceMode::Trim;
        }
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert!(app.mesh_job.is_none());
        assert_eq!(app.document.history_counts(), (0, 0));
        assert!(!app.document.is_dirty());
    }
}

fn slice_key(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    }
}
#[test]
fn slice_space_moves_the_line_without_changing_its_direction() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::SliceTool(true), &ctx);
    let before = app.document.geometry_snapshot();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let start = rect.center() + Vec2::new(-120., -280.);
    let end = rect.center() + Vec2::new(-120., 280.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    let moved = end + Vec2::new(165., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            slice_key(egui::Key::Space, true),
            egui::Event::PointerMoved(moved),
        ],
    );
    assert_eq!(app.document.geometry_snapshot(), before);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            primary_event(moved, false),
            slice_key(egui::Key::Space, false),
        ],
    );
    finish_slice_job(&mut app);
    assert_eq!(app.document.history_counts(), (1, 0), "{}", app.ui.status);
    let mesh = app.document.mesh();
    let mut boundary = Vec::new();
    for (i, p) in mesh
        .vertices
        .iter()
        .enumerate()
        .skip(before.mesh.vertices.len())
    {
        boundary.push((i, p.0));
    }
    assert!(!boundary.is_empty());
    let x = boundary[0].1;
    assert!(x > 0.);
    assert!(boundary.iter().all(|(_, v)| (*v - x).abs() < 1e-5));
    assert_eq!(app.camera.yaw, 0.);
}
#[test]
fn slice_path_click_edit_move_and_apply_are_one_undo_in_all_modes() {
    use crate::sculpt::SliceMode;
    for mode in SliceMode::ALL {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.dispatch(Command::SliceTool(true), &ctx);
        app.ui.slice_path_mode = true;
        app.ui.slice_options.mode = mode;
        let before = app.document.geometry_snapshot();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let points = [
            rect.center() + Vec2::new(-250., 75.),
            rect.center() + Vec2::new(0., -45.),
            rect.center() + Vec2::new(250., 90.),
        ];
        for p in points {
            navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(p), primary_event(p, true)],
            );
            navigation_frame(&mut app, &ctx, vec![primary_event(p, false)]);
            assert!(app.mesh_job.is_none());
        }
        let edited = points[1] + Vec2::new(15., 30.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(points[1]),
                primary_event(points[1], true),
            ],
        );
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edited)]);
        navigation_frame(&mut app, &ctx, vec![primary_event(edited, false)]);
        // Space on an empty part of the viewport translates without adding a point.
        let grab = rect.center() + Vec2::new(0., 200.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                slice_key(egui::Key::Space, true),
                egui::Event::PointerMoved(grab),
                primary_event(grab, true),
            ],
        );
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(grab + Vec2::new(0., 15.))],
        );
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                primary_event(grab + Vec2::new(0., 15.), false),
                slice_key(egui::Key::Space, false),
            ],
        );
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), (0, 0));
        navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Enter, true)]);
        assert!(app.mesh_job.is_some(), "{mode:?}");
        finish_slice_job(&mut app);
        assert_eq!(app.document.history_counts(), (1, 0), "{}", app.ui.status);
        app.document.mesh().validate_closed().unwrap();
        assert_eq!(
            app.document.model_count(),
            if mode == SliceMode::Separate { 2 } else { 1 }
        );
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
    }
}

#[test]
fn slice_path_fast_clicks_and_failed_cut_keep_an_editable_draft() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::SliceTool(true), &ctx);
    app.ui.slice_path_mode = true;
    app.ui.slice_options.mode = crate::sculpt::SliceMode::Trim;
    let before = app.document.geometry_snapshot();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    for delta in [
        Vec2::new(-200., -100.),
        Vec2::new(200., 100.),
        Vec2::new(-200., 100.),
        Vec2::new(200., -100.),
    ] {
        let p = rect.center() + delta;
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(p),
                primary_event(p, true),
                primary_event(p, false),
            ],
        );
    }
    navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Enter, true)]);
    assert!(app.mesh_job.is_some());
    finish_slice_job(&mut app);
    assert!(app.ui.status.contains("cross"), "{}", app.ui.status);
    assert_eq!(app.document.geometry_snapshot(), before);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            slice_key(egui::Key::Enter, false),
            slice_key(egui::Key::Delete, true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            slice_key(egui::Key::Enter, true),
            slice_key(egui::Key::Delete, false),
        ],
    );
    assert!(app.mesh_job.is_some(), "draft was lost after failure");
    app.cancel_mesh_job();
    navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Enter, false)]);
    navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Enter, true)]);
    assert!(app.mesh_job.is_some(), "draft was lost after cancellation");
    finish_slice_job(&mut app);
    assert_eq!(app.document.history_counts(), (1, 0), "{}", app.ui.status);
    app.document.mesh().validate_closed().unwrap();
}

#[test]
fn slice_corner_handles_round_and_sharpen_without_moving_or_adding_points() {
    let mut results = Vec::new();
    for edit in 0..3 {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.dispatch(Command::SliceTool(true), &ctx);
        app.ui.slice_path_mode = true;
        app.ui.slice_options.mode = crate::sculpt::SliceMode::Trim;
        let before = app.document.geometry_snapshot();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let points = [
            rect.center() + Vec2::new(-240., 100.),
            rect.center() + Vec2::new(0., -100.),
            rect.center() + Vec2::new(240., 100.),
        ];
        for p in points.into_iter().chain([points[1]]) {
            navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(p), primary_event(p, true)],
            );
            navigation_frame(&mut app, &ctx, vec![primary_event(p, false)]);
        }
        if edit > 0 {
            let handle = points[1] + Vec2::new(0., 22.);
            let end = handle + Vec2::new(0., 40.);
            navigation_frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(handle),
                    primary_event(handle, true),
                ],
            );
            navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
            if edit == 2 {
                // The linked tangent handle now sits 80px along the incoming edge.
                let handle = points[1] + (points[0] - points[1]).normalized() * 80.;
                navigation_frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(handle),
                        primary_event(handle, true),
                    ],
                );
                // Move beyond the corner to exercise clamping to a sharp vertex.
                let end = points[1] - (points[0] - points[1]).normalized() * 10.;
                navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
            }
        }
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), (0, 0));
        navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Enter, true)]);
        finish_slice_job(&mut app);
        assert_eq!(app.document.history_counts(), (1, 0), "{}", app.ui.status);
        app.document.mesh().validate_closed().unwrap();
        results.push(app.document.geometry_snapshot());
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
    }
    assert_ne!(
        results[0], results[1],
        "round handle did not change the cut"
    );
    assert_eq!(
        results[0], results[2],
        "sharpening should restore the exact original path"
    );
}

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
                assert!(app.ui.toolbar(ctx, &app.document).is_empty());
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

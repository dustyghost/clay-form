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
    assert!(app.viewport.session.gesture == Some(Gesture::Orbit));
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
    assert!(app.viewport.session.gesture == Some(Gesture::Gizmo));
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
    app.document.invert_mask().unwrap();
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
    app.document.invert_mask().unwrap();
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
                assert!(app.ui.toolbar(ctx, &app.document, &app.camera).is_empty());
                assert!(
                    app.ui
                        .brush_panel(ctx, &app.document, "Test tablet", 1., &app.camera)
                        .is_empty()
                );
                app.ui.settings(ctx, &app.document);
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
    }
    app.ui.preferences.validate().unwrap();
    assert!(!app.document.is_dirty());
}

fn finish_mesh_job(app: &mut App) {
    finish_mesh_job_with_timeout(app, std::time::Duration::from_secs(10));
}
fn finish_mesh_job_with_timeout(app: &mut App, timeout: std::time::Duration) {
    let deadline = std::time::Instant::now() + timeout;
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
    assert!(app.mesh_job.as_ref().unwrap().is_cancelled());
    app.dispatch(Command::Subdivide, &ctx);
    assert!(
        app.mesh_job.as_ref().unwrap().is_cancelled(),
        "retry must not replace the draining worker"
    );
    finish_mesh_job(&mut app);
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
    for algorithm in [
        crate::sculpt::QuadRemeshAlgorithm::SurfaceNets,
        crate::sculpt::QuadRemeshAlgorithm::Relaxed,
    ] {
        automatic_remeshing_with_algorithm(algorithm);
    }
}

fn automatic_remeshing_with_algorithm(algorithm: crate::sculpt::QuadRemeshAlgorithm) {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::SetDynamicMesh(true), &ctx);
    app.dispatch(Command::SetRemeshSettings(16, true), &ctx);
    app.dispatch(Command::SetQuadRemeshAlgorithm(algorithm), &ctx);
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
    finish_mesh_job_with_timeout(&mut app, std::time::Duration::from_secs(60));
    assert_ne!(
        app.document.mesh().triangles.len(),
        sculpted.triangles.len()
    );
    let expected = sculpted
        .remesh_algorithm_cancellable(16, crate::sculpt::Topology::Quads, algorithm, &|| false)
        .unwrap();
    assert_eq!(app.document.mesh(), &expected);
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
    finish_mesh_job_with_timeout(&mut app, std::time::Duration::from_secs(60));
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
    app.document.invert_mask().unwrap();
    assert_eq!(app.document.undo_label(), Some("Invert mask"));
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
    assert_eq!(app.ui.status, "Undid: Invert mask");
    let _ = ctx.run(
        press(egui::Modifiers::CTRL | egui::Modifiers::SHIFT),
        |ctx| app.shortcuts(ctx),
    );
    assert_eq!(app.document.history_counts(), (1, 0));
    assert_eq!(app.ui.status, "Redid: Invert mask");
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
    assert!(app.viewport.session.gesture == Some(Gesture::Orbit));
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
    assert!(app.viewport.session.gesture.is_none());
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
                for command in app.ui.toolbar(ctx, &app.document, &app.camera) {
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
    assert!(app.viewport.session.gesture == Some(Gesture::Primitive));
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
            app.ui.brush_panel(ctx, &app.document, "", 1., &app.camera);
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
    assert!(app.viewport.session.gesture.is_none());
    assert_eq!(app.camera.yaw, 0.);
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::ConvertPrimitive, &ctx);
    let output = frame(&mut app, &ctx, vec![]);
    assert!(!output.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text() == "Segments" || t.galley.text().starts_with("Primitive ·"))));
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
fn stabilized_freehand_respects_spacing_release_and_stroke_boundaries() {
    let mut app = patterned_mask_app(crate::sculpt::StrokeMode::Freehand);
    let mut brush = app.document.brushes()[0].clone();
    brush.stabilize = true;
    brush.stabilizer_distance = 30.;
    brush.spacing = 1.;
    app.document.update_brush(0, brush).unwrap();
    let ctx = egui::Context::default();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    let first_dab = app.document.geometry_snapshot();
    assert_ne!(first_dab, before);
    for delta in [10., -12., 40.] {
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(center + egui::vec2(delta, 0.))],
        );
        assert_eq!(
            app.document.geometry_snapshot(),
            first_dab,
            "jitter or movement below dab spacing must not build up"
        );
    }
    let end = center + egui::vec2(90., 0.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    let painted = app.document.geometry_snapshot();
    assert_ne!(painted, first_dab);
    for _ in 0..3 {
        navigation_frame(&mut app, &ctx, vec![]);
        assert_eq!(app.document.geometry_snapshot(), painted);
    }
    // The release position and zero pen pressure must never flush the tail.
    app.pressure = 0.;
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(end + egui::vec2(100., 0.), false)],
    );
    assert_eq!(app.document.geometry_snapshot(), painted);
    assert_eq!(app.document.history_counts().0, count + 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.pressure = 1.;
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    assert_eq!(
        app.document.geometry_snapshot(),
        first_dab,
        "a new stroke starts with a fresh trailing tip"
    );
    app.dispatch(Command::Undo, &ctx);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(
        app.document.geometry_snapshot(),
        before,
        "undo during a drag blocks further dabs until release"
    );
}

#[test]
fn stabilized_freehand_preserves_pressure_modifiers_masks_symmetry_and_subdivision() {
    use crate::sculpt::{Kind, StrokeMode};
    for kind in [Kind::Standard, Kind::Mask] {
        let run = |stabilize| {
            let mut app = patterned_mask_app(StrokeMode::Freehand);
            let mut geometry = app.document.geometry_snapshot();
            for (i, vertex) in geometry.mesh.vertices.iter_mut().enumerate() {
                vertex.2 = (vertex.0 * 20.).sin() * 0.03;
                geometry.mesh.mask[i] = if vertex.1 > 0. { 1. } else { 0.25 };
            }
            geometry.subdivide().unwrap();
            app.document.replace_geometry(geometry.clone()).unwrap();
            let mut brush = app.document.brushes()[0].clone();
            brush.kind = kind;
            brush.stabilize = stabilize;
            brush.stabilizer_distance = 30.;
            brush.radius = 40.;
            brush.pressure = true;
            brush.pressure_radius = true;
            brush.pressure_curve = 1.5;
            app.document.update_brush(0, brush).unwrap();
            app.document
                .set_symmetry(crate::sculpt::Symmetry {
                    enabled: true,
                    ..Default::default()
                })
                .unwrap();
            let count = app.document.history_counts().0;
            let ctx = egui::Context::default();
            let center = navigation_frame(&mut app, &ctx, vec![]).center() + egui::vec2(0., 10.);
            for (step, modifiers) in [
                egui::Modifiers::NONE,
                egui::Modifiers::NONE,
                egui::Modifiers::ALT,
                egui::Modifiers::ALT,
                egui::Modifiers::NONE,
            ]
            .into_iter()
            .enumerate()
            {
                let offset = step as f32 * 25. + if stabilize && step > 0 { 30. } else { 0. };
                let pos = center + egui::vec2(offset, 0.);
                app.pressure = [0.4, 0.8, 0.6, 1., 0.5][step];
                let mut events = vec![egui::Event::PointerMoved(pos)];
                if step == 0 {
                    events.push(primary_event(pos, true));
                }
                visibility_frame(&mut app, &ctx, modifiers, events);
            }
            let after = app.document.geometry_snapshot();
            assert_ne!(after, geometry);
            if kind == Kind::Standard {
                for (i, vertex) in geometry.mesh.vertices.iter().enumerate() {
                    if geometry.mesh.mask[i] == 1. {
                        assert_eq!(after.mesh.vertices[i], *vertex);
                    }
                }
            }
            app.pressure = 0.;
            navigation_frame(
                &mut app,
                &ctx,
                vec![primary_event(center + egui::vec2(200., 0.), false)],
            );
            assert_eq!(app.document.geometry_snapshot(), after);
            assert_eq!(app.document.history_counts().0, count + 1);
            app.document.set_mesh_level(1).unwrap();
            app.document.set_mesh_level(2).unwrap();
            let propagated = app.document.geometry_snapshot();
            app.document.undo();
            app.document.undo();
            app.document.undo();
            assert_eq!(app.document.geometry_snapshot(), geometry);
            propagated
        };
        assert_eq!(
            run(true),
            run(false),
            "stabilized input must use the same dab pipeline as direct input"
        );
    }
}

#[test]
fn stabilization_toggle_is_per_brush_and_available_to_custom_actions() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::Action(Action::StrokeStabilisation), &ctx);
    assert!(app.document.brushes()[0].stabilize);
    app.ui.selected = 1;
    assert!(!app.document.brushes()[1].stabilize);
    app.ui.selected = 0;
    app.dispatch(Command::Action(Action::StrokeStabilisation), &ctx);
    assert!(!app.document.brushes()[0].stabilize);
    for kind in [crate::sculpt::Kind::Grab, crate::sculpt::Kind::Snake] {
        app.document
            .update_brush(0, crate::sculpt::Brush::new(kind))
            .unwrap();
        app.dispatch(Command::Action(Action::StrokeStabilisation), &ctx);
        assert!(!app.document.brushes()[0].stabilize);
    }
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_undo());
}

#[test]
fn stabilization_leaves_displacement_brushes_and_patterned_strokes_unchanged() {
    use crate::sculpt::{Kind, StrokeMode};
    for (kind, mode) in [
        (Kind::Grab, StrokeMode::Freehand),
        (Kind::Snake, StrokeMode::Freehand),
        (Kind::Mask, StrokeMode::Line),
        (Kind::Mask, StrokeMode::DragRect),
        (Kind::Mask, StrokeMode::DragDot),
    ] {
        let run = |stabilize| {
            let mut app = patterned_mask_app(mode);
            let mut brush = app.document.brushes()[0].clone();
            brush.kind = kind;
            brush.stabilize = stabilize;
            app.document.update_brush(0, brush).unwrap();
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
            let end = center + egui::vec2(50., 20.);
            navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
            app.document.geometry_snapshot()
        };
        assert_eq!(
            run(true),
            run(false),
            "stabilization must not affect {kind:?} in {mode:?}"
        );
    }
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
        assert!(app.viewport.session.gesture == Some(Gesture::Slice));
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
    assert!(!app.ui.slice_active);
    app.dispatch(Command::SliceTool(true), &ctx);
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
    finish_mesh_job(&mut app);
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

fn visibility_frame(
    app: &mut App,
    ctx: &egui::Context,
    modifiers: egui::Modifiers,
    events: Vec<egui::Event>,
) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            modifiers,
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
}
#[test]
fn visibility_rectangle_latches_operation_and_hold_release_or_escape_cancel() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    navigation_frame(&mut app, &ctx, vec![]);
    let before = app.document.geometry_snapshot();
    let start = Pos2::new(490., 160.);
    let end = Pos2::new(760., 670.);
    let modifiers = egui::Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    visibility_frame(
        &mut app,
        &ctx,
        modifiers,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    assert!(app.viewport.session.gesture == Some(Gesture::Select));
    visibility_frame(
        &mut app,
        &ctx,
        modifiers | egui::Modifiers::ALT,
        vec![egui::Event::PointerMoved(end)],
    );
    assert_eq!(
        app.document.geometry_snapshot(),
        before,
        "preview does not mutate geometry"
    );
    visibility_frame(
        &mut app,
        &ctx,
        modifiers | egui::Modifiers::ALT,
        vec![primary_event(end, false)],
    );
    let hidden = app.document.mesh().hidden_face_count();
    assert!(hidden > 0 && hidden < app.document.mesh().triangles.len());
    assert_eq!(app.document.mesh().vertices, before.mesh.vertices);
    assert_eq!(app.camera.zoom, 1.);
    assert_eq!(app.camera.yaw, 0.);
    assert_eq!(app.document.history_counts(), (1, 0));
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), before);
    visibility_frame(
        &mut app,
        &ctx,
        modifiers,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::NONE,
        vec![egui::Event::PointerMoved(end)],
    );
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::NONE,
        vec![primary_event(end, false)],
    );
    assert_eq!(
        app.document.geometry_snapshot(),
        before,
        "hold release cancels the uncommitted region"
    );

    visibility_frame(
        &mut app,
        &ctx,
        modifiers,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    visibility_frame(
        &mut app,
        &ctx,
        modifiers,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::NONE,
        vec![primary_event(end, false)],
    );
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), (0, 1));
}
#[test]
fn visibility_background_click_reveals_and_empty_drag_inverts() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.select_face_region(|p| p.0 > 0., true).unwrap();
    navigation_frame(&mut app, &ctx, vec![]);
    let before: Vec<_> = (0..app.document.mesh().triangles.len())
        .map(|i| app.document.mesh().face_visible(i))
        .collect();
    let modifiers = egui::Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    let start = Pos2::new(30., 50.);
    let end = Pos2::new(60., 90.);
    visibility_frame(
        &mut app,
        &ctx,
        modifiers,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    visibility_frame(
        &mut app,
        &ctx,
        modifiers,
        vec![egui::Event::PointerMoved(end)],
    );
    visibility_frame(&mut app, &ctx, modifiers, vec![primary_event(end, false)]);
    for (i, &visible) in before.iter().enumerate() {
        assert_eq!(app.document.mesh().face_visible(i), !visible);
    }
    visibility_frame(
        &mut app,
        &ctx,
        modifiers,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    visibility_frame(&mut app, &ctx, modifiers, vec![primary_event(start, false)]);
    assert_eq!(app.document.mesh().hidden_face_count(), 0);
    assert_eq!(app.camera.zoom, 1.);
}

#[test]
fn visibility_single_group_click_flips_faces_and_background_click_recovers() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let center = navigation_frame(&mut app, &ctx, vec![]).center();
    let modifiers = egui::Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    // A one-group mesh flips to hidden; the next click hits background and
    // restores it. Rapid clicks must still follow the same visibility rule.
    let total = app.document.mesh().triangles.len();
    for hidden in [total, 0] {
        visibility_frame(
            &mut app,
            &ctx,
            modifiers,
            vec![
                egui::Event::PointerMoved(center),
                primary_event(center, true),
            ],
        );
        visibility_frame(
            &mut app,
            &ctx,
            modifiers,
            vec![primary_event(center, false)],
        );
        assert_eq!(app.document.mesh().hidden_face_count(), hidden);
    }
    app.document.select_face_region(|p| p.0 > 0., true).unwrap();
    let visible: Vec<_> = (0..total)
        .map(|i| app.document.mesh().face_visible(i))
        .collect();
    let left = center - egui::vec2(60., 0.);
    visibility_frame(
        &mut app,
        &ctx,
        modifiers,
        vec![egui::Event::PointerMoved(left), primary_event(left, true)],
    );
    visibility_frame(&mut app, &ctx, modifiers, vec![primary_event(left, false)]);
    for (i, before) in visible.into_iter().enumerate() {
        assert_eq!(app.document.mesh().face_visible(i), !before);
    }
}

#[test]
fn visibility_polygroup_click_hides_clicked_faces_then_flips_the_last_group() {
    let mut app = App::default();
    let mut plane = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
    plane.divisions = [2, 1, 1];
    let mut mesh = plane.mesh().unwrap();
    mesh.create_vertex_group("Left".into(), 0.5, true).unwrap();
    mesh.mask = mesh
        .vertices
        .iter()
        .map(|p| if p.0 >= 0. { 1. } else { 0. })
        .collect();
    mesh.create_vertex_group("Right".into(), 0.5, false)
        .unwrap();
    app.document
        .replace_geometry(crate::sculpt::Geometry::new(mesh))
        .unwrap();
    let ctx = egui::Context::default();
    let right = navigation_frame(&mut app, &ctx, vec![]).center() + egui::vec2(80., 0.);
    let modifiers = egui::Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    let left = right - egui::vec2(160., 0.);
    for (point, hidden_id) in [(right, 2), (left, 1), (right, 2)] {
        visibility_frame(
            &mut app,
            &ctx,
            modifiers,
            vec![egui::Event::PointerMoved(point), primary_event(point, true)],
        );
        visibility_frame(&mut app, &ctx, modifiers, vec![primary_event(point, false)]);
        assert_eq!(app.document.mesh().hidden_face_count(), 2);
        for (i, &id) in app.document.mesh().polygroups.iter().enumerate() {
            assert_eq!(app.document.mesh().face_visible(i), id != hidden_id);
        }
    }
}

#[test]
fn escape_exits_line_and_path_slice_with_or_without_a_draft() {
    for path in [false, true] {
        for draft in [false, true] {
            let mut app = App::default();
            let ctx = egui::Context::default();
            app.ui.slice_path_mode = path;
            app.dispatch(Command::SliceTool(true), &ctx);
            let center = navigation_frame(&mut app, &ctx, vec![]).center();
            let before = app.document.geometry_snapshot();
            if draft {
                navigation_frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(center),
                        primary_event(center, true),
                    ],
                );
                if path {
                    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
                } else {
                    navigation_frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(center + egui::vec2(0., 80.))],
                    );
                }
            }
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
            assert!(!app.ui.slice_active);
            assert!(app.viewport.session.gesture.is_none());
            navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
            navigation_frame(&mut app, &ctx, vec![]);
            assert!(app.mesh_job.is_none());
            assert_eq!(app.document.geometry_snapshot(), before);
            assert_eq!(app.document.history_counts(), (0, 0));
            navigation_frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(center),
                    primary_event(center, true),
                ],
            );
            assert!(app.viewport.session.gesture == Some(Gesture::Sculpt));
        }
    }
}

#[test]
fn custom_buttons_and_number_keys_use_current_brush_identity_and_command_guards() {
    use crate::preferences::{Action, Shortcut};
    let mut app = App::default();
    let ctx = egui::Context::default();
    let id = app.document.brushes()[1].identity();
    app.ui
        .preferences
        .bind(
            Action::SelectBrush(id.clone()),
            Shortcut::new(egui::Key::Num1, egui::Modifiers::NONE),
        )
        .unwrap();
    app.ui
        .preferences
        .bind(
            Action::InvertMask,
            Shortcut::new(egui::Key::Num1, egui::Modifiers::SHIFT),
        )
        .unwrap();
    let press = |key, physical, modifiers| egui::RawInput {
        events: vec![
            egui::Event::Key {
                key,
                physical_key: Some(physical),
                pressed: true,
                repeat: false,
                modifiers,
            },
            egui::Event::Key {
                key,
                physical_key: Some(physical),
                pressed: false,
                repeat: false,
                modifiers,
            },
        ],
        ..Default::default()
    };
    let _ = ctx.run(
        press(egui::Key::Num1, egui::Key::Num1, egui::Modifiers::NONE),
        |ctx| app.shortcuts(ctx),
    );
    assert_eq!(app.document.brushes()[app.ui.selected].identity(), id);
    let before = app.document.mesh().mask.clone();
    let _ = ctx.run(
        press(
            egui::Key::Exclamationmark,
            egui::Key::Num1,
            egui::Modifiers::SHIFT,
        ),
        |ctx| app.shortcuts(ctx),
    );
    for (a, b) in app.document.mesh().mask.iter().zip(&before) {
        assert!((a - (1. - b)).abs() < 1e-6);
    }
    app.dispatch(Command::Action(Action::Undo), &ctx);
    assert_eq!(app.document.mesh().mask, before);
    let selected = app.ui.selected;
    app.dispatch(
        Command::Action(Action::SelectBrush("missing-brush".into())),
        &ctx,
    );
    assert_eq!(app.ui.selected, selected);
    app.ui.pending = Some(PendingAction::New);
    app.dispatch(Command::Action(Action::InvertMask), &ctx);
    assert_eq!(app.document.mesh().mask, before);
    app.ui.pending = None;
    app.ui.mesh_busy = true;
    app.dispatch(Command::Action(Action::InvertMask), &ctx);
    assert_eq!(app.document.mesh().mask, before);
}

#[test]
fn custom_menu_shortcut_and_all_shelf_placements_render_without_document_edits() {
    use crate::preferences::{Action, CustomItem, Placement, Shortcut};
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.preferences.menus.clear();
    let id = app.ui.preferences.add_menu();
    app.ui.preferences.menus[0].items = vec![
        CustomItem::action(Action::Wireframe),
        CustomItem::action(Action::ClearMask),
    ];
    app.ui
        .preferences
        .bind(
            Action::OpenMenu(id),
            Shortcut::new(egui::Key::Num0, egui::Modifiers::SHIFT),
        )
        .unwrap();
    app.dispatch(Command::Action(Action::OpenMenu(id)), &ctx);
    assert_eq!(app.ui.customization.popup_menu, Some(id));
    for placement in Placement::ALL {
        app.ui.preferences.menus[0].placement = placement;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1440., 1000.),
                )),
                ..Default::default()
            },
            |ctx| {
                assert!(
                    app.ui
                        .toolbar(ctx, &app.document, &crate::camera::Camera::default())
                        .is_empty()
                );
                assert!(app.ui.custom_surfaces(ctx, &app.document).is_empty());
                app.ui.layout_open = true;
                app.ui.settings(ctx, &app.document);
                egui::CentralPanel::default().show(ctx, |ui| {
                    assert!(ui.available_width() > 0.);
                });
            },
        );
    }
    app.ui.preferences.validate().unwrap();
    assert!(!app.document.is_dirty());
}

#[test]
fn alt_pan_drag_moves_only_the_camera_on_background_gizmo_and_secondary_drag() {
    for (button, location) in [
        (egui::PointerButton::Primary, 1),
        (egui::PointerButton::Primary, 2),
        (egui::PointerButton::Secondary, 0),
    ] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let start = match location {
            1 => rect.min + Vec2::splat(35.),
            2 => Pos2::new(rect.right() - 79., rect.top() + 79.),
            _ => rect.center(),
        };
        let before = app.document.geometry_snapshot();
        let frame =
            |app: &mut App, pos: Pos2, modifiers: egui::Modifiers, pressed: Option<bool>| {
                let mut events = vec![egui::Event::PointerMoved(pos)];
                if let Some(pressed) = pressed {
                    events.push(egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers,
                    });
                }
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1000., 800.),
                        )),
                        modifiers,
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                    },
                );
            };
        frame(&mut app, start, egui::Modifiers::ALT, Some(true));
        assert_eq!(
            app.camera.pan,
            Vec2::ZERO,
            "press-frame motion must not jump the view"
        );
        frame(
            &mut app,
            start + Vec2::new(45., 15.),
            egui::Modifiers::ALT,
            None,
        );
        assert_eq!(app.camera.pan, Vec2::new(45., 15.));
        assert_eq!(app.camera.yaw, 0.);
        assert_eq!(app.camera.pitch, 0.);
        assert_eq!(app.camera.zoom, 1.);
        frame(
            &mut app,
            start + Vec2::new(55., 20.),
            egui::Modifiers::NONE,
            None,
        );
        assert_eq!(app.camera.pan, Vec2::new(45., 15.));
        assert_ne!(app.camera.yaw, 0.);
        frame(
            &mut app,
            start + Vec2::new(55., 20.),
            egui::Modifiers::NONE,
            Some(false),
        );
        assert_eq!(app.document.geometry_snapshot(), before);
        assert!(!app.document.is_dirty());
        assert!(!app.document.can_undo());
    }
}

#[test]
fn pan_hold_can_be_rebound_or_disabled() {
    for (hold, expect_pan) in [
        (crate::preferences::Hold::Shift, true),
        (crate::preferences::Hold::Disabled, false),
    ] {
        let mut app = App::default();
        app.ui.preferences.pan_hold = hold;
        let ctx = egui::Context::default();
        let start = navigation_frame(&mut app, &ctx, vec![]).min + Vec2::splat(35.);
        for (pos, pressed) in [
            (start, Some(true)),
            (start + Vec2::new(-30., 20.), None),
            (start + Vec2::new(-30., 20.), Some(false)),
        ] {
            let mut events = vec![egui::Event::PointerMoved(pos)];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::SHIFT,
                });
            }
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1000., 800.),
                    )),
                    modifiers: egui::Modifiers::SHIFT,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
        }
        assert_eq!(app.camera.pan != Vec2::ZERO, expect_pan);
        assert_eq!(app.camera.yaw == 0., expect_pan);
        assert!(!app.document.is_dirty());
    }
}

#[test]
fn group_all_command_replaces_menu_entries_and_selects_the_new_group() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::CreateVertexGroup("Unified".into(), true), &ctx);
    assert_eq!(app.document.mesh().vertex_groups.len(), 1);
    assert_eq!(app.document.mesh().vertex_groups[0].name, "Unified");
    assert_eq!(app.ui.selected_vertex_group, Some((0, 0)));
    assert_eq!(app.ui.vertex_group_rename, "Unified");
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.mesh().vertex_groups[0].name, "Base");
}

#[test]
fn alt_primary_on_clay_sculpts_and_reverses_without_panning() {
    use crate::sculpt::{Brush, Kind, StrokeMode};
    for mode in [StrokeMode::Freehand, StrokeMode::DragDot] {
        for click_only in [false, true] {
            let mut app = App::default();
            let mut brush = Brush::new(Kind::Clay);
            brush.stroke_mode = mode;
            brush.pressure = false;
            app.document.update_brush(0, brush).unwrap();
            app.document.mark_saved("alt-sculpt-test.clay");
            let original = app.document.mesh().clone();
            let ctx = egui::Context::default();
            let center = navigation_frame(&mut app, &ctx, vec![]).center();
            let end = center
                + if click_only {
                    Vec2::ZERO
                } else {
                    Vec2::new(24., 12.)
                };
            for (pos, pressed) in [(center, Some(true)), (end, None), (end, Some(false))] {
                let mut events = vec![egui::Event::PointerMoved(pos)];
                if let Some(pressed) = pressed {
                    events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::ALT,
                    });
                }
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1000., 800.),
                        )),
                        events,
                        modifiers: egui::Modifiers::ALT,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                    },
                );
                if pressed == Some(true) {
                    assert!(app.viewport.session.gesture == Some(Gesture::Sculpt));
                }
            }
            assert_eq!(app.camera.pan, Vec2::ZERO);
            assert_eq!(app.camera.yaw, 0.);
            assert_eq!(app.camera.pitch, 0.);
            assert_eq!(app.camera.zoom, 1.);
            assert_ne!(*app.document.mesh(), original);
            assert!(app.document.can_undo());
            // Visible front faces point toward +Z; Alt should carve inward.
            assert!(
                app.document
                    .mesh()
                    .vertices
                    .iter()
                    .zip(&original.vertices)
                    .any(|(after, before)| before.2 > 0.5 && after.2 < before.2 - 1e-5)
            );
            app.dispatch(Command::Undo, &ctx);
            assert_eq!(*app.document.mesh(), original);
        }
    }
}

#[test]
fn primary_and_secondary_navigation_continue_past_vertical_poles() {
    for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let start = navigation_frame(&mut app, &ctx, vec![]).min + Vec2::splat(30.);
        let original = app.document.geometry_snapshot();
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
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start + Vec2::new(0., 250.))],
        );
        assert!(app.camera.pitch < -std::f32::consts::FRAC_PI_2);
        let at_pole = app.camera.basis().2;
        let end = start + Vec2::new(0., 500.);
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert!((app.camera.basis().2 - at_pole).len() > 0.5);
        navigation_frame(&mut app, &ctx, vec![event(end, false)]);
        assert_eq!(app.document.geometry_snapshot(), original);
        assert!(!app.document.is_dirty());
        assert!(!app.document.can_undo());
    }
}

#[test]
fn mask_filter_actions_support_shortcuts_guard_jobs_and_end_drag_input() {
    for (action, key) in [
        (Action::BlurMask, egui::Key::F6),
        (Action::SharpenMask, egui::Key::F7),
    ] {
        let mut app = patterned_mask_app(crate::sculpt::StrokeMode::Freehand);
        let mut geometry = app.document.geometry_snapshot();
        geometry.mesh.mask = geometry
            .mesh
            .vertices
            .iter()
            .map(|p| if p.0 < 0. { 0.2 } else { 0.8 })
            .collect();
        app.document.replace_geometry(geometry).unwrap();
        let ctx = egui::Context::default();
        app.ui
            .preferences
            .bind(
                action.clone(),
                crate::preferences::Shortcut::new(key, egui::Modifiers::NONE),
            )
            .unwrap();
        let before = app.document.geometry_snapshot();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| app.shortcuts(ctx),
        );
        assert_ne!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Subdivide, &ctx);
        app.dispatch(
            if action == Action::BlurMask {
                Command::BlurMask
            } else {
                Command::SharpenMask
            },
            &ctx,
        );
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::CancelMeshJob, &ctx);
        app.poll_mesh_job();
        let center = navigation_frame(&mut app, &ctx, vec![]).center();
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(center),
                primary_event(center, true),
            ],
        );
        app.dispatch(Command::Action(action), &ctx);
        let filtered = app.document.geometry_snapshot();
        let end = center + egui::vec2(60., 0.);
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_eq!(
            app.document.geometry_snapshot(),
            filtered,
            "filter shortcuts must not restart a held sculpt stroke"
        );
    }
}

mod pinned_menus {
    use super::*;
    use crate::preferences::{BuiltinMenu, PinnedMenu};

    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1400., 900.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                for command in app.ui.toolbar(ctx, &app.document, &app.camera) {
                    app.dispatch(command, ctx);
                }
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        )
    }
    fn text_point(output: &egui::FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .unwrap_or_else(|| panic!("Missing control: {label}"))
    }
    fn icon_point(ctx: &egui::Context, label: &str) -> Pos2 {
        ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("icon_button", label))))
            .unwrap()
            .center()
    }
    fn click(app: &mut App, ctx: &egui::Context, point: Pos2) -> egui::FullOutput {
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(point), primary_event(point, true)],
        );
        frame(app, ctx, vec![primary_event(point, false)])
    }

    #[test]
    fn pinned_menu_can_pin_move_use_and_unpin_without_sculpting() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let before = app.document.geometry_snapshot();
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Mask"));
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, icon_point(&ctx, "Pin menu open"));
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.ui.preferences.pinned_menus.len(), 1);
        assert_eq!(app.ui.preferences.pinned_menus[0].menu, BuiltinMenu::Mask);
        let position = app.ui.preferences.pinned_menus[0].position;
        let title = Pos2::new(position[0] + 100., position[1] + 16.);
        let end = title + Vec2::new(-250., 180.);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(title), primary_event(title, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![primary_event(end, false)]);
        let moved = app.ui.preferences.pinned_menus[0].position;
        assert!(
            moved[1] > position[1] + 100.,
            "title drag must move the window"
        );
        assert_eq!(app.document.geometry_snapshot(), before);
        assert!(!app.document.is_dirty());
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Invert mask"));
        assert!(app.document.mesh().mask.iter().all(|m| *m == 1.));
        assert_eq!(
            app.document.history_counts().0,
            1,
            "window control must not also sculpt"
        );
        assert_eq!(app.ui.preferences.pinned_menus.len(), 1);
        app.ui.preferences.compact_toolbar = true;
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Clear mask"));
        assert!(app.document.mesh().mask.iter().all(|m| *m == 0.));
        assert_eq!(app.ui.preferences.pinned_menus[0].position, moved);
        frame(&mut app, &ctx, vec![]);
        let before_close = app.document.geometry_snapshot();
        click(&mut app, &ctx, icon_point(&ctx, "Unpin menu"));
        assert!(app.ui.preferences.pinned_menus.is_empty());
        assert_eq!(app.document.geometry_snapshot(), before_close);
    }

    #[test]
    fn modeling_menu_bar_pin_move_and_dock_preserve_geometry_and_choices() {
        use crate::preferences::Dock;
        use crate::viewport::{ComponentMode, ModelTool, SelectionSpan};
        let mut app = App::default();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let before = app.document.geometry_snapshot();
        let history = app.document.history_counts();
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Modeling"));
        assert!(
            !app.ui.modeling.active,
            "opening the menu alone does not switch tools"
        );
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Edge"));
        assert_eq!(app.ui.modeling.mode, ComponentMode::Edge);
        assert!(egui::Popup::is_any_open(&ctx));
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Parallel edges"));
        assert_eq!(app.ui.modeling.span, SelectionSpan::Ring);
        assert!(egui::Popup::is_any_open(&ctx));
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Move components"));
        assert_eq!(app.ui.modeling.tool, ModelTool::Move);
        assert!(!egui::Popup::is_any_open(&ctx));
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Modeling"));
        let selection = app.viewport.modeling.selection().clone();
        frame(
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
        assert!(!egui::Popup::is_any_open(&ctx));
        assert!(app.ui.modeling.active);
        assert_eq!(app.ui.modeling.tool, ModelTool::Move);
        assert_eq!(app.viewport.modeling.selection(), &selection);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Modeling"));
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, icon_point(&ctx, "Pin menu open"));
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.ui.preferences.pinned_menus[0].menu,
            BuiltinMenu::Modeling
        );
        let position = app.ui.preferences.pinned_menus[0].position;
        let title = Pos2::new(position[0] + 100., position[1] + 16.);
        let end = title + Vec2::new(-120., 60.);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(title), primary_event(title, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_ne!(app.ui.preferences.pinned_menus[0].position, position);
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Individual"));
        assert_eq!(app.ui.modeling.span, SelectionSpan::Single);
        assert_eq!(app.ui.modeling.tool, ModelTool::Move);
        assert_eq!(app.ui.preferences.pinned_menus.len(), 1);
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, icon_point(&ctx, "Make collapsible on left"));
        assert!(app.ui.preferences.pinned_menus.is_empty());
        assert_eq!(
            app.ui.preferences.menu_sections[0].menu,
            BuiltinMenu::Modeling
        );
        assert_eq!(app.ui.preferences.menu_sections[0].dock, Dock::Left);
        app.ui
            .preferences
            .save_workspace_as("Modeling placement")
            .unwrap();
        let restored: crate::preferences::Preferences =
            serde_json::from_str(&serde_json::to_string(&app.ui.preferences).unwrap()).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.menu_sections, app.ui.preferences.menu_sections);
        assert_eq!(restored.workspaces, app.ui.preferences.workspaces);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), history);
    }

    #[test]
    fn transform_placement_icons_move_shared_controls_without_duplicate_toolbar() {
        use crate::preferences::{Dock, Tool};
        let mut app = App::default();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let before = app.document.geometry_snapshot();
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_point(&output, "Transform"));
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, icon_point(&ctx, "Pin menu open"));
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.ui.preferences.pinned_menus[0].menu,
            BuiltinMenu::Transform
        );
        assert_eq!(
            app.ui
                .preferences
                .panels
                .iter()
                .find(|p| p.tool == Tool::Transform)
                .unwrap()
                .dock,
            Dock::Hidden
        );
        assert!(
            app.ui
                .transform_toolbar(&ctx, egui::Rect::EVERYTHING, true)
                .1
                .is_none()
        );
        for dock in [Dock::Right, Dock::Left] {
            if dock == Dock::Left {
                app.ui
                    .preferences
                    .pin_menu(BuiltinMenu::Transform, [400., 200.]);
            }
            frame(&mut app, &ctx, vec![]);
            let label = if dock == Dock::Left {
                "Make collapsible on left"
            } else {
                "Make collapsible on right"
            };
            click(&mut app, &ctx, icon_point(&ctx, label));
            assert!(app.ui.preferences.pinned_menus.is_empty());
            assert_eq!(app.ui.preferences.menu_sections[0].dock, dock);
            assert!(
                app.ui
                    .transform_toolbar(&ctx, egui::Rect::EVERYTHING, true)
                    .1
                    .is_none()
            );
        }
        app.ui
            .preferences
            .save_workspace_as("Transform layout")
            .unwrap();
        let encoded = serde_json::to_vec(&app.ui.preferences).unwrap();
        let restored: crate::preferences::Preferences = serde_json::from_slice(&encoded).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.menu_sections, app.ui.preferences.menu_sections);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert!(!app.document.is_dirty());
    }

    #[test]
    fn pinned_menus_restore_workspace_positions_and_keep_symmetry_controls_live() {
        let mut app = App::default();
        app.ui.preferences.pinned_menus = vec![
            PinnedMenu {
                menu: BuiltinMenu::Mask,
                position: [320., 180.],
            },
            PinnedMenu {
                menu: BuiltinMenu::Symmetry,
                position: [780., 180.],
            },
        ];
        app.ui.preferences.save_workspace_as("Sculpt").unwrap();
        app.ui.preferences.save_workspace_as("Detail").unwrap();
        app.ui.preferences.pinned_menus[0].position = [80., 200.];
        app.ui.preferences.pinned_menus[1].position = [500., 200.];
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        let before = app.document.geometry_snapshot();
        click(&mut app, &ctx, text_point(&output, "Enable symmetry"));
        assert!(app.document.symmetry().enabled);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.ui.preferences.pinned_menus.len(), 2);
        app.ui.preferences.switch_workspace(0);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.ui.preferences.pinned_menus[0].position, [320., 180.]);
        assert_eq!(app.ui.preferences.pinned_menus[1].position, [780., 180.]);
        // Loading into a fresh UI context restores both pin state and position.
        let saved = serde_json::to_string(&app.ui.preferences).unwrap();
        app.ui.preferences = serde_json::from_str(&saved).unwrap();
        let fresh = egui::Context::default();
        frame(&mut app, &fresh, vec![]);
        frame(&mut app, &fresh, vec![]);
        assert_eq!(app.ui.preferences.pinned_menus[0].position, [320., 180.]);
        assert_eq!(app.ui.preferences.pinned_menus[1].position, [780., 180.]);
    }
}

#[test]
fn gizmo_axis_taps_tolerate_jitter_and_keep_the_pressed_target() {
    use crate::sculpt::Vec3;
    for (axis, yaw, pitch) in [
        (Vec3(1., 0., 0.), std::f32::consts::FRAC_PI_2, 0.),
        (Vec3(0., 1., 0.), 0., -std::f32::consts::FRAC_PI_2),
        (Vec3(0., 0., 1.), 0., 0.),
    ] {
        let mut app = App::default();
        app.camera.yaw = 0.65;
        app.camera.pitch = -0.35;
        let ctx = egui::Context::default();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let center = Pos2::new(rect.right() - 79., rect.top() + 79.);
        let (right, up, _) = app.camera.basis();
        let start = center + Vec2::new(axis.dot(right), -axis.dot(up)) * 42. + Vec2::new(10., 0.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        for delta in [Vec2::new(3., 2.), Vec2::new(-3., -2.), Vec2::new(4., 1.)] {
            navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start + delta)],
            );
            assert_eq!(
                (app.camera.yaw, app.camera.pitch),
                (0.65, -0.35),
                "tap jitter must not rotate the view"
            );
        }
        // Release beyond the original hit circle, but still within tap tolerance.
        navigation_frame(
            &mut app,
            &ctx,
            vec![primary_event(start + Vec2::new(4., 1.), false)],
        );
        assert_eq!((app.camera.yaw, app.camera.pitch), (yaw, pitch));
        assert!(!app.document.is_dirty());
        assert!(!app.document.can_undo());
    }
}

#[test]
fn gizmo_deliberate_drag_latches_and_fast_tap_snaps() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let center = Pos2::new(rect.right() - 79., rect.top() + 79.);
    let x = center + Vec2::new(42., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(x + Vec2::new(10., 0.)),
            primary_event(x + Vec2::new(10., 0.), true),
            egui::Event::PointerMoved(x + Vec2::new(15., 0.)),
            primary_event(x + Vec2::new(15., 0.), false),
        ],
    );
    assert_eq!(app.camera.yaw, std::f32::consts::FRAC_PI_2);
    app.camera.reset();
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
        vec![egui::Event::PointerMoved(center + Vec2::new(8., 0.))],
    );
    assert!(
        (app.camera.yaw + 0.064).abs() < 1e-6,
        "first deliberate drag includes accumulated movement"
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::new(2., 0.))],
    );
    let yaw = app.camera.yaw;
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(center + Vec2::new(2., 0.), false)],
    );
    assert_eq!(
        app.camera.yaw, yaw,
        "a drag returning near its start must not snap"
    );
    assert_ne!(yaw, 0.);
    assert!(!app.document.is_dirty());
}

fn lasso_points() -> [Pos2; 7] {
    [
        Pos2::new(490., 160.),
        Pos2::new(770., 160.),
        Pos2::new(770., 400.),
        Pos2::new(620., 400.),
        Pos2::new(620., 670.),
        Pos2::new(490., 670.),
        Pos2::new(490., 160.),
    ]
}

#[test]
fn lasso_previews_then_commits_once_at_multiple_angles_and_keeps_other_models() {
    use crate::viewport::SelectionTool;
    for tool in [SelectionTool::MaskLasso, SelectionTool::Lasso] {
        for yaw in [0., 1.2, 3.1] {
            for alt in [false, true] {
                let mut app = App::default();
                app.camera.yaw = yaw;
                app.camera.pitch = 0.4;
                if tool == SelectionTool::MaskLasso && alt {
                    app.document.invert_mask().unwrap();
                }
                app.document.duplicate_model(false).unwrap();
                let before = app.document.geometry_snapshot();
                let count = app.document.history_counts().0;
                let ctx = egui::Context::default();
                app.dispatch(Command::SelectTool(Some(tool)), &ctx);
                navigation_frame(&mut app, &ctx, vec![]);
                let points = lasso_points();
                visibility_frame(
                    &mut app,
                    &ctx,
                    egui::Modifiers::NONE,
                    vec![
                        egui::Event::PointerMoved(points[0]),
                        primary_event(points[0], true),
                    ],
                );
                let modifiers = if alt {
                    egui::Modifiers::ALT
                } else {
                    egui::Modifiers::NONE
                };
                // A batch of pen samples, returning exactly to the first point,
                // must still form a lasso, not a click or a zero-size rectangle.
                visibility_frame(
                    &mut app,
                    &ctx,
                    modifiers,
                    points[1..]
                        .iter()
                        .copied()
                        .map(egui::Event::PointerMoved)
                        .collect(),
                );
                assert_eq!(
                    app.document.geometry_snapshot(),
                    before,
                    "preview must not edit"
                );
                assert!(app.viewport.session.gesture == Some(Gesture::Select));
                visibility_frame(
                    &mut app,
                    &ctx,
                    modifiers,
                    vec![primary_event(points[0], false)],
                );
                let after = app.document.geometry_snapshot();
                assert_ne!(after, before);
                assert_eq!(after.submodels, before.submodels);
                assert_eq!(after.mesh.vertices, before.mesh.vertices);
                assert_eq!(after.mesh.vertex_groups, before.mesh.vertex_groups);
                assert_eq!(app.document.history_counts().0, count + 1);
                assert_eq!(app.camera.yaw, yaw);
                assert_eq!(app.camera.zoom, 1.);
                if tool == SelectionTool::MaskLasso {
                    let changed = after
                        .mesh
                        .mask
                        .iter()
                        .zip(&before.mesh.mask)
                        .filter(|(a, b)| a != b)
                        .count();
                    assert!(changed > 0 && changed < after.mesh.vertices.len());
                    let view = app.camera.basis().2;
                    // Through-depth selection must hit both sides of the sphere.
                    for sign in [-1., 1.] {
                        assert!(
                            after
                                .mesh
                                .vertices
                                .iter()
                                .enumerate()
                                .any(|(i, p)| p.dot(view) * sign > 0.3
                                    && after.mesh.mask[i] != before.mesh.mask[i])
                        );
                    }
                } else {
                    assert!(after.mesh.hidden_face_count() > 0);
                    assert!(after.mesh.hidden_face_count() < after.mesh.triangles.len());
                    assert_eq!(after.mesh.mask, before.mesh.mask);
                }
                app.dispatch(Command::Undo, &ctx);
                assert_eq!(app.document.geometry_snapshot(), before);
                app.dispatch(Command::Redo, &ctx);
                assert_eq!(app.document.geometry_snapshot(), after);
            }
        }
    }
}

#[test]
fn lasso_cancellation_degenerate_paths_and_tool_switches_preserve_redo() {
    use crate::viewport::SelectionTool;
    for interruption in [
        "escape",
        "tool",
        "camera",
        "focus",
        "degenerate",
        "click",
        "lost_release",
        "command",
    ] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.document.invert_mask().unwrap();
        app.document.undo();
        app.document.mark_saved("lasso.clay");
        let before = app.document.geometry_snapshot();
        app.dispatch(Command::SelectTool(Some(SelectionTool::Lasso)), &ctx);
        navigation_frame(&mut app, &ctx, vec![]);
        let points = lasso_points();
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(points[0]),
                primary_event(points[0], true),
            ],
        );
        if !matches!(interruption, "degenerate" | "click") {
            navigation_frame(
                &mut app,
                &ctx,
                points[1..]
                    .iter()
                    .copied()
                    .map(egui::Event::PointerMoved)
                    .collect(),
            );
        }
        match interruption {
            "escape" => navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            ),
            "tool" => {
                app.dispatch(Command::SelectTool(Some(SelectionTool::MaskLasso)), &ctx);
                navigation_frame(&mut app, &ctx, vec![])
            }
            "command" => {
                app.dispatch(Command::ShowAllFaces, &ctx);
                navigation_frame(&mut app, &ctx, vec![]);
                navigation_frame(
                    &mut app,
                    &ctx,
                    points[1..]
                        .iter()
                        .copied()
                        .map(egui::Event::PointerMoved)
                        .collect(),
                )
            }
            "camera" => {
                app.dispatch(Command::ZoomIn, &ctx);
                navigation_frame(&mut app, &ctx, vec![])
            }
            "focus" => {
                let _ = ctx.run(
                    egui::RawInput {
                        focused: false,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                    },
                );
                egui::Rect::NOTHING
            }
            "degenerate" => {
                navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(points[1])])
            }
            "lost_release" => navigation_frame(&mut app, &ctx, vec![egui::Event::PointerGone]),
            _ => egui::Rect::NOTHING,
        };
        navigation_frame(&mut app, &ctx, vec![primary_event(points[0], false)]);
        assert_eq!(app.document.geometry_snapshot(), before, "{interruption}");
        assert_eq!(app.document.history_counts(), (0, 1), "{interruption}");
        assert!(!app.document.is_dirty(), "{interruption}");
        if interruption != "camera" {
            assert_eq!(app.camera.yaw, 0., "{interruption}");
            assert_eq!(app.camera.zoom, 1., "{interruption}");
        }
    }
}

#[test]
fn lasso_space_translation_matches_drawing_at_destination_and_fast_drag() {
    use crate::viewport::SelectionTool;
    let mut results = Vec::new();
    for translate in [false, true] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.dispatch(Command::SelectTool(Some(SelectionTool::MaskLasso)), &ctx);
        navigation_frame(&mut app, &ctx, vec![]);
        let points = lasso_points();
        let shift = Vec2::new(-180., -40.);
        let offset = if translate { Vec2::ZERO } else { shift };
        let start = points[0] + offset;
        let mut events = vec![egui::Event::PointerMoved(start), primary_event(start, true)];
        events.extend(
            points[1..]
                .iter()
                .map(|p| egui::Event::PointerMoved(*p + offset)),
        );
        if translate {
            navigation_frame(&mut app, &ctx, events);
            navigation_frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::Key {
                        key: egui::Key::Space,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerMoved(points[0] + shift),
                    primary_event(points[0] + shift, false),
                ],
            );
        } else {
            events.push(primary_event(start, false));
            navigation_frame(&mut app, &ctx, events);
        }
        assert_eq!(app.document.history_counts(), (1, 0));
        results.push(app.document.geometry_snapshot());
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn lasso_actions_toggle_from_shortcuts_and_obey_busy_guard() {
    use crate::viewport::SelectionTool;
    for (action, tool) in [
        (Action::SelectLasso, SelectionTool::Lasso),
        (Action::MaskLasso, SelectionTool::MaskLasso),
        (
            Action::MaskLine,
            SelectionTool::MaskLine {
                erase: false,
                flip: false,
            },
        ),
    ] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.ui
            .preferences
            .bind(
                action.clone(),
                crate::preferences::Shortcut::new(egui::Key::F8, egui::Modifiers::NONE),
            )
            .unwrap();
        let json = serde_json::to_string(&app.ui.preferences).unwrap();
        app.ui.preferences = serde_json::from_str(&json).unwrap();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::F8,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| app.shortcuts(ctx),
        );
        assert_eq!(app.ui.selection_tool, Some(tool));
        app.dispatch(Command::Action(action.clone()), &ctx);
        assert_eq!(app.ui.selection_tool, None);
        app.dispatch(Command::Subdivide, &ctx);
        app.dispatch(Command::Action(action), &ctx);
        assert_eq!(app.ui.selection_tool, None);
        app.dispatch(Command::CancelMeshJob, &ctx);
        app.poll_mesh_job();
    }
}

mod line_mask;
mod rotation;
mod visibility_selection;

mod obj;

mod references;

mod navigation_smooth;

mod clay;
mod detail;
mod relax;

#[cfg(target_os = "linux")]
mod guide_captures;
mod perspective;

mod mirror;

mod edges;

mod modeling;

mod files;

mod projection;

#[test]
fn delete_submodel_action_removes_hidden_reference_clears_targets_and_locks_during_jobs() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::KeepProjectionReference, &ctx);
    app.dispatch(Command::SelectSubmodel(1), &ctx);
    app.ui.selected_vertex_group = Some((1, 0));
    app.ui.projection.source = Some(crate::document::ProjectionSource::Submodel(0));
    let before = app.document.geometry_snapshot();
    assert!(!app.document.active_visible());
    assert!(
        app.ui
            .action_enabled(&Action::DeleteSubmodel, &app.document)
    );
    app.dispatch(Command::Action(Action::DeleteSubmodel), &ctx);
    assert_eq!(app.document.model_count(), 1);
    assert!(app.document.active_visible());
    assert!(app.ui.selected_vertex_group.is_none());
    assert!(app.ui.projection.source.is_none());
    let after = app.document.geometry_snapshot();
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
    app.dispatch(Command::Subdivide, &ctx);
    assert!(
        !app.ui
            .action_enabled(&Action::DeleteSubmodel, &app.document)
    );
    app.dispatch(Command::DeleteSubmodel, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
    app.dispatch(Command::CancelMeshJob, &ctx);
    finish_mesh_job(&mut app);
    app.dispatch(Command::OpenProjection, &ctx);
    app.dispatch(Command::DeleteSubmodel, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
    app.dispatch(Command::CancelProjection, &ctx);
    app.ui.settings_open = true;
    app.dispatch(Command::DeleteSubmodel, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
    app.ui.settings_open = false;
    app.dispatch(Command::DeleteSubmodel, &ctx);
    assert!(app.document.mesh().vertices.is_empty());
    // The empty slot remains renderable and can host a new primitive.
    navigation_frame(&mut app, &ctx, vec![]);
    app.document.initialize_primitive().unwrap();
    assert!(app.document.primitive().is_some());
}

mod alphas;
mod path_brush;
mod paths;
#[cfg(target_os = "linux")]
mod ring_diagnostics;

mod paint;

mod brush_controls;

#[test]
fn continuous_texture_paint_builds_while_stationary_and_stops_on_release_or_cancel() {
    use crate::app::textures::TextureCommand as T;
    use crate::sculpt::{
        Geometry, Material, PaintMode, Primitive, PrimitiveKind,
        texture::{Channel, Precision},
    };
    for mode in [PaintMode::Stroke, PaintMode::Airbrush, PaintMode::Spray] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        mesh.uv = mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
        app.document.replace_geometry(Geometry::new(mesh)).unwrap();
        app.camera.yaw = 0.;
        app.camera.pitch = 0.;
        app.ui.uv.open = true;
        app.texture_command(T::CreateAndPaint(Channel::Color, 128, Precision::U16))
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.texture_exchange.is_some() {
            assert!(std::time::Instant::now() < deadline);
            app.poll_texture_exchange(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!app.ui.uv.open && !app.ui.modeling.active);
        let mut brush = app.document.brushes()[app.ui.selected].clone();
        brush.paint_effect.mode = mode;
        brush.paint_effect.dot_size = 0.08;
        brush.paint_effect.density = 0.8;
        brush.paint_effect.flow = 0.8;
        brush.material = Material::from_srgb([1., 0., 0.], 0.5, 0.);
        brush.radius = 100.;
        brush.strength = 0.6;
        app.dispatch(Command::UpdateBrush(app.ui.selected, brush), &ctx);
        let pixels = |app: &App| app.document.mesh().uv.textures[0].pixels.clone();
        let original = pixels(&app);
        let vertices = app.document.mesh().vertices.clone();
        let mut now = 0.;
        let mut frame = |app: &mut App, events| {
            now += 0.1;
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(now),
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
            );
        };
        frame(&mut app, vec![]);
        let point = Pos2::new(500., 400.);
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(point), primary_event(point, true)],
        );
        let first = pixels(&app);
        assert_ne!(first, original);
        for _ in 0..8 {
            frame(&mut app, vec![]);
        }
        let held = pixels(&app);
        if mode == PaintMode::Stroke {
            assert_eq!(held, first);
        } else {
            assert_ne!(held, first);
        }
        frame(&mut app, vec![primary_event(point, false)]);
        let after = pixels(&app);
        for _ in 0..3 {
            frame(&mut app, vec![]);
        }
        assert_eq!(pixels(&app), after);
        assert_eq!(app.document.mesh().vertices, vertices);
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(pixels(&app), original);
        app.dispatch(Command::Redo, &ctx);
        assert_eq!(pixels(&app), after);
        frame(&mut app, vec![primary_event(point, true)]);
        for _ in 0..3 {
            frame(&mut app, vec![]);
        }
        frame(
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        frame(&mut app, vec![primary_event(point, false)]);
        assert_eq!(pixels(&app), after, "Escape restores the whole held stroke");
    }
}

#[test]
fn uv_island_selection_highlight_does_not_rebuild_topology_for_every_face() {
    use crate::sculpt::{Geometry, Mesh, Primitive, PrimitiveKind};
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
    plane.divisions = [32, 32, 1];
    let mut mesh = plane.mesh().unwrap();
    mesh.uv.topology = mesh.triangles.clone();
    mesh.uv.corners = mesh
        .triangles
        .iter()
        .map(|t| {
            t.map(|i| {
                [
                    (mesh.vertices[i].0 + 1.) * 0.5,
                    (mesh.vertices[i].1 + 1.) * 0.5,
                ]
            })
        })
        .collect();
    app.document.replace_geometry(Geometry::new(mesh)).unwrap();
    navigation_frame(&mut app, &ctx, vec![]);
    let revision = app.document.display_revision();
    let history = app.document.history_counts();
    let start = std::time::Instant::now();
    app.dispatch(
        Command::UvSelect((0..app.document.mesh().triangles.len()).collect()),
        &ctx,
    );
    let builds = Mesh::polygon_build_count();
    navigation_frame(&mut app, &ctx, vec![]);
    let builds = Mesh::polygon_build_count() - builds;
    eprintln!(
        "UV island: {} selected polygons, {builds} polygon builds, selection + CPU viewport frame {:?}",
        app.viewport.modeling.selected.len(),
        start.elapsed()
    );
    assert_eq!(app.viewport.modeling.selected.len(), 1024);
    assert!(
        builds < 12,
        "highlight must build topology a bounded number of times, not per selected face: {builds}"
    );
    assert_eq!(app.document.display_revision(), revision);
    assert_eq!(app.document.history_counts(), history);
}

use super::*;
use crate::{
    app::paths::PathCommand,
    document::PathStrokeOptions,
    sculpt::{
        Geometry, Kind, Primitive, PrimitiveKind, Vec3,
        paths::{PathMode, PathPoint},
    },
};

fn setup() -> (App, egui::Context, u64) {
    let mut app = App::default();
    // Controller fixtures intentionally exercise explicit model-unit overrides.
    app.paths.use_brush_size = false;
    let mut grid = Primitive::for_kind(PrimitiveKind::Grid);
    grid.divisions = [12; 3];
    app.document
        .replace_geometry(Geometry::new(grid.mesh().unwrap()))
        .unwrap();
    let points = [Vec3(-0.5, -0.2, 0.), Vec3(0., 0.2, 0.), Vec3(0.5, -0.2, 0.)]
        .into_iter()
        .map(|position| PathPoint {
            position,
            normal: Vec3(0., 0., 1.),
        })
        .collect();
    let id = app
        .document
        .create_path(
            "Controller curve".into(),
            PathMode::Surface,
            points,
            false,
            0.5,
        )
        .unwrap();
    let ctx = egui::Context::default();
    app.document.mark_saved("path-test.clay");
    app.dispatch(Command::Path(PathCommand::Open), &ctx);
    app.dispatch(Command::Path(PathCommand::Select(id)), &ctx);
    assert_eq!(app.viewport.paths.draft.as_ref().unwrap().id, id);
    (app, ctx, id)
}

fn finish_preview(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while app.paths.busy() {
        app.poll_paths();
        assert!(
            std::time::Instant::now() < deadline,
            "path preview timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(app.paths.has_preview(), "{}", app.ui.status);
}

#[test]
fn path_brush_controller_uses_selected_options_and_reuses_attached_guide() {
    let (mut app, ctx, id) = setup();
    app.ui.selected = app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == Kind::Inflate)
        .unwrap();
    let mut brush = app.document.brushes()[app.ui.selected].clone();
    brush.strength = 0.12;
    brush.spacing = 0.35;
    app.dispatch(Command::UpdateBrush(app.ui.selected, brush.clone()), &ctx);
    app.paths.options = PathStrokeOptions {
        radius: 0.24,
        pressure: 0.4,
        reverse: true,
        invert: true,
    };
    app.document.mark_saved("path-test.clay");
    let expected = app
        .document
        .prepare_path_stroke(id, &brush, app.paths.options)
        .unwrap()
        .run(&|| false)
        .unwrap();
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    app.dispatch(Command::Path(PathCommand::Preview), &ctx);
    app.dispatch(Command::InvertMask, &ctx);
    let mut changed_brush = brush.clone();
    changed_brush.strength = 1.;
    app.dispatch(Command::UpdateBrush(app.ui.selected, changed_brush), &ctx);
    assert!(app.document.brushes()[app.ui.selected] == brush);
    finish_preview(&mut app);
    assert_eq!(app.paths.preview_mesh.as_deref(), Some(expected.mesh()));
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), counts);
    assert!(!app.document.is_dirty());
    app.dispatch(Command::Subdivide, &ctx);
    assert!(app.mesh_job.is_none());
    app.dispatch(Command::Path(PathCommand::Apply), &ctx);
    assert_eq!(app.document.mesh(), expected.mesh());
    assert_eq!(app.document.history_counts().0, counts.0 + 1);
    assert_eq!(app.document.undo_label(), Some("Path brush stroke"));
    assert!(!app.paths.has_preview());
    assert!(app.paths.preview_mesh.is_none());
    assert!(app.viewport.paths.active);
    assert!(!app.document.path(id).unwrap().is_stale(app.document.mesh()));
    assert_eq!(app.viewport.paths.draft.as_ref(), app.document.path(id));
    assert!(!app.viewport.paths.dirty(&app.document));
    app.ui.selected = app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == Kind::Clay)
        .unwrap();
    app.paths.options.invert = false;
    let second_before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    app.dispatch(Command::Path(PathCommand::Preview), &ctx);
    finish_preview(&mut app);
    app.dispatch(Command::Path(PathCommand::Apply), &ctx);
    assert_eq!(app.document.history_counts().0, count + 1);
    assert_eq!(
        app.document.path(id).unwrap().target,
        before.paths[0].target
    );
    app.dispatch(Command::Path(PathCommand::Close), &ctx);
    assert!(!app.viewport.paths.active);
    assert!(!app.ui.paths_active);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), second_before);
}

#[test]
fn path_brush_cancel_and_close_discard_running_and_ready_previews() {
    let (mut app, ctx, id) = setup();
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    app.dispatch(Command::Path(PathCommand::Preview), &ctx);
    app.dispatch(Command::Path(PathCommand::CancelPreview), &ctx);
    app.poll_paths();
    assert!(!app.paths.busy());
    assert!(!app.paths.has_preview());
    assert!(app.paths.preview_mesh.is_none());
    assert!(app.viewport.paths.active);

    app.dispatch(Command::Path(PathCommand::Preview), &ctx);
    finish_preview(&mut app);
    app.dispatch(Command::Path(PathCommand::CancelPreview), &ctx);
    assert!(app.paths.preview_mesh.is_none());
    app.dispatch(Command::Path(PathCommand::Preview), &ctx);
    finish_preview(&mut app);
    app.dispatch(Command::Path(PathCommand::Close), &ctx);
    assert!(!app.paths.busy());
    assert!(!app.paths.has_preview());
    assert!(app.paths.preview_mesh.is_none());
    assert!(!app.viewport.paths.active);
    assert!(!app.ui.paths_active);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), counts);
    assert!(!app.document.is_dirty());

    app.dispatch(Command::Path(PathCommand::Open), &ctx);
    app.dispatch(Command::Path(PathCommand::Select(id)), &ctx);
    app.dispatch(Command::Path(PathCommand::Preview), &ctx);
    app.dispatch(Command::Path(PathCommand::Close), &ctx);
    app.poll_paths();
    assert!(!app.paths.busy());
    assert!(app.paths.preview_mesh.is_none());
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn path_brush_stale_apply_clears_rendered_preview_and_reports_error() {
    let (mut app, ctx, _) = setup();
    app.dispatch(Command::Path(PathCommand::Preview), &ctx);
    finish_preview(&mut app);
    // Defensive guard: external document edits can bypass controller locking.
    app.document.invert_mask().unwrap();
    let changed = app.document.geometry_snapshot();
    let count = app.document.history_counts();
    app.dispatch(Command::Path(PathCommand::Apply), &ctx);
    assert!(app.ui.status.contains("scene changed"), "{}", app.ui.status);
    assert_eq!(app.document.geometry_snapshot(), changed);
    assert_eq!(app.document.history_counts(), count);
    assert!(!app.paths.has_preview());
    assert!(app.paths.preview_mesh.is_none());
    app.dispatch(Command::Path(PathCommand::Close), &ctx);
    assert!(!app.viewport.paths.active);
}

#[test]
fn path_authoring_draft_blocks_other_tools_and_reopening_preserves_edits() {
    let (mut app, ctx, _) = setup();
    let before = app.document.geometry_snapshot();
    app.viewport.paths.draft.as_mut().unwrap().name = "Unsaved curve".into();
    let draft = app.viewport.paths.draft.clone();
    for command in [
        Command::InvertMask,
        Command::Undo,
        Command::Subdivide,
        Command::OpenProjection,
    ] {
        app.dispatch(command, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert!(app.mesh_job.is_none());
        assert!(!app.ui.projection.open);
        assert!(
            app.ui.status.contains("Finish editing paths"),
            "{}",
            app.ui.status
        );
    }
    app.dispatch(Command::Path(PathCommand::Open), &ctx);
    assert_eq!(app.viewport.paths.draft, draft);
    app.dispatch(Command::Path(PathCommand::Close), &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::InvertMask, &ctx);
    assert_ne!(app.document.geometry_snapshot(), before);
}

#[test]
fn save_and_sculpt_commits_once_then_guidance_can_be_disabled_without_editing() {
    let (mut app, ctx, id) = setup();
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    app.ui.guide_paths = false;
    app.viewport.paths.draft.as_mut().unwrap().name = "Ready to sculpt".into();

    app.dispatch(Command::Path(PathCommand::SaveAndSculpt), &ctx);
    assert!(!app.viewport.paths.active, "{}", app.ui.status);
    assert!(!app.ui.paths_active);
    assert!(app.ui.guide_paths);
    assert_eq!(app.document.path(id).unwrap().name, "Ready to sculpt");
    assert_eq!(app.document.history_counts().0, counts.0 + 1);
    let saved = app.document.geometry_snapshot();
    let saved_counts = app.document.history_counts();

    // Disabling capture is a tool preference, not a mesh or path edit.
    app.dispatch(Command::Action(Action::TogglePathGuides), &ctx);
    assert!(!app.ui.guide_paths);
    assert!(!app.viewport.guided.enabled);
    assert_eq!(app.document.geometry_snapshot(), saved);
    assert_eq!(app.document.history_counts(), saved_counts);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn failed_save_and_sculpt_keeps_the_invalid_draft_open_for_correction() {
    let (mut app, ctx, _) = setup();
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    app.ui.guide_paths = false;
    app.viewport
        .paths
        .draft
        .as_mut()
        .unwrap()
        .points
        .truncate(1);
    let invalid = app.viewport.paths.draft.clone();

    app.dispatch(Command::Path(PathCommand::SaveAndSculpt), &ctx);
    assert!(app.viewport.paths.active);
    assert!(app.ui.paths_active);
    assert!(!app.ui.guide_paths);
    assert_eq!(app.viewport.paths.draft, invalid);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), counts);
    assert!(!app.document.is_dirty());
    assert!(app.ui.status.contains("point"), "{}", app.ui.status);
}

fn finish_automatic_stroke(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while app.paths.busy() {
        app.poll_paths();
        assert!(
            std::time::Instant::now() < deadline,
            "automatic path stroke timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(!app.paths.has_preview());
    assert!(app.paths.preview_mesh.is_none());
}

#[test]
fn entire_path_command_restores_last_selection_and_commits_without_an_apply_click() {
    let (mut app, ctx, id) = setup();
    let mut points = app.document.path(id).unwrap().points.clone();
    for point in &mut points {
        point.position.1 += 0.3;
    }
    let selected = app
        .document
        .create_path("Second guide".into(), PathMode::Surface, points, false, 0.5)
        .unwrap();
    app.dispatch(Command::Path(PathCommand::Select(selected)), &ctx);
    app.paths.options.radius = 0.17;
    app.paths.options.pressure = 0.4;
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    app.dispatch(Command::Path(PathCommand::Close), &ctx);
    app.dispatch(Command::Path(PathCommand::StrokeEntirePath), &ctx);
    assert!(app.paths.applying_entire_path(), "{}", app.ui.status);
    assert_eq!(app.viewport.paths.draft.as_ref().unwrap().id, selected);
    assert_eq!(app.document.geometry_snapshot(), before);
    finish_automatic_stroke(&mut app);
    assert_ne!(
        app.document.geometry_snapshot(),
        before,
        "{}",
        app.ui.status
    );
    assert_eq!(app.document.history_counts().0, counts.0 + 1);
    assert_eq!(app.document.undo_label(), Some("Path brush stroke"));
    assert_eq!(
        app.viewport.paths.draft.as_ref(),
        app.document.path(selected)
    );
    assert!(
        !app.document
            .path(selected)
            .unwrap()
            .is_stale(app.document.mesh())
    );
    let stroked = app.document.geometry_snapshot();
    app.viewport.paths.selected = Some(1);
    // Actions are the same route used by keyboard shortcuts and custom menus.
    app.dispatch(Command::Action(Action::Undo), &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(app.viewport.paths.active && app.ui.paths_active);
    assert_eq!(app.viewport.paths.selected, Some(1));
    assert_eq!(
        app.viewport.paths.draft.as_ref(),
        app.document.path(selected)
    );
    assert!(!app.viewport.paths.dirty(&app.document));
    assert!(app.ui.action_enabled(&Action::Redo, &app.document));
    app.dispatch(Command::Action(Action::Redo), &ctx);
    assert_eq!(app.document.geometry_snapshot(), stroked);
    assert!(app.viewport.paths.active && app.ui.paths_active);
    assert_eq!(app.viewport.paths.selected, Some(1));
    assert_eq!(
        app.viewport.paths.draft.as_ref(),
        app.document.path(selected)
    );
    assert!(app.document.path_samples(selected, 0.025).is_ok());
}

#[test]
fn entire_path_cancel_noop_and_unsaved_draft_never_commit_geometry() {
    let (mut app, ctx, _) = setup();
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    app.dispatch(Command::Path(PathCommand::StrokeEntirePath), &ctx);
    app.dispatch(Command::Path(PathCommand::CancelPreview), &ctx);
    app.poll_paths();
    assert!(!app.paths.busy());
    assert!(!app.paths.has_preview());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), counts);
    assert!(app.ui.status.contains("cancelled"));

    app.paths.options.pressure = 0.;
    app.dispatch(Command::Path(PathCommand::StrokeEntirePath), &ctx);
    finish_automatic_stroke(&mut app);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), counts);
    assert!(!app.document.is_dirty());
    assert!(app.ui.status.contains("no change"), "{}", app.ui.status);

    app.viewport.paths.draft.as_mut().unwrap().name = "Uncommitted".into();
    let draft = app.viewport.paths.draft.clone();
    app.dispatch(Command::Path(PathCommand::StrokeEntirePath), &ctx);
    assert!(!app.paths.busy());
    assert_eq!(app.viewport.paths.draft, draft);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), counts);
    assert!(app.ui.status.contains("Save the path"));
}

#[test]
fn entire_path_worker_cannot_overwrite_a_newer_document_edit() {
    let (mut app, ctx, _) = setup();
    app.dispatch(Command::Path(PathCommand::StrokeEntirePath), &ctx);
    // Simulate an external edit that bypassed controller locking before commit.
    app.document.invert_mask().unwrap();
    let newer = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    finish_automatic_stroke(&mut app);
    assert_eq!(app.document.geometry_snapshot(), newer);
    assert_eq!(app.document.history_counts(), counts);
    assert!(app.ui.status.contains("scene changed"), "{}", app.ui.status);
}

#[test]
fn rounded_path_save_and_sculpt_preserves_corner_amounts_and_seam_in_one_undo() {
    let (mut app, ctx, _) = setup();
    app.dispatch(Command::Path(PathCommand::New(PathMode::Surface)), &ctx);
    let draft = app.viewport.paths.draft.as_mut().unwrap();
    draft.points = [Vec3(-0.5, -0.3, 0.), Vec3(0., 0.4, 0.), Vec3(0.5, -0.3, 0.)]
        .into_iter()
        .map(|position| PathPoint {
            position,
            normal: Vec3(0., 0., 1.),
        })
        .collect();
    draft.closed = true;
    draft.seam = 1;
    draft.corner_rounding = vec![0., 0.1, 0.07];
    let seam = draft.points[1].position;
    let before = app.document.geometry_snapshot();
    let undo_count = app.document.history_counts().0;
    app.dispatch(Command::Path(PathCommand::SaveAndSculpt), &ctx);
    assert!(!app.viewport.paths.active, "{}", app.ui.status);
    let path = app.document.paths().last().unwrap();
    assert_eq!(path.corner_rounding, vec![0.1, 0.07, 0.]);
    assert!((path.points[0].position - seam).len() < 1e-5);
    assert_eq!(path.seam, 0);
    assert!(path.closed);
    let samples = app.document.path_samples(path.id, 0.01).unwrap();
    assert!(
        samples
            .iter()
            .all(|sample| (sample.position - seam).len() > 0.02)
    );
    assert_eq!(app.document.history_counts().0, undo_count + 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn path_editor_history_discards_private_preview_and_redo_restores_removed_path() {
    let (mut app, ctx, id) = setup();
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::Path(PathCommand::Preview), &ctx);
    finish_preview(&mut app);
    assert!(app.paths.preview_mesh.is_some());
    app.dispatch(Command::Undo, &ctx);
    assert!(app.viewport.paths.active && app.ui.paths_active);
    assert!(!app.paths.has_preview() && app.paths.preview_mesh.is_none());
    assert!(app.document.path(id).is_none());
    assert!(app.viewport.paths.draft.is_none());
    assert_eq!(app.paths.selected_path, Some(id));
    assert!(app.ui.action_enabled(&Action::Redo, &app.document));
    app.dispatch(Command::Action(Action::Redo), &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.viewport.paths.draft.as_ref(), app.document.path(id));
    assert!(app.viewport.paths.active);
    assert!(!app.viewport.paths.dirty(&app.document));
}

#[test]
fn path_editor_history_rejects_running_workers_and_unsaved_drafts() {
    let (mut app, ctx, _) = setup();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.dispatch(Command::Path(PathCommand::StrokeEntirePath), &ctx);
    assert!(app.paths.busy());
    assert!(!app.ui.action_enabled(&Action::Undo, &app.document));
    app.dispatch(Command::Undo, &ctx);
    assert!(
        app.ui.status.contains("running path stroke"),
        "{}",
        app.ui.status
    );
    assert!(app.paths.busy());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::Path(PathCommand::CancelPreview), &ctx);
    app.viewport.paths.draft.as_mut().unwrap().name = "Unsaved draft".into();
    app.sync_path_history_state();
    assert!(!app.ui.action_enabled(&Action::Undo, &app.document));
    let draft = app.viewport.paths.draft.clone();
    for command in [Command::Action(Action::Undo), Command::Undo, Command::Redo] {
        app.dispatch(command, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), history);
        assert_eq!(app.viewport.paths.draft, draft);
    }
    app.dispatch(Command::Path(PathCommand::Discard), &ctx);
    assert!(app.ui.action_enabled(&Action::Undo, &app.document));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn automatic_path_settings_offscreen_preview() {
    let (mut app, _, id) = setup();
    app.paths.use_brush_size = true;
    let mut brush = app.document.brushes()[app.ui.selected].clone();
    brush.radius = 32.;
    brush.strength = 0.35;
    brush.pressure_radius = false;
    app.document.update_brush(app.ui.selected, brush).unwrap();
    app.camera.pan = Vec2::new(190., 0.);
    app.camera.pitch = 0.25;
    let ctx = egui::Context::default();
    let mut capture = crate::viewport::UiCapture::new([1100, 900]);
    app.viewport.initialize_renderer(&capture.state).unwrap();
    let render = |app: &mut App, capture: &mut crate::viewport::UiCapture, name: Option<&str>| {
        let mut texts = Vec::new();
        for frame in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1100., 900.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
            texts = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if shape.clip_rect.intersects(egui::Rect::from_min_size(
                            text.pos,
                            text.galley.size(),
                        )) =>
                    {
                        Some(text.galley.text().to_owned())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let rgba = capture.render(&ctx, output);
            if frame == 2
                && let Some(name) = name
            {
                capture.save(format!("/tmp/clay-automatic-path-{name}.png"), &rgba);
            }
        }
        texts
    };
    render(&mut app, &mut capture, None);
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::Path(PathCommand::StrokeEntirePath), &ctx);
    finish_automatic_stroke(&mut app);
    assert_eq!(app.document.undo_label(), Some("Path brush stroke"));
    let texts = render(&mut app, &mut capture, Some("settings"));
    for required in ["Undo", "Redo", "Stroke entire path", "Strength"] {
        assert!(
            texts.iter().any(|text| text.contains(required)),
            "missing visible setting {required}: {texts:?}"
        );
    }
    assert_eq!(app.viewport.paths.draft.as_ref(), app.document.path(id));
    app.dispatch(Command::Action(Action::Undo), &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(app.viewport.paths.active);
    assert!(app.ui.action_enabled(&Action::Redo, &app.document));
    render(&mut app, &mut capture, Some("undone"));
    app.viewport.destroy_renderer();
}

use super::*;
use crate::{
    document::ProjectionSource,
    sculpt::{Geometry, Mesh, ProjectionOptions},
};

fn finish_projection(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while app.ui.projection.busy {
        app.poll_projection();
        assert!(std::time::Instant::now() < deadline, "projection timed out");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
fn setup_projection() -> (App, egui::Context) {
    let mut app = App::default();
    app.document
        .replace_geometry(Geometry::new(Mesh::sphere()))
        .unwrap();
    let ctx = egui::Context::default();
    app.dispatch(Command::KeepProjectionReference, &ctx);
    let mut geometry = app.document.geometry_snapshot();
    for p in &mut geometry.mesh.vertices {
        *p = *p * 0.9;
    }
    app.document.replace_geometry(geometry).unwrap();
    app.document.mark_saved("projection.clay");
    (app, ctx)
}
#[test]
fn projection_preview_is_private_apply_is_one_undo_and_reference_persists() {
    let (mut app, ctx) = setup_projection();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.dispatch(Command::OpenProjection, &ctx);
    assert_eq!(
        app.ui.projection.source,
        Some(ProjectionSource::Submodel(1))
    );
    app.ui.projection.options.max_distance = 0.2;
    app.dispatch(Command::PreviewProjection, &ctx);
    app.dispatch(Command::SelectSubmodel(1), &ctx);
    app.dispatch(Command::Remesh, &ctx);
    finish_projection(&mut app);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert!(!app.document.is_dirty());
    assert!(
        app.ui.projection.report.as_ref().unwrap().moved() > 0,
        "{}",
        app.ui.status
    );
    let preview = app.projection.draft.as_ref().unwrap().mesh.clone();
    app.dispatch(Command::ApplyProjection, &ctx);
    assert_eq!(app.document.mesh(), preview.as_ref());
    assert_eq!(app.document.geometry_snapshot().submodels, before.submodels);
    assert_eq!(app.document.undo_label(), Some("Project details"));
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    let after = app.document.geometry_snapshot();
    let encoded = serde_json::to_string(&app.document.project()).unwrap();
    let mut loaded = Document::default();
    loaded
        .replace_project(serde_json::from_str(&encoded).unwrap(), "saved.clay")
        .unwrap();
    assert_eq!(loaded.geometry_snapshot(), after);
    assert!(!loaded.model(1).unwrap().metadata.visible);
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), before);
    app.document.redo();
    assert_eq!(app.document.geometry_snapshot(), after);
}
#[test]
fn projection_cancellation_noop_and_stale_draft_preserve_history_and_dirty_state() {
    let (mut app, ctx) = setup_projection();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.dispatch(Command::OpenProjection, &ctx);
    app.ui.projection.options.max_distance = 0.2;
    app.dispatch(Command::PreviewProjection, &ctx);
    app.dispatch(Command::CancelProjection, &ctx);
    app.poll_projection();
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert!(!app.document.is_dirty());
    app.dispatch(Command::OpenProjection, &ctx);
    app.ui.projection.options.strength = 0.;
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    app.dispatch(Command::ApplyProjection, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert!(!app.document.is_dirty());
    app.dispatch(Command::OpenProjection, &ctx);
    app.ui.projection.options.strength = 1.;
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    app.dispatch(Command::CancelProjection, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::OpenProjection, &ctx);
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    app.document.duplicate_model(false).unwrap(); // Defensive guard against external scene changes.
    let changed = app.document.geometry_snapshot();
    app.dispatch(Command::ApplyProjection, &ctx);
    assert_eq!(app.document.geometry_snapshot(), changed);
    assert!(app.ui.status.contains("scene changed"), "{}", app.ui.status);
}
#[test]
fn projection_preserved_reference_survives_remesh_and_lower_level_sources_keep_stack() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::KeepProjectionReference, &ctx);
    let source = app.document.model(1).unwrap().mesh.clone();
    app.dispatch(Command::SetDynamicMesh(true), &ctx);
    app.dispatch(Command::SetRemeshSettings(24, false), &ctx);
    app.dispatch(Command::Remesh, &ctx);
    finish_mesh_job(&mut app);
    app.dispatch(Command::OpenProjection, &ctx);
    app.ui.projection.options.max_distance = 0.2;
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    assert!(app.ui.projection.report.as_ref().unwrap().moved() > 0);
    app.dispatch(Command::ApplyProjection, &ctx);
    assert_eq!(app.document.model(1).unwrap().mesh, &source);
    app.document.mesh().validate_closed().unwrap();

    let mut geometry = Geometry::new(Mesh::sphere());
    geometry.subdivide().unwrap();
    app.document.replace_geometry(geometry).unwrap();
    let low = app
        .document
        .projection_source(ProjectionSource::Level(1))
        .unwrap();
    let (result, _) = app
        .document
        .mesh()
        .project_details(&low, ProjectionOptions::default(), &|| false)
        .unwrap();
    app.document
        .apply_projection(result, app.document.display_revision(), None)
        .unwrap();
    assert_eq!(app.document.mesh_settings().levels(), 2);
    app.document.set_mesh_level(1).unwrap();
    app.document.set_mesh_level(2).unwrap();
    app.document
        .mesh_settings()
        .validate(app.document.mesh())
        .unwrap();
    assert!(
        app.document
            .projection_source(ProjectionSource::Level(2))
            .is_err()
    );
    assert!(
        app.document
            .projection_source(ProjectionSource::Level(7))
            .is_err()
    );
}
#[test]
fn projection_navigation_cannot_sculpt_and_escape_discards_preview() {
    let (mut app, ctx) = setup_projection();
    app.dispatch(Command::OpenProjection, &ctx);
    app.ui.projection.options.max_distance = 0.2;
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    let before = app.document.geometry_snapshot();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let pos = rect.center();
    let yaw = app.camera.yaw;
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(pos), primary_event(pos, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(pos + Vec2::new(40., 10.))],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(pos + Vec2::new(40., 10.), false)],
    );
    assert_ne!(app.camera.yaw, yaw);
    assert_eq!(app.document.geometry_snapshot(), before);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(!app.ui.projection.open);
    assert!(app.projection.draft.is_none());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.is_dirty());
}

#[test]
fn projection_rejects_incompatible_levels_and_noop_keeps_redo() {
    let mut document = Document::default();
    document.preserve_projection_reference().unwrap();
    document.undo();
    let history = document.history_counts();
    document.mark_saved("unchanged.clay");
    document
        .apply_projection(document.mesh().clone(), document.display_revision(), None)
        .unwrap();
    assert_eq!(document.history_counts(), history);
    assert!(document.can_redo());
    assert!(!document.is_dirty());
    let mut geometry = document.geometry_snapshot();
    geometry.subdivide().unwrap();
    geometry.settings.lower[0].triangles.pop();
    // Invalid correspondence is rejected before it reaches persistent state.
    assert!(document.replace_geometry(geometry).is_err());
    assert_eq!(document.history_counts(), history);
}

#[test]
fn relaxation_preview_apply_repeat_reference_undo_and_persistence() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document
        .replace_geometry(Geometry::new(crate::sculpt::relaxation_fixture()))
        .unwrap();
    app.document.duplicate_model(false).unwrap();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.document.mark_saved("relaxation.clay");
    app.dispatch(Command::Action(Action::RelaxMesh), &ctx);
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    assert!(
        app.ui.projection.report.as_ref().unwrap().moved() > 0,
        "{}",
        app.ui.status
    );
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::CancelProjection, &ctx);
    assert!(!app.document.is_dirty());
    app.dispatch(Command::OpenRelaxation, &ctx);
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    let reference = app.projection.reference.as_ref().unwrap().1.clone();
    let draft = app.projection.draft.as_ref().unwrap().mesh.clone();
    app.dispatch(Command::ApplyProjection, &ctx);
    assert_eq!(app.document.mesh(), draft.as_ref());
    assert_eq!(app.document.undo_label(), Some("Relax mesh"));
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    assert_eq!(app.document.geometry_snapshot().submodels, before.submodels);
    app.dispatch(Command::OpenRelaxation, &ctx);
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    assert!(std::sync::Arc::ptr_eq(
        &reference,
        &app.projection.reference.as_ref().unwrap().1
    ));
    app.dispatch(Command::ApplyProjection, &ctx);
    let after = app.document.geometry_snapshot();
    let encoded = serde_json::to_string(&app.document.project()).unwrap();
    let mut loaded = Document::default();
    loaded
        .replace_project(serde_json::from_str(&encoded).unwrap(), "saved.clay")
        .unwrap();
    assert_eq!(loaded.geometry_snapshot(), after);
    app.dispatch(Command::Undo, &ctx);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
    app.dispatch(Command::OpenRelaxation, &ctx);
    app.dispatch(Command::PreviewProjection, &ctx);
    finish_projection(&mut app);
    assert!(!std::sync::Arc::ptr_eq(
        &reference,
        &app.projection.reference.as_ref().unwrap().1
    ));
    app.document.duplicate_model(false).unwrap();
    let changed = app.document.geometry_snapshot();
    app.dispatch(Command::ApplyProjection, &ctx);
    assert_eq!(app.document.geometry_snapshot(), changed);
    assert!(app.ui.status.contains("scene changed"));
}

#[test]
fn relaxation_retains_subdivision_stack_and_noop_preserves_redo() {
    let mut p = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
    p.divisions = [4; 3];
    let mut mesh = p.mesh().unwrap();
    for v in &mut mesh.vertices {
        v.0 += 0.2 * (v.0 * std::f32::consts::PI).sin();
    }
    let mut geometry = Geometry::new(mesh);
    geometry.subdivide().unwrap();
    let mut document = Document::default();
    document.replace_geometry(geometry).unwrap();
    let before = document.geometry_snapshot();
    let (relaxed, report) = document
        .mesh()
        .relax_mesh(document.mesh(), Default::default(), &|| false)
        .unwrap();
    assert!(report.moved > 0);
    document
        .apply_relaxation(relaxed.clone(), document.display_revision(), None)
        .unwrap();
    assert_eq!(document.mesh_settings().lower, before.settings.lower);
    assert_eq!(document.mesh_settings().levels(), 2);
    document.set_mesh_level(1).unwrap();
    document.set_mesh_level(2).unwrap();
    document.mesh_settings().validate(document.mesh()).unwrap();
    assert_eq!(document.mesh(), &relaxed);
    document.undo();
    document.undo();
    document.undo();
    assert_eq!(document.geometry_snapshot(), before);
    let history = document.history_counts();
    document
        .apply_relaxation(document.mesh().clone(), document.display_revision(), None)
        .unwrap();
    assert_eq!(document.history_counts(), history);
    assert!(document.can_redo());
}

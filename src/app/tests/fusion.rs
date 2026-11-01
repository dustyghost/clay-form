use super::*;
use crate::sculpt::{Geometry, Submodel, fusion::tests::tube};

pub(super) fn setup_fusion() -> (App, egui::Context) {
    let mut app = App::default();
    let mut geometry = Geometry::new(tube(8, -1.5, -0.3, 0.45));
    geometry.name = "Lower tube".into();
    geometry.submodels.push(Submodel {
        name: "Upper tube".into(),
        metadata: Default::default(),
        mesh: tube(12, 0.3, 1.5, 0.65),
        mesh_settings: Default::default(),
    });
    app.document.replace_geometry(geometry).unwrap();
    app.document.mark_saved("join.clay");
    (app, egui::Context::default())
}
pub(super) fn finish_fusion(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while app.ui.fusion.busy {
        app.poll_fusion();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
#[test]
fn fusion_preview_is_private_and_apply_restores_both_sources_in_one_undo() {
    let (mut app, ctx) = setup_fusion();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.dispatch(Command::OpenFusion, &ctx);
    finish_fusion(&mut app);
    assert_eq!(app.ui.fusion.loops[0].len(), 2, "{}", app.ui.fusion.notice);
    app.ui.fusion.selected = [Some(1), Some(0)];
    app.dispatch(Command::PreviewFusion, &ctx);
    finish_fusion(&mut app);
    assert!(app.ui.fusion.ready, "{}", app.ui.fusion.notice);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert!(!app.document.is_dirty());
    app.dispatch(Command::SelectSubmodel(1), &ctx);
    assert_eq!(app.document.active_model_index(), 0);
    app.dispatch(Command::ApplyFusion, &ctx);
    assert_eq!(app.document.model_count(), 3);
    assert!(app.document.active_visible());
    for i in 0..2 {
        assert!(!app.document.model(i).unwrap().metadata.visible);
    }
    assert_eq!(app.document.model(0).unwrap().mesh, &before.mesh);
    assert_eq!(
        app.document.model(1).unwrap().mesh,
        &before.submodels[0].mesh
    );
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    let after = app.document.geometry_snapshot();
    let encoded = serde_json::to_string(&app.document.project()).unwrap();
    let mut restored = Document::default();
    restored
        .replace_project(serde_json::from_str(&encoded).unwrap(), "joined.clay")
        .unwrap();
    assert_eq!(restored.geometry_snapshot(), after);
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), before);
    app.document.redo();
    assert_eq!(app.document.geometry_snapshot(), after);
}
#[test]
fn fusion_cancel_failure_and_stale_preview_are_atomic() {
    let (mut app, ctx) = setup_fusion();
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::OpenFusion, &ctx);
    finish_fusion(&mut app);
    app.ui.fusion.selected = [Some(1), Some(0)];
    app.dispatch(Command::PreviewFusion, &ctx);
    app.dispatch(Command::CancelFusion, &ctx);
    finish_fusion(&mut app);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::OpenFusion, &ctx);
    finish_fusion(&mut app);
    app.ui.fusion.selected = [Some(1), Some(0)];
    app.dispatch(Command::PreviewFusion, &ctx);
    finish_fusion(&mut app);
    assert!(app.ui.fusion.ready, "{}", app.ui.fusion.notice);
    app.document.select_submodel(1).unwrap();
    let changed = app.document.geometry_snapshot();
    app.dispatch(Command::ApplyFusion, &ctx);
    assert_eq!(app.document.geometry_snapshot(), changed);
    assert!(app.ui.fusion.notice.contains("changed"));
}

#[test]
fn fusion_settings_change_blocks_stale_apply_and_new_cancels_the_preview() {
    let (mut app, ctx) = setup_fusion();
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::OpenFusion, &ctx);
    finish_fusion(&mut app);
    app.ui.fusion.selected = [Some(1), Some(0)];
    app.dispatch(Command::PreviewFusion, &ctx);
    finish_fusion(&mut app);
    app.ui.fusion.options.segments += 1;
    app.dispatch(Command::ApplyFusion, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.ui.fusion.ready);
    assert!(app.ui.fusion.notice.contains("settings changed"));
    app.dispatch(Command::Request(crate::command::PendingAction::New), &ctx);
    assert!(!app.ui.fusion.open);
    assert!(app.fusion.display.is_none());
}

#[test]
fn fusion_can_join_within_one_model_and_retains_the_source_level_stack() {
    let mut geometry = Geometry::new(
        crate::sculpt::fusion::merge(&tube(8, -1.5, -0.3, 0.45), &tube(12, 0.3, 1.5, 0.65))
            .unwrap(),
    );
    geometry.subdivide().unwrap();
    geometry.set_level(1).unwrap();
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.replace_geometry(geometry.clone()).unwrap();
    app.dispatch(Command::OpenFusion, &ctx);
    finish_fusion(&mut app);
    assert_eq!(app.ui.fusion.sources, [0, 0]);
    app.ui.fusion.selected = [Some(1), Some(2)];
    app.dispatch(Command::PreviewFusion, &ctx);
    finish_fusion(&mut app);
    assert!(app.ui.fusion.ready, "{}", app.ui.fusion.notice);
    app.dispatch(Command::ApplyFusion, &ctx);
    assert_eq!(app.document.model_count(), 2);
    let original = app.document.model(0).unwrap();
    assert_eq!(original.mesh, &geometry.mesh);
    assert_eq!(original.settings, &geometry.settings);
    assert!(!original.metadata.visible);
    assert_eq!(app.document.mesh_settings().levels(), 1);
    assert_eq!(
        crate::sculpt::fusion::boundaries(app.document.mesh())
            .unwrap()
            .len(),
        2
    );
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), geometry);
}

#[test]
fn fusion_closed_bridge_previews_and_commits_the_cull_in_one_undo() {
    use crate::sculpt::{
        Vec3,
        fusion::{
            self,
            tests::{cap_tube_end, small_cube},
        },
    };
    let (mut app, ctx) = setup_fusion();
    let mut scene = app.document.geometry_snapshot();
    cap_tube_end(&mut scene.mesh, false);
    cap_tube_end(&mut scene.submodels[0].mesh, true);
    let interior = small_cube(Vec3::default(), 0.07);
    scene.mesh = fusion::merge(&scene.mesh, &interior).unwrap();
    app.document.replace_geometry(scene.clone()).unwrap();
    app.dispatch(Command::OpenFusion, &ctx);
    finish_fusion(&mut app);
    app.ui.fusion.selected = [Some(0), Some(0)];
    app.dispatch(Command::PreviewFusion, &ctx);
    finish_fusion(&mut app);
    assert!(app.ui.fusion.ready, "{}", app.ui.fusion.notice);
    assert!(app.ui.fusion.notice.contains("Culled 6 enclosed polygons"));
    assert!(app.ui.fusion.has_removed_faces);
    assert!(
        app.fusion
            .removed
            .iter()
            .flatten()
            .all(|v| interior.vertices.contains(v))
    );
    assert_eq!(app.document.geometry_snapshot(), scene);
    app.dispatch(Command::ApplyFusion, &ctx);
    app.document.mesh().validate_closed().unwrap();
    assert!(
        app.document
            .mesh()
            .vertices
            .iter()
            .all(|v| !interior.vertices.contains(v))
    );
    let joined = app.document.geometry_snapshot();
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), scene);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), joined);
}

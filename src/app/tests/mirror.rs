use super::*;
use crate::sculpt::{Axis, Geometry, Mesh, Project, Symmetry, Vec3};

#[test]
fn mirror_actions_use_center_commit_once_and_round_trip_with_undo() {
    for axis in Axis::ALL {
        for positive in [true, false] {
            let mut app = App::default();
            let ctx = egui::Context::default();
            app.document
                .set_symmetry(Symmetry {
                    center: axis.vector() * 0.13,
                    ..Default::default()
                })
                .unwrap();
            // Keep a real subdivision stack to verify that Undo restores it.
            let mut geometry = Geometry::new(Mesh::starting_sphere());
            geometry.subdivide().unwrap();
            app.document
                .replace_geometry_named(geometry, "Fixture")
                .unwrap();
            let before = app.document.geometry_snapshot();
            let count = app.document.history_counts().0;
            let action = Action::Mirror(axis, positive);
            assert!(
                app.ui
                    .preferences
                    .bindings
                    .iter()
                    .find(|(a, _)| *a == action)
                    .unwrap()
                    .1
                    .is_none()
            );
            app.dispatch(Command::Action(action), &ctx);
            assert!(app.mesh_job.is_some());
            app.dispatch(Command::Undo, &ctx); // Busy: cannot change the snapshot.
            assert_eq!(app.document.geometry_snapshot(), before);
            finish_mesh_job(&mut app);
            assert_eq!(
                app.document.history_counts(),
                (count + 1, 0),
                "{}",
                app.ui.status
            );
            assert_eq!(app.document.undo_label(), Some("Mirror one side"));
            assert_eq!(app.document.mesh_settings().levels(), 1);
            app.document.mesh().validate_closed().unwrap();
            let mesh = app.document.mesh();
            let min = mesh
                .vertices
                .iter()
                .map(|p| p.dot(axis.vector()))
                .fold(f32::INFINITY, f32::min);
            let max = mesh
                .vertices
                .iter()
                .map(|p| p.dot(axis.vector()))
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((min + max - 0.26).abs() < 1e-5);
            assert!(!app.document.symmetry().enabled);
            let after = app.document.geometry_snapshot();
            let project: Project =
                serde_json::from_slice(&serde_json::to_vec(&app.document.project()).unwrap())
                    .unwrap();
            let mut restored = crate::document::Document::default();
            restored.replace_project(project, "mirror.clay").unwrap();
            assert_eq!(restored.geometry_snapshot(), after);
            app.dispatch(Command::Undo, &ctx);
            assert_eq!(app.document.geometry_snapshot(), before);
            app.dispatch(Command::Redo, &ctx);
            assert_eq!(app.document.geometry_snapshot(), after);
        }
    }
}

#[test]
fn mirror_cancel_failure_and_modal_guards_preserve_document() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::Mirror(Axis::X, true), &ctx);
    app.dispatch(Command::CancelMeshJob, &ctx);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), (0, 0));
    for modal in 0..4 {
        app.ui.settings_open = modal == 0;
        app.ui.recovery_open = modal == 1;
        app.ui.layout_open = modal == 2;
        app.ui.pending = (modal == 3).then_some(PendingAction::New);
        assert!(
            !app.ui
                .action_enabled(&Action::Mirror(Axis::X, true), &app.document)
        );
        app.dispatch(Command::Mirror(Axis::X, true), &ctx);
        assert!(app.mesh_job.is_none());
        assert_eq!(app.document.geometry_snapshot(), before);
    }
    app.ui.pending = None;
    app.document
        .set_symmetry(Symmetry {
            center: Vec3(10., 0., 0.),
            ..Default::default()
        })
        .unwrap();
    let count = app.document.history_counts();
    app.dispatch(Command::Mirror(Axis::X, true), &ctx);
    finish_mesh_job(&mut app);
    assert!(app.ui.status.contains("no surface"), "{}", app.ui.status);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), count);
}

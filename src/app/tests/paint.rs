use super::*;
use crate::sculpt::{Geometry, Kind};

#[test]
fn material_brush_menu_old_project_pointer_release_escape_and_undo() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut project = app.document.project();
    project.brushes.retain(|b| b.kind != Kind::Paint);
    app.document.replace_project(project, "old.clay").unwrap();
    app.document
        .replace_geometry(Geometry::new(crate::sculpt::Mesh::sphere()))
        .unwrap();
    app.dispatch(Command::Action(Action::NewBrush("Paint".into())), &ctx);
    assert_eq!(app.document.brushes()[app.ui.selected].kind, Kind::Paint);
    let mut project = app.document.project();
    project.mesh_settings.dynamic = true;
    project.mesh_settings.auto_remesh = true;
    app.document
        .replace_project(project, "dynamic.clay")
        .unwrap();
    app.camera.yaw = 0.;
    app.camera.pitch = 0.;
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let start = rect.center() + Vec2::new(70., 0.);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let draw = |app: &mut App| {
        navigation_frame(
            app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        for i in 1..=4 {
            navigation_frame(
                app,
                &ctx,
                vec![egui::Event::PointerMoved(
                    start + Vec2::new(0., i as f32 * 12.),
                )],
            );
        }
    };
    draw(&mut app);
    assert_ne!(app.document.geometry_snapshot(), before);
    // A temporary Smooth modifier on release must not remesh the Paint result.
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::SHIFT,
        vec![primary_event(start, false)],
    );
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    assert!(
        app.mesh_job.is_none(),
        "Paint must not launch automatic voxel remeshing"
    );
    let after = app.document.geometry_snapshot();
    assert_eq!(after.mesh.triangles, before.mesh.triangles);
    assert_eq!(after.mesh.vertices, before.mesh.vertices);
    assert!(!after.mesh.materials.is_empty());
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    let history = app.document.history_counts();
    draw(&mut app);
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
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
}

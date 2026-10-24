use super::*;
use crate::sculpt::{Geometry, MeshSettings, Primitive, PrimitiveKind, Submodel, Vec3};
struct ObjFile(std::path::PathBuf);
impl ObjFile {
    fn new(text: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "clay-import-{}-{}.obj",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, text).unwrap();
        Self(path)
    }
    fn path(&self) -> String {
        self.0.to_string_lossy().into()
    }
}
impl Drop for ObjFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
const PLANE: &str = "v 2 3 4\nv 4 3 4\nv 4 5 4\nv 2 5 4\ng Imported_surface\nf 1 2 3 4\n";

#[test]
fn import_action_preserves_project_and_adds_one_undoable_submodel() {
    let file = ObjFile::new(PLANE);
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.document.initialize_primitive().unwrap();
    app.document.duplicate_model(false).unwrap();
    app.document.select_submodel(1).unwrap();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.document.mark_saved("original.clay");
    app.ui.path = "original.clay".into();
    app.ui.obj_path = file.path();
    app.execute_action(Action::ImportObj, &ctx);
    assert!(app.ui.file_picker.is_some());
    assert!(app.mesh_job.is_none());
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert!(app.mesh_job.is_some() && app.ui.mesh_busy);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(
        app.ui
            .action_enabled(&Action::CancelMeshOperation, &app.document)
    );
    assert!(!app.ui.action_enabled(&Action::ImportObj, &app.document));
    // Brush edits are independent of the worker-owned scene snapshot.
    let mut brush = app.document.brushes()[0].clone();
    brush.radius = 47.;
    app.dispatch(Command::UpdateBrush(0, brush), &ctx);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.model_count(), 3);
    assert_eq!(app.document.active_model_index(), 2);
    assert_eq!(app.document.mesh().vertices[0], Vec3(2., 3., 4.));
    assert!(app.document.primitive().is_none());
    assert_eq!(app.document.mesh_settings(), &MeshSettings::default());
    assert_eq!(app.document.brushes()[0].radius, 47.);
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    assert_eq!(app.document.undo_label(), Some("Import OBJ"));
    assert!(app.document.is_dirty());
    assert_eq!(app.document.current_path(), "original.clay");
    assert_eq!(app.ui.path, "original.clay");
    assert_eq!(app.ui.obj_path, file.path());
    assert!(app.document.model(1).unwrap().settings.primitive.is_some());
    let after = app.document.geometry_snapshot();
    let project = app.document.project();
    let roundtrip: crate::sculpt::Project =
        serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
    roundtrip.validate().unwrap();
    assert!(roundtrip == project);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
}
#[test]
fn failed_cancelled_and_interrupted_imports_leave_scene_and_history_intact() {
    let valid = ObjFile::new(PLANE);
    let invalid = ObjFile::new(&format!("{PLANE}f 1 2 99\n"));
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.document.duplicate_model(false).unwrap();
    app.document.undo();
    app.document.mark_saved("original.clay");
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.ui.obj_path = invalid.path();
    app.dispatch(Command::Action(Action::ImportObj), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    finish_mesh_job(&mut app);
    assert!(app.ui.status.contains("line 7"), "{}", app.ui.status);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert!(!app.document.is_dirty());
    app.ui.obj_path = valid.path();
    app.dispatch(Command::Action(Action::ImportObj), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    app.dispatch(Command::CancelMeshJob, &ctx);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::Action(Action::ImportObj), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    app.dispatch(Command::Request(PendingAction::New), &ctx);
    finish_mesh_job(&mut app);
    assert!(app.mesh_job.is_none());
    assert_eq!(app.document.model_count(), 1);
    assert_eq!(app.document.current_path(), "untitled.clay");
    app.poll_mesh_job();
    assert_eq!(app.document.model_count(), 1);
}
#[test]
fn scene_limit_and_name_collisions_are_handled_without_partial_edits() {
    let mut p = Primitive::for_kind(PrimitiveKind::Cube);
    p.divisions = [1; 3];
    let mesh = p.mesh().unwrap();
    let mut geometry = Geometry::new(mesh.clone());
    geometry.import_mesh(mesh.clone(), "Model").unwrap();
    assert_eq!(geometry.name, "Model (2)");
    geometry.import_mesh(mesh.clone(), "Model").unwrap();
    assert_eq!(geometry.name, "Model (3)");
    while geometry.submodels.len() < 63 {
        geometry.submodels.push(Submodel {
            name: "Existing".into(),
            mesh: mesh.clone(),
            metadata: Default::default(),
            mesh_settings: Default::default(),
        });
    }
    let before = geometry.clone();
    assert!(geometry.import_mesh(mesh, "Another").is_err());
    assert_eq!(geometry, before);
    let mut app = App::default();
    app.document.replace_geometry(geometry).unwrap();
    assert!(!app.ui.action_enabled(&Action::ImportObj, &app.document));
    assert!(
        app.start_mesh_job(Operation::ImportObj("missing.obj".into()))
            .unwrap_err()
            .contains("64")
    );
    assert!(app.mesh_job.is_none());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn obj_import_offscreen_preview() {
    use crate::{
        preferences::{Dock, Tool},
        viewport::UiCapture,
    };
    let mut capture = UiCapture::new([1200, 850]);
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    let file = ObjFile::new(
        "v -0.7 -0.7 0.8\nv 0.7 -0.7 0.8\nv 0.7 0.7 0.8\nv -0.7 0.7 0.8\nv 0 0 1.8\ng Base\nf 4 3 2 1\ng Sides\nf 1 2 5\nf 2 3 5\nf 3 4 5\nf 4 1 5\n",
    );
    let mut app = App::default();
    app.viewport.initialize_renderer(&capture.state).unwrap();
    for panel in &mut app.ui.preferences.panels {
        panel.dock = if panel.tool == Tool::Submodels {
            Dock::Right
        } else {
            Dock::Hidden
        };
    }
    app.ui.obj_path = file.path();
    app.dispatch(Command::Action(Action::ImportObj), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.model_count(), 2);
    assert_eq!(app.document.mesh().quads.len(), 1);
    app.camera.yaw = 0.4;
    app.camera.pitch = 0.3;
    app.ui.wire = true;
    let before = app.document.geometry_snapshot();
    for step in 0..3 {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200., 850.),
                )),
                time: Some(step as f64),
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
        if step == 2 {
            capture.save(
                std::env::var("CLAY_OBJ_PREVIEW")
                    .unwrap_or_else(|_| "/tmp/clay-obj-import.png".into()),
                &rgba,
            );
        }
    }
    assert_eq!(app.document.geometry_snapshot(), before);
    app.viewport.destroy_renderer();
}

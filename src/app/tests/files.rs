use super::*;

struct Folder(std::path::PathBuf);
impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "clay-picker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn project_file_dialog_first_save_save_as_and_overwrite_are_explicit() {
    let folder = Folder::new();
    std::fs::create_dir(folder.0.join("models")).unwrap();
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.path = folder.0.join("first").to_string_lossy().into();
    app.document.invert_mask().unwrap();
    app.dispatch(Command::Action(Action::Save), &ctx);
    let picker = app.ui.file_picker.as_ref().unwrap();
    assert!(picker.save);
    assert_eq!(picker.entries[0].name, "models");
    assert!(!folder.0.join("first.clay").exists());
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    let first = folder.0.join("first.clay");
    assert!(first.is_file());
    assert!(app.document.has_saved_path());
    assert!(app.ui.file_picker.is_none());
    assert_eq!(app.ui.path, first.to_string_lossy());
    app.document.invert_mask().unwrap();
    app.dispatch(Command::Save, &ctx);
    assert!(
        app.ui.file_picker.is_none(),
        "Subsequent Save writes the established destination"
    );
    let saved = std::fs::read(&first).unwrap();
    app.document.invert_mask().unwrap();
    app.dispatch(Command::Action(Action::SaveAs), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert_eq!(
        app.ui.file_picker.as_ref().unwrap().overwrite.as_ref(),
        Some(&first)
    );
    assert_eq!(std::fs::read(&first).unwrap(), saved);
    app.dispatch(Command::CancelProjectFile, &ctx);
    assert!(app.document.is_dirty());
    assert_eq!(std::fs::read(&first).unwrap(), saved);
    app.dispatch(Command::Action(Action::SaveAs), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    app.dispatch(Command::AcceptProjectFile(true), &ctx);
    assert_ne!(std::fs::read(&first).unwrap(), saved);
    assert!(!app.document.is_dirty());
}

#[test]
fn project_file_dialog_open_preserves_unsaved_work_until_a_decision() {
    let folder = Folder::new();
    let target = folder.0.join("target.clay");
    persistence::save_project(&target.to_string_lossy(), &Document::default().project()).unwrap();
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.path = target.to_string_lossy().into();
    app.document.invert_mask().unwrap();
    let original = app.document.geometry_snapshot();
    app.dispatch(Command::Action(Action::Open), &ctx);
    assert!(!app.ui.file_picker.as_ref().unwrap().save);
    assert!(app.ui.pending.is_none());
    app.dispatch(Command::CancelProjectFile, &ctx);
    assert_eq!(app.document.geometry_snapshot(), original);
    app.dispatch(Command::Action(Action::Open), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert!(app.ui.pending.is_some());
    assert!(app.ui.file_picker.is_none());
    assert_eq!(app.document.geometry_snapshot(), original);
    app.ui.pending_save_path = folder.0.join("my-work.clay").to_string_lossy().into();
    app.dispatch(Command::SaveAndContinue, &ctx);
    assert!(app.ui.file_picker.as_ref().unwrap().resume_pending);
    app.ui.file_picker.as_mut().unwrap().filename = "target.clay".into();
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert!(
        app.ui
            .file_picker
            .as_ref()
            .unwrap()
            .error
            .contains("avoid overwriting")
    );
    app.ui.file_picker.as_mut().unwrap().filename = "my-work.clay".into();
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert!(app.ui.pending.is_none());
    assert!(app.ui.file_picker.is_none());
    assert_eq!(app.document.current_path(), target.to_string_lossy());
    assert_eq!(app.ui.path, target.to_string_lossy());
    assert!(app.document.mesh().mask.iter().all(|&m| m == 0.));
    let saved =
        persistence::load_project(&folder.0.join("my-work.clay").to_string_lossy()).unwrap();
    assert_eq!(saved.mesh, original.mesh);
}

#[test]
fn project_file_dialog_errors_and_modal_input_do_not_change_the_document() {
    let folder = Folder::new();
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.path = folder.0.join("new.clay").to_string_lossy().into();
    app.document.invert_mask().unwrap();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.dispatch(Command::Save, &ctx);
    app.dispatch(Command::NavigateProjectFiles("missing-folder".into()), &ctx);
    let picker = app.ui.file_picker.as_ref().unwrap();
    assert!(!picker.error.is_empty());
    assert_eq!(picker.directory, folder.0);
    app.dispatch(Command::Undo, &ctx);
    app.dispatch(Command::InvertMask, &ctx);
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(rect.center()),
            primary_event(rect.center(), true),
        ],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(rect.center(), false)]);
    app.ui.file_picker.as_mut().unwrap().filename = "missing-folder/file.clay".into();
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert!(!app.ui.file_picker.as_ref().unwrap().error.is_empty());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert!(app.document.is_dirty());
    assert!(!app.document.has_saved_path());
}

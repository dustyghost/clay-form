use super::*;
use crate::command::AssetKind;

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

#[test]
fn asset_file_dialog_actions_filter_files_and_cancel_without_edits() {
    let folder = Folder::new();
    std::fs::create_dir(folder.0.join("Studies")).unwrap();
    for name in [
        "front study.PNG",
        "side.jpeg",
        "model.OBJ",
        "brush.json",
        "notes.txt",
    ] {
        std::fs::write(folder.0.join(name), b"fixture").unwrap();
    }
    for (action, asset, expected) in [
        (
            Action::AddReference,
            AssetKind::Reference,
            vec!["Studies", "front study.PNG", "side.jpeg"],
        ),
        (
            Action::LoadAlpha,
            AssetKind::Alpha,
            vec!["Studies", "front study.PNG", "side.jpeg"],
        ),
        (
            Action::ImportObj,
            AssetKind::Obj,
            vec!["Studies", "model.OBJ"],
        ),
        (
            Action::LoadPreset,
            AssetKind::Brush,
            vec!["Studies", "brush.json"],
        ),
        (
            Action::ImportLibrary,
            AssetKind::BrushLibrary,
            vec!["Studies", "brush.json"],
        ),
    ] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        // Choosing a file must work before a path has been entered.
        app.ui.asset_path_mut(asset).clear();
        assert!(app.ui.action_enabled(&action, &app.document));
        let before = app.document.project();
        let history = app.document.history_counts();
        app.dispatch(Command::Action(action), &ctx);
        app.dispatch(
            Command::NavigateProjectFiles(folder.0.to_string_lossy().into()),
            &ctx,
        );
        let picker = app.ui.file_picker.as_ref().unwrap();
        assert_eq!(picker.asset, Some(asset));
        assert!(!picker.save);
        assert_eq!(
            picker
                .entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
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
        // egui determines the top modal from the preceding frame.
        let _ = ctx.run(Default::default(), |ctx| {
            assert!(app.ui.project_file_dialog(ctx).is_none());
        });
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                ..Default::default()
            },
            |ctx| {
                let command = app
                    .ui
                    .project_file_dialog(ctx)
                    .expect("Escape cancels the picker");
                app.dispatch(command, ctx);
            },
        );
        assert!(app.ui.file_picker.is_none());
        assert!(app.document.project() == before);
        assert_eq!(app.document.history_counts(), history);
        assert!(!app.document.is_dirty());
        assert!(app.ui.asset_path_mut(asset).is_empty());
    }
}

#[test]
fn asset_file_dialog_reference_errors_stay_open_then_import_and_remember_location() {
    let folder = Folder::new();
    std::fs::create_dir(folder.0.join("Images")).unwrap();
    let directory = folder.0.join("Images");
    std::fs::write(directory.join("broken.png"), b"invalid PNG").unwrap();
    std::fs::write(directory.join("notes.txt"), b"not an image").unwrap();
    image::RgbImage::from_pixel(8, 8, image::Rgb([30, 100, 180]))
        .save_with_format(directory.join("front study.PNG"), image::ImageFormat::Png)
        .unwrap();
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.project();
    app.dispatch(Command::Action(Action::AddReference), &ctx);
    app.dispatch(
        Command::NavigateProjectFiles(folder.0.to_string_lossy().into()),
        &ctx,
    );
    app.ui.file_picker.as_mut().unwrap().filename = "Images".into();
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert_eq!(app.ui.file_picker.as_ref().unwrap().directory, directory);
    for name in ["missing.png", "notes.txt", "broken.png"] {
        app.ui.file_picker.as_mut().unwrap().filename = name.into();
        app.dispatch(Command::AcceptProjectFile(false), &ctx);
        let picker = app.ui.file_picker.as_ref().unwrap();
        assert!(!picker.error.is_empty(), "{name}");
        assert_eq!(picker.filename, name);
        assert!(app.document.project() == before);
        assert!(!app.document.is_dirty());
    }
    app.ui.file_picker.as_mut().unwrap().filename = "front study.PNG".into();
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert!(app.ui.file_picker.is_none());
    assert_eq!(app.document.references().len(), 1);
    assert!(app.ui.arrange_references);
    assert!(app.document.is_dirty());
    app.dispatch(Command::Action(Action::AddReference), &ctx);
    let picker = app.ui.file_picker.as_ref().unwrap();
    assert_eq!(picker.directory, directory);
    assert_eq!(picker.filename, "front study.PNG");
    app.dispatch(Command::CancelProjectFile, &ctx);
    assert_eq!(app.document.references().len(), 1);
}

#[test]
fn asset_file_dialog_imports_brushes_and_libraries_and_preserves_invalid_inputs() {
    let folder = Folder::new();
    let mut brush = crate::sculpt::Brush::new(crate::sculpt::Kind::Clay);
    brush.name = "Chosen brush".into();
    persistence::save_brush(
        &folder.0.join("chosen.brush.json").to_string_lossy(),
        &brush,
    )
    .unwrap();
    persistence::save_brush_library(
        &folder.0.join("chosen.brushes.json").to_string_lossy(),
        &[brush],
    )
    .unwrap();
    std::fs::write(folder.0.join("broken.json"), b"{}").unwrap();
    for (action, asset, filename) in [
        (Action::LoadPreset, AssetKind::Brush, "chosen.brush.json"),
        (
            Action::ImportLibrary,
            AssetKind::BrushLibrary,
            "chosen.brushes.json",
        ),
    ] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        *app.ui.asset_path_mut(asset) = folder.0.join("broken.json").to_string_lossy().into();
        let before = app.document.project();
        app.dispatch(Command::Action(action), &ctx);
        app.dispatch(Command::AcceptProjectFile(false), &ctx);
        assert!(!app.ui.file_picker.as_ref().unwrap().error.is_empty());
        assert!(app.document.project() == before);
        app.ui.file_picker.as_mut().unwrap().filename = filename.into();
        app.dispatch(Command::AcceptProjectFile(false), &ctx);
        assert!(app.ui.file_picker.is_none());
        assert!(
            app.document
                .brushes()
                .iter()
                .any(|b| b.name == "Chosen brush")
        );
        assert!(app.document.is_dirty());
    }
}

#[test]
fn material_export_picker_confirms_overwrite_and_cancellation_keeps_destination() {
    use crate::persistence::material_export::ExportFormat;
    let folder = Folder::new();
    let path = folder.0.join("paint.ply");
    std::fs::write(&path, b"previous export").unwrap();
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.path = folder.0.join("paint.clay").to_string_lossy().into();
    let original = app.document.project();
    app.dispatch(Command::BrowseExport(ExportFormat::Ply), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert!(app.export_job.is_none());
    assert_eq!(
        app.ui.file_picker.as_ref().unwrap().overwrite.as_ref(),
        Some(&path)
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"previous export");
    app.dispatch(Command::AcceptProjectFile(true), &ctx);
    app.dispatch(Command::CancelExport, &ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.export_job.is_some() {
        app.poll_export(&ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(std::fs::read(&path).unwrap(), b"previous export");
    app.dispatch(Command::BrowseExport(ExportFormat::Ply), &ctx);
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    app.dispatch(Command::AcceptProjectFile(true), &ctx);
    while app.export_job.is_some() {
        app.poll_export(&ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(std::fs::read(&path).unwrap().starts_with(b"ply\n"));
    assert!(app.document.project() == original);
    assert!(app.ui.path.ends_with("paint.clay"));
    app.dispatch(Command::BrowseExport(ExportFormat::Blender), &ctx);
    app.ui.file_picker.as_mut().unwrap().filename = "paint.clay".into();
    app.dispatch(Command::AcceptProjectFile(false), &ctx);
    assert!(app.export_job.is_none());
    assert!(
        app.ui
            .file_picker
            .as_ref()
            .unwrap()
            .error
            .contains(".blend")
    );
}

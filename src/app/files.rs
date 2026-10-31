//! File operations and unsaved-change decisions.
use super::*;
use crate::command::AssetKind;

impl App {
    pub(super) fn browse_project(&mut self, save: bool, resume_pending: bool) {
        let initial = if resume_pending {
            self.ui.pending_save_path.clone()
        } else {
            self.ui.path.clone()
        };
        self.browse_files(initial, save, resume_pending, None);
    }
    pub(super) fn browse_asset(&mut self, asset: AssetKind) {
        let initial = self.ui.asset_path_mut(asset).clone();
        self.browse_files(initial, false, false, Some(asset));
    }
    pub(super) fn browse_files(
        &mut self,
        initial: String,
        save: bool,
        resume_pending: bool,
        asset: Option<AssetKind>,
    ) {
        self.cancel_mesh_job();
        self.viewport.cancel(&mut self.document);
        let path = std::path::PathBuf::from(initial);
        let directory = if path.is_dir() {
            path.clone()
        } else {
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf()
        };
        let filename = if path.is_dir() {
            String::new()
        } else {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        self.ui.file_picker = Some(crate::ui::FilePicker {
            asset,
            export: None,
            include_lights: false,
            save,
            resume_pending,
            directory: std::env::current_dir().unwrap_or_else(|_| ".".into()),
            location: String::new(),
            filename,
            entries: Vec::new(),
            error: String::new(),
            overwrite: None,
        });
        self.navigate_project_files(&directory.to_string_lossy());
    }
    pub(super) fn navigate_project_files(&mut self, location: &str) {
        let Some(picker) = &mut self.ui.file_picker else {
            return;
        };
        let path = if location == "~" {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| picker.directory.clone())
        } else {
            picker.directory.join(location)
        };
        let result = (|| -> Result<_, std::io::Error> {
            let directory = path.canonicalize()?;
            let mut entries = Vec::new();
            for entry in std::fs::read_dir(&directory)? {
                let entry = entry?;
                let Ok(metadata) = std::fs::metadata(entry.path()) else {
                    continue;
                };
                if !metadata.is_file() && !metadata.is_dir() {
                    continue;
                }
                if metadata.is_file()
                    && picker
                        .asset
                        .is_some_and(|asset| !asset.accepts(&entry.path()))
                {
                    continue;
                }
                entries.push(crate::ui::FileEntry {
                    path: entry.path(),
                    name: entry.file_name().to_string_lossy().into_owned(),
                    directory: metadata.is_dir(),
                });
            }
            entries.sort_by_key(|e| (!e.directory, e.name.to_lowercase()));
            Ok((directory, entries))
        })();
        match result {
            Ok((directory, entries)) => {
                picker.location = directory.to_string_lossy().into_owned();
                picker.directory = directory;
                picker.entries = entries;
                picker.overwrite = None;
                picker.error.clear();
            }
            Err(error) => picker.error = format!("Cannot open folder {}: {error}", path.display()),
        }
    }
    pub(super) fn accept_project_file(&mut self, overwrite: bool, ctx: &egui::Context) {
        let Some(picker) = &mut self.ui.file_picker else {
            return;
        };
        if picker.filename.trim().is_empty() {
            picker.error = "Enter a file name first".into();
            return;
        }
        let mut path = picker.directory.join(picker.filename.trim());
        if path.is_dir() {
            self.navigate_project_files(&path.to_string_lossy());
            return;
        }
        if let Some(format) = picker.export {
            if path.extension().is_none() {
                path.set_extension(format.extension());
            }
            if path
                .extension()
                .and_then(|e| e.to_str())
                .is_none_or(|e| !e.eq_ignore_ascii_case(format.extension()))
            {
                picker.error = format!("Choose a .{} file name", format.extension());
                return;
            }
            if path.exists() && !(overwrite && picker.overwrite.as_ref() == Some(&path)) {
                picker.overwrite = Some(path);
                return;
            }
            let include_lights = picker.include_lights;
            match self.start_export(path, format, include_lights) {
                Ok(()) => self.ui.file_picker = None,
                Err(error) => self.ui.file_picker.as_mut().unwrap().error = error,
            }
            self.viewport.wait_for_release();
            return;
        }
        if let Some(asset) = picker.asset {
            if !path.is_file() {
                picker.error = "Choose an existing file".into();
                return;
            }
            if !asset.accepts(&path) {
                picker.error = format!("Choose {}", asset.filter_label());
                return;
            }
            // Keep the chooser and its selection on validation errors. Alpha
            // success opens its existing private preview, never applies it here.
            match self.import_asset(asset, path.to_string_lossy().into_owned()) {
                Ok(()) => self.ui.file_picker = None,
                Err(error) => self.ui.file_picker.as_mut().unwrap().error = error,
            }
            self.viewport.wait_for_release();
            return;
        }
        if picker.save && path.extension().is_none() {
            path.set_extension("clay");
        }
        let save = picker.save;
        let resume_pending = picker.resume_pending;
        if save {
            if resume_pending
                && matches!(&self.ui.pending, Some(PendingAction::Open(target)) if persistence::same_path(&path.to_string_lossy(), target))
            {
                picker.error =
                    "Choose a different save path to avoid overwriting the project you are opening"
                        .into();
                return;
            }
            if path.exists() && !(overwrite && picker.overwrite.as_ref() == Some(&path)) {
                picker.overwrite = Some(path);
                return;
            }
            if !self.save_to(&path.to_string_lossy()) {
                let picker = self.ui.file_picker.as_mut().unwrap();
                picker.overwrite = None;
                picker.error = self.ui.status.clone();
                return;
            }
            self.ui.file_picker = None;
            if resume_pending && let Some(action) = self.ui.pending.take() {
                self.execute_pending(action, ctx);
            }
        } else if !path.is_file() {
            picker.error = "Choose an existing project file".into();
        } else {
            self.ui.file_picker = None;
            self.request(
                PendingAction::Open(path.to_string_lossy().into_owned()),
                ctx,
            );
        }
        self.viewport.wait_for_release();
    }
    pub(super) fn import_asset(&mut self, asset: AssetKind, path: String) -> Result<(), String> {
        match asset {
            AssetKind::PaintWorking => self.texture_command(
                crate::app::textures::TextureCommand::Relink(self.ui.uv.channel, path.clone()),
            )?,
            AssetKind::Texture => {
                let texture =
                    persistence::texture::load(&path, crate::sculpt::texture::Channel::Color)?;
                let id = texture.id;
                self.document.save_texture_asset(texture)?;
                self.ui.uv.select_asset(&self.document, id);
                self.ui.status =
                    "Imported image into the library. Assign to a model to paint.".into();
            }
            AssetKind::ReplaceTexture(id) => {
                let old = self
                    .document
                    .texture_asset(id)
                    .ok_or("Image no longer exists")?;
                let mut texture = persistence::texture::load(&path, old.channel)?;
                if old.pixels.precision() == crate::sculpt::texture::Precision::Float32
                    && texture.pixels.precision() != crate::sculpt::texture::Precision::Float32
                {
                    return Err("Use an EXR replacement to preserve float precision. Import integer images as a separate asset.".into());
                }
                texture.id = id;
                texture.name = old.name.clone();
                self.document.save_texture_asset(texture)?;
                self.ui.status =
                    "Replaced image in all shared uses. Undo restores the original pixels.".into();
            }
            AssetKind::Reference => self.add_reference(egui::DroppedFile {
                path: Some(path.clone().into()),
                ..Default::default()
            })?,
            AssetKind::Alpha => {
                let image = persistence::load_alpha(&path)?;
                self.viewport.cancel(&mut self.document);
                let name = std::path::Path::new(&path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.ui.alphas.edit(
                    None,
                    crate::sculpt::alpha::Asset {
                        name,
                        category: String::new(),
                        image,
                    },
                );
            }
            AssetKind::Obj => self.start_mesh_job(Operation::ImportObj(path.clone()))?,
            AssetKind::Brush => {
                let brush = persistence::load_brush(&path)?;
                let (added, updated, index) = self.document.import_brushes(vec![brush])?;
                if let Some(index) = index {
                    self.ui.select_brush(index, &self.document);
                }
                self.ui.status = format!("Imported brush: {added} added, {updated} updated");
            }
            AssetKind::BrushLibrary => {
                let brushes = persistence::load_brush_library(&path)?;
                let (added, updated, _) = self.document.import_brushes(brushes)?;
                self.ui.status = format!("Imported library: {added} added, {updated} updated");
            }
        }
        *self.ui.asset_path_mut(asset) = path;
        Ok(())
    }
    pub(super) fn save_to(&mut self, path: &str) -> bool {
        match persistence::save_project(path, &self.document.project()) {
            Ok(()) => {
                self.document.mark_saved(path);
                self.ui.path = path.into();
                self.retire_recovery();
                self.ui.status = format!("Saved {path}");
                true
            }
            Err(error) => {
                self.ui.status = error;
                false
            }
        }
    }
    pub(super) fn request(&mut self, action: PendingAction, ctx: &egui::Context) {
        self.ui.file_picker = None;
        self.cancel_mesh_job();
        self.viewport.cancel(&mut self.document);
        if self.document.is_dirty() {
            self.ui.pending_save_path = self.document.current_path().into();
            self.ui.pending = Some(action);
        } else {
            self.execute_pending(action, ctx);
        }
    }
    pub(super) fn execute_pending(&mut self, action: PendingAction, ctx: &egui::Context) {
        self.ui.arrange_references = false;
        self.ui.selected_reference = 0;
        match action {
            PendingAction::New => {
                self.document.new_sphere();
                self.ui.path = self.document.current_path().into();
                self.retire_recovery();
                self.ui.status = "New sphere".into();
            }
            PendingAction::Open(path) => {
                let result = persistence::load_project(&path)
                    .and_then(|project| self.document.replace_project(project, &path));
                match result {
                    Ok(()) => {
                        self.ui.path = path.clone();
                        self.retire_recovery();
                        self.ui.selected = 0;
                        self.ui.status = format!("Opened {path}");
                    }
                    Err(error) => self.ui.status = error,
                }
            }
            PendingAction::Recover(entry) => {
                if let Err(error) = self.restore_recovery(entry) {
                    self.ui.status = format!("Could not recover project: {error}");
                    self.recovery_error = Some(error);
                    self.ui.recovery_open = true;
                }
            }
            PendingAction::Quit => {
                self.retire_recovery();
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

//! File chooser presentation; directory reads and writes belong to App.
use super::*;
use crate::command::AssetKind;
use std::path::PathBuf;

impl AssetKind {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Reference => "Add reference image",
            Self::Alpha => "Preview alpha image",
            Self::Obj => "Import OBJ",
            Self::Brush => "Import brush",
            Self::BrushLibrary => "Import brush library",
        }
    }
    pub(crate) fn filter_label(self) -> &'static str {
        match self {
            Self::Reference | Self::Alpha => "PNG / JPEG images (.png, .jpg, .jpeg)",
            Self::Obj => "Wavefront models (.obj)",
            Self::Brush => "Brush presets (.brush.json, .json)",
            Self::BrushLibrary => "Brush libraries (.brushes.json, .json)",
        }
    }
    pub(crate) fn accepts(self, path: &std::path::Path) -> bool {
        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            return false;
        };
        let extensions: &[&str] = match self {
            Self::Reference | Self::Alpha => &["png", "jpg", "jpeg"],
            Self::Obj => &["obj"],
            Self::Brush | Self::BrushLibrary => &["json"],
        };
        extensions
            .iter()
            .any(|ext| extension.eq_ignore_ascii_case(ext))
    }
}

pub(crate) struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
}
pub(crate) struct FilePicker {
    pub asset: Option<AssetKind>,
    pub export: Option<crate::persistence::material_export::ExportFormat>,
    pub save: bool,
    pub resume_pending: bool,
    pub directory: PathBuf,
    pub location: String,
    pub filename: String,
    pub entries: Vec<FileEntry>,
    pub error: String,
    pub overwrite: Option<PathBuf>,
}

impl UiState {
    pub(crate) fn asset_path_mut(&mut self, asset: AssetKind) -> &mut String {
        match asset {
            AssetKind::Reference => &mut self.reference_path,
            AssetKind::Alpha => &mut self.alpha_path,
            AssetKind::Obj => &mut self.obj_path,
            AssetKind::Brush => &mut self.preset_path,
            AssetKind::BrushLibrary => &mut self.library_path,
        }
    }
    pub(crate) fn project_file_dialog(&mut self, ctx: &egui::Context) -> Option<Command> {
        let picker = self.file_picker.as_mut()?;
        let mut command = None;
        let modal = egui::Modal::new(egui::Id::new("project_file_picker")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 60.).clamp(260., 620.));
            ui.heading(if let Some(format) = picker.export {
                format.title()
            } else if let Some(asset) = picker.asset {
                asset.title()
            } else if picker.save {
                "Save project as"
            } else {
                "Open project"
            });
            if let Some(path) = &picker.overwrite {
                ui.label(format!("Replace the existing file {}?", path.display()));
                ui.horizontal(|ui| {
                    if ui.button("Replace file").clicked() {
                        command = Some(Command::AcceptProjectFile(true));
                    }
                    if ui.button("Choose another name").clicked() {
                        picker.overwrite = None;
                    }
                    if ui.button("Cancel").clicked() {
                        command = Some(Command::CancelProjectFile);
                    }
                });
                return;
            }
            ui.horizontal(|ui| {
                if ui.button("Home").clicked() {
                    command = Some(Command::NavigateProjectFiles("~".into()));
                }
                if ui
                    .add_enabled(picker.directory.parent().is_some(), egui::Button::new("Up"))
                    .clicked()
                    && let Some(parent) = picker.directory.parent()
                {
                    command = Some(Command::NavigateProjectFiles(
                        parent.to_string_lossy().into(),
                    ));
                }
                if ui.button("Refresh").clicked() {
                    command = Some(Command::NavigateProjectFiles(
                        picker.directory.to_string_lossy().into(),
                    ));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Folder");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut picker.location)
                        .desired_width(ui.available_width() - 45.),
                );
                if ui.button("Go").clicked()
                    || response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    command = Some(Command::NavigateProjectFiles(picker.location.clone()));
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("project_files")
                .max_height((ctx.content_rect().height() - 300.).clamp(100., 320.))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    if picker.entries.is_empty() {
                        ui.weak(if picker.asset.is_some() {
                            "No matching files in this folder."
                        } else {
                            "This folder is empty."
                        });
                    }
                    for entry in &picker.entries {
                        let label = if entry.directory {
                            format!("{}/", entry.name)
                        } else {
                            entry.name.clone()
                        };
                        let response = ui.selectable_label(
                            !entry.directory && picker.filename == entry.name,
                            label,
                        );
                        if response.clicked() && !entry.directory {
                            picker.filename = entry.name.clone();
                        }
                        if response.double_clicked() {
                            command = Some(if entry.directory {
                                Command::NavigateProjectFiles(entry.path.to_string_lossy().into())
                            } else {
                                Command::AcceptProjectFile(false)
                            });
                        }
                    }
                });
            ui.separator();
            ui.label("File name");
            let filename = ui
                .add(egui::TextEdit::singleline(&mut picker.filename).desired_width(f32::INFINITY));
            let enter = filename.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.weak(if let Some(format) = picker.export {
                format.description()
            } else if let Some(asset) = picker.asset {
                asset.filter_label()
            } else if picker.save {
                "Clay Form project (.clay) · existing files require confirmation"
            } else {
                "Double-click a folder to browse or a project to open it."
            });
            if !picker.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &picker.error);
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !picker.filename.trim().is_empty(),
                        egui::Button::new(if let Some(format) = picker.export {
                            format.title()
                        } else if let Some(asset) = picker.asset {
                            asset.title()
                        } else if picker.save {
                            "Save project"
                        } else {
                            "Open project"
                        }),
                    )
                    .clicked()
                    || enter
                {
                    command = Some(Command::AcceptProjectFile(false));
                }
                if ui.button("Cancel").clicked() {
                    command = Some(Command::CancelProjectFile);
                }
            });
        });
        if modal.should_close() && command.is_none() {
            command = Some(Command::CancelProjectFile);
        }
        command
    }
}

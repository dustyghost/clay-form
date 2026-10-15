//! Project file chooser presentation; directory reads and writes belong to App.
use super::*;
use std::path::PathBuf;

pub(crate) struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
}
pub(crate) struct FilePicker {
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
    pub(crate) fn project_file_dialog(&mut self, ctx: &egui::Context) -> Option<Command> {
        let picker = self.file_picker.as_mut()?;
        let mut command = None;
        let modal = egui::Modal::new(egui::Id::new("project_file_picker")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 60.).clamp(260., 620.));
            ui.heading(if picker.save {
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
                        ui.weak("This folder is empty.");
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
            ui.weak(if picker.save {
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
                        egui::Button::new(if picker.save {
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

//! Recovery scheduling and explicit restore decisions reuse the normal project workflow.
use super::*;
use crate::recovery::{self, Entry, Recovery};

impl App {
    pub(super) fn initialize_recovery(&mut self, directory: std::path::PathBuf) {
        match recovery::entries(&directory) {
            Ok(entries) => {
                self.ui.recovery_open = !entries.is_empty();
                self.recovery_entries = entries;
            }
            Err(error) => self.recovery_error = Some(error),
        }
        match Recovery::new(directory) {
            Ok(recovery) => self.recovery = Some(recovery),
            Err(error) => self.recovery_error = Some(error),
        }
    }
    fn refresh_recoveries(&mut self) {
        if let Some(recovery) = &self.recovery {
            match recovery::entries(&recovery.directory) {
                Ok(entries) => self.recovery_entries = entries,
                Err(error) => self.recovery_error = Some(error),
            }
        }
    }
    pub(super) fn recovery_ready(&self, ctx: &egui::Context) -> bool {
        self.document.is_dirty()
            && !self.document.edit_in_progress()
            && self.mesh_job.is_none()
            && self.viewport.session.gesture.is_none()
            && self.ui.pending.is_none()
            && self.ui.primitive_warning.is_none()
            && !self.closing
            && !ctx.input(|i| i.pointer.any_down())
    }
    pub(super) fn save_recovery(&mut self, ctx: &egui::Context) {
        if !self.recovery_ready(ctx) {
            self.ui.status = "Recovery waits for a completed edit with unsaved changes".into();
            return;
        }
        if let Some(recovery) = &mut self.recovery {
            if recovery.busy {
                return;
            }
            if let Err(error) = recovery.save(
                self.document.project(),
                self.document.current_path().into(),
                self.ui.preferences.recovery.versions,
            ) {
                self.recovery_error = Some(error);
            }
        } else {
            self.recovery_error = Some("Recovery storage is unavailable".into());
        }
    }
    pub(super) fn tick_recovery(&mut self, ctx: &egui::Context) {
        if let Some(recovery) = &mut self.recovery {
            let completions = recovery.poll();
            let refresh = !completions.is_empty();
            for completion in completions {
                match completion.result {
                    Ok(()) if completion.written => {
                        self.recovery_error = None;
                        if self.document.is_dirty() {
                            self.ui.status =
                                "Recovery copy saved • project still has unsaved changes".into();
                        }
                    }
                    Ok(()) => {}
                    Err(error) => {
                        self.ui.status = format!("Recovery failed: {error}");
                        self.recovery_error = Some(error);
                    }
                }
            }
            if refresh {
                self.refresh_recoveries();
            }
        }
        if !self.ui.recovery_open
            && self.recovery_ready(ctx)
            && self
                .recovery
                .as_ref()
                .is_some_and(|r| r.due(&self.ui.preferences.recovery))
        {
            self.save_recovery(ctx);
        }
    }
    pub(super) fn retire_recovery(&mut self) {
        if let Some(recovery) = &mut self.recovery {
            recovery.retire();
        }
    }
    pub(super) fn restore_recovery(&mut self, entry: Entry) -> Result<(), String> {
        let (project, source) = recovery::load(&entry)?;
        self.document.recover_project(project)?;
        if let Some(recovery) = &mut self.recovery {
            recovery.recovered(&entry);
        }
        self.ui.path = self.document.current_path().into();
        self.ui.selected = 0;
        self.ui.recovery_open = false;
        self.recovery_error = None;
        self.ui.status = format!("Recovered work from {source} • save to a new project path");
        Ok(())
    }
    pub(super) fn show_recovery(&mut self, ctx: &egui::Context) {
        if !self.ui.recovery_open {
            return;
        }
        let mut restore = None;
        let mut save_now = false;
        let mut close = false;
        let ready = self.recovery_ready(ctx) && self.recovery.as_ref().is_some_and(|r| !r.busy);
        let response = egui::Modal::new(egui::Id::new("recovery_settings")).show(ctx, |ui| {
            ui.set_width(440.);
            ui.heading("Recovery saves");
            ui.label("Recovery copies protect unsaved work, including new projects. Restoring opens a modified project at recovered.clay; the original project file stays untouched.");
            ui.separator();
            let settings = &mut self.ui.preferences.recovery;
            ui.checkbox(&mut settings.enabled, "Save recovery copies automatically");
            ui.horizontal(|ui| {
                ui.label("Every");
                ui.add(egui::DragValue::new(&mut settings.interval_minutes).range(1..=60).suffix(" minutes"));
                ui.label("Keep");
                ui.add(egui::DragValue::new(&mut settings.versions).range(1..=20).suffix(" versions"));
            });
            ui.weak("Snapshots wait until edits finish. Keep the newest copies across projects; retention applies on the next recovery save. Saving or discarding clears that project’s copies.");
            if let Some(recovery) = &self.recovery {
                ui.label("Recovery folder");
                ui.label(recovery.directory.display().to_string());
                if recovery.busy { ui.horizontal(|ui| { ui.spinner(); ui.label("Writing recovery copy…"); }); }
            }
            if let Some(error) = &self.recovery_error { ui.colored_label(egui::Color32::LIGHT_RED, format!("Recovery failed: {error}")); }
            if ui.add_enabled(ready, egui::Button::new("Save recovery now")).on_hover_text("Save a separate copy of completed, unsaved edits").clicked() { save_now = true; }
            ui.separator();
            ui.strong("Available recovery copies");
            if self.recovery_entries.is_empty() { ui.label("No recovery copies available."); }
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 450.).clamp(80., 260.)).show(ui, |ui| {
                for (i, entry) in self.recovery_entries.iter().enumerate() {
                    ui.push_id(&entry.path, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Recover").clicked() { restore = Some(entry.clone()); }
                            ui.label(format!("{}{}", entry.age_label(), if i == 0 { " · newest" } else { "" })).on_hover_text(entry.path.display().to_string());
                        });
                    });
                }
            });
            ui.separator();
            if ui.button("Continue without restoring").on_hover_text("Keep available copies for later; open Recovery saves from Settings").clicked() { close = true; }
        });
        if close || response.should_close() {
            self.ui.recovery_open = false;
        }
        if save_now {
            self.save_recovery(ctx);
        }
        if let Some(entry) = restore {
            self.ui.recovery_open = false;
            self.request(PendingAction::Recover(entry), ctx);
        }
    }
    pub(super) fn open_recovery(&mut self) {
        self.refresh_recoveries();
        self.ui.recovery_open = true;
    }
}

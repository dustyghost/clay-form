//! File operations and unsaved-change decisions.
use super::*;

impl App {
    pub(super) fn save_to(&mut self, path: &str) -> bool {
        match persistence::save_project(path, &self.document.project()) {
            Ok(()) => {
                self.document.mark_saved(path);
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
        match action {
            PendingAction::New => {
                self.document.new_sphere();
                self.retire_recovery();
                self.ui.status = "New sphere".into();
            }
            PendingAction::Open(path) => {
                let result = persistence::load_project(&path)
                    .and_then(|project| self.document.replace_project(project, &path));
                match result {
                    Ok(()) => {
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

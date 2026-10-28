//! A version-specific development notice shown before the workspace opens.
use super::*;

impl UiState {
    pub(crate) fn initialize_startup_notice(&mut self) {
        self.startup_notice_open = !env!("CARGO_PKG_VERSION_PRE").is_empty()
            && self.preferences.dismissed_startup_version != env!("CARGO_PKG_VERSION");
        self.startup_notice_skip = false;
    }

    pub(crate) fn startup_notice(&mut self, ctx: &egui::Context) {
        design::appearance(ctx, &self.preferences.appearance);
        egui::CentralPanel::default().show(ctx, |_| {});
        egui::Modal::new(egui::Id::new("development_release_notice")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 64.).clamp(220., 460.));
            ui.heading("Welcome to Clay Form");
            ui.add_space(8.);
            ui.label(format!(
                "This is Clay Form {}, an early development release.",
                env!("CARGO_PKG_VERSION")
            ));
            ui.label("Some features may be incomplete or behave unexpectedly. Save often and keep backups of important work.");
            ui.add_space(8.);
            ui.label("Bug reports and feedback are welcome. Thank you for trying Clay Form!");
            ui.hyperlink_to("Report an issue", "https://github.com/dustyghost/clay-form/issues");
            ui.separator();
            ui.checkbox(&mut self.startup_notice_skip, "Don’t show again for this version");
            ui.add_space(8.);
            if ui.button("Continue to Clay Form").clicked() {
                if self.startup_notice_skip {
                    self.preferences.dismissed_startup_version = env!("CARGO_PKG_VERSION").into();
                }
                self.startup_notice_open = false;
            }
        });
    }
}

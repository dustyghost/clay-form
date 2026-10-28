//! Offline licence information travels with the executable.
use super::*;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Notice {
    name: String,
    version: String,
    repository: String,
    license: String,
    text: String,
}

fn notices() -> &'static [Notice] {
    static NOTICES: OnceLock<Vec<Notice>> = OnceLock::new();
    NOTICES.get_or_init(|| {
        serde_json::from_str(include_str!("../../legal/third-party.json"))
            .expect("checked-in third-party notices must be valid")
    })
}

impl UiState {
    pub(crate) fn about(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        egui::Window::new("About Clay Form")
            .open(&mut self.about_open)
            .default_width(620.)
            .max_width((ctx.content_rect().width() - 40.).max(240.))
            .max_height((ctx.content_rect().height() - 80.).max(160.))
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading(format!("Clay Form {}", env!("CARGO_PKG_VERSION")));
                ui.label(format!("Build: {}", env!("CLAY_BUILD_ID")));
                ui.label("Free, open-source sculpting • GPL-3.0-only");
                ui.label("Copyright © 2026 Clay Form contributors. No warranty.");
                ui.horizontal_wrapped(|ui| {
                    ui.hyperlink_to("Source & contributions", env!("CARGO_PKG_REPOSITORY"));
                    ui.hyperlink_to("Releases & corresponding source", "https://github.com/dustyghost/clay-form/releases");
                });
                ui.separator();
                ui.strong("Your creations belong to you");
                ui.label("Use, sell, license or keep your models, textures, renders and other artwork private. You do not need to credit Clay Form or publish your work. Using the app does not put your work under GPLv3.");
                ui.label("Imported third-party material retains its own terms. Official Clay Form downloads are free of charge.");
                ui.collapsing("Clay Form licence", |ui| {
                    ui.label(include_str!("../../LICENSE"));
                });
                ui.separator();
                ui.heading("Third-party software and fonts");
                ui.label("These components retain their own licences. The inventory covers the supported native build targets; some components are platform-specific or build tools.");
                for (index, notice) in notices().iter().enumerate() {
                    egui::CollapsingHeader::new(format!("{} {} — {}", notice.name, notice.version, notice.license))
                        .id_salt(index)
                        .show(ui, |ui| {
                            ui.hyperlink_to("Source repository", &notice.repository);
                            ui.label(&notice.text);
                        });
                }
                ui.separator();
                ui.hyperlink_to("Rust standard library source", "https://github.com/rust-lang/rust");
                ui.label("Rust runtime licence and copyright notices are included in the download under legal/rust/.");
                ui.label("Blender and GIMP are optional, separately installed applications. Their names identify interoperability and do not imply endorsement.");
                ui.horizontal(|ui| {
                    ui.hyperlink_to("Blender source", "https://projects.blender.org/blender/blender");
                    ui.hyperlink_to("GIMP source", "https://gitlab.gnome.org/GNOME/gimp");
                });
            });
    }
}

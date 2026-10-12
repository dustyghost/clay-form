use super::{
    UiState,
    design::{self, Glyph},
};
use crate::{
    command::Command,
    document::Document,
    sculpt::{Brush, BrushIcon},
};
use eframe::egui;
use std::collections::HashMap;
#[derive(Default)]
pub(super) struct Icons {
    custom: HashMap<usize, (Brush, egui::TextureHandle)>,
    pub(super) shot: Option<Shot>,
}
pub(super) struct Shot {
    pub index: usize,
    pub identity: String,
    pub name: String,
    pub icon: BrushIcon,
    texture: egui::TextureHandle,
    path: String,
}
fn texture(ctx: &egui::Context, name: String, icon: &BrushIcon) -> egui::TextureHandle {
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied([icon.size; 2], &icon.rgba),
        egui::TextureOptions::LINEAR,
    )
}
impl UiState {
    pub(super) fn brush_texture(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        brush: &Brush,
    ) -> egui::TextureId {
        if self
            .icons
            .custom
            .get(&index)
            .is_none_or(|(old, _)| old != brush)
        {
            let icon = crate::brush_icon::projected(brush);
            self.icons.custom.insert(
                index,
                (
                    brush.clone(),
                    texture(ctx, format!("brush-icon-{index}"), &icon),
                ),
            );
        }
        self.icons.custom[&index].1.id()
    }

    pub(crate) fn set_icon_shot(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        brush: &Brush,
        icon: BrushIcon,
    ) {
        let texture = texture(ctx, "brush-icon-shot".into(), &icon);
        self.icons.shot = Some(Shot {
            index,
            identity: brush.identity(),
            name: brush.name.clone(),
            icon,
            texture,
            path: "brush-icon.png".into(),
        });
    }
    pub(crate) fn icon_capture_open(&self) -> bool {
        self.icons.shot.is_some()
    }
    pub(crate) fn icon_capture_popup(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
    ) -> Option<Command> {
        let shot = self.icons.shot.as_mut()?;
        let mut command = None;
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("brush_icon_capture")).show(ctx, |ui| {
            ui.heading("Square model capture");
            ui.label(format!("Brush: {}", shot.name));
            ui.image((shot.texture.id(), egui::vec2(256., 256.)));
            ui.weak("Active model, current viewing angle. No handles or UI.");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut shot.path);
                if ui
                    .add(design::button(ui, Glyph::Save, "Save PNG"))
                    .clicked()
                {
                    command = Some(Command::ExportIconImage(
                        shot.path.clone(),
                        shot.icon.clone(),
                    ));
                }
            });
            ui.horizontal(|ui| {
                if ui.add(design::button(ui, Glyph::Close, "Cancel")).clicked() {
                    close = true;
                }
                if ui
                    .add(design::button(ui, Glyph::Camera, "Use as brush icon"))
                    .clicked()
                {
                    if let Some(b) = document
                        .brushes()
                        .get(shot.index)
                        .filter(|b| b.identity() == shot.identity)
                    {
                        let mut b = b.clone();
                        b.icon = Some(shot.icon.clone());
                        command = Some(Command::UpdateBrush(shot.index, b));
                    }
                    close = true;
                }
            });
        });
        if close || response.should_close() {
            self.icons.shot = None;
        }
        command
    }
}

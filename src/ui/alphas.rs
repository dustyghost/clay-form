//! Alpha library presentation and private image drafts.
#[cfg(test)]
mod tests;

use super::*;
use crate::sculpt::{
    Brush,
    alpha::{Adjustments, Asset, Stencil, builtins},
};

/// Built-ins are always available without adding assets to the saved project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Selection {
    Builtin(usize),
    Project(usize),
}
impl Selection {
    fn project_index(self) -> Option<usize> {
        match self {
            Self::Project(index) => Some(index),
            _ => None,
        }
    }

    fn asset(self, document: &Document) -> Option<Asset> {
        match self {
            Self::Builtin(index) => builtins().get(index).cloned(),
            Self::Project(index) => document.alphas().get(index).cloned(),
        }
    }
}

pub(crate) struct Draft {
    index: Option<usize>,
    asset: Asset,
    adjustments: Adjustments,
    preview: Result<Alpha, String>,
}
pub(crate) struct AlphaControls {
    pub selected: Option<Selection>,
    pub draft: Option<Draft>,
    pub export_path: String,
    pub overwrite: Option<(usize, String)>,
    search: String,
    textures: Vec<(Alpha, egui::TextureHandle)>,
}
impl Default for AlphaControls {
    fn default() -> Self {
        Self {
            selected: None,
            draft: None,
            export_path: "alpha.png".into(),
            overwrite: None,
            search: String::new(),
            textures: Vec::new(),
        }
    }
}
impl AlphaControls {
    pub fn edit(&mut self, index: Option<usize>, asset: Asset) {
        self.draft = Some(Draft {
            index,
            preview: Ok(asset.image.clone()),
            asset,
            adjustments: Default::default(),
        });
    }
    fn texture(&mut self, ctx: &egui::Context, alpha: &Alpha) -> egui::TextureId {
        if let Some((_, t)) = self.textures.iter().find(|(a, _)| a == alpha) {
            return t.id();
        }
        while !self.textures.is_empty()
            && (self.textures.len() >= 70
                || self
                    .textures
                    .iter()
                    .map(|(a, _)| a.pixel_count())
                    .sum::<usize>()
                    + alpha.pixel_count()
                    > 2 * 2048 * 2048)
        {
            drop(self.textures.remove(0));
        }
        let pixels: Vec<_> = (0..128 * 128)
            .map(|i| {
                Color32::from_gray(
                    (alpha.sample((i % 128) as f32 / 127., (i / 128) as f32 / 127.) * 255.).round()
                        as u8,
                )
            })
            .collect();
        let t = ctx.load_texture(
            "alpha preview",
            egui::ColorImage::new([128, 128], pixels),
            egui::TextureOptions::LINEAR,
        );
        let id = t.id();
        self.textures.push((alpha.clone(), t));
        id
    }
}
impl UiState {
    pub(super) fn alpha_panel(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        selected: usize,
        brush: &mut Brush,
        commands: &mut Vec<Command>,
    ) {
        ui.heading("Alphas & stencils");
        let footprint = Alpha::Image {
            width: 32,
            height: 32,
            pixels: (0..32 * 32)
                .map(|i| brush.weight((i % 32) as f32 / 15.5 - 1., 1. - (i / 32) as f32 / 15.5))
                .collect::<Vec<_>>()
                .into(),
        };
        let texture = self.alphas.texture(ui.ctx(), &footprint);
        ui.horizontal(|ui| {
            design::help(ui.image((texture, Vec2::splat(48.))), "Current brush footprint, including rotation and falloff. The library selection below does not change it until you choose Use on brush.");
            ui.vertical(|ui| {
                ui.label(format!("Brush: {}", brush.name));
                ui.weak(match brush.alpha { Alpha::Soft => "Soft alpha", Alpha::Hard => "Hard alpha", Alpha::Square => "Square alpha", Alpha::Image { .. } => "Image alpha" });
            });
        });
        if ui
            .add(design::button(
                ui,
                Glyph::Plus,
                "Copy brush alpha to library",
            ))
            .clicked()
        {
            self.alphas.edit(
                None,
                Asset {
                    name: format!("{} alpha", brush.name),
                    category: String::new(),
                    image: brush.alpha.clone(),
                },
            );
        }
        if document.stencil().is_some() {
            ui.collapsing("Stencil placement", |ui| {
                self.stencil_controls(ui, document, commands);
            });
        }
        ui.separator();
        ui.label("Alpha library");
        ui.add(
            egui::TextEdit::singleline(&mut self.alphas.search)
                .hint_text("Search names or categories")
                .desired_width(f32::INFINITY),
        );
        let query = self.alphas.search.to_lowercase();
        let height_id = ui.make_persistent_id("alpha_library_height");
        let mut library_height = ui
            .ctx()
            .data_mut(|data| data.get_persisted::<f32>(height_id).unwrap_or(352.));
        egui::ScrollArea::vertical()
            .id_salt("alpha_library_thumbnails")
            .max_height(library_height)
            // Earlier panels can exhaust the parent scroll area's suggested height.
            // Keep the chosen height and let the outer sidebar scroll to fit it.
            .min_scrolled_height(library_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let columns = ((ui.available_width() / 100.).floor() as usize).clamp(1, 4);
                let width = (ui.available_width() - (columns - 1) as f32 * 6.) / columns as f32;
                egui::Grid::new("alpha_library")
                    .spacing(Vec2::splat(6.))
                    .show(ui, |ui| {
                        let mut count = 0;
                        for selection in (0..builtins().len())
                            .map(Selection::Builtin)
                            .chain((0..document.alphas().len()).map(Selection::Project))
                        {
                            let asset = selection.asset(document).expect("existing library entry");
                            if !format!("{} {}", asset.name, asset.category)
                                .to_lowercase()
                                .contains(&query)
                            {
                                continue;
                            }
                            let id = self.alphas.texture(ui.ctx(), &asset.image);
                            let response = design::brush_tile(
                                ui,
                                id,
                                &asset.name,
                                self.alphas.selected == Some(selection),
                                width,
                            )
                            .on_hover_text(&asset.category);
                            let response = match (&selection, &asset.image) {
                                (Selection::Builtin(_), Alpha::Soft) => {
                                    response.customize(Action::AlphaSoft)
                                }
                                (Selection::Builtin(_), Alpha::Hard) => {
                                    response.customize(Action::AlphaHard)
                                }
                                (Selection::Builtin(_), Alpha::Square) => {
                                    response.customize(Action::AlphaSquare)
                                }
                                _ => response,
                            };
                            if response.clicked() {
                                self.alphas.selected = Some(selection);
                            }
                            count += 1;
                            if count % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
        // Keep the grab area in the middle, clear of both nested scrollbars.
        let resize = ui
            .horizontal(|ui| {
                let width = (ui.available_width() - 32.).clamp(64., 192.);
                ui.add_space(((ui.available_width() - width) * 0.5).max(0.));
                ui.add_sized(
                    [width, 32.],
                    egui::Button::new("Drag to resize library")
                        .frame(false)
                        .sense(egui::Sense::drag()),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                .on_hover_text("Drag up or down to change the library height")
            })
            .inner;
        let drag_height_id = height_id.with("drag_start");
        if resize.drag_started() {
            ui.ctx()
                .data_mut(|data| data.insert_temp(drag_height_id, library_height));
        }
        if let Some(delta) = resize.total_drag_delta() {
            let start_height = ui
                .ctx()
                .data(|data| data.get_temp::<f32>(drag_height_id))
                .unwrap_or(library_height);
            // Measure from the press, excluding pointer motion before the button
            // went down and avoiding repeated accumulation during layout passes.
            library_height = (start_height + delta.y).clamp(258., 704.);
            ui.ctx()
                .data_mut(|data| data.insert_persisted(height_id, library_height));
            ui.ctx().request_repaint();
        }
        if resize.drag_stopped() {
            ui.ctx().data_mut(|data| data.remove::<f32>(drag_height_id));
        }
        if let Some((selection, asset)) = self
            .alphas
            .selected
            .and_then(|selection| selection.asset(document).map(|asset| (selection, asset)))
        {
            let project_index = selection.project_index();
            ui.label(format!(
                "{} · {}",
                asset.name,
                if asset.category.is_empty() {
                    "Uncategorized"
                } else {
                    &asset.category
                }
            ));
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(design::button(ui, Glyph::Edit, "Use on brush"))
                    .clicked()
                {
                    if let Some(i) = project_index {
                        commands.push(Command::UseAlpha(i, selected));
                    } else {
                        brush.alpha = asset.image.clone();
                    }
                }
                if ui
                    .add(design::button(ui, Glyph::Pin, "Place stencil"))
                    .clicked()
                {
                    commands.push(Command::SetStencil(Some(Stencil::new(asset.clone()))));
                }
                if let Some(i) = project_index
                    && ui
                        .add(design::button(ui, Glyph::Edit, "Edit alpha"))
                        .clicked()
                {
                    self.alphas.edit(Some(i), asset.clone());
                }
                if ui
                    .add(design::button(ui, Glyph::Copy, "Duplicate alpha"))
                    .clicked()
                {
                    let mut copy = asset.clone();
                    copy.name = format!("{} copy", copy.name);
                    self.alphas.edit(None, copy);
                }
                if let Some(i) = project_index
                    && ui
                        .add(design::button(ui, Glyph::Delete, "Remove alpha"))
                        .clicked()
                {
                    commands.push(Command::RemoveAlpha(i));
                }
            });
            if project_index.is_none() {
                ui.weak("Built-in alpha. Duplicate it to edit and save a variation.");
            } else {
                ui.weak(
                    "Brushes and stencils keep independent copies when a library asset changes.",
                );
            }
            if let Some(i) = project_index {
                ui.collapsing("Export selected alpha", |ui| {
                    ui.text_edit_singleline(&mut self.alphas.export_path);
                    if ui
                        .add(design::button(ui, Glyph::Export, "Export PNG"))
                        .clicked()
                    {
                        commands.push(Command::ExportAlpha(i, false));
                    }
                });
            }
        }
        ui.collapsing("Import image", |ui| {
            ui.label("Image path (PNG/JPEG, up to 2048 × 2048)");
            ui.text_edit_singleline(&mut self.alpha_path);
            if ui
                .add(design::button(ui, Glyph::Open, "Choose image…"))
                .customize(Action::LoadAlpha)
                .clicked()
            {
                commands.push(Command::Action(Action::LoadAlpha));
            }
            ui.weak("White applies; black protects. Adjust the image before saving it.");
        });
    }
    fn stencil_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if let Some(original) = document.stencil() {
            ui.separator();
            ui.label(format!("Stencil: {}", original.asset.name));
            let mut stencil = original.clone();
            let mut remove = false;
            ui.horizontal_wrapped(|ui| {
                design::checkbox(ui, &mut stencil.enabled, "Stencil enabled")
                    .customize(Action::ToggleStencil);
                if ui
                    .add(design::button(ui, Glyph::Close, "Remove stencil"))
                    .customize(Action::RemoveStencil)
                    .clicked()
                {
                    remove = true;
                }
            });
            ui.add(
                egui::Label::new(if stencil.surface.is_some() {
                    "Surface plane · model units"
                } else {
                    "View · fractions of viewport"
                })
                .wrap(),
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(design::button(ui, Glyph::Pin, "Attach to surface…"))
                    .customize(Action::AttachStencil)
                    .clicked()
                {
                    commands.push(Command::AttachStencil);
                }
                if stencil.surface.is_some()
                    && ui
                        .add(design::button(ui, Glyph::View, "Follow view"))
                        .clicked()
                {
                    stencil.surface = None;
                    stencil.position = [0.; 2];
                    stencil.size = [0.5; 2];
                }
            });
            for (i, axis) in ["Horizontal", "Vertical"].into_iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(axis);
                    ui.add(
                        egui::DragValue::new(&mut stencil.position[i])
                            .speed(0.005)
                            .range(-10000. ..=10000.),
                    );
                });
            }
            for (i, axis) in ["Width", "Height"].into_iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(axis);
                    ui.add(
                        egui::DragValue::new(&mut stencil.size[i])
                            .speed(0.005)
                            .range(0.001..=10000.),
                    );
                });
            }
            ui.add(egui::Slider::new(&mut stencil.rotation, -180. ..=180.).text("Rotation"));
            design::help(
                ui.add(egui::Slider::new(&mut stencil.opacity, 0. ..=1.).text("Overlay opacity")),
                "Changes the guide only, not sculpt strength. Use zero to inspect the sculpt while keeping the stencil active.",
            );
            ui.collapsing("Stencil help", |ui| {
                ui.weak("Brush through the stencil using Freehand, Line or stamps. Brush alpha, strength and pressure multiply its coverage. Symmetry copies the stencil. Adaptive sculpting refines through the stencil. Grab, Snake and Detail require disabling the stencil.");
            });
            if remove {
                commands.push(Command::SetStencil(None));
            } else if stencil != *original {
                commands.push(Command::SetStencil(Some(stencil)));
            }
        }
    }
    pub(crate) fn alpha_popup(&mut self, ctx: &egui::Context) -> Option<Command> {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.alphas.draft = None;
            self.alphas.overwrite = None;
            return None;
        }
        let mut command = None;
        if let Some((index, path)) = self.alphas.overwrite.clone() {
            let modal = egui::Modal::new(egui::Id::new("alpha_overwrite")).show(ctx, |ui| {
                ui.label(format!("Replace {}?", path));
                if ui.button("Replace PNG").clicked() {
                    self.alphas.export_path = path;
                    command = Some(Command::ExportAlpha(index, true));
                }
                if ui.button("Cancel").clicked() {
                    self.alphas.overwrite = None;
                }
            });
            if modal.should_close() {
                self.alphas.overwrite = None;
            }
            return command;
        }
        let mut draft = self.alphas.draft.take()?;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("alpha_editor")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 80.).clamp(260., 540.));
            ui.heading("Edit alpha");
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 160.).max(180.))
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.asset.name)
                            .desired_width(ui.available_width()),
                    );
                    ui.label("Category");
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.asset.category)
                            .desired_width(ui.available_width()),
                    );
                    let before = draft.adjustments.clone();
                    ui.columns(2, |columns| {
                        let previews = &mut columns[0];
                        let source = self.alphas.texture(ctx, &draft.asset.image);
                        previews.label("Source");
                        let side = previews.available_width().min(120.);
                        previews.image((source, Vec2::splat(side)));
                        previews.label("Result");
                        if let Ok(alpha) = &draft.preview {
                            let id = self.alphas.texture(ctx, alpha);
                            previews.image((id, Vec2::splat(side)));
                        }
                        let controls = &mut columns[1];
                        controls.spacing_mut().slider_width =
                            (controls.available_width() - 130.).clamp(25., 100.);
                        controls.label("Crop (source fractions)");
                        for (i, name) in ["Left", "Top", "Right", "Bottom"].into_iter().enumerate()
                        {
                            controls.add(
                                egui::Slider::new(&mut draft.adjustments.crop[i], 0. ..=1.)
                                    .text(name),
                            );
                        }
                        controls.add(
                            egui::Slider::new(&mut draft.adjustments.black, 0. ..=1.).text("Black"),
                        );
                        controls.add(
                            egui::Slider::new(&mut draft.adjustments.white, 0. ..=1.).text("White"),
                        );
                        controls.add(
                            egui::Slider::new(&mut draft.adjustments.gamma, 0.1..=4.).text("Gamma"),
                        );
                        controls.add(
                            egui::Slider::new(&mut draft.adjustments.edge_fade, 0. ..=0.5)
                                .text("Edge fade"),
                        );
                        controls.checkbox(&mut draft.adjustments.invert, "Invert heights");
                        if controls.button("Reset adjustments").clicked() {
                            draft.adjustments = Default::default();
                        }
                    });
                    if before != draft.adjustments {
                        draft.preview = draft.adjustments.apply(&draft.asset.image);
                    }
                    if let Err(error) = &draft.preview {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                    if let Err(error) = draft.asset.validate() {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        draft.preview.is_ok() && draft.asset.validate().is_ok(),
                        design::button(ui, Glyph::Save, "Save alpha"),
                    )
                    .clicked()
                {
                    let mut asset = draft.asset.clone();
                    asset.image = draft.preview.as_ref().unwrap().clone();
                    command = Some(Command::SaveAlpha(draft.index, asset));
                }
                if ui.add(design::button(ui, Glyph::Close, "Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if !cancel && !modal.should_close() {
            self.alphas.draft = Some(draft);
        }
        command
    }
}

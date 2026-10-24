use crate::{
    preferences::MaterialLibrary,
    sculpt::{Brush, Material, PaintChannels, Vec3},
};
use eframe::egui;

#[derive(Default)]
pub(crate) struct MaterialControls {
    textures: Vec<(Material, egui::TextureHandle)>,
    selected_copy: Option<usize>,
    pub sampling: bool,
}
impl MaterialControls {
    pub fn show(&mut self, ui: &mut egui::Ui, brush: &mut Brush, library: &mut MaterialLibrary) {
        if self.selected_copy.is_some_and(|i| {
            library
                .copies
                .get(i)
                .is_none_or(|p| p.material != brush.material)
        }) {
            self.selected_copy = None;
        }
        ui.strong("Materials");
        let presets: Vec<_> = Material::examples()
            .into_iter()
            .map(|(n, m)| (n.to_owned(), m))
            .chain(library.copies.iter().map(|p| (p.name.clone(), p.material)))
            .collect();
        egui::ScrollArea::vertical()
            .id_salt("material_swatches")
            .max_height(252.)
            .show(ui, |ui| {
                let width =
                    ((ui.available_width() - 2. * ui.spacing().item_spacing.x) / 3.).max(48.);
                egui::Grid::new("materials").num_columns(3).show(ui, |ui| {
                    for (index, (name, material)) in presets.iter().enumerate() {
                        let texture = self.texture(ui.ctx(), index, *material);
                        let selected = if let Some(copy) = self.selected_copy {
                            index == copy + 6
                        } else {
                            index < 6 && *material == brush.material
                        };
                        if super::design::brush_tile(ui, texture, name, selected, width).clicked() {
                            brush.material = *material;
                            self.selected_copy = index.checked_sub(6);
                        }
                        if index % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
            });
        let name = self
            .selected_copy
            .and_then(|i| library.copies.get(i))
            .map(|p| p.name.as_str())
            .or_else(|| {
                presets
                    .iter()
                    .take(6)
                    .find(|(_, m)| *m == brush.material)
                    .map(|(n, _)| n.as_str())
            })
            .unwrap_or("Custom material")
            .to_owned();
        ui.horizontal(|ui| {
            ui.label(&name);
            if ui
                .add_enabled(
                    library.copies.len() < 64,
                    egui::Button::new("Copy material"),
                )
                .on_hover_text("Save an editable copy in your material library")
                .clicked()
            {
                self.selected_copy = library.copy(&name, brush.material);
            }
        });
        if let Some(index) = self.selected_copy {
            let mut name = library.copies[index].name.clone();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut name)
                        .char_limit(32)
                        .hint_text("Material name"),
                )
                .changed()
                && !name.trim().is_empty()
            {
                library.copies[index].name = name;
            }
        }
        egui::ComboBox::from_label("Paint channels")
            .selected_text(brush.paint_channels.label())
            .show_ui(ui, |ui| {
                for channel in PaintChannels::ALL {
                    ui.selectable_value(&mut brush.paint_channels, channel, channel.label());
                }
            });
        ui.horizontal(|ui| {
            ui.label("Colour");
            let mut color = brush.material.base_color;
            if ui.color_edit_button_rgb(&mut color).changed() {
                brush.material.base_color = color;
            }
            if ui
                .selectable_label(self.sampling, "Sample colour")
                .on_hover_text("Click a visible surface to sample its paint colour. Esc cancels.")
                .clicked()
            {
                self.sampling = !self.sampling;
            }
        });
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            for (index, &colour) in library.palette.iter().enumerate() {
                let colour32 =
                    egui::Color32::from(egui::Rgba::from_rgb(colour[0], colour[1], colour[2]));
                let response = ui
                    .add(
                        egui::Button::new("")
                            .fill(colour32)
                            .min_size(egui::vec2(24., 24.))
                            .stroke(if brush.material.base_color == colour {
                                ui.visuals().selection.stroke
                            } else {
                                egui::Stroke::NONE
                            }),
                    )
                    .on_hover_text("Select colour · right-click to remove");
                if response.clicked() {
                    brush.material.base_color = colour;
                }
                response.context_menu(|ui| {
                    if ui.button("Remove colour").clicked() {
                        remove = Some(index);
                        ui.close();
                    }
                });
            }
            if ui
                .add_enabled(library.palette.len() < 32, egui::Button::new("+ Colour"))
                .on_hover_text("Save the current colour to your palette")
                .clicked()
            {
                library.add_colour(brush.material.base_color);
            }
        });
        if let Some(index) = remove {
            library.palette.remove(index);
        }
        ui.add(egui::Slider::new(&mut brush.material.roughness, 0. ..=1.).text("Roughness"));
        ui.add(egui::Slider::new(&mut brush.material.metallic, 0. ..=1.).text("Metallic"));
        if let Some(index) = self.selected_copy {
            library.copies[index].material = brush.material;
        }
    }
    fn texture(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        material: Material,
    ) -> egui::TextureId {
        if index == self.textures.len() {
            self.textures.push((
                material,
                ctx.load_texture(
                    format!("material-{index}"),
                    render_swatch(material),
                    egui::TextureOptions::LINEAR,
                ),
            ));
        } else if self.textures[index].0 != material {
            self.textures[index].0 = material;
            self.textures[index]
                .1
                .set(render_swatch(material), egui::TextureOptions::LINEAR);
        }
        self.textures[index].1.id()
    }
}
fn render_swatch(material: Material) -> egui::ColorImage {
    const SIZE: usize = 96;
    let mut rgba = vec![0; SIZE * SIZE * 4];
    let light = Vec3(-0.45, 0.65, 1.).norm();
    for y in 0..SIZE {
        for x in 0..SIZE {
            let nx = (x as f32 + 0.5 - SIZE as f32 / 2.) / 44.;
            let ny = -(y as f32 + 0.5 - SIZE as f32 / 2.) / 44.;
            let r = nx * nx + ny * ny;
            if r >= 1. {
                continue;
            }
            let normal = Vec3(nx, ny, (1. - r).sqrt());
            let rgb = material.studio_color(normal, Vec3(0., 0., 1.), light);
            let i = (y * SIZE + x) * 4;
            for c in 0..3 {
                rgba[i + c] = (rgb[c].clamp(0., 1.) * 255.).round() as u8;
            }
            rgba[i + 3] = (((1. - r.sqrt()) * 44.).clamp(0., 1.) * 255.) as u8;
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([SIZE, SIZE], &rgba)
}

use crate::{
    preferences::MaterialLibrary,
    sculpt::{Brush, Material, Vec3},
};
use eframe::egui;

#[derive(Default)]
pub(crate) struct MaterialControls {
    textures: Vec<(Material, egui::TextureHandle)>,

    pub sampling: bool,
    usage: Option<(u64, u64, usize, usize)>,
    pending: Option<(u64, crate::sculpt::material::layers::Asset)>,
}
impl MaterialControls {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        brush: &mut Brush,
        library: &mut MaterialLibrary,
        document: &crate::document::Document,
        commands: &mut Vec<crate::command::Command>,
    ) {
        use crate::command::Command;
        use crate::sculpt::material::layers::{Asset, Layer, Surface};
        let mut presets: Vec<Asset> = library
            .names
            .iter()
            .zip(&library.values)
            .enumerate()
            .filter(|(i, _)| !library.hidden_builtins.contains(&(*i as u64 + 1)))
            .map(|(i, (name, m))| Asset {
                id: i as u64 + 1,
                name: name.clone(),
                material: m.unlinked(),
                original: None,
            })
            .chain(library.copies.iter().map(|p| Asset {
                id: p.id,
                name: p.name.clone(),
                material: p.material.unlinked(),
                original: p.original.clone(),
            }))
            .collect();
        if let Some(id) = brush.material.primary_asset()
            && !presets.iter().any(|a| a.id == id)
            && !document.materials().iter().any(|a| a.id == id)
        {
            presets.push(Asset {
                id,
                name: "Custom material".into(),
                material: brush.material.unlinked(),
                original: None,
            });
        }
        for asset in document.materials() {
            if let Some(existing) = presets.iter_mut().find(|p| p.id == asset.id) {
                *existing = asset.clone();
            } else {
                presets.push(asset.clone());
            }
        }
        for asset in &mut presets {
            asset.original = Some(asset.reset_material());
        }
        // Keep slider scrubbing local to the brush. Rebase the scene once on
        // release, rather than traversing every painted level every frame.
        if self
            .pending
            .as_ref()
            .is_some_and(|(revision, _)| *revision != document.display_revision())
            || ui.input(|input| input.key_pressed(egui::Key::Escape))
        {
            self.pending = None;
        }
        if let Some((_, draft)) = &self.pending {
            if let Some(asset) = presets.iter_mut().find(|asset| asset.id == draft.id) {
                *asset = draft.clone();
            }
        }
        if !ui.input(|input| input.pointer.any_down())
            && let Some((_, asset)) = self.pending.take()
        {
            commands.push(Command::SaveMaterial(asset));
        }

        if let Some(id) = brush.material.primary_asset()
            && let Some(asset) = presets.iter().find(|a| a.id == id)
        {
            brush.material = asset.material.with_link(id);
            if !document.materials().iter().any(|a| a.id == id) {
                commands.push(Command::SaveMaterial(asset.clone()));
            }
        }
        let mut selected = brush.material.primary_asset();
        ui.weak("Choose a material for future strokes. Existing paint changes only in the shared material editor below.");
        ui.scope(|ui| {
            let height_id = ui.make_persistent_id("material_library_height");
            let mut height = ui
                .ctx()
                .data_mut(|data| data.get_persisted::<f32>(height_id).unwrap_or(176.));
            egui::ScrollArea::vertical()
                .id_salt("material_swatches")
                .max_height(height)
                .min_scrolled_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let width =
                        ((ui.available_width() - 2. * ui.spacing().item_spacing.x) / 3.).max(48.);
                    egui::Grid::new("materials").num_columns(3).show(ui, |ui| {
                        for (index, asset) in presets.iter().enumerate() {
                            let (name, material) = (&asset.name, &asset.material);
                            let texture = self.texture(ui.ctx(), index, material.clone());
                            if super::design::brush_tile(
                                ui,
                                texture,
                                name,
                                selected == Some(asset.id),
                                width,
                            )
                            .clicked()
                            {
                                brush.material = material.with_link(asset.id);
                                selected = Some(asset.id);
                                commands.push(Command::SaveMaterial(asset.clone()));
                            }
                            if index % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                });
            let resize = ui
                .horizontal(|ui| {
                    let width = (ui.available_width() - 32.).clamp(64., 192.);
                    ui.add_space(((ui.available_width() - width) * 0.5).max(0.));
                    ui.add_sized(
                        [width, 32.],
                        egui::Button::new("Drag to resize materials")
                            .frame(false)
                            .sense(egui::Sense::drag()),
                    )
                    .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                    .on_hover_text("Drag up or down to change the material library height")
                })
                .inner;
            let drag_id = height_id.with("drag_start");
            if resize.drag_started() {
                ui.ctx().data_mut(|data| data.insert_temp(drag_id, height));
            }
            if let Some(delta) = resize.total_drag_delta() {
                let start = ui
                    .ctx()
                    .data(|data| data.get_temp::<f32>(drag_id))
                    .unwrap_or(height);
                height = (start + delta.y).clamp(82., 704.);
                ui.ctx()
                    .data_mut(|data| data.insert_persisted(height_id, height));
                ui.ctx().request_repaint();
            }
            if resize.drag_stopped() {
                ui.ctx().data_mut(|data| data.remove::<f32>(drag_id));
            }
        });
        let name = selected
            .and_then(|id| presets.iter().find(|a| a.id == id))
            .map_or("Custom material", |a| a.name.as_str())
            .to_owned();
        ui.label(format!("Brush material: {name}"));
        ui.collapsing("Edit shared project material", |ui| {
        if let Some(id) = selected {
            let revision = document.display_revision();
            if !self.usage.is_some_and(|(r, i, _, _)| r == revision && i == id) {
                let (models, vertices) = document.material_usage(id);
                self.usage = Some((revision, id, models, vertices));
            }
            let (_, _, models, vertices) = self.usage.unwrap();
            ui.weak(format!("Edits update {vertices} painted vertices across {models} models, including stored subdivision levels. Project Undo restores them."));
        } else {
            ui.weak("Create a named project material from this brush. Future strokes can share its settings.");
        }
        let mut edited_name = name.clone();
        ui.horizontal(|ui| {
            ui.label("Material name");
            ui.add(
                egui::TextEdit::singleline(&mut edited_name)
                    .char_limit(32)
                    .desired_width(140.),
            );
        });
        let before = brush.material.clone();
        let mut original = selected
            .and_then(|id| presets.iter().find(|a| a.id == id))
            .map(Asset::reset_material)
            .unwrap_or_else(|| before.unlinked());
        let mut remove_material = None;
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(document.materials().len() < 256, egui::Button::new("Copy material"))
                .on_hover_text("Create an independent project material for future strokes; existing paint keeps its current material.")
                .clicked()
            {
                let id = crate::sculpt::material::layers::fresh_id();
                selected = Some(id);
                edited_name = format!("{} copy", name.chars().take(24).collect::<String>());
                original = brush.material.unlinked();
                brush.material = original.with_link(id);
                commands.push(Command::SaveMaterial(Asset {
                    id,
                    name: edited_name.clone(),
                    material: original.clone(),
                    original: Some(original.clone()),
                }));
            }
            if ui.add_enabled(brush.material.values() != original.values(), egui::Button::new("Reset material"))
                .on_hover_text("Restore original surface settings and layers; keep the name. Updates painted uses and can be undone.")
                .clicked()
            {
                brush.material = selected.map_or_else(|| original.clone(), |id| original.with_link(id));
            }
        });
        if ui.add_enabled(selected.is_some(), egui::Button::new("Remove from project"))
            .on_hover_text("Remove this project definition and detach painted uses, preserving their appearance. Personal library presets remain available. Undo restores the project material.")
            .clicked() {
            remove_material = selected;
        }
        if !library.hidden_builtins.is_empty() && ui.button("Restore built-in materials").clicked()
        {
            library.hidden_builtins.clear();
        }
        if let Some(id) = remove_material {
            // Register an unlinked matching preset before removal so Undo can restore it.
            if !document.materials().iter().any(|a| a.id == id)
                && !commands
                    .iter()
                    .any(|c| matches!(c, Command::SaveMaterial(a) if a.id == id))
                && let Some(asset) = presets.iter().find(|a| a.id == id)
            {
                commands.push(Command::SaveMaterial(asset.clone()));
            }
            brush.material.detach_asset(id);
            commands.push(Command::RemoveMaterial(id));
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Shared colour");
            let mut color = brush.material.base_color;
            if ui.color_edit_button_rgb(&mut color).changed() {
                brush.material.base_color = color;
            }
        });
        ui.add(egui::Slider::new(&mut brush.material.roughness, 0. ..=1.).text("Roughness"));
        ui.add(egui::Slider::new(&mut brush.material.metallic, 0. ..=1.).text("Metallic"));
        ui.collapsing("Surface options", |ui| {
            ui.add(egui::Slider::new(&mut brush.material.ior, 1. ..=4.).text("IOR"));
            ui.add(egui::Slider::new(&mut brush.material.specular, 0. ..=1.).text("Specular"));
            ui.horizontal(|ui| {
                ui.label("Emission");
                ui.color_edit_button_rgb(&mut brush.material.emission);
            });
            ui.add(
                egui::Slider::new(&mut brush.material.emission_strength, 0. ..=100.)
                    .text("Emission strength"),
            );
        });
        ui.collapsing("Subsurface scattering", |ui| {
            subsurface_controls(
                ui,
                &mut brush.material.subsurface,
                &mut brush.material.subsurface_radius,
                &mut brush.material.subsurface_scale,
            );
        });
        ui.collapsing("Material layers", |ui| {
            ui.weak("Layers are evaluated from top to bottom. Editing a named material updates its painted uses.");
            let occupied:Vec<_>=brush.material.layers.iter().enumerate().filter_map(|(i,layer)|(!layer.is_empty()).then_some(i)).collect();
            let mut remove=None;
            let mut swap=None;
            for (number,&i) in occupied.iter().enumerate() {
                let layer=&mut brush.material.layers[i];
                ui.push_id(i,|ui| {
                    ui.collapsing(format!("Layer {}",number+1),|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Remove layer").on_hover_text("Delete this layer and keep the remaining layers in order. Undo restores it.").clicked() {remove=Some(i);}
                            if number>0 && ui.button("Move up").clicked() {swap=Some((i,occupied[number-1]));}
                        });
                        ui.add(egui::Slider::new(&mut layer.weight,0. ..=1.).text("Weight"));
                        ui.horizontal(|ui| {ui.selectable_value(&mut layer.add,0.,"Mix");ui.selectable_value(&mut layer.add,1.,"Add");});
                        egui::ComboBox::from_label("Start from").selected_text("Surface preset").show_ui(ui,|ui| {
                            for (name,m) in Material::examples() {if ui.button(name).clicked() {layer.surface=m.surface();layer.weight=1.;ui.close();}}
                        });
                        surface_controls(ui,&mut layer.surface);
                    });
                });
            }
            if let Some(i)=remove {brush.material.layers.remove(i);}
            if let Some((a,b))=swap {brush.material.layers.swap(a,b);}
            if occupied.is_empty() {ui.weak("No additional layers");}
            if ui.add_enabled(brush.material.layers.iter().any(Layer::is_empty),egui::Button::new("Add layer")).clicked() {
                brush.material.layers.add(Layer{surface:Surface{base_color:[0.5;3],..Default::default()},weight:0.5,add:0.});
            }
        });
        if (before.values() != brush.material.values() || name != edited_name)
            && !edited_name.trim().is_empty()
        {
            let id = selected.unwrap_or_else(crate::sculpt::material::layers::fresh_id);
            let material = brush.material.unlinked();
            brush.material = material.with_link(id);
            let asset = Asset {
                id,
                name: edited_name.clone(),
                material,
                original: Some(original.clone()),
            };
            if ui.input(|input| input.pointer.any_down()) {
                self.pending = Some((document.display_revision(), asset));
            } else {
                commands.push(Command::SaveMaterial(asset));
            }
        }
        if self.pending.is_some() { ui.weak("Release to apply to shared painted surfaces. Esc cancels."); }
        if ui.button("Save to personal library")
            .on_hover_text("Save these settings for future projects. This explicit library change is independent of project Undo.")
            .clicked()
        {
            let id = brush.material.primary_asset().unwrap_or_else(crate::sculpt::material::layers::fresh_id);
            commands.push(Command::SaveMaterialToLibrary(Asset {
                id,
                name: edited_name,
                material: brush.material.unlinked(),
                original: Some(original),
            }));
        }
        if let Some(id) = selected {
            if ui.button("Hide personal library preset")
                .on_hover_text("Hide this preset in future projects. Existing project materials and painted surfaces are unchanged; project Undo does not affect your library.")
                .clicked()
            {
                library.remove(id);
            }
        }
        });
    }

    pub(super) fn sample_control(&mut self, ui: &mut egui::Ui, label: &str) {
        if ui.selectable_label(self.sampling, label)
            .on_hover_text("Click the visible surface to sample the active paint channel into this brush. Esc cancels.")
            .clicked()
        {
            self.sampling = !self.sampling;
        }
    }

    /// Ordinary brush colour changes never edit a shared project definition.
    pub(super) fn brush_palette(
        &mut self,
        ui: &mut egui::Ui,
        brush: &mut Brush,
        library: &mut MaterialLibrary,
    ) {
        self.sample_control(ui, "Sample colour");
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            for (index, &colour) in library.palette.iter().enumerate() {
                let colour32 =
                    egui::Color32::from(egui::Rgba::from_rgb(colour[0], colour[1], colour[2]));
                let response = ui
                    .add(
                        egui::Button::new("")
                            .fill(colour32)
                            .min_size(egui::vec2(20., 20.))
                            .stroke(if brush.material.base_color == colour {
                                ui.visuals().selection.stroke
                            } else {
                                egui::Stroke::NONE
                            }),
                    )
                    .on_hover_text(
                        "Brush colour · existing paint stays unchanged · right-click to remove",
                    );
                if response.clicked() {
                    brush.material = brush.material.unlinked();
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
                .on_hover_text("Save this brush colour to your personal palette")
                .clicked()
            {
                library.add_colour(brush.material.base_color);
            }
        });
        if let Some(index) = remove {
            library.palette.remove(index);
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
                material.clone(),
                ctx.load_texture(
                    format!("material-{index}"),
                    render_swatch(material),
                    egui::TextureOptions::LINEAR,
                ),
            ));
        } else if self.textures[index].0 != material {
            self.textures[index].0 = material.clone();
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

fn subsurface_controls(
    ui: &mut egui::Ui,
    weight: &mut f32,
    radius: &mut [f32; 3],
    scale: &mut f32,
) {
    ui.weak(
        "Fast viewport approximation; Blender export uses full Principled subsurface scattering.",
    );
    ui.add(egui::Slider::new(weight, 0. ..=1.).text("Subsurface weight"));
    ui.add(
        egui::Slider::new(scale, 0. ..=10.)
            .logarithmic(true)
            .text("Subsurface scale"),
    );
    ui.horizontal(|ui| {
        ui.label("Radius RGB");
        for (value, label) in radius.iter_mut().zip(["R ", "G ", "B "]) {
            ui.add(
                egui::DragValue::new(value)
                    .range(0. ..=10.)
                    .speed(0.01)
                    .prefix(label),
            );
        }
    });
}
fn surface_controls(ui: &mut egui::Ui, s: &mut crate::sculpt::material::layers::Surface) {
    ui.horizontal(|ui| {
        ui.label("Colour");
        ui.color_edit_button_rgb(&mut s.base_color);
    });
    ui.add(egui::Slider::new(&mut s.roughness, 0. ..=1.).text("Roughness"));
    ui.add(egui::Slider::new(&mut s.metallic, 0. ..=1.).text("Metallic"));
    ui.add(egui::Slider::new(&mut s.ior, 1. ..=4.).text("IOR"));
    ui.add(egui::Slider::new(&mut s.specular, 0. ..=1.).text("Specular"));
    ui.horizontal(|ui| {
        ui.label("Emission");
        ui.color_edit_button_rgb(&mut s.emission);
    });
    ui.add(egui::Slider::new(&mut s.emission_strength, 0. ..=100.).text("Emission strength"));
    subsurface_controls(
        ui,
        &mut s.subsurface,
        &mut s.subsurface_radius,
        &mut s.subsurface_scale,
    );
}

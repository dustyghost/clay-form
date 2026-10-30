use super::customization::ActionResponse;
use crate::action::Action;
use crate::{
    preferences::MaterialLibrary,
    sculpt::{Brush, Material, Vec3},
};
use eframe::egui;

#[derive(Default)]
pub(crate) struct MaterialControls {
    textures: Vec<(Material, egui::TextureHandle)>,
    selected_preview: Option<(Material, egui::TextureHandle)>,
    secondary_edit: bool,
    pub sampling_secondary: bool,
    palette_draft: (egui::ecolor::Hsva, [f32; 3]),

    pub sampling: bool,
    pub sample_colour_only: bool,
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
        presets.sort_by_key(|asset| asset.id != Material::DEFAULT_PRESET_ID);
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
        if let Some((_, draft)) = &self.pending
            && let Some(asset) = presets.iter_mut().find(|asset| asset.id == draft.id)
        {
            *asset = draft.clone();
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
            .map_or(
                if brush.material == Material::default() {
                    "Default clay"
                } else {
                    "Custom material"
                },
                |a| a.name.as_str(),
            )
            .to_owned();
        ui.label(format!("Selected material: {name}"));
        if ui
            .add_enabled(
                document.materials().len() < 256,
                egui::Button::new("New project material"),
            )
            .clicked()
        {
            let id = crate::sculpt::material::layers::fresh_id();
            let material = Material::default();
            brush.material = material.with_link(id);
            commands.push(Command::SaveMaterial(Asset {
                id,
                name: "New material".into(),
                material: material.clone(),
                original: Some(material),
            }));
            return;
        }
        ui.separator();
        ui.label(format!(
            "Model: {}",
            document.model(document.active_model_index()).unwrap().name
        ));
        if ui.add_enabled(document.primitive().is_none(), egui::Button::new("Assign material to model"))
            .on_hover_text("Replace the whole material on all vertices, including masks, hidden regions and stored subdivision levels. Keeps UVs and image pixels. One Undo restores it.")
            .clicked() {
            commands.push(Command::AssignModelMaterial(brush.material.clone()));
        }
        ui.weak("Assign changes this model’s vertex appearance. Undo restores it.");
        if !document.mesh().uv.textures.is_empty() {
            ui.weak("Assigned images still overlay the material; unassign in Textures to reveal its base colour.");
        }
        ui.separator();
        ui.collapsing("Edit shared project material", |ui| {
        if let Some(id) = selected {
            let revision = document.display_revision();
            if !self.usage.is_some_and(|(r, i, _, _)| r == revision && i == id) {
                let (models, vertices) = document.material_usage(id);
                self.usage = Some((revision, id, models, vertices));
            }
            let (_, _, models, vertices) = self.usage.unwrap();
            ui.weak(format!("Edits update {vertices} stored vertices across {models} models, including inactive subdivision levels. Project Undo restores them."));
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
        if let Some(id) = selected
            && ui.button("Hide personal library preset")
                .on_hover_text("Hide this preset in future projects. Existing project materials and painted surfaces are unchanged; project Undo does not affect your library.")
                .clicked()
            {
                library.remove(id);
        }
        });
    }

    pub(super) fn selected_name(
        brush: &Brush,
        library: &MaterialLibrary,
        document: &crate::document::Document,
    ) -> String {
        if let Some(id) = brush.material.primary_asset() {
            if let Some(asset) = document.materials().iter().find(|a| a.id == id) {
                return asset.name.clone();
            }
            if let Some(asset) = library.copies.iter().find(|a| a.id == id) {
                return asset.name.clone();
            }
            if let Some(name) = id
                .checked_sub(1)
                .and_then(|i| library.names.get(i as usize))
            {
                return name.clone();
            }
        }
        if brush.material == Material::default() {
            "Default clay"
        } else {
            "Custom material"
        }
        .into()
    }

    pub(super) fn selected_swatch(
        &mut self,
        ctx: &egui::Context,
        material: &Material,
    ) -> egui::TextureId {
        if self
            .selected_preview
            .as_ref()
            .is_none_or(|(old, _)| old != material)
        {
            let image = render_swatch(material.clone());
            if let Some((old, handle)) = &mut self.selected_preview {
                handle.set(image, egui::TextureOptions::LINEAR);
                *old = material.clone();
            } else {
                self.selected_preview = Some((
                    material.clone(),
                    ctx.load_texture("selected_material", image, egui::TextureOptions::LINEAR),
                ));
            }
        }
        self.selected_preview.as_ref().unwrap().1.id()
    }

    pub(super) fn colour_slots(&mut self, ui: &mut egui::Ui, brush: &mut Brush) {
        for (secondary, label) in [(false, "Primary"), (true, "Secondary")] {
            ui.selectable_value(&mut self.secondary_edit, secondary, label)
                .on_hover_text(if secondary {
                    "Edit the standby colour. Swap colours to paint with it."
                } else {
                    "Primary is used for strokes. Select to edit it with the palette."
                });
            let mut colour = if secondary {
                brush.secondary_colour
            } else {
                brush.material.base_color
            };
            if ui
                .color_edit_button_rgb(&mut colour)
                .on_hover_text(format!("Choose {label} colour for future strokes"))
                .changed()
            {
                brush.set_paint_colour(secondary, colour);
            }
        }
        if ui.button("Swap colours").customize(Action::SwapPaintColours).on_hover_text("Exchange Primary and Secondary, including HDR values. Primary paints; existing surfaces stay unchanged.").clicked() {
            brush.swap_paint_colours();
        }
    }

    pub(super) fn colour_input(
        &mut self,
        ui: &mut egui::Ui,
        brush: &mut Brush,
        library: &mut MaterialLibrary,
    ) {
        ui.horizontal_wrapped(|ui| self.colour_slots(ui, brush));
        ui.weak(format!(
            "Primary paints. Palette edits {}.",
            if self.secondary_edit {
                "Secondary"
            } else {
                "Primary"
            }
        ));
        ui.horizontal_wrapped(|ui| {
            let colour = Material::default().base_color;
            let colour32 = egui::Color32::from(egui::Rgba::from_rgb(colour[0], colour[1], colour[2]));
            if ui.add(egui::Button::new(egui::RichText::new("Default colour").color(egui::Color32::BLACK)).fill(colour32)).on_hover_text("Select the initial teal colour for the highlighted slot. Leaves the other slot and material properties unchanged.").clicked() {
                brush.set_paint_colour(self.secondary_edit, colour);
            }
            if ui.button("Reset colours").customize(Action::ResetPaintColours).on_hover_text("Restore teal Primary and white Secondary. Resets HDR colour overrides; keeps other material properties and existing surfaces.").clicked() {
                brush.reset_paint_colours();
            }
        });
        self.brush_palette(ui, brush, library);
    }

    pub(super) fn sample_control(&mut self, ui: &mut egui::Ui, label: &str) {
        if ui.selectable_label(self.sampling, label)
            .on_hover_text("Click the visible surface to sample these stroke inputs. Does not paint or edit shared materials. Esc cancels.")
            .clicked()
        {
            self.sampling = !self.sampling;
            self.sample_colour_only = false;
            self.sampling_secondary = false;
        }
    }

    /// Ordinary brush colour changes never edit a shared project definition.
    pub(super) fn brush_palette(
        &mut self,
        ui: &mut egui::Ui,
        brush: &mut Brush,
        library: &mut MaterialLibrary,
    ) {
        let was_sampling = self.sampling;
        self.sample_control(ui, "Sample colour");
        if self.sampling && !was_sampling {
            self.sample_colour_only = true;
            self.sampling_secondary = self.secondary_edit;
        }
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
                            .stroke(
                                if (if self.secondary_edit {
                                    brush.secondary_colour
                                } else {
                                    brush.material.base_color
                                }) == colour
                                {
                                    ui.visuals().selection.stroke
                                } else {
                                    egui::Stroke::NONE
                                },
                            ),
                    )
                    .on_hover_text(
                        "Brush colour · existing paint stays unchanged · right-click to remove",
                    );
                if response.clicked() {
                    brush.set_paint_colour(self.secondary_edit, colour);
                }
                response.context_menu(|ui| {
                    if ui.button("Remove colour").clicked() {
                        remove = Some(index);
                        ui.close();
                    }
                });
            }
            let add = ui
                .add_enabled(library.palette.len() < 32, egui::Button::new("+ Colour"))
                .on_hover_text("Choose a colour to add to your personal palette");
            if add.clicked() {
                let colour = if self.secondary_edit {
                    brush.secondary_colour
                } else {
                    brush.material.base_color
                };
                self.palette_draft = (egui::ecolor::Hsva::from_rgb(colour), colour);
            }
            egui::Popup::menu(&add)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.strong("Add palette colour");
                    let (hsva, colour) = &mut self.palette_draft;
                    if egui::color_picker::color_picker_hsva_2d(
                        ui,
                        hsva,
                        egui::color_picker::Alpha::Opaque,
                    ) {
                        *colour = hsva.to_rgb();
                    }
                    if library.palette.contains(colour) {
                        ui.weak("Already in your palette");
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add colour").clicked() {
                            library.add_colour(*colour);
                            brush.set_paint_colour(self.secondary_edit, *colour);
                            ui.close();
                        }
                        if ui.button("Cancel").clicked() {
                            ui.close();
                        }
                    });
                });
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

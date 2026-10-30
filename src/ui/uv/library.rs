//! Browsing an image never changes a model assignment or the paint destination.
use super::*;
use crate::{
    app::textures::TextureCommand as T,
    sculpt::texture::{Channel, Precision, Texture},
};

impl UiState {
    pub(in crate::ui) fn texture_library_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if self.mesh_busy || self.pending.is_some() {
            ui.disable();
        }
        let model = document.model(document.active_model_index()).unwrap();
        let assigned = model
            .mesh
            .uv
            .textures
            .iter()
            .find(|t| t.channel == Channel::Color);
        self.uv.sync_asset_context(document);
        egui::ComboBox::from_label("Assignment model")
            .selected_text(model.name)
            .show_ui(ui, |ui| {
                for i in 0..document.model_count() {
                    if ui
                        .selectable_label(
                            i == document.active_model_index(),
                            document.model(i).unwrap().name,
                        )
                        .clicked()
                    {
                        commands.push(Command::SelectSubmodel(i));
                    }
                }
            });
        ui.label(format!(
            "Assigned image: {}",
            assigned.map_or("None", |t| t.name.as_str())
        ));
        ui.weak(match document.texture_paint {
            Some(Channel::Color) => format!(
                "Paint destination: {} / {}",
                model.name,
                assigned.map_or("Unavailable image", |t| t.name.as_str())
            ),
            _ => format!("Paint destination: {} / vertices", model.name),
        });
        let assets: Vec<_> = document
            .texture_assets()
            .into_iter()
            .filter(|t| t.channel == Channel::Color)
            .collect();
        if self
            .uv
            .asset_selected
            .is_some_and(|id| !assets.iter().any(|t| t.id == id))
        {
            self.uv.asset_selected = assigned.map(|t| t.id);
        }
        self.uv
            .asset_previews
            .retain(|id, _| assets.iter().any(|t| t.id == *id));
        ui.strong("Project images");
        ui.weak("Select to inspect. Assign explicitly to change the model. Images are embedded in the project.");
        if assets.is_empty() {
            ui.label("No colour images yet. Create or import one below.");
        } else {
            let height_id = ui.make_persistent_id("texture_library_height");
            let mut height = ui
                .ctx()
                .data_mut(|d| d.get_persisted::<f32>(height_id).unwrap_or(90.));
            egui::ScrollArea::vertical()
                .id_salt("texture_library_grid")
                .max_height(height)
                .min_scrolled_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let columns = ((ui.available_width() / 100.).floor() as usize).clamp(2, 5);
                    let width = ((ui.available_width()
                        - (columns - 1) as f32 * ui.spacing().item_spacing.x)
                        / columns as f32)
                        .max(48.);
                    egui::Grid::new("texture_assets")
                        .num_columns(columns)
                        .show(ui, |ui| {
                            for (i, texture) in assets.iter().enumerate() {
                                let preview = self.texture_asset_preview(ui.ctx(), texture);
                                let label = if assigned.is_some_and(|t| t.id == texture.id) {
                                    format!("{} · assigned", texture.name)
                                } else {
                                    texture.name.clone()
                                };
                                if design::brush_tile(
                                    ui,
                                    preview,
                                    &label,
                                    self.uv.asset_selected == Some(texture.id),
                                    width,
                                )
                                .clicked()
                                {
                                    self.uv.asset_selected = Some(texture.id);
                                    self.uv.asset_replace = None;
                                    self.uv.asset_delete = None;
                                }
                                if i % columns == columns - 1 {
                                    ui.end_row();
                                }
                            }
                        });
                });
            let response = ui
                .add_sized(
                    [ui.available_width(), 24.],
                    egui::Button::new("Drag to resize images")
                        .frame(false)
                        .sense(egui::Sense::drag()),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeVertical);
            if response.dragged() {
                height = (height + response.drag_delta().y).clamp(80., 500.);
                ui.ctx().data_mut(|d| d.insert_persisted(height_id, height));
            }
        }
        egui::CollapsingHeader::new("Create / import image").default_open(assets.is_empty()).show(ui, |ui| {
            if self.uv.size == 0 { self.uv.size = 512; }
            if self.uv.asset_new_name.is_empty() { self.uv.asset_new_name = "Colour image".into(); }
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut self.uv.asset_new_name).desired_width(ui.available_width()).char_limit(64));
            egui::ComboBox::from_label("Dimensions").selected_text(format!("{} × {} pixels", self.uv.size, self.uv.size)).show_ui(ui, |ui| {
                for size in [256, 512, 1024, 2048, 4096] { ui.selectable_value(&mut self.uv.size, size, format!("{size} × {size}")); }
            });
            ui.checkbox(&mut self.uv.asset_transparent, "Transparent background");
            if !self.uv.asset_transparent { ui.weak("Starts with the selected Paint input preset’s colour."); }
            ui.collapsing("Precision and encoding", |ui| {
                ui.selectable_value(&mut self.uv.precision, Precision::U16, "16-bit colour · PNG");
                ui.selectable_value(&mut self.uv.precision, Precision::Float32, "32-bit float · linear Rec.709 · EXR");
                ui.weak("Float keeps HDR and negative values. Preview is display encoded; source pixels remain unchanged.");
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Create library image").clicked() { commands.push(Command::Texture(T::NewAsset(self.uv.asset_new_name.clone(), self.uv.size, self.uv.precision, self.uv.asset_transparent))); }
                if ui.button("Import PNG / JPEG / EXR…").clicked() { commands.push(Command::BrowseAsset(crate::command::AssetKind::Texture)); }
            });
            ui.weak("Create/import adds an image without changing any model. Generate UVs and assign when ready.");
        });
        let Some(texture) = self
            .uv
            .asset_selected
            .and_then(|id| document.texture_asset(id))
        else {
            return;
        };
        let id = texture.id;
        let users = document.texture_users(id);
        ui.separator();
        ui.strong(format!("Selected image: {}", texture.name));
        ui.label(format!(
            "{} × {} · {}",
            texture.width,
            texture.height,
            if texture.pixels.precision() == Precision::Float32 {
                "32-bit float / linear"
            } else {
                "16-bit colour"
            }
        ));
        ui.weak(if users.is_empty() {
            "Uses: unassigned — ready to assign".into()
        } else {
            format!(
                "Uses ({}): {}. Replacing pixels updates all these models.",
                users.len(),
                users.join(", ")
            )
        });
        ui.collapsing("Inspect selected image", |ui| {
            let preview = self.texture_asset_preview(ui.ctx(), texture);
            let source = egui::vec2(texture.width as f32, texture.height as f32);
            let scale = (ui.available_width().min(320.) / source.x).min(240. / source.y);
            ui.image((preview, source * scale));
            ui.weak("Image pixels only. UV layout belongs to each model, not to this asset.");
        });
        let current = assigned.is_some_and(|t| t.id == id);
        let used_on_model = std::iter::once(model.mesh)
            .chain(&model.settings.lower)
            .chain(&model.settings.upper)
            .any(|mesh| mesh.uv.textures.iter().any(|t| t.id == id));
        let ready = model.mesh.uv.current(model.mesh)
            && (0..model.mesh.uv.corners.len()).any(|f| model.mesh.uv.mapped(f));
        if !ready {
            ui.label("This model needs current UVs before assignment or painting.");
            if ui.button("Generate UVs for this model").clicked() {
                commands.push(Command::UvUnwrap(true, 0.01));
            }
        }
        if !current && assigned.is_some() {
            ui.weak("Assign replaces this model’s image use. The previous image remains in the library.");
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    ready && !current,
                    egui::Button::new("Assign selected to model"),
                )
                .clicked()
            {
                commands.push(Command::Texture(T::AssignAsset(id)));
            }
            if ui
                .add_enabled(ready && current, egui::Button::new("Paint assigned image"))
                .clicked()
            {
                commands.push(Command::Texture(T::Paint(Some(Channel::Color))));
            }
            if ui
                .add_enabled(used_on_model, egui::Button::new("Unassign selected image"))
                .clicked()
            {
                commands.push(Command::Texture(T::UnassignAsset(id)));
            }
        });
        if !current {
            ui.weak(
                "Assign this image first to make it available as this model’s paint destination.",
            );
        }
        if self.uv.asset_name_id != Some(id) || self.uv.asset_name_source != texture.name {
            self.uv.asset_name_id = Some(id);
            self.uv.asset_name = texture.name.clone();
            self.uv.asset_name_source = texture.name.clone();
            self.uv.asset_export_path = if texture.pixels.precision() == Precision::Float32 {
                "texture.exr"
            } else {
                "texture.png"
            }
            .into();
        }
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.uv.asset_name)
                    .desired_width(150.)
                    .char_limit(64),
            );
            if ui.button("Rename image").clicked() {
                commands.push(Command::Texture(T::RenameAsset(
                    id,
                    self.uv.asset_name.clone(),
                )));
            }
            if ui.button("Duplicate image").clicked() {
                commands.push(Command::Texture(T::DuplicateAsset(id)));
            }
        });
        ui.weak("Duplicate creates an unassigned independent copy; it does not change any model.");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Replace image pixels…").clicked() {
                self.uv.asset_replace = Some(id);
            }
            if ui
                .add_enabled(users.is_empty(), egui::Button::new("Delete unused image…"))
                .on_hover_text("Unassign every model first. Deletion is undoable.")
                .clicked()
            {
                self.uv.asset_delete = Some(id);
            }
        });
        if self.uv.asset_replace == Some(id) {
            ui.label(format!("Replace ‘{}’ and {} model uses? This changes shared pixels and disconnects the external working file. Undo restores both.", texture.name, users.len()));
            ui.horizontal_wrapped(|ui| {
                if ui.button("Choose replacement file…").clicked() {
                    commands.push(Command::BrowseAsset(
                        crate::command::AssetKind::ReplaceTexture(id),
                    ));
                    self.uv.asset_replace = None;
                }
                if ui.button("Keep current pixels").clicked() {
                    self.uv.asset_replace = None;
                }
            });
        }
        if self.uv.asset_delete == Some(id) {
            ui.label("Delete this unused image from the project? Undo restores its pixels.");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Delete unused image").clicked() {
                    commands.push(Command::Texture(T::DeleteAsset(id)));
                    self.uv.asset_delete = None;
                }
                if ui.button("Keep image").clicked() {
                    self.uv.asset_delete = None;
                }
            });
        }
        ui.collapsing("Export selected image", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.uv.asset_export_path).desired_width(ui.available_width()));
            ui.weak("PNG: 16-bit display colour. EXR: linear float. Use EXR to retain HDR/negative samples.");
            if ui.button("Export selected pixels").clicked() { commands.push(Command::Texture(T::ExportAsset(id, self.uv.asset_export_path.clone(), false))); }
            if let Some((pending_id, path)) = self.uv.asset_export_pending.clone() {
                ui.label(format!("Replace existing file {path}?"));
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Overwrite export").clicked() { commands.push(Command::Texture(T::ExportAsset(pending_id, path, true))); }
                    if ui.button("Cancel export").clicked() { self.uv.asset_export_pending = None; }
                });
            }
        });
    }

    fn texture_asset_preview(&mut self, ctx: &egui::Context, texture: &Texture) -> egui::TextureId {
        let key = (texture.pixels.storage_id(), texture.upload.revision());
        if self
            .uv
            .asset_previews
            .get(&texture.id)
            .is_none_or(|(old, _)| *old != key)
        {
            let width = texture.width.min(320) as usize;
            let height = texture.height.min(320) as usize;
            let mut rgba = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let source = (y * texture.height as usize / height) * texture.width as usize
                        + x * texture.width as usize / width;
                    let mut p = texture.pixels.pixel(source);
                    if texture.pixels.precision() == Precision::Float32 {
                        for v in &mut p[..3] {
                            *v = crate::sculpt::material::linear_to_srgb(*v);
                        }
                    }
                    rgba.extend(p.map(|v| (v.clamp(0., 1.) * 255.).round() as u8));
                }
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);
            if let Some((old, handle)) = self.uv.asset_previews.get_mut(&texture.id) {
                handle.set(image, egui::TextureOptions::LINEAR);
                *old = key;
            } else {
                self.uv.asset_previews.insert(
                    texture.id,
                    (
                        key,
                        ctx.load_texture(
                            format!("texture-asset-{}", texture.id),
                            image,
                            egui::TextureOptions::LINEAR,
                        ),
                    ),
                );
            }
        }
        self.uv.asset_previews[&texture.id].1.id()
    }
}

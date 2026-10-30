use super::*;
mod interaction;
mod library;
#[cfg(test)]
mod tests;
#[derive(Default)]
pub(crate) struct UvControls {
    pub asset_selected: Option<u64>,
    pub asset_export_pending: Option<(u64, String)>,
    asset_export_path: String,
    asset_model: Option<(usize, u64)>,
    asset_assignment: Option<(usize, u64, Option<u64>)>,
    asset_name: String,
    asset_name_id: Option<u64>,
    asset_name_source: String,
    asset_new_name: String,
    asset_transparent: bool,
    asset_delete: Option<u64>,
    asset_replace: Option<u64>,
    asset_previews: std::collections::HashMap<u64, ((usize, u64), egui::TextureHandle)>,
    islands: Vec<Vec<usize>>,
    expanded: bool,
    status_at_expand: String,
    tool: interaction::Tool,
    radius: f32,
    drag: Option<interaction::Drag>,
    report: crate::sculpt::uv::Report,
    stretch: bool,
    pub texture_tab: bool,
    pub channel: crate::sculpt::texture::Channel,
    pub image_path: String,
    pub export_overwrite: Option<String>,
    pub reload_conflict: bool,
    pub exchange_busy: bool,
    pub operation: String,
    pub operation_progress: Option<f32>,
    pub completed: bool,
    pub retry: Option<crate::app::textures::TextureCommand>,
    pub initial: crate::app::textures::TextureStart,
    pub bake_override: bool,
    pub working_format: crate::app::textures::WorkingFormat,
    pub working_path: String,
    bake_replace: Option<bool>,
    image_preview: Option<((u64, u64, u32, bool), egui::TextureHandle)>,
    geometry_hash: Option<u64>,
    size: u32,
    precision: crate::sculpt::texture::Precision,
    pub(super) export_path: String,
    pub open: bool,
    pub checker: bool,
    pub exposure: f32,
    pub selected: Vec<usize>,
    padding: f32,
    offset: [f32; 2],
    scale: f32,
    angle: f32,
    revision: Option<u64>,
}
impl UvControls {
    fn sync_asset_context(&mut self, document: &Document) {
        let model = document.model(document.active_model_index()).unwrap();
        let assigned = model
            .mesh
            .uv
            .textures
            .iter()
            .find(|t| t.channel == crate::sculpt::texture::Channel::Color);
        let model_key = (document.active_model_index(), model.metadata.model_id);
        let assignment_key = (model_key.0, model_key.1, assigned.map(|t| t.id));
        if self.asset_assignment != Some(assignment_key) {
            self.asset_assignment = Some(assignment_key);
            self.reload_conflict = false;
            self.bake_replace = None;
            self.export_overwrite = None;
        }
        if self.asset_model != Some(model_key) {
            if self.asset_model.is_some() || self.asset_selected.is_none() {
                self.asset_selected = assigned.map(|t| t.id);
            }
            self.asset_model = Some(model_key);
            self.asset_replace = None;
            self.asset_delete = None;
        }
    }
    pub(crate) fn select_asset(&mut self, document: &Document, id: u64) {
        self.asset_selected = Some(id);
        self.asset_model = Some((
            document.active_model_index(),
            document
                .model(document.active_model_index())
                .unwrap()
                .metadata
                .model_id,
        ));
        self.asset_replace = None;
        self.asset_delete = None;
    }
}
impl UiState {
    pub(crate) fn uv_window(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        model_faces: &[usize],
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        if !self.uv.open {
            return commands;
        }
        self.uv.sync_asset_context(document);
        // Colour is the editable image workflow; retained scalar assets still render/export.
        if self.uv.channel != crate::sculpt::texture::Channel::Color {
            self.uv.channel = crate::sculpt::texture::Channel::Color;
            self.uv.reload_conflict = false;
            self.uv.export_overwrite = None;
            self.uv.bake_replace = None;
        }
        if self.uv.revision.is_none() {
            self.uv.padding = 0.01;
            self.uv.export_path = "texture.png".into();
        }
        let mesh = document.mesh();
        let uv = &mesh.uv;
        let current = uv.current(mesh);
        if self.uv.revision != Some(document.display_revision()) {
            self.uv.drag = None;
            self.uv.selected.retain(|&f| uv.mapped(f));
            self.uv.revision = Some(document.display_revision());
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            for p in &mesh.vertices {
                [p.0.to_bits(), p.1.to_bits(), p.2.to_bits()].hash(&mut hash);
            }
            mesh.triangles.hash(&mut hash);
            current.hash(&mut hash);
            uv.removed.hash(&mut hash);
            for q in uv.corners.iter().flatten() {
                q.map(f32::to_bits).hash(&mut hash);
            }
            let hash = hash.finish();
            if self.uv.geometry_hash != Some(hash) {
                self.uv.islands = if current { uv.islands() } else { Vec::new() };
                self.uv.report = uv.inspect(mesh);
                self.uv.geometry_hash = Some(hash);
            }
        }
        if self.uv.expanded {
            self.uv_workspace(ctx, document, model_faces, &mut commands);
            return commands;
        }
        let mut open = true;
        egui::Window::new(if self.uv.texture_tab {
            "Texture library"
        } else {
            "UV layout"
        })
        .id(egui::Id::new("uv_editor"))
        .open(&mut open)
        .default_width(480.)
        .default_pos(egui::pos2(20., 110.))
        .default_height(800.)
        .max_height((ctx.content_rect().height() - 140.).max(200.))
        .vscroll(true)
        .show(ctx, |ui| {
            self.uv_contents(ui, document, model_faces, &mut commands)
        });
        if !open {
            self.uv.drag = None;
        }
        self.uv.open = open;
        commands
    }
    fn uv_contents(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        model_faces: &[usize],
        commands: &mut Vec<Command>,
    ) {
        if self.mesh_busy || self.pending.is_some() {
            ui.disable();
        }
        self.uv_header(ui);
        if self.uv.texture_tab {
            ui.scope(|ui| self.texture_controls(ui, document, commands));
        }
        if self.uv.texture_tab {
            ui.separator();
            ui.strong("Assigned image / UV preview");
        }
        ui.add_enabled_ui(!self.mesh_busy && self.pending.is_none(), |ui| {
            self.uv_layout(ui, document, model_faces, commands)
        });
    }
    fn uv_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.uv.texture_tab, false, "UV layout");
            ui.selectable_value(&mut self.uv.texture_tab, true, "Texture library");
            if ui
                .button(if self.uv.expanded {
                    "Restore window"
                } else {
                    "Expand editor"
                })
                .clicked()
            {
                self.uv.expanded = !self.uv.expanded;
                self.uv.status_at_expand = self.status.clone();
                self.uv.drag = None;
            }
        });
    }
    fn uv_workspace(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        model_faces: &[usize],
        commands: &mut Vec<Command>,
    ) {
        let bounds = ctx.content_rect();
        let id = egui::Id::new("uv_workspace");
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
        egui::Area::new(id)
            .order(egui::Order::Middle)
            .fixed_pos(bounds.min)
            .movable(false)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_min_size(bounds.size());
                ui.set_max_size(bounds.size());
                ui.set_clip_rect(bounds);
                ui.painter().rect_filled(bounds, 0., Color32::from_gray(18));
                let sidebar_width = 300_f32.min(bounds.width() * 0.42);
                let split = bounds.right() - sidebar_width;
                let sidebar = egui::Rect::from_min_max(egui::pos2(split, bounds.top()), bounds.max);
                ui.painter()
                    .rect_filled(sidebar, 0., ui.visuals().panel_fill);
                let mut controls = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("uv_sidebar")
                        .max_rect(sidebar.shrink(12.)),
                );
                controls.set_clip_rect(sidebar);
                controls.heading("UVs and textures");
                self.uv_header(&mut controls);
                if controls.button("Close editor").clicked() {
                    self.uv.open = false;
                    self.uv.drag = None;
                }
                if self.mesh_busy && controls.button("Cancel operation").clicked() {
                    commands.push(Command::CancelMeshJob);
                }
                controls.separator();
                egui::ScrollArea::vertical()
                    .id_salt("uv_sidebar_scroll")
                    .auto_shrink([false, false])
                    .show(&mut controls, |ui| {
                        ui.set_max_width(sidebar_width - 24.);
                        if self.mesh_busy || self.pending.is_some() {
                            ui.disable();
                        }
                        if self.uv.texture_tab {
                            self.texture_controls(ui, document, commands);
                        }
                        self.uv_unwrap_controls(ui, document, commands);
                        ui.add_enabled_ui(document.mesh().uv.current(document.mesh()), |ui| {
                            self.uv_view_controls(ui, document, model_faces, commands);
                            if !self.uv.texture_tab {
                                self.uv_transform_controls(ui, document, commands);
                            }
                        });
                        if !self.status.is_empty() && self.status != self.uv.status_at_expand {
                            ui.separator();
                            ui.label(&self.status);
                        }
                    });
                let viewport =
                    egui::Rect::from_min_max(bounds.min, egui::pos2(split, bounds.bottom()));
                let mut canvas = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("uv_workspace_canvas")
                        .max_rect(viewport),
                );
                canvas.set_clip_rect(viewport);
                canvas.add_enabled_ui(
                    !self.mesh_busy
                        && self.pending.is_none()
                        && document.mesh().uv.current(document.mesh()),
                    |ui| {
                        self.uv_canvas(ui, document, commands, Some(viewport));
                    },
                );
            });
    }
    fn texture_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        egui::CollapsingHeader::new("Project image library")
            .default_open(true)
            .show(ui, |ui| {
                self.texture_library_controls(ui, document, commands);
            });
        let uv = &document.mesh().uv;

        use crate::app::textures::TextureCommand as T;
        use crate::app::textures::{TextureStart, WorkingFormat};
        use crate::sculpt::texture::{Channel, Precision};
        let ready = uv.current(document.mesh()) && (0..uv.corners.len()).any(|f| uv.mapped(f));
        if !ready {
            ui.label("Start texture painting: this model needs current UVs.");
            if ui.button("Generate UVs for painting").clicked() {
                commands.push(Command::UvUnwrap(true, 0.01));
            }
            ui.weak("Then choose a starting image and Create & paint on model.");
        }
        if self.uv.exchange_busy {
            ui.label(&self.uv.operation);
            if let Some(progress) = self.uv.operation_progress {
                ui.add(egui::ProgressBar::new(progress).show_percentage());
            } else {
                ui.spinner();
            }
            if ui.button("Cancel texture operation").clicked() {
                commands.push(Command::Texture(T::Cancel));
            }
        } else if let Some(retry) = self.uv.retry.clone() {
            ui.colored_label(Color32::YELLOW, &self.uv.operation);
            if ui.button("Retry texture operation").clicked() {
                commands.push(Command::Texture(retry));
            }
        }
        ui.separator();
        ui.strong("Active model image workflow");
        if self.uv.size == 0 {
            self.uv.size = 512;
        }
        let has_image = uv.textures.iter().any(|t| t.channel == self.uv.channel);
        egui::CollapsingHeader::new("Create from model surface")
            .id_salt(("new_texture", has_image))
            .default_open(!has_image)
            .show(ui, |ui| {
                ui.label("Create an image and assign it to the active model");
                egui::ComboBox::from_id_salt("texture_start")
                    .selected_text(match self.uv.initial {
                        TextureStart::Surface => "Use existing surface / vertex colour",
                        TextureStart::Transparent => "Blank transparent",
                        TextureStart::Solid => "Solid brush colour",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.uv.initial,
                            TextureStart::Surface,
                            "Use existing surface / vertex colour",
                        );
                        ui.selectable_value(
                            &mut self.uv.initial,
                            TextureStart::Transparent,
                            "Blank transparent",
                        );
                        ui.selectable_value(
                            &mut self.uv.initial,
                            TextureStart::Solid,
                            "Solid brush colour",
                        );
                    });
                let can_create = ready
                    && !self.uv.exchange_busy
                    && !uv.textures.iter().any(|t| t.channel == self.uv.channel);
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("texture_resolution")
                        .selected_text(format!("{} × {}", self.uv.size, self.uv.size))
                        .show_ui(ui, |ui| {
                            for size in [256, 512, 1024, 2048, 4096] {
                                ui.selectable_value(&mut self.uv.size, size, size.to_string());
                            }
                        });
                    use crate::sculpt::texture::Precision;
                    egui::ComboBox::from_id_salt("texture_precision")
                        .selected_text(if self.uv.precision == Precision::U16 {
                            "16-bit integer"
                        } else {
                            "32-bit float"
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.uv.precision,
                                Precision::U16,
                                "16-bit integer",
                            );
                            ui.selectable_value(
                                &mut self.uv.precision,
                                Precision::Float32,
                                "32-bit float",
                            );
                        });
                    if ui
                        .add_enabled(can_create, egui::Button::new("Create and assign image"))
                        .clicked()
                    {
                        commands.push(Command::Texture(T::Start(
                            self.uv.channel,
                            self.uv.size,
                            self.uv.precision,
                            self.uv.initial,
                            false,
                        )));
                    }
                    if ui
                        .add_enabled(can_create, egui::Button::new("Create & paint on model"))
                        .clicked()
                    {
                        commands.push(Command::Texture(T::CreateAndPaint(
                            self.uv.channel,
                            self.uv.size,
                            self.uv.precision,
                        )));
                    }
                });
            });
        ui.separator();
        ui.collapsing("Capture colour to texture", |ui| {
            ui.checkbox(&mut self.uv.bake_override, "Override colour image size and precision");
            ui.weak(if self.uv.bake_override {
                "Capture uses the size and precision in Create / import image."
            } else {
                "Capture keeps the current colour image's size and precision. New images use Create / import image settings."
            });
            if ui.add_enabled(ready && !self.uv.exchange_busy, egui::Button::new("Capture vertex colour")).clicked() {
                if uv.textures.iter().any(|texture| texture.channel == Channel::Color) {
                    self.uv.bake_replace = Some(false);
                } else {
                    commands.push(Command::Texture(T::Bake(false, self.uv.size, self.uv.precision)));
                }
            }
            ui.weak("Copies vertex paint into the colour image without lighting or shading.");
            if self.uv.bake_replace.is_some() {
                ui.label("Replace the colour image with captured vertex paint? One Undo restores it.");
                let old = uv.textures.iter().find(|texture| texture.channel == Channel::Color);
                let (width, height, precision) = old.filter(|_| !self.uv.bake_override)
                    .map_or((self.uv.size, self.uv.size, self.uv.precision), |texture| (texture.width, texture.height, texture.pixels.precision()));
                let describe = |width, height, precision| format!("{width} × {height}, {}", if precision == Precision::Float32 { "32-bit float" } else { "16-bit integer" });
                let before = old.map_or("new image".into(), |texture| describe(texture.width, texture.height, texture.pixels.precision()));
                ui.label(format!("Base colour: {before} → {}", describe(width, height, precision)));
                if let Some(texture) = old {
                    let users = (0..document.model_count()).filter(|&index| document.model(index).is_some_and(|model| model.mesh.uv.textures.iter().any(|image| image.id == texture.id))).count();
                    if users > 1 { ui.colored_label(Color32::YELLOW, format!("Updates {users} linked models")); }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!self.uv.exchange_busy, egui::Button::new("Replace colour image")).clicked() {
                        commands.push(Command::Texture(T::Bake(false, self.uv.size, self.uv.precision)));
                        self.uv.bake_replace = None;
                    }
                    if ui.button("Cancel capture").clicked() { self.uv.bake_replace = None; }
                });
            }
        });
        if let Some(texture) = uv.textures.iter().find(|t| t.channel == self.uv.channel) {
            if self.uv.export_path == "texture.png"
                && texture.pixels.precision() == crate::sculpt::texture::Precision::Float32
            {
                self.uv.export_path = "texture.exr".into();
            }

            ui.label(format!(
                "{} · {} × {} · {}",
                texture.name,
                texture.width,
                texture.height,
                if texture.pixels.precision() == crate::sculpt::texture::Precision::Float32 {
                    "32-bit float colour"
                } else {
                    "16-bit integer colour"
                }
            ));
            ui.horizontal_wrapped(|ui| {
                if ui
                    .selectable_label(
                        document.texture_paint == Some(self.uv.channel),
                        "Paint on model",
                    )
                    .clicked()
                {
                    commands.push(Command::Texture(T::Paint(Some(self.uv.channel))));
                }
                if ui.button("Paint vertices").clicked() {
                    commands.push(Command::Texture(T::Paint(None)));
                }
                if ui.button("Make independent").clicked() {
                    commands.push(Command::Texture(T::Copy(self.uv.channel)));
                }
                if ui.button("Unassign from model").clicked() {
                    commands.push(Command::Texture(T::Remove(self.uv.channel)));
                }
            });
            ui.label("Unassign keeps the pixels in the library. Make independent copies this model’s image; shared models keep the original. Overlapping UVs share pixels.");
            ui.weak("Paint on model returns to the 3D view. Strokes there update this image; this canvas previews the texture.");
            ui.horizontal_wrapped(|ui| {
                ui.label("PNG / EXR export");
                ui.add(egui::TextEdit::singleline(&mut self.uv.export_path).desired_width(150.));
                if ui.button("Export image").clicked() {
                    commands.push(Command::Texture(T::Export(
                        self.uv.channel,
                        self.uv.export_path.clone(),
                        false,
                    )));
                }
            });
            if let Some(path) = self.uv.export_overwrite.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Replace {path}?"));
                    if ui.button("Replace image").clicked() {
                        commands.push(Command::Texture(T::Export(self.uv.channel, path, true)));
                    }
                    if ui.button("Keep existing file").clicked() {
                        self.uv.export_overwrite = None;
                    }
                });
            }
            ui.collapsing("External painting", |ui| {
            ui.label(format!("Assigned image: {}", texture.name));
            let users = document.texture_users(texture.id);
            ui.weak(format!("Reload updates {} shared model uses: {}", users.len(), users.join(", ")));
            egui::ComboBox::from_id_salt("paint_working_format")
                .selected_text(match self.uv.working_format { WorkingFormat::Auto => "Automatic working format", WorkingFormat::Psd => "PSD", WorkingFormat::Xcf => "XCF (GIMP)" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.uv.working_format, WorkingFormat::Auto, "Automatic working format");
                    ui.selectable_value(&mut self.uv.working_format, WorkingFormat::Psd, "PSD");
                    ui.selectable_value(&mut self.uv.working_format, WorkingFormat::Xcf, "XCF (GIMP)");
                });
            ui.weak("Automatic uses XCF for float textures in GIMP; otherwise PSD. PNG and EXR export are separate from working format.");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.uv.exchange_busy,
                        egui::Button::new(if texture.exchange.is_some() { "Open working document" } else { "Edit in external paint app" }),
                    )
                    .clicked()
                {
                    commands.push(Command::Texture(T::EditExternal(self.uv.channel)));
                }
                if ui
                    .add_enabled(
                        texture.exchange.is_some() && !self.uv.exchange_busy,
                        egui::Button::new("Reload external image"),
                    )
                    .clicked()
                {
                    commands.push(Command::Texture(T::Reload(self.uv.channel, false)));
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!self.uv.exchange_busy, egui::Button::new("New snapshot")).clicked() {
                    commands.push(Command::Texture(T::NewSnapshot(self.uv.channel)));
                }
                if ui.add_enabled(!self.uv.exchange_busy, egui::Button::new("Choose working file…")).clicked() {
                    commands.push(Command::BrowseAsset(crate::command::AssetKind::PaintWorking));
                }
            });
            if self.uv.reload_conflict {
                ui.colored_label(
                    Color32::YELLOW,
                    "Local paint changed after export. Reload would replace it.",
                );
                ui.horizontal(|ui| {
                    if ui.button("Replace local paint").clicked() {
                        commands.push(Command::Texture(T::Reload(self.uv.channel, true)));
                    }
                    if ui.button("Keep local paint").clicked() {
                        self.uv.reload_conflict = false;
                    }
                });
            }
            if let Some(exchange) = &texture.exchange {
                if let Some(path) = &exchange.layered_path {
                    ui.label(format!("Working document: {path}"));
                    ui.label("Save back to this file, then Reload here. For PSD in GIMP use Export; for XCF use Save. Keep the UV guide name and separate layer; it is excluded even when visible.");
                } else {
                    ui.label(format!("Exchange image: {}", exchange.path));
                    ui.label("Export back to this PNG or EXR before Reload. Use Edit in external paint app again to create a layered copy with a UV guide.");
                }
            }
            ui.label("Layered reload uses GIMP 3 with Python. Reloaded pixels are embedded in the project; the editable working document stays at its linked path.");
            if texture.pixels.precision() == crate::sculpt::texture::Precision::Float32 {
                ui.weak("32-bit float: use XCF with GIMP or float PSD with a compatible editor. EXR also retains float precision and can be relinked for reload.");
            }
        ui.horizontal(|ui| {
            ui.label("External paint app");
            ui.add(
                egui::TextEdit::singleline(&mut self.preferences.external_paint_app)
                    .desired_width(130.)
                    .hint_text("gimp"),
            );
        });
        ui.collapsing("Layered reader settings", |ui| {
            ui.label("GIMP 3 reader executable");
            ui.add(
                egui::TextEdit::singleline(&mut self.preferences.gimp_path)
                    .desired_width(130.)
                    .hint_text("gimp"),
            );
        });
            });
            if self.uv.completed {
                ui.label("Colour texture ready. One Undo restores the previous image.");
                ui.weak(
                    "Continue with Paint on model, inspect the image here, or Export image above.",
                );
            }
        }
    }

    fn uv_unwrap_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let mesh = document.mesh();
        let uv = &mesh.uv;
        let current = uv.current(mesh);

        if !self.uv.texture_tab {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Automatic unwrap").clicked() {
                    commands.push(Command::UvUnwrap(true, self.uv.padding));
                }
                if ui.button("Unwrap marked seams").clicked() {
                    commands.push(Command::UvUnwrap(false, self.uv.padding));
                }
                if ui.button("Mark selected edges").clicked() {
                    commands.push(Command::UvSeams(true));
                }
                if ui.button("Clear selected seams").clicked() {
                    commands.push(Command::UvSeams(false));
                }
            });
            ui.label("Select edges in Modeling to define seams. Unwrap runs in the background and can be cancelled.");
        }
        if !current {
            ui.colored_label(Color32::YELLOW,if uv.corners.is_empty(){"Generate UVs to begin, or import a UV-mapped OBJ."}else{"Topology changed: these UVs are inactive. Undo the topology edit or unwrap again."});
        }
    }
    fn uv_layout(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        model_faces: &[usize],
        commands: &mut Vec<Command>,
    ) {
        self.uv_unwrap_controls(ui, document, commands);
        ui.add_enabled_ui(document.mesh().uv.current(document.mesh()), |ui| {
            self.uv_view_controls(ui, document, model_faces, commands);
            self.uv_canvas(ui, document, commands, None);
            if !self.uv.texture_tab {
                self.uv_transform_controls(ui, document, commands);
            }
        });
    }
    fn uv_view_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        model_faces: &[usize],
        commands: &mut Vec<Command>,
    ) {
        if self.uv.expanded {
            ui.label("Preview exposure (EV)");
        }
        let exposure = egui::Slider::new(&mut self.uv.exposure, -12. ..=12.);
        ui.add(if self.uv.expanded {
            exposure
        } else {
            exposure.text("Preview exposure (EV)")
        })
        .on_hover_text(
            "View adjustment only; exported pixels are unchanged. Useful for HDR EXR images.",
        );
        let mesh = document.mesh();
        let uv = &mesh.uv;

        if !self.uv.texture_tab {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.uv.checker, "3D checker");
                ui.checkbox(&mut self.uv.stretch, "UV stretch");
                ui.add(
                    egui::DragValue::new(&mut self.uv.padding)
                        .range(0.001..=0.1)
                        .speed(0.001)
                        .prefix("Padding "),
                );
                if ui.button("Auto arrange all").clicked() {
                    commands.push(Command::UvPack(self.uv.padding));
                }
                if ui.add_enabled(!self.uv.selected.is_empty(),egui::Button::new("Arrange selected"))
                    .on_hover_text("Straighten and pack selected islands around the others. Masked islands stay fixed.").clicked() {
                    commands.push(Command::UvPackSelected(self.uv.selected.clone(),self.uv.padding));
                }
                if ui.button("Select all").clicked() {
                    self.uv.selected = (0..uv.corners.len()).filter(|&f| uv.mapped(f)).collect();
                    commands.push(Command::UvSelect(self.uv.selected.clone()));
                }
                if ui.button("From 3D selection").clicked() {
                    let polygons = crate::sculpt::uv::polygon_triangles(mesh);
                    let selected: std::collections::BTreeSet<_> = model_faces
                        .iter()
                        .filter_map(|&f| polygons.get(f))
                        .flatten()
                        .copied()
                        .collect();
                    self.uv.selected = self
                        .uv
                        .islands
                        .iter()
                        .filter(|fs| fs.iter().any(|f| selected.contains(f)))
                        .flatten()
                        .copied()
                        .collect();
                }
            });
        }
        if !self.uv.texture_tab {
            self.uv_tools(ui, commands);
        }
    }
    fn uv_canvas(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
        viewport: Option<egui::Rect>,
    ) {
        let ctx = ui.ctx().clone();
        let mesh = document.mesh();
        let uv = &mesh.uv;
        let current = uv.current(mesh);
        let (rect, response) = if let Some(viewport) = viewport {
            let size = (viewport.width().min(viewport.height()) - 32.).max(1.);
            let rect = egui::Rect::from_center_size(viewport.center(), Vec2::splat(size));
            (rect, ui.allocate_rect(rect, Sense::click_and_drag()))
        } else {
            let size = ui
                .available_width()
                .clamp(180., if self.uv.texture_tab { 320. } else { 500. });
            ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag())
        };
        let size = rect.width();
        let consumed = if !self.uv.texture_tab && current {
            self.uv_interaction(ui, &response, uv, commands)
        } else {
            false
        };
        let uv = self.uv.drag.as_ref().map_or(uv, |d| &d.preview);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0., Color32::from_gray(24));
        let map = |q: [f32; 2]| egui::pos2(rect.left() + q[0] * size, rect.bottom() - q[1] * size);
        for y in 0..16 {
            for x in 0..16 {
                if (x + y) % 2 == 0 {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            map([x as f32 / 16., (y + 1) as f32 / 16.]),
                            map([(x + 1) as f32 / 16., y as f32 / 16.]),
                        ),
                        0.,
                        Color32::from_gray(38),
                    );
                }
            }
        }
        if let Some(texture) = uv.textures.iter().find(|t| t.channel == self.uv.channel) {
            let id = (
                texture.id,
                document.display_revision(),
                self.uv.exposure.to_bits(),
                self.uv.expanded,
            );
            if self
                .uv
                .image_preview
                .as_ref()
                .is_none_or(|(old, _)| *old != id)
            {
                // Bound display memory independently of the source image resolution.
                let limit = if self.uv.expanded { 1024 } else { 512 };
                let width = texture.width.min(limit) as usize;
                let height = texture.height.min(limit) as usize;
                let mut rgba = Vec::with_capacity(width * height * 4);
                for y in 0..height {
                    for x in 0..width {
                        let source = (y * texture.height as usize / height)
                            * texture.width as usize
                            + x * texture.width as usize / width;
                        let mut p = texture.pixels.pixel(source);
                        if texture.channel == crate::sculpt::texture::Channel::Color
                            && texture.pixels.precision()
                                == crate::sculpt::texture::Precision::Float32
                        {
                            for v in &mut p[..3] {
                                *v = crate::sculpt::material::linear_to_srgb(
                                    *v * self.uv.exposure.exp2(),
                                );
                            }
                        }
                        rgba.extend(p.map(|v| (v.clamp(0., 1.) * 255.).round() as u8));
                    }
                }
                self.uv.image_preview = Some((
                    id,
                    ctx.load_texture(
                        "uv_texture_image",
                        egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba),
                        egui::TextureOptions::LINEAR,
                    ),
                ));
            }
            if let Some((_, image)) = &self.uv.image_preview {
                painter.image(
                    image.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                    Color32::WHITE,
                );
            }
        } else {
            self.uv.image_preview = None;
        }
        let mut selection = vec![false; uv.corners.len()];
        for &f in &self.uv.selected {
            if let Some(selected) = selection.get_mut(f) {
                *selected = true;
            }
        }
        let mask_shape = painter.add(egui::Shape::Noop);
        let mut overlay = egui::Mesh::default();
        for (f, t) in uv
            .corners
            .iter()
            .enumerate()
            .filter(|(f, _)| current && uv.mapped(*f))
        {
            let color = if selection[f] {
                Color32::from_rgb(255, 185, 70)
            } else if crate::sculpt::uv::area(*t).abs() < 1e-10 {
                Color32::RED
            } else if self.uv.stretch {
                let amount = ((self.uv.report.stretch[f] - 1.) / 3.).clamp(0., 1.);
                Color32::from_rgb((amount * 255.) as u8, ((1. - amount) * 180.) as u8, 90)
            } else {
                Color32::from_rgb(115, 185, 215)
            };
            if !self.uv.texture_tab && (0..3).any(|c| uv.mask(f, c) > 0.) {
                let first = overlay.vertices.len() as u32;
                for (c, &q) in t.iter().enumerate() {
                    overlay.colored_vertex(
                        map(q),
                        Color32::from_rgba_unmultiplied(170, 75, 220, (uv.mask(f, c) * 180.) as u8),
                    );
                }
                overlay.add_triangle(first, first + 1, first + 2);
            }
            for c in 0..3 {
                painter.line_segment([map(t[c]), map(t[(c + 1) % 3])], (1., color));
            }
        }
        if !overlay.indices.is_empty() {
            painter.set(mask_shape, egui::Shape::mesh(overlay));
        }
        if !self.uv.texture_tab {
            self.uv_paint_handles(ui, &response, uv);
        }
        if !self.uv.texture_tab
            && !consumed
            && !self.uv.tool.is_mask()
            && response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let q = [(p.x - rect.left()) / size, (rect.bottom() - p.y) / size];
            let face = uv.corners.iter().enumerate().position(|(f, t)| {
                if !uv.mapped(f) {
                    return false;
                }
                let a = crate::sculpt::uv::area(*t);
                if a.abs() < 1e-12 {
                    return false;
                }
                let weights = [
                    crate::sculpt::uv::area([q, t[1], t[2]]) / a,
                    crate::sculpt::uv::area([t[0], q, t[2]]) / a,
                    crate::sculpt::uv::area([t[0], t[1], q]) / a,
                ];
                weights.iter().all(|v| *v >= -1e-5)
            });
            if !ui.input(|i| i.modifiers.shift) {
                self.uv.selected.clear();
            }
            if let Some(f) = face
                && let Some(island) = self.uv.islands.iter().find(|fs| fs.contains(&f))
            {
                self.uv.selected.extend(island.iter().copied());
                self.uv.selected.sort_unstable();
                self.uv.selected.dedup();
            }
            commands.push(Command::UvSelect(self.uv.selected.clone()));
        }
    }
    fn uv_transform_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let uv = &document.mesh().uv;
        if !self.uv.texture_tab {
            ui.label(
                "Click an island; Shift-click adds. Drag the handle to transform · Esc cancels.",
            );
            ui.horizontal_wrapped(|ui| {
                for k in 0..2 {
                    ui.add(
                        egui::DragValue::new(&mut self.uv.offset[k])
                            .speed(0.01)
                            .prefix(if k == 0 { "U " } else { "V " }),
                    );
                }
                if self.uv.scale == 0. {
                    self.uv.scale = 1.;
                }
                ui.add(
                    egui::DragValue::new(&mut self.uv.scale)
                        .range(0.001..=100.)
                        .speed(0.01)
                        .prefix("Scale "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.uv.angle)
                        .speed(1.)
                        .prefix("Rotate ° "),
                );
                if ui.button("Apply transform").clicked() {
                    commands.push(Command::UvTransform(
                        self.uv.selected.clone(),
                        self.uv.offset,
                        self.uv.scale,
                        self.uv.angle.to_radians(),
                    ));
                    self.uv.offset = [0.; 2];
                    self.uv.scale = 1.;
                    self.uv.angle = 0.;
                }
            });
            ui.label(format!(
                "{} degenerate triangles · {} overlapping pairs · {} outside 0–1{}",
                self.uv.report.degenerate,
                self.uv.report.overlaps,
                self.uv.report.outside,
                if self.uv.report.complete {
                    ""
                } else {
                    " · overlap check reached its limit"
                }
            ));
            ui.label(format!(
                "{} islands · {} seams · UV changes are undoable",
                self.uv.islands.len(),
                uv.seams.len()
            ));
        }
    }
}

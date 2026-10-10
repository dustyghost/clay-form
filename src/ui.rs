//! Immediate-mode UI. Widgets emit commands; they never perform IO or sculpt edits.
use crate::{
    command::{Command, PendingAction},
    document::Document,
    preferences::{Action, Dock, Hold, Preferences, Shortcut, Tool},
    sculpt::{Alpha, Axis, Kind},
};
use eframe::egui::{self, Color32, Sense, Vec2};
pub(crate) struct UiState {
    pub preferences: Preferences,
    pub settings_open: bool,
    pub layout_open: bool,
    pub capturing: Option<Action>,
    pub settings_message: String,
    pub mesh_busy: bool,
    pub selected: usize,
    pub path: String,
    pub preset_path: String,
    pub alpha_path: String,
    pub status: String,
    pub pending: Option<PendingAction>,
    pub pending_save_path: String,
    pub wire: bool,
    pub show_symmetry_guides: bool,
    pub guide_opacity: f32,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            preferences: Preferences::default(),
            settings_open: false,
            layout_open: false,
            capturing: None,
            settings_message: String::new(),
            mesh_busy: false,
            selected: 0,
            path: "sculpt.clay".into(),
            preset_path: "brush.brush.json".into(),
            alpha_path: String::new(),
            status: "Ready • Sculpt with left drag; Alt reverses; Shift smooths".into(),
            pending: None,
            pending_save_path: "sculpt.clay".into(),
            wire: false,
            show_symmetry_guides: true,
            guide_opacity: 0.85,
        }
    }
}
impl UiState {
    pub fn toolbar(&mut self, ctx: &egui::Context, document: &Document) -> Vec<Command> {
        let mut commands = Vec::new();
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("CLAY FORM");
                ui.menu_button("Settings", |ui| {
                    if ui.button("Key settings…").clicked() {
                        self.settings_open = true;
                        ui.close();
                    }
                    if ui.button("Customize workspace…").clicked() {
                        self.layout_open = true;
                        ui.close();
                    }
                });
                if ui.button("New sphere").clicked() {
                    commands.push(Command::Request(PendingAction::New));
                }
                ui.label("Project path");
                ui.text_edit_singleline(&mut self.path);
                if ui.button("Open").clicked() {
                    commands.push(Command::Request(PendingAction::Open(self.path.clone())));
                }
                if ui.button("Save").clicked() {
                    commands.push(Command::Save);
                }
                if ui.button("Export OBJ").clicked() {
                    commands.push(Command::Export);
                }
                ui.menu_button("Mesh", |ui| self.mesh_controls(ui, document, &mut commands));
                if self.mesh_busy {
                    ui.spinner();
                    if ui.button("Cancel mesh operation").clicked() {
                        commands.push(Command::CancelMeshJob);
                    }
                }
                if document.is_dirty() {
                    ui.label("• Modified");
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(document.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    commands.push(Command::Undo);
                }
                if ui
                    .add_enabled(document.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    commands.push(Command::Redo);
                }
                ui.checkbox(&mut self.wire, "Wireframe");
                let mut symmetry = document.symmetry().clone();
                ui.checkbox(&mut symmetry.enabled, "Symmetry");
                ui.menu_button("Symmetry settings", |ui| {
                    ui.label("Mirror planes");
                    ui.horizontal(|ui| {
                        for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
                            ui.checkbox(&mut symmetry.mirror[i], *label);
                        }
                    });
                    ui.checkbox(&mut symmetry.radial, "Radial symmetry");
                    ui.add_enabled_ui(symmetry.radial, |ui| {
                        egui::ComboBox::from_label("Radial axis")
                            .selected_text(format!("{:?}", symmetry.radial_axis))
                            .show_ui(ui, |ui| {
                                for axis in Axis::ALL {
                                    ui.selectable_value(
                                        &mut symmetry.radial_axis,
                                        axis,
                                        format!("{axis:?}"),
                                    );
                                }
                            });
                        ui.add(
                            egui::Slider::new(&mut symmetry.radial_count, 2..=16).text("Repeats"),
                        );
                    });
                    ui.separator();
                    ui.label("Symmetry center (object coordinates)");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut symmetry.center.0)
                                .speed(0.01)
                                .range(-1_000_000. ..=1_000_000.)
                                .prefix("X "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut symmetry.center.1)
                                .speed(0.01)
                                .range(-1_000_000. ..=1_000_000.)
                                .prefix("Y "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut symmetry.center.2)
                                .speed(0.01)
                                .range(-1_000_000. ..=1_000_000.)
                                .prefix("Z "),
                        );
                    });
                    if ui.button("Center on origin").clicked() {
                        symmetry.center = crate::sculpt::Vec3::default();
                    }
                    ui.checkbox(&mut self.show_symmetry_guides, "Show symmetry lines");
                    ui.add(
                        egui::Slider::new(&mut self.guide_opacity, 0.1..=1.).text("Guide opacity"),
                    );
                    ui.label("Mirror planes and radial repeats can be combined.");
                });
                if symmetry != *document.symmetry() {
                    commands.push(Command::SetSymmetry(symmetry));
                }
                if ui.button("Clear mask").clicked() {
                    commands.push(Command::ClearMask);
                }
                if ui.button("Reset view").clicked() {
                    commands.push(Command::ResetView);
                }
            });
        });
        commands
    }
    pub fn brush_panel(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        tablet_status: &str,
        pressure: f32,
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        for dock in [Dock::Left, Dock::Right] {
            if !self.preferences.panels.iter().any(|p| p.dock == dock) {
                continue;
            }
            let panel = if dock == Dock::Left {
                egui::SidePanel::left("tools_left")
            } else {
                egui::SidePanel::right("tools_right")
            };
            let width = if dock == Dock::Left {
                self.preferences.left_width
            } else {
                self.preferences.right_width
            };
            let response = panel
                .default_width(width)
                .width_range(180. ..=600.)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(format!("{dock:?}"))
                        .show(ui, |ui| {
                            for index in 0..self.preferences.panels.len() {
                                if self.preferences.panels[index].dock != dock {
                                    continue;
                                }
                                let tool = self.preferences.panels[index].tool;
                                let header = egui::CollapsingHeader::new(tool.label())
                                    .id_salt(tool.label())
                                    .open(Some(self.preferences.panels[index].open))
                                    .show(ui, |ui| {
                                        self.tool_controls(
                                            tool,
                                            ui,
                                            document,
                                            tablet_status,
                                            pressure,
                                            &mut commands,
                                        );
                                    });
                                if header.header_response.clicked() {
                                    self.preferences.panels[index].open =
                                        !self.preferences.panels[index].open;
                                }
                            }
                        });
                });
            if dock == Dock::Left {
                self.preferences.left_width = response.response.rect.width();
            } else {
                self.preferences.right_width = response.response.rect.width();
            }
        }
        for index in 0..self.preferences.panels.len() {
            let panel = self.preferences.panels[index].clone();
            if panel.dock != Dock::Floating {
                continue;
            }
            let mut visible = true;
            if let Some(response) = egui::Window::new(panel.tool.label())
                .id(egui::Id::new(("tool", panel.tool.label())))
                .open(&mut visible)
                .default_pos(panel.position)
                .default_width(260.)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(600.)
                        .show(ui, |ui| {
                            self.tool_controls(
                                panel.tool,
                                ui,
                                document,
                                tablet_status,
                                pressure,
                                &mut commands,
                            )
                        });
                })
            {
                self.preferences.panels[index].position =
                    [response.response.rect.min.x, response.response.rect.min.y];
            }
            if !visible {
                self.preferences.panels[index].dock = Dock::Hidden;
            }
        }
        commands
    }
    pub fn status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });
    }
    pub fn confirmation(&mut self, ctx: &egui::Context) -> Option<Command> {
        self.pending.as_ref()?;
        let mut command = None;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Save your current project before continuing?");
                ui.label("Save current project to:");
                ui.text_edit_singleline(&mut self.pending_save_path);
                ui.horizontal(|ui| {
                    if ui.button("Save and continue").clicked() {
                        command = Some(Command::SaveAndContinue);
                    }
                    if ui.button("Discard changes").clicked() {
                        command = Some(Command::DiscardAndContinue);
                    }
                    if ui.button("Cancel").clicked() {
                        command = Some(Command::CancelPending);
                    }
                });
                ui.label(&self.status);
            });
        command
    }
    fn tool_controls(
        &mut self,
        tool: Tool,
        ui: &mut egui::Ui,
        document: &Document,
        tablet_status: &str,
        pressure: f32,
        commands: &mut Vec<Command>,
    ) {
        let original = &document.brushes()[self.selected];
        let mut edited = original.clone();
        let b = &mut edited;
        match tool {
            Tool::Library => {
                for (i, b) in document.brushes().iter().enumerate() {
                    ui.selectable_value(&mut self.selected, i, &b.name);
                }
                if ui.button("Duplicate brush").clicked() {
                    commands.push(Command::DuplicateBrush(self.selected));
                }
            }
            Tool::Brush => {
                ui.text_edit_singleline(&mut b.name);
                egui::ComboBox::from_label("Operation")
                    .selected_text(format!("{:?}", b.kind))
                    .show_ui(ui, |ui| {
                        for k in Kind::ALL {
                            ui.selectable_value(&mut b.kind, k, format!("{k:?}"));
                        }
                    });
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut b.subtract, false, "+ Add");
                    ui.selectable_value(&mut b.subtract, true, "− Subtract");
                });
                ui.add(egui::Slider::new(&mut b.radius, 1. ..=500.).text("Radius (px)"));
                ui.add(egui::Slider::new(&mut b.strength, 0. ..=1.).text("Strength"));
                ui.add(egui::Slider::new(&mut b.spacing, 0.01..=1.).text("Spacing"));
                ui.add(egui::Slider::new(&mut b.falloff, 0.1..=8.).text("Falloff"));
                ui.add(egui::Slider::new(&mut b.rotation, 0. ..=360.).text("Alpha rotation"));
                ui.checkbox(&mut b.pressure, "Pressure → strength");
                ui.checkbox(&mut b.pressure_radius, "Pressure → radius");
                ui.add(egui::Slider::new(&mut b.pressure_curve, 0.1..=4.).text("Pressure curve"));
            }
            Tool::Alpha => {
                ui.label("Alpha");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut b.alpha, Alpha::Soft, "Soft");
                    ui.selectable_value(&mut b.alpha, Alpha::Hard, "Hard");
                    ui.selectable_value(&mut b.alpha, Alpha::Square, "Square");
                });
                if matches!(b.alpha, Alpha::Image { .. }) {
                    ui.label("Custom image alpha loaded");
                }
                let (r, p) = ui.allocate_painter(Vec2::splat(100.), Sense::hover());

                for y in 0..32 {
                    for x in 0..32 {
                        let value =
                            (255. * b.weight(x as f32 / 15.5 - 1., 1. - y as f32 / 15.5)) as u8;
                        p.rect_filled(
                            egui::Rect::from_min_size(
                                r.rect.min + Vec2::new(x as f32, y as f32) * 3.125,
                                Vec2::splat(3.125),
                            ),
                            0.,
                            Color32::from_gray(value),
                        );
                    }
                }
                ui.label("Image path (PNG/JPEG)");
                ui.text_edit_singleline(&mut self.alpha_path);
                if ui.button("Load alpha").clicked() {
                    commands.push(Command::LoadAlpha(self.selected));
                }
            }
            Tool::Presets => {
                ui.label("Preset path");
                ui.text_edit_singleline(&mut self.preset_path);
                ui.horizontal(|ui| {
                    if ui.button("Save preset").clicked() {
                        commands.push(Command::SavePreset(self.selected));
                    }
                    if ui.button("Load preset").clicked() {
                        commands.push(Command::LoadPreset);
                    }
                });
            }
            Tool::Mesh => self.mesh_controls(ui, document, commands),
            Tool::Tablet => {
                ui.label(tablet_status);
                ui.label(format!("Pressure: {:.0}%", pressure * 100.));
                ui.label(format!("Left / pen: sculpt; empty space: orbit\nCorner gizmo: drag or click axis\n{:?}: reverse brush\n{:?}: smooth\nRight drag: orbit\nMiddle drag: pan\nWheel: zoom", self.preferences.reverse, self.preferences.smooth));
            }
        }
        if &edited != original {
            commands.push(Command::UpdateBrush(self.selected, edited));
        }
    }
}

impl UiState {
    pub fn settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            self.capturing = None;
        }
        if let Some(action) = self.capturing {
            let event = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, modifiers)) = event {
                if key == egui::Key::Escape {
                    self.capturing = None;
                } else {
                    match self.preferences.bind(action, Shortcut::new(key, modifiers)) {
                        Ok(()) => {
                            self.settings_message = format!("Updated {}", action.label());
                            self.capturing = None;
                        }
                        Err(error) => self.settings_message = error,
                    }
                }
                ctx.input_mut(|i| {
                    i.events
                        .retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_)))
                });
            }
        }
        egui::Window::new("Key settings")
            .open(&mut self.settings_open)
            .default_width(460.)
            .show(ctx, |ui| {
                ui.label("Click a shortcut, then press the new combination. Escape cancels.");
                ui.label(
                    "Shortcuts pause while typing or editing settings. Changes save automatically.",
                );
                egui::Grid::new("bindings").striped(true).show(ui, |ui| {
                    for (action, binding) in &mut self.preferences.bindings {
                        ui.label(action.label());
                        let label = if self.capturing == Some(*action) {
                            "Press keys…".into()
                        } else {
                            binding.as_ref().map_or("Unbound".into(), |b| b.label())
                        };
                        if ui.button(label).clicked() {
                            self.capturing = Some(*action);
                            self.settings_message.clear();
                        }
                        if ui.small_button("Clear").clicked() {
                            *binding = None;
                            if self.capturing == Some(*action) {
                                self.capturing = None;
                            }
                        }
                        ui.end_row();
                    }
                });
                ui.separator();
                for (label, hold) in [
                    ("Hold to smooth", &mut self.preferences.smooth),
                    ("Hold to reverse", &mut self.preferences.reverse),
                ] {
                    egui::ComboBox::from_label(label)
                        .selected_text(format!("{hold:?}"))
                        .show_ui(ui, |ui| {
                            for value in Hold::ALL {
                                ui.selectable_value(hold, value, format!("{value:?}"));
                            }
                        });
                }
                if self.preferences.smooth == self.preferences.reverse
                    && self.preferences.smooth != Hold::Disabled
                {
                    ui.label("Both effects will apply with this modifier.");
                }
                if ui.button("Reset keys to defaults").clicked() {
                    let defaults = Preferences::default();
                    self.preferences.bindings = defaults.bindings;
                    self.preferences.smooth = defaults.smooth;
                    self.preferences.reverse = defaults.reverse;
                    self.capturing = None;
                }
                ui.label(&self.settings_message);
            });
        egui::Window::new("Customize workspace").open(&mut self.layout_open).default_width(520.).show(ctx, |ui| {
            ui.label("Choose where each tool group lives. Use arrows to change its order.");
            ui.label("Side panels resize by dragging their edge. Floating windows drag by their title.");
            ui.label("Changes save automatically; hidden groups can be restored here.");
            let mut reorder = None;
            for index in 0..self.preferences.panels.len() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        let panel = &mut self.preferences.panels[index];
                        ui.label(panel.tool.label());
                        egui::ComboBox::from_id_salt("dock").selected_text(format!("{:?}",panel.dock)).show_ui(ui, |ui| {
                            for dock in Dock::ALL { ui.selectable_value(&mut panel.dock, dock, format!("{dock:?}")); }
                        });
                        ui.add_enabled(matches!(panel.dock,Dock::Left | Dock::Right), egui::Checkbox::new(&mut panel.open, "Expanded"));
                        if ui.add_enabled(index > 0, egui::Button::new("↑")).clicked() { reorder = Some((index,index-1)); }
                        if ui.add_enabled(index < Tool::ALL.len()-1, egui::Button::new("↓")).clicked() { reorder = Some((index,index+1)); }
                    });
                });
            }
            if let Some((a,b)) = reorder { self.preferences.panels.swap(a,b); }
            if ui.add(egui::Slider::new(&mut self.preferences.left_width,180. ..=600.).text("Left width")).changed() { ctx.memory_mut(|m| m.data.remove::<egui::containers::panel::PanelState>(egui::Id::new("tools_left"))); }
            if ui.add(egui::Slider::new(&mut self.preferences.right_width,180. ..=600.).text("Right width")).changed() { ctx.memory_mut(|m| m.data.remove::<egui::containers::panel::PanelState>(egui::Id::new("tools_right"))); }
            if ui.button("Reset workspace to defaults").clicked() {
                let defaults = Preferences::default(); self.preferences.panels = defaults.panels; self.preferences.left_width = defaults.left_width; self.preferences.right_width = defaults.right_width;
                // Remove remembered panel sizes/positions so reset takes effect immediately.
                ctx.memory_mut(|m| { m.reset_areas(); for id in ["tools_left","tools_right"] { m.data.remove::<egui::containers::panel::PanelState>(egui::Id::new(id)); } });
            }
        });
    }
}

impl UiState {
    fn mesh_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let settings = document.mesh_settings();
        ui.label(format!(
            "{} vertices • {} faces",
            document.mesh().vertices.len(),
            document.mesh().triangles.len()
        ));
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(!settings.dynamic,"Subdivision").clicked() { commands.push(Command::SetDynamicMesh(false)); }
                if ui.selectable_label(settings.dynamic,"Dynamic mesh").clicked() { commands.push(Command::SetDynamicMesh(true)); }
            });
            if settings.dynamic {
                ui.label("Voxel remeshing replaces topology and clears masks. Small details below the voxel size may be lost.");
                let mut resolution=settings.resolution;
                let mut auto=settings.auto_remesh;
                ui.add(egui::Slider::new(&mut resolution,16..=96).text("Resolution"));
                ui.label("Voxels along the longest bounding-box axis.");
                ui.checkbox(&mut auto,"Remesh after each stroke");
                if resolution!=settings.resolution || auto!=settings.auto_remesh { commands.push(Command::SetRemeshSettings(resolution,auto)); }
                if ui.button("Remesh now").clicked() { commands.push(Command::Remesh); }
            } else {
                ui.label(format!("Level {} of {}",settings.level(),settings.levels()));
                let mut level=settings.level();
                ui.add(egui::Slider::new(&mut level,1..=settings.levels()).text("Active level"));
                if level!=settings.level() { commands.push(Command::SetMeshLevel(level)); }
                ui.horizontal(|ui| {
                    if ui.add_enabled(settings.level()>1,egui::Button::new("Lower")).clicked() { commands.push(Command::SetMeshLevel(settings.level()-1)); }
                    if ui.add_enabled(settings.level()<settings.levels(),egui::Button::new("Higher")).clicked() { commands.push(Command::SetMeshLevel(settings.level()+1)); }
                });
                if ui.add_enabled(settings.upper.is_empty() && document.mesh().triangles.len() <= 250_000,egui::Button::new("Subdivide (+ level)")).clicked() { commands.push(Command::Subdivide); }
                ui.label("Each division creates four triangles per face. Edit broad forms low and detail high.");
                ui.label("Switching to dynamic keeps the active surface and discards stored levels. Undo restores them.");
            }
        });
        if self.mesh_busy {
            ui.spinner();
            ui.label("Working in the background…");
        }
    }
}

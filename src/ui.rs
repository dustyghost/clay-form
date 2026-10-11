//! Immediate-mode UI. Widgets emit commands; they never perform IO or sculpt edits.
use crate::{
    command::{Command, PendingAction},
    document::Document,
    preferences::{Action, Dock, Hold, Preferences, Shortcut, Tool},
    sculpt::{Alpha, Axis, Kind},
};
use eframe::egui::{self, Color32, Sense, Vec2};
pub(crate) struct UiState {
    pub transform_mode: crate::viewport::TransformMode,
    pub transform_operation: crate::viewport::TransformOperation,
    pub respect_transform_mask: bool,
    group_name: String,
    rename_model: Option<(usize, String, String)>,
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
            transform_mode: crate::viewport::TransformMode::Sculpt,
            transform_operation: crate::viewport::TransformOperation::Move,
            respect_transform_mask: true,
            group_name: String::new(),
            rename_model: None,
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
    pub fn toolbar(&mut self, ctx: &egui::Context, document: &Document, zoom: f32) -> Vec<Command> {
        let mut commands = Vec::new();
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("CLAY FORM");
                ui.menu_button("Edit", |ui| {
                    for (action, command, label, available) in [
                        (
                            Action::Undo,
                            Command::Undo,
                            document.undo_label(),
                            document.can_undo(),
                        ),
                        (
                            Action::Redo,
                            Command::Redo,
                            document.redo_label(),
                            document.can_redo(),
                        ),
                    ] {
                        let shortcut = self
                            .preferences
                            .bindings
                            .iter()
                            .find(|(a, _)| *a == action)
                            .and_then(|(_, binding)| binding.as_ref())
                            .map_or(String::new(), |b| b.label());
                        let text = format!(
                            "{}{}",
                            action.label(),
                            label.map_or(String::new(), |s| format!(" {s}"))
                        );
                        if ui
                            .add_enabled(
                                available && !self.mesh_busy && self.pending.is_none(),
                                egui::Button::new(text).shortcut_text(shortcut),
                            )
                            .clicked()
                        {
                            commands.push(command);
                            ui.close();
                        }
                    }
                    ui.separator();
                    let (undo, redo) = document.history_counts();
                    ui.label(format!("{undo} undo steps • {redo} redo steps"));
                    ui.label("Geometry and masks • history resets on New/Open");
                });
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
                if ui
                    .add_enabled(
                        !self.mesh_busy,
                        egui::Button::new("Transform").selected(
                            self.transform_mode != crate::viewport::TransformMode::Sculpt,
                        ),
                    )
                    .on_hover_text("Activate transforms and show the viewport controls")
                    .clicked()
                    && self.transform_mode == crate::viewport::TransformMode::Sculpt
                {
                    commands.push(Command::TransformMode(
                        crate::viewport::TransformMode::World,
                    ));
                }
                ui.menu_button("Submodels", |ui| {
                    self.submodel_controls(ui, document, &mut commands)
                });
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
                    .add_enabled(
                        document.can_undo() && !self.mesh_busy && self.pending.is_none(),
                        egui::Button::new("Undo"),
                    )
                    .on_hover_text(document.undo_label().unwrap_or("Nothing to undo"))
                    .clicked()
                {
                    commands.push(Command::Undo);
                }
                if ui
                    .add_enabled(
                        document.can_redo() && !self.mesh_busy && self.pending.is_none(),
                        egui::Button::new("Redo"),
                    )
                    .on_hover_text(document.redo_label().unwrap_or("Nothing to redo"))
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
                ui.menu_button("View", |ui| {
                    if ui.button("Zoom in").clicked() {
                        commands.push(Command::ZoomIn);
                    }
                    if ui.button("Zoom out").clicked() {
                        commands.push(Command::ZoomOut);
                    }
                    let mut value = zoom;
                    ui.add(
                        egui::Slider::new(&mut value, 0.2..=8.)
                            .logarithmic(true)
                            .text("Zoom")
                            .custom_formatter(|value, _| format!("{:.0}%", value * 100.))
                            .custom_parser(|text| {
                                text.trim()
                                    .trim_end_matches('%')
                                    .trim()
                                    .parse::<f64>()
                                    .ok()
                                    .map(|percent| percent / 100.)
                            }),
                    );
                    if value != zoom {
                        commands.push(Command::SetZoom(value));
                    }
                    if ui.button("Reset zoom to 100%").clicked() {
                        commands.push(Command::SetZoom(1.));
                    }
                    if ui.button("Reset view").clicked() {
                        commands.push(Command::ResetView);
                    }
                });
                if ui
                    .add_enabled(zoom > 0.2, egui::Button::new("−"))
                    .on_hover_text("Zoom out")
                    .clicked()
                {
                    commands.push(Command::ZoomOut);
                }
                if ui
                    .button(format!("{:.0}%", zoom * 100.))
                    .on_hover_text("Reset zoom to 100% (keep rotation and pan)")
                    .clicked()
                {
                    commands.push(Command::SetZoom(1.));
                }
                if ui
                    .add_enabled(zoom < 8., egui::Button::new("+"))
                    .on_hover_text("Zoom in")
                    .clicked()
                {
                    commands.push(Command::ZoomIn);
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
                                    if tool == Tool::Transform
                                        && self.preferences.panels[index].open
                                        && !self.mesh_busy
                                        && self.transform_mode
                                            == crate::viewport::TransformMode::Sculpt
                                    {
                                        commands.push(Command::TransformMode(
                                            crate::viewport::TransformMode::World,
                                        ));
                                    }
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
                ui.menu_button("Add brush", |ui| {
                    for kind in Kind::ALL {
                        if ui.button(format!("{kind:?}")).clicked() {
                            commands.push(Command::AddBrush(kind));
                            ui.close();
                        }
                    }
                });
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
            Tool::Transform => self.transform_controls(ui, commands),
            Tool::Submodels => self.submodel_controls(ui, document, commands),
            Tool::Tablet => {
                ui.label(tablet_status);
                ui.label(format!("Pressure: {:.0}%", pressure * 100.));
                ui.label(format!("Left / pen: sculpt; empty space: orbit\nCorner gizmo: drag or click axis\n{:?}: reverse brush\n{:?}: smooth\nRight drag: orbit\nMiddle drag: pan\nWheel: zoom\n{:?} + drag: zoom (up=in, down=out)", self.preferences.reverse, self.preferences.smooth, self.preferences.zoom_hold));
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
                    (
                        "Hold to zoom while dragging",
                        &mut self.preferences.zoom_hold,
                    ),
                ] {
                    egui::ComboBox::from_label(label)
                        .selected_text(format!("{hold:?}"))
                        .show_ui(ui, |ui| {
                            for value in Hold::ALL {
                                ui.selectable_value(hold, value, format!("{value:?}"));
                            }
                        });
                }
                if self.preferences.zoom_hold != Hold::Disabled
                    && (self.preferences.zoom_hold == self.preferences.smooth
                        || self.preferences.zoom_hold == self.preferences.reverse)
                {
                    ui.label(
                        "Zoom takes priority when starting a pen drag with a shared modifier.",
                    );
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
                    self.preferences.zoom_hold = defaults.zoom_hold;
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
    pub(crate) fn transform_toolbar(
        &mut self,
        ctx: &egui::Context,
        viewport: egui::Rect,
        enabled: bool,
    ) -> (Vec<Command>, Option<egui::Rect>) {
        use crate::viewport::{TransformMode as Mode, TransformOperation as Operation};
        if self.transform_mode == Mode::Sculpt {
            return (Vec::new(), None);
        }
        let mut commands = Vec::new();
        let response = egui::Area::new(egui::Id::new("viewport_transform_toolbar"))
            .order(egui::Order::Foreground)
            .fixed_pos(viewport.left_top() + Vec2::splat(12.))
            .constrain_to(viewport)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width((viewport.width() - 32.).clamp(160., 300.));
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong("Transform");
                            for (mode, label) in [(Mode::World, "World axes"), (Mode::ActionLine, "Action Line")] {
                                if ui.selectable_label(self.transform_mode == mode, label).clicked() {
                                    commands.push(Command::TransformMode(mode));
                                }
                            }
                            if ui.small_button("Done").on_hover_text("Return to sculpting").clicked() {
                                commands.push(Command::TransformMode(Mode::Sculpt));
                            }
                        });
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            for (operation, label, hint) in [
                                (Operation::PlaceLine, "Set line", "Drag to place the pivot and direction; drag either endpoint to adjust"),
                                (Operation::Move, "Move", "Drag an axis or the middle handle to move the model"),
                                (Operation::Scale, "Scale", "Drag an axis/end to stretch, or the middle to scale uniformly"),
                            ] {
                                if operation == Operation::PlaceLine && self.transform_mode != Mode::ActionLine { continue; }
                                if ui.selectable_label(self.transform_operation == operation, label).on_hover_text(hint).clicked() {
                                    commands.push(Command::TransformOperation(operation));
                                }
                            }
                        });
                        let mut respect = self.respect_transform_mask;
                        if ui.checkbox(&mut respect, "Respect mask").on_hover_text("Protect masked vertices; turn off to transform the whole model").changed() {
                            commands.push(Command::RespectTransformMask(respect));
                        }
                    });
                });
            });
        (commands, Some(response.response.rect))
    }
    fn transform_controls(&mut self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        use crate::viewport::{TransformMode as Mode, TransformOperation as Operation};
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [(Mode::Sculpt, "Sculpt"), (Mode::World, "World axes"), (Mode::ActionLine, "Action Line")] {
                    if ui.selectable_label(self.transform_mode == mode, label).clicked() { commands.push(Command::TransformMode(mode)); }
                }
            });
            if self.transform_mode != Mode::Sculpt {
                ui.horizontal_wrapped(|ui| {
                    for (op, label) in [(Operation::Move, "Move"), (Operation::Scale, "Scale"), (Operation::PlaceLine, "Set line")] {
                        if op == Operation::PlaceLine && self.transform_mode != Mode::ActionLine { continue; }
                        if ui.selectable_label(self.transform_operation == op, label).clicked() { commands.push(Command::TransformOperation(op)); }
                    }
                });
                let mut respect = self.respect_transform_mask;
                if ui.checkbox(&mut respect, "Respect mask").changed() { commands.push(Command::RespectTransformMask(respect)); }
                if self.transform_mode == Mode::World {
                    ui.label("Drag X/Y/Z handles. Center moves in the view plane or scales uniformly. Orbit to expose an axis facing the camera.");
                } else {
                    ui.label("Set line: drag to place the pivot and direction; adjust either endpoint. Move: drag the middle. Scale: drag the end to stretch, or the middle to scale uniformly.");
                }
                ui.label("Each drag is one undo step; all subdivision levels follow.");
            }
        });
    }
    fn submodel_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        ui.label("Click a model row to select it. The eye toggles visibility.");
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Duplicate").clicked() { commands.push(Command::DuplicateModel); }
                if ui.button("Extract masked").on_hover_text("Copy faces at least 50% masked at every vertex; keep the source. Boundaries remain open.").clicked() { commands.push(Command::ExtractMasked); }
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.group_name).desired_width(120.).hint_text("New group"));
                if ui.button("+ Group").clicked() { commands.push(Command::AddGroup(std::mem::take(&mut self.group_name))); }
            });
            ui.separator();
            let groups = std::iter::once(String::new()).chain(document.groups().iter().cloned()).collect::<Vec<_>>();
            for (section, group) in groups.iter().enumerate() {
                let indices: Vec<_> = (0..document.model_count()).filter(|&i| document.model(i).unwrap().metadata.group == *group).collect();
                if section == 0 && indices.is_empty() { continue; }
                ui.push_id(("submodel_group", group), |ui| {
                    egui::CollapsingHeader::new(if group.is_empty() { "Ungrouped" } else { group }).default_open(true).show(ui, |ui| {
                        if section > 0 {
                            ui.horizontal(|ui| {
                                let all_visible = indices.iter().all(|&i|document.model(i).unwrap().metadata.visible);
                                if visibility_button(ui, all_visible).on_hover_text("Show/hide this group").clicked() { commands.push(Command::GroupVisible(group.clone(), !all_visible)); }
                                let index = section - 1;
                                if reorder_button(ui, true, index > 0).on_hover_text("Move group up").clicked() { commands.push(Command::ReorderGroup(index, index-1)); }
                                if reorder_button(ui, false, index+1 < document.groups().len()).on_hover_text("Move group down").clicked() { commands.push(Command::ReorderGroup(index, index+1)); }
                                if ui.small_button("Ungroup").clicked() { commands.push(Command::RemoveGroup(group.clone())); }
                            });
                        }
                        for (position, &index) in indices.iter().enumerate() {
                            let model = document.model(index).unwrap();
                            ui.push_id(index, |ui| {
                                let active = index == document.active_model_index();
                                let mut control_hit = false;
                                let mut select_clicked = false;
                                let mut frame = egui::Frame::group(ui.style());
                                if active {
                                    frame = frame.fill(Color32::from_rgb(37, 58, 68)).stroke(egui::Stroke::new(1.5_f32, Color32::from_rgb(125, 210, 245)));
                                }
                                let row = frame.show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let eye = visibility_button(ui, model.metadata.visible).on_hover_text(if model.metadata.visible { "Hide model" } else { "Show model" });
                                        control_hit |= eye.contains_pointer();
                                        if eye.clicked() {
                                            let mut metadata = model.metadata.clone(); metadata.visible = !metadata.visible;
                                            commands.push(Command::EditSubmodel(index, model.name.into(), metadata));
                                        }
                                        model_thumbnail(ui, model.display_mesh(false), active);
                                        ui.vertical(|ui| {
                                            select_clicked |= ui.selectable_label(active, model.name).on_hover_text("Select this submodel for sculpting or transforms").clicked();
                                            ui.horizontal(|ui| {
                                                let up = reorder_button(ui, true, position > 0).on_hover_text("Move model up in group");
                                                let down = reorder_button(ui, false, position+1 < indices.len()).on_hover_text("Move model down in group");
                                                control_hit |= up.contains_pointer() || down.contains_pointer();
                                                if up.clicked() { commands.push(Command::ReorderSubmodel(index, indices[position-1])); }
                                                if down.clicked() { commands.push(Command::ReorderSubmodel(index, indices[position+1])); }
                                                if active { ui.small("Active"); }
                                            });
                                        });
                                    });
                                });
                                if select_clicked || (ui.is_enabled() && row.response.contains_pointer() && !control_hit && ui.input(|i| i.pointer.primary_clicked())) {
                                    commands.push(Command::SelectSubmodel(index));
                                }
                            });
                        }
                        if indices.is_empty() { ui.weak("Empty group"); }
                    });
                });
            }
            ui.separator();
            let index = document.active_model_index();
            let model = document.model(index).unwrap();
            if self.rename_model.as_ref().is_none_or(|(target, original, _)| *target != index || original != model.name) {
                self.rename_model = Some((index, model.name.into(), model.name.into()));
            }
            let (_, _, draft) = self.rename_model.as_mut().unwrap();
            ui.horizontal(|ui| {
                ui.label("Name");
                let response = ui.add(egui::TextEdit::singleline(draft).desired_width(140.));
                if (response.lost_focus() || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))) && draft != model.name {
                    commands.push(Command::EditSubmodel(index, draft.clone(), model.metadata.clone()));
                }
            });
            let mut group = model.metadata.group.clone();
            egui::ComboBox::from_id_salt("active_model_group").selected_text(if group.is_empty() { "Ungrouped" } else { &group }).show_ui(ui, |ui| {
                ui.selectable_value(&mut group, String::new(), "Ungrouped");
                for name in document.groups() { ui.selectable_value(&mut group, name.clone(), name); }
            });
            if group != model.metadata.group { let mut metadata = model.metadata.clone(); metadata.group = group; commands.push(Command::EditSubmodel(index, model.name.into(), metadata)); }
            if !document.active_visible() { ui.weak("Active model is hidden; show it to sculpt or transform."); }
        });
    }
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
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(!settings.dynamic && !settings.adaptive.enabled,"Subdivision").clicked() { commands.push(Command::SetDynamicMesh(false)); }
                if ui.selectable_label(settings.dynamic,"Dynamic mesh").clicked() { commands.push(Command::SetDynamicMesh(true)); }
                if ui.selectable_label(settings.adaptive.enabled,"Adaptive sculpt").clicked() { let mut adaptive=settings.adaptive.clone(); adaptive.enabled=true; commands.push(Command::SetAdaptive(adaptive)); }
            });
            if settings.adaptive.enabled {
                ui.label("Local triangle refinement while sculpting.");
                let mut adaptive=settings.adaptive.clone();
                ui.checkbox(&mut adaptive.relative,"Detail relative to brush size");
                let label=if adaptive.relative { "Edge size (% of radius)" } else { "Edge size (px)" };
                ui.add(egui::Slider::new(&mut adaptive.detail,1. ..=32.).text(label));
                ui.checkbox(&mut adaptive.refine,"Refine stretched edges");
                ui.checkbox(&mut adaptive.simplify,"Simplify dense areas");
                ui.add_enabled(adaptive.simplify,egui::Slider::new(&mut adaptive.collapse_ratio,0.1..=0.8).text("Simplify ratio"));
                if adaptive!=settings.adaptive { commands.push(Command::SetAdaptive(adaptive)); }
                ui.label("Smaller edge sizes add detail. Masks are preserved; Mask brush keeps topology fixed.");
                ui.label("Smooth + Simplify cleans up without adding vertices. Larger edge sizes and simplify ratios remove more density.");
                ui.label("Local adaptation preserves separate parts; use voxel remeshing to fuse them.");
            } else if settings.dynamic {
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
                ui.horizontal(|ui| {
                    if ui.add_enabled(!settings.lower.is_empty(),egui::Button::new("Delete lower")).on_hover_text("Remove all levels below the active level; keep the active surface and higher detail. Undo restores them.").clicked() { commands.push(Command::DeleteLowerLevels); }
                    if ui.add_enabled(!settings.upper.is_empty(),egui::Button::new("Delete higher")).on_hover_text("Remove all levels above the active level; keep the active surface and lower levels. Undo restores them.").clicked() { commands.push(Command::DeleteHigherLevels); }
                });
                if ui.add_enabled(settings.upper.is_empty() && document.mesh().triangles.len() <= 250_000,egui::Button::new("Subdivide (+ level)")).clicked() { commands.push(Command::Subdivide); }
                ui.label("Each division creates four triangles per face. Edit broad forms low and detail high.");
                ui.label("Switching to dynamic or adaptive keeps the active surface and discards stored levels. Undo restores them.");
            }
        });
        if self.mesh_busy {
            ui.spinner();
            ui.label("Working in the background…");
        }
    }
}

fn visibility_button(ui: &mut egui::Ui, visible: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.), Sense::click());
    let painter = ui.painter();
    let color = if visible {
        Color32::from_rgb(150, 215, 245)
    } else {
        Color32::GRAY
    };
    let c = rect.center();
    let points = vec![
        c + Vec2::new(-9., 0.),
        c + Vec2::new(-4., -5.),
        c + Vec2::new(4., -5.),
        c + Vec2::new(9., 0.),
        c + Vec2::new(4., 5.),
        c + Vec2::new(-4., 5.),
        c + Vec2::new(-9., 0.),
    ];
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.5_f32, color)));
    painter.circle_filled(c, 2.5, color);
    if !visible {
        painter.line_segment(
            [c + Vec2::new(-8., 8.), c + Vec2::new(8., -8.)],
            egui::Stroke::new(2_f32, color),
        );
    }
    response
}
fn model_thumbnail(ui: &mut egui::Ui, mesh: &crate::sculpt::Mesh, active: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(42.), Sense::hover());
    let mut points: Vec<Vec2> = mesh
        .vertices
        .iter()
        .step_by((mesh.vertices.len() / 256).max(1))
        .map(|v| Vec2::new(v.0 * 0.8 - v.2 * 0.6, -v.1 * 0.9 + v.0 * 0.2 + v.2 * 0.25))
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return;
    }
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let mut hull: Vec<Vec2> = Vec::new();
    for &p in &points {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0. {
            hull.pop();
        }
        hull.push(p);
    }
    let lower = hull.len();
    for &p in points.iter().rev().skip(1) {
        while hull.len() > lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0. {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    let min = points
        .iter()
        .fold(Vec2::splat(f32::INFINITY), |a, b| a.min(*b));
    let max = points
        .iter()
        .fold(Vec2::splat(f32::NEG_INFINITY), |a, b| a.max(*b));
    let scale = 34. / (max - min).max_elem().max(0.001);
    let center = (min + max) * 0.5;
    let color = if active {
        Color32::from_rgb(190, 155, 125)
    } else {
        Color32::from_rgb(110, 128, 145)
    };
    ui.painter()
        .rect_filled(rect, 4., Color32::from_rgb(25, 29, 35));
    ui.painter().add(egui::Shape::convex_polygon(
        hull.into_iter()
            .map(|p| rect.center() + (p - center) * scale)
            .collect(),
        color,
        egui::Stroke::new(1_f32, Color32::from_gray(170)),
    ));
}

/// Draw reorder arrows directly so the controls do not depend on font glyphs.
fn reorder_button(ui: &mut egui::Ui, up: bool, enabled: bool) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(24., 20.), Sense::click());
        let visuals = ui.style().interact(&response);
        ui.painter().rect_filled(rect, 3., visuals.bg_fill);
        let c = rect.center();
        let direction = if up { -1. } else { 1. };
        let tip = c + Vec2::new(0., direction * 5.);
        let stroke = egui::Stroke::new(1.5_f32, visuals.fg_stroke.color);
        ui.painter()
            .line_segment([c - Vec2::new(0., direction * 5.), tip], stroke);
        ui.painter()
            .line_segment([tip + Vec2::new(-4., -direction * 4.), tip], stroke);
        ui.painter()
            .line_segment([tip + Vec2::new(4., -direction * 4.), tip], stroke);
        response
    })
    .inner
}

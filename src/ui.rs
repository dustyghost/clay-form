//! Immediate-mode UI. Widgets emit commands; they never perform IO or sculpt edits.
use crate::{
    command::{Command, PendingAction},
    document::Document,
    preferences::{Action, BuiltinMenu, Dock, Hold, Preferences, Shortcut, SidebarSection, Tool},
    sculpt::{Alpha, Axis, Kind},
};
use eframe::egui::{self, Color32, Sense, Vec2};
mod design;
use design::Glyph;
mod customization;
mod edges;
mod icons;
mod menus;
mod mirror;
mod modeling;
mod primitive;
mod references;
mod sidebar;
mod transform;
use customization::ActionResponse;
pub(crate) struct UiState {
    icons: icons::Icons,
    pinned_menu_rects: Vec<egui::Rect>,
    pub library_path: String,
    pub(crate) primitive_warning: Option<(usize, String)>,
    pub(crate) primitive_controls_requested: bool,
    pub transform_mode: crate::viewport::TransformMode,
    pub edges: edges::EdgeControls,
    pub modeling: modeling::ModelingControls,
    pub mirror_axis: Axis,
    pub slice_active: bool,
    pub selection_tool: Option<crate::viewport::SelectionTool>,
    pub slice_path_mode: bool,
    pub slice_options: crate::sculpt::SliceOptions,
    pub transform_operation: crate::viewport::TransformOperation,
    pub respect_transform_mask: bool,
    group_name: String,
    pub selected_vertex_group: Option<(usize, usize)>,
    pub vertex_group_name: String,
    pub vertex_group_rename: String,
    rename_model: Option<(usize, String, String)>,
    pub preferences: Preferences,
    pub(super) customization: customization::Customization,
    pub settings_open: bool,
    pub recovery_open: bool,
    pub layout_open: bool,
    pub capturing: Option<Action>,
    pub settings_message: String,
    pub mesh_busy: bool,
    pub selected: usize,
    smooth_brush: Option<String>,
    brush_editor_focus: Option<(egui::Id, String)>,
    pub path: String,
    pub obj_path: String,
    pub reference_path: String,
    pub arrange_references: bool,
    pub references_visible: bool,
    pub selected_reference: usize,
    pub preset_path: String,
    pub alpha_path: String,
    pub status: String,
    pub pending: Option<PendingAction>,
    pub pending_save_path: String,
    pub wire: bool,
    pub wire_mode: crate::viewport::WireMode,
    pub wire_triangles: bool,
    pub show_polygroups: bool,
    pub show_backfaces: bool,
    pub backface_opacity: f32,
    pub show_symmetry_guides: bool,
    pub guide_opacity: f32,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            icons: Default::default(),
            pinned_menu_rects: Vec::new(),
            library_path: "brush-library.brushes.json".into(),
            primitive_warning: None,
            primitive_controls_requested: false,
            transform_mode: crate::viewport::TransformMode::Sculpt,
            edges: Default::default(),
            modeling: modeling::ModelingControls {
                amount: 0.1,
                snap: 0.04,
                protect_mask: true,
                join: true,
                loop_count: 1,
                ..Default::default()
            },
            mirror_axis: Axis::X,
            slice_active: false,
            selection_tool: None,
            slice_path_mode: false,
            slice_options: Default::default(),
            transform_operation: crate::viewport::TransformOperation::Move,
            respect_transform_mask: true,
            group_name: String::new(),
            selected_vertex_group: None,
            vertex_group_name: String::new(),
            vertex_group_rename: String::new(),
            rename_model: None,
            preferences: Preferences::default(),
            customization: Default::default(),
            settings_open: false,
            recovery_open: false,
            layout_open: false,
            capturing: None,
            settings_message: String::new(),
            mesh_busy: false,
            selected: 0,
            smooth_brush: None,
            brush_editor_focus: None,
            path: "sculpt.clay".into(),
            obj_path: "model.obj".into(),
            reference_path: String::new(),
            arrange_references: false,
            references_visible: true,
            selected_reference: 0,
            preset_path: "brush.brush.json".into(),
            alpha_path: String::new(),
            status: "Ready • Drag clay to sculpt; Alt-drag empty space to pan; Shift smooths"
                .into(),
            pending: None,
            pending_save_path: "sculpt.clay".into(),
            wire: false,
            wire_mode: Default::default(),
            wire_triangles: false,
            show_polygroups: true,
            show_backfaces: false,
            backface_opacity: 0.15,
            show_symmetry_guides: true,
            guide_opacity: 0.85,
        }
    }
}
impl UiState {
    /// Keep the ordinary selection intact while a modifier borrows a Smooth preset.
    pub(crate) fn active_brush_index(
        &self,
        document: &Document,
        modifiers: egui::Modifiers,
    ) -> Option<usize> {
        if !self.preferences.smooth.active(modifiers)
            || document.brushes()[self.selected].kind == Kind::Smooth
        {
            return Some(self.selected);
        }
        self.smooth_brush
            .as_ref()
            .and_then(|id| {
                document
                    .brushes()
                    .iter()
                    .position(|b| b.kind == Kind::Smooth && b.identity() == *id)
            })
            .or_else(|| {
                document
                    .brushes()
                    .iter()
                    .position(|b| b.kind == Kind::Smooth)
            })
    }

    pub(crate) fn brush_index_for_context(
        &self,
        document: &Document,
        ctx: &egui::Context,
    ) -> Option<usize> {
        if ctx.wants_keyboard_input() {
            // Text entry must stay attached to the preset whose field took focus.
            if let Some((field, id)) = &self.brush_editor_focus
                && ctx.memory(|m| m.focused()) == Some(*field)
                && let Some(index) = document.brushes().iter().position(|b| b.identity() == *id)
            {
                return Some(index);
            }
            return Some(self.selected);
        }
        self.active_brush_index(document, ctx.input(|i| i.modifiers))
    }

    pub(crate) fn select_brush(&mut self, index: usize, document: &Document) {
        // Remember both the preset being left and a newly chosen Smooth variation.
        for i in [self.selected, index] {
            if let Some(brush) = document.brushes().get(i)
                && brush.kind == Kind::Smooth
            {
                self.smooth_brush = Some(brush.identity());
            }
        }
        self.selected = index;
    }
    pub fn toolbar(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        camera: &crate::camera::Camera,
    ) -> Vec<Command> {
        design::theme(ctx);
        design::appearance(ctx, &self.preferences.appearance);
        let mut commands = Vec::new();
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6., 5.);
            ui.horizontal_wrapped(|ui| {
                ui.strong("CLAY FORM");
                self.workspace_selector(ui);
                ui.separator();
                self.builtin_menu(ui, BuiltinMenu::Edit, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Settings, document, camera, &mut commands);
                self.custom_menu_bar(ui, document, &mut commands);
                if self.preferences.compact_toolbar {
                    if self.mesh_busy {
                        ui.spinner();
                        if ui
                            .button("Cancel mesh operation")
                            .customize(Action::CancelMeshOperation)
                            .clicked()
                        {
                            commands.push(Command::CancelMeshJob);
                        }
                    }
                    return;
                }
                if ui
                    .add(design::button(ui, Glyph::New, "New sphere"))
                    .customize(Action::New)
                    .clicked()
                {
                    commands.push(Command::Request(PendingAction::New));
                }
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .desired_width((ui.available_width() - 340.).clamp(120., 280.))
                        .hint_text("Project path"),
                );
                if ui
                    .add(design::button(ui, Glyph::Open, "Open"))
                    .customize(Action::Open)
                    .clicked()
                {
                    commands.push(Command::Request(PendingAction::Open(self.path.clone())));
                }
                if ui
                    .add(design::button(ui, Glyph::Save, "Save"))
                    .customize(Action::Save)
                    .clicked()
                {
                    commands.push(Command::Save);
                }
                if ui
                    .add(design::button(ui, Glyph::Export, "Export OBJ"))
                    .customize(Action::Export)
                    .clicked()
                {
                    commands.push(Command::Export);
                }
                if self.mesh_busy {
                    ui.spinner();
                    if ui
                        .add(design::button(ui, Glyph::Close, "Cancel mesh operation"))
                        .clicked()
                    {
                        commands.push(Command::CancelMeshJob);
                    }
                }
                if document.is_dirty() {
                    ui.label("• Modified");
                }
            });
            if !self.preferences.compact_toolbar {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            document.can_undo() && !self.mesh_busy && self.pending.is_none(),
                            design::button(ui, Glyph::Undo, "Undo"),
                        )
                        .on_hover_text(document.undo_label().unwrap_or("Nothing to undo"))
                        .customize(Action::Undo)
                        .clicked()
                    {
                        commands.push(Command::Undo);
                    }
                    if ui
                        .add_enabled(
                            document.can_redo() && !self.mesh_busy && self.pending.is_none(),
                            design::button(ui, Glyph::Redo, "Redo"),
                        )
                        .on_hover_text(document.redo_label().unwrap_or("Nothing to redo"))
                        .customize(Action::Redo)
                        .clicked()
                    {
                        commands.push(Command::Redo);
                    }
                    ui.separator();
                    self.builtin_menu(ui, BuiltinMenu::Visibility, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::Mesh, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::Slice, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::Transform, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::Primitive, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::Submodels, document, camera, &mut commands);
                    ui.separator();
                    if ui
                        .add(design::button(ui, Glyph::Wire, "Wireframe").selected(self.wire))
                        .customize(Action::Wireframe)
                        .clicked()
                    {
                        self.wire = !self.wire;
                    }
                    self.builtin_menu(ui, BuiltinMenu::Wireframe, document, camera, &mut commands);
                    let mut symmetry = document.symmetry().clone();
                    if ui
                        .add(
                            design::button(ui, Glyph::Symmetry, "Symmetry")
                                .selected(symmetry.enabled),
                        )
                        .customize(Action::Symmetry)
                        .clicked()
                    {
                        symmetry.enabled = !symmetry.enabled;
                    }
                    if symmetry != *document.symmetry() {
                        commands.push(Command::SetSymmetry(symmetry));
                    }
                    self.builtin_menu(ui, BuiltinMenu::Symmetry, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::Mask, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::View, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::References, document, camera, &mut commands);
                    if ui
                        .add_enabled(camera.zoom > 0.2, design::button(ui, Glyph::Minus, ""))
                        .on_hover_text("Zoom out")
                        .customize(Action::ZoomOut)
                        .clicked()
                    {
                        commands.push(Command::ZoomOut);
                    }
                    if ui
                        .button(format!("{:.0}%", camera.zoom * 100.))
                        .on_hover_text("Reset zoom to 100% (keep rotation and pan)")
                        .customize(Action::ResetZoom)
                        .clicked()
                    {
                        commands.push(Command::SetZoom(1.));
                    }
                    if ui
                        .add_enabled(camera.zoom < 8., design::button(ui, Glyph::Plus, ""))
                        .on_hover_text("Zoom in")
                        .customize(Action::ZoomIn)
                        .clicked()
                    {
                        commands.push(Command::ZoomIn);
                    }
                });
            }
        });
        self.pinned_menus(ctx, document, camera, &mut commands);
        if self.arrange_references {
            egui::TopBottomPanel::top("reference_arrangement").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Arrange reference images");
                    ui.label("Drag an unlocked image to move · drag its lower-right corner to resize · Esc to cancel / finish");
                    if ui.button("Done").clicked() { commands.push(Command::ArrangeReferences(false)); }
                });
            });
        }
        if let Some(tool) = self.selection_tool {
            egui::TopBottomPanel::top("visibility_selection_controls").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(tool.label());
                    ui.separator();
                    ui.label(tool.instructions());
                    ui.separator();
                    ui.label("Through all depths · Space to move · Esc to cancel");
                    if ui.button("Done").clicked() {
                        commands.push(Command::SelectTool(None));
                    }
                });
            });
        }
        if self.slice_active && !self.preferences.menu_placed(BuiltinMenu::Slice) {
            egui::TopBottomPanel::top("slice_options").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.menu_placement_controls(ui, BuiltinMenu::Slice, false);
                    self.slice_controls(ui, document, &mut commands);
                });
            });
        }
        commands
    }
    pub fn brush_panel(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        tablet_status: &str,
        pressure: f32,
        camera: &crate::camera::Camera,
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        if self.primitive_controls_requested && document.primitive().is_some() {
            for panel in &mut self.preferences.panels {
                if panel.tool == Tool::Submodels {
                    panel.open = true;
                    if panel.dock == Dock::Hidden {
                        panel.dock = Dock::Right;
                    }
                }
            }
        }
        for dock in [Dock::Left, Dock::Right] {
            let placement = if dock == Dock::Left {
                crate::preferences::Placement::LeftPanel
            } else {
                crate::preferences::Placement::RightPanel
            };
            let sections = self.preferences.sidebar_sections(dock);
            if sections.is_empty() {
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
                    let background = ui.interact(
                        ui.max_rect(),
                        ui.id().with("workspace_background"),
                        egui::Sense::click(),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt(format!("{dock:?}"))
                        .show(ui, |ui| {
                            for section in sections {
                                match section {
                                    SidebarSection::Custom(id) => {
                                        let menu = self
                                            .preferences
                                            .menus
                                            .iter()
                                            .find(|m| m.id == id)
                                            .unwrap()
                                            .clone();
                                        self.custom_section(ui, &menu, document, &mut commands);
                                    }
                                    SidebarSection::Builtin(menu) => {
                                        self.builtin_section(
                                            ui,
                                            menu,
                                            document,
                                            camera,
                                            &mut commands,
                                        );
                                    }
                                    SidebarSection::Tool(tool) => {
                                        let index = self
                                            .preferences
                                            .panels
                                            .iter()
                                            .position(|p| p.tool == tool)
                                            .unwrap();
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
                                        let clicked = header.header_response.clicked();
                                        self.section_context(header.header_response, section);
                                        if clicked {
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
                                }
                            }
                        });
                    customization::sidebar_context(background, placement);
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
                    if ui
                        .add(design::button(ui, Glyph::Save, "Save and continue"))
                        .clicked()
                    {
                        command = Some(Command::SaveAndContinue);
                    }
                    if ui
                        .add(design::button(ui, Glyph::Delete, "Discard changes"))
                        .clicked()
                    {
                        command = Some(Command::DiscardAndContinue);
                    }
                    if ui.add(design::button(ui, Glyph::Close, "Cancel")).clicked() {
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
        let focus_before = ui.ctx().memory(|m| m.focused());
        let active = self.brush_index_for_context(document, ui.ctx());
        let fallback = crate::sculpt::Brush::new(Kind::Smooth);
        let original = active.map_or(&fallback, |index| &document.brushes()[index]);
        let selected = active.unwrap_or(self.selected);
        if active.is_none() && matches!(tool, Tool::Brush | Tool::Alpha | Tool::Presets) {
            ui.label(
                "Temporary Smooth uses default settings. Add a Smooth preset to customize it.",
            );
            if ui.button("Add Smooth preset").clicked() {
                commands.push(Command::AddBrush(Kind::Smooth));
            }
            return;
        }
        let mut edited = original.clone();
        let b = &mut edited;
        match tool {
            Tool::Library => {
                self.builtin_menu(
                    ui,
                    BuiltinMenu::Brushes,
                    document,
                    &crate::camera::Camera::default(),
                    commands,
                );
                let columns = (ui.available_width() / 76.).floor().clamp(1., 4.) as usize;
                let width =
                    ((ui.available_width() - 6. * (columns - 1) as f32) / columns as f32).max(32.);
                egui::Grid::new("brush_tiles")
                    .num_columns(columns)
                    .spacing(Vec2::splat(6.))
                    .show(ui, |ui| {
                        for (i, b) in document.brushes().iter().enumerate() {
                            let id = self.brush_texture(ui.ctx(), i, b);
                            if design::brush_tile(ui, id, &b.name, active == Some(i), width)
                                .customize(Action::SelectBrush(b.identity()))
                                .clicked()
                            {
                                self.select_brush(i, document);
                            }
                            if (i + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
                ui.add_space(4.);
                if active.is_none() {
                    ui.weak("Temporary Smooth · default settings");
                    return;
                }
                if ui
                    .add(design::button(
                        ui,
                        Glyph::Camera,
                        "Capture model for brush icon",
                    ))
                    .customize(Action::CaptureBrushIcon)
                    .clicked()
                {
                    commands.push(Command::CaptureBrushIcon(selected));
                }
                if ui
                    .add(design::button(ui, Glyph::Copy, "Duplicate brush"))
                    .customize(Action::DuplicateBrush)
                    .clicked()
                {
                    commands.push(Command::DuplicateBrush(selected));
                }
            }
            Tool::Brush => {
                let id = self.brush_texture(ui.ctx(), selected, b);
                ui.horizontal(|ui| {
                    ui.image((id, Vec2::splat(64.)));
                    ui.vertical(|ui| {
                        if ui
                            .add(design::button(ui, Glyph::Camera, "Capture model icon"))
                            .customize(Action::CaptureBrushIcon)
                            .clicked()
                        {
                            commands.push(Command::CaptureBrushIcon(selected));
                        }
                        if ui
                            .add_enabled(
                                b.icon.is_some(),
                                design::button(ui, Glyph::Restore, "Restore default icon"),
                            )
                            .customize(Action::RestoreBrushIcon)
                            .clicked()
                        {
                            b.icon = None;
                        }
                        if ui
                            .add(design::button(ui, Glyph::Restore, "Reset settings"))
                            .customize(Action::ResetBrushSettings)
                            .on_hover_text(format!("Restore the default {:?} brush settings, including alpha, stroke mode, and pressure. Keeps this preset's name and icon.", b.kind))
                            .clicked()
                        {
                            b.reset_settings();
                        }
                    });
                });
                ui.text_edit_singleline(&mut b.name);
                egui::ComboBox::from_label("Operation")
                    .selected_text(format!("{:?}", b.kind))
                    .show_ui(ui, |ui| {
                        for k in Kind::ALL {
                            ui.selectable_value(&mut b.kind, k, format!("{k:?}"));
                        }
                    });
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut b.subtract, false, "+ Add")
                        .customize(Action::BrushAdd);
                    ui.selectable_value(&mut b.subtract, true, "− Subtract")
                        .customize(Action::BrushSubtract);
                });
                ui.add(egui::Slider::new(&mut b.radius, 1. ..=500.).text("Radius (px)"));
                ui.add(egui::Slider::new(&mut b.inner_radius, 0. ..=1.).text("Inner radius (fraction)")).on_hover_text("Full-strength core; falloff fades from here to the outer radius. Applies to Soft and image alphas.");
                ui.checkbox(&mut b.surface_cursor, "Surface footprint and normal");
                ui.add(egui::Slider::new(&mut b.strength, 0. ..=1.).text("Strength"));
                ui.label("Stroke mode");
                let supported = !matches!(b.kind, Kind::Grab | Kind::Snake);
                ui.add_enabled_ui(supported, |ui| {
                    egui::ComboBox::from_id_salt("brush_stroke_mode")
                        .selected_text(b.effective_stroke_mode().label())
                        .show_ui(ui, |ui| {
                            for mode in crate::sculpt::StrokeMode::ALL {
                                ui.selectable_value(&mut b.stroke_mode, mode, mode.label())
                                    .customize(match mode {
                                        crate::sculpt::StrokeMode::Freehand => {
                                            Action::StrokeFreehand
                                        }
                                        crate::sculpt::StrokeMode::Line => Action::StrokeLine,
                                        crate::sculpt::StrokeMode::DragRect => {
                                            Action::StrokeDragRect
                                        }
                                        crate::sculpt::StrokeMode::DragDot => Action::StrokeDragDot,
                                    })
                                    .on_hover_text(mode.description());
                            }
                        });
                });
                if supported {
                    ui.weak(b.stroke_mode.description());
                } else {
                    ui.weak("Grab and Snake use Freehand for continuous displacement.");
                }
                ui.add(egui::Slider::new(&mut b.spacing, 0.01..=1.).text("Spacing"));
                ui.add_enabled_ui(b.supports_stabilization(), |ui| {
                    ui.checkbox(&mut b.stabilize, "Stroke stabilisation")
                        .customize(Action::StrokeStabilisation)
                        .on_hover_text("Freehand brush trails the pointer to smooth small movements. Settings apply to the next stroke. Grab and Snake are unaffected.");
                    ui.add_enabled(b.stabilize, egui::Slider::new(&mut b.stabilizer_distance, 1. ..=200.).text("Trailing distance (px)"));
                });
                ui.add(egui::Slider::new(&mut b.falloff, 0.1..=8.).text("Falloff"));
                ui.add(egui::Slider::new(&mut b.rotation, 0. ..=360.).text("Alpha rotation"));
                ui.checkbox(&mut b.pressure, "Pressure → strength");
                ui.checkbox(&mut b.pressure_radius, "Pressure → radius");
                ui.add(egui::Slider::new(&mut b.pressure_curve, 0.1..=4.).text("Pressure curve"));
            }
            Tool::Alpha => {
                ui.label("Alpha");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut b.alpha, Alpha::Soft, "Soft")
                        .customize(Action::AlphaSoft);
                    ui.selectable_value(&mut b.alpha, Alpha::Hard, "Hard")
                        .customize(Action::AlphaHard);
                    ui.selectable_value(&mut b.alpha, Alpha::Square, "Square")
                        .customize(Action::AlphaSquare);
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
                if ui
                    .add(design::button(ui, Glyph::Open, "Load alpha"))
                    .customize(Action::LoadAlpha)
                    .clicked()
                {
                    commands.push(Command::LoadAlpha(selected));
                }
            }
            Tool::Presets => {
                ui.label("Preset path");
                ui.text_edit_singleline(&mut self.preset_path);
                ui.horizontal(|ui| {
                    if ui
                        .add(design::button(ui, Glyph::Export, "Export brush"))
                        .customize(Action::SavePreset)
                        .clicked()
                    {
                        commands.push(Command::SavePreset(selected));
                    }
                    if ui
                        .add(design::button(ui, Glyph::Open, "Import brush"))
                        .customize(Action::LoadPreset)
                        .clicked()
                    {
                        commands.push(Command::LoadPreset);
                    }
                });
                ui.separator();
                ui.label("Brush library path");
                ui.text_edit_singleline(&mut self.library_path);
                ui.horizontal(|ui| {
                    if ui
                        .add(design::button(ui, Glyph::Export, "Export library"))
                        .customize(Action::ExportLibrary)
                        .clicked()
                    {
                        commands.push(Command::ExportBrushLibrary);
                    }
                    if ui
                        .add(design::button(ui, Glyph::Open, "Import library"))
                        .customize(Action::ImportLibrary)
                        .clicked()
                    {
                        commands.push(Command::ImportBrushLibrary);
                    }
                });
                ui.weak("Imports update brushes with matching IDs; new IDs are added.");
            }
            Tool::Mesh => self.mesh_controls(ui, document, commands),
            Tool::Transform => self.transform_controls(ui, commands),
            Tool::Submodels => self.submodel_controls(ui, document, commands),
            Tool::Tablet => {
                ui.label(tablet_status);
                ui.label(format!("Pressure: {:.0}%", pressure * 100.));
                ui.label(format!("Left / pen: sculpt; empty space: orbit\nCorner gizmo: drag or click axis\n{:?}: reverse brush\n{:?}: smooth\nRight drag: orbit\nMiddle drag: pan\nWheel: zoom\n{:?} + drag: zoom (up=in, down=out)\n{:?} + drag: pan (sideways / up / down)", self.preferences.reverse, self.preferences.smooth, self.preferences.zoom_hold, self.preferences.pan_hold));
            }
        }
        if matches!(tool, Tool::Brush | Tool::Alpha | Tool::Presets)
            && let Some(focus) = ui.ctx().memory(|m| m.focused())
            && Some(focus) != focus_before
        {
            self.brush_editor_focus = Some((focus, original.identity()));
        }
        if &edited != original
            && let Some(selected) = active
        {
            commands.push(Command::UpdateBrush(selected, edited));
        }
    }
}

impl UiState {
    fn submodel_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        ui.label("Click a model row to select it. The eye toggles visibility.");
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            ui.label("OBJ path");
            ui.add(egui::TextEdit::singleline(&mut self.obj_path).hint_text("/path/to/model.obj"));
            if ui.add_enabled(document.model_count() < 64, design::button(ui, Glyph::Open, "Import OBJ"))
                .on_hover_text("Add a new submodel at its original position and scale. Keeps this project; one undo step. Geometry and face groups only.")
                .customize(Action::ImportObj).clicked() { commands.push(Command::ImportObj); }
        });
        ui.separator();
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                if document.primitive().is_none() && ui.add(design::button(ui, Glyph::Primitive, "Primitive")).customize(Action::Primitive).clicked() { commands.push(Command::RequestPrimitive); }
                if ui.add(design::button(ui, Glyph::Copy, "Duplicate")).customize(Action::DuplicateModel).clicked() { commands.push(Command::DuplicateModel); }
                let merge_target = document.merge_down_target();
                let merge_hint = merge_target.map_or_else(|| "No submodel below the active one".into(), |i| format!("Combine with {} below. Keeps positions, masks and groups without welding. Bakes primitives and thickness; resets subdivision levels. Undo restores both models.", document.model(i).unwrap().name));
                if ui.add_enabled(merge_target.is_some(), design::button(ui, Glyph::Layers, "Merge down")).on_hover_text(merge_hint).customize(Action::MergeDown).clicked() { commands.push(Command::MergeDown); }
                if ui.add(design::button(ui, Glyph::Mask, "Extract masked")).on_hover_text("Copy faces meeting the mask threshold at every vertex; keep the source. Boundaries remain open.").customize(Action::ExtractMasked).clicked() { commands.push(Command::ExtractMasked); }
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.group_name).desired_width(120.).hint_text("New submodel group"));
                if ui.add(design::button(ui, Glyph::Layers, "+ Group")).clicked() { commands.push(Command::AddGroup(std::mem::take(&mut self.group_name))); }
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
                                    frame = frame.fill(ui.visuals().selection.bg_fill).stroke(ui.visuals().selection.stroke);
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
            ui.separator();
            if !self.preferences.menu_placed(BuiltinMenu::Primitive) { self.primitive_controls(ui, document, commands); }
            ui.label("Vertex groups");
            ui.add_enabled_ui(document.primitive().is_none(), |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.vertex_group_name).hint_text("Group name (optional)"));
                ui.horizontal(|ui| {
                    if ui.button("Group masked").customize(Action::GroupMasked).clicked() { commands.push(Command::CreateVertexGroup(self.vertex_group_name.clone(), false)); }
                    if ui.button("Group all").on_hover_text("Replace all vertex groups on the active model with one group. Includes every subdivision level; Undo restores the previous groups.").customize(Action::GroupAll).clicked() { commands.push(Command::CreateVertexGroup(self.vertex_group_name.clone(), true)); }
                });
                if ui.add(design::button(ui, Glyph::Restore, "Flip visibility")).on_hover_text("Swap visible and hidden faces on the active submodel. Includes partially hidden groups; supports Undo.").customize(Action::InvertVisibility).clicked() {
                    commands.push(Command::InvertVisibility);
                }
                let mut group_visibility = vec![None; model.mesh.vertex_groups.len()];
                for (face, &id) in model.mesh.polygroups.iter().enumerate() {
                    if id != 0 { *group_visibility[id - 1].get_or_insert(false) |= model.mesh.face_visible(face); }
                }
                for (g, group) in model.mesh.vertex_groups.iter().enumerate() {
                    let color = crate::sculpt::Mesh::group_color(g).map(|c| (c * 255.) as u8);
                    let visible = group_visibility[g].unwrap_or(group.visible);
                    ui.horizontal(|ui| {
                        if visibility_button(ui, visible).on_hover_text("Show/hide faces assigned to this vertex group. Hidden faces cannot be picked or sculpted.").clicked() { commands.push(Command::VertexGroupVisible(g, !visible)); }
                        ui.colored_label(Color32::from_rgb(color[0], color[1], color[2]), "●");
                        if ui.selectable_label(self.selected_vertex_group == Some((index, g)), &group.name).on_hover_text(format!("{} weighted vertices", group.weights.iter().filter(|&&w| w > 0.).count())).clicked() {
                            self.selected_vertex_group = Some((index, g));
                            self.vertex_group_rename = group.name.clone();
                        }
                    });
                }
                if let Some((target, g)) = self.selected_vertex_group
                    && target == index && g < model.mesh.vertex_groups.len() {
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Mask group").customize(Action::MaskGroup).clicked() { commands.push(Command::MaskVertexGroup(g)); }
                            if ui.button("Add masked").customize(Action::AddMaskedToGroup).clicked() { commands.push(Command::AssignMaskToGroup(g, false)); }
                            if ui.button("Remove masked").customize(Action::RemoveMaskedFromGroup).clicked() { commands.push(Command::AssignMaskToGroup(g, true)); }
                        });
                        ui.add(egui::TextEdit::singleline(&mut self.vertex_group_rename).hint_text("Rename group"));
                        ui.horizontal(|ui| {
                            if ui.button("Rename").customize(Action::RenameGroup).clicked() { commands.push(Command::RenameVertexGroup(g, self.vertex_group_rename.clone())); }
                            if ui.button("Delete group").customize(Action::DeleteGroup).clicked() { commands.push(Command::DeleteVertexGroup(g)); }
                        });
                }
                ui.weak("Vertex groups store soft weights independently of the mask. Complete selected faces receive the group’s color and visibility; each face is assigned to at most one group.");
            });
            ui.separator();
            ui.label("Thickness · open meshes");
            let mut thickness=model.metadata.temporary_thickness;
            let mut enabled=thickness>0.;
            ui.add_enabled_ui(!model.mesh.triangles.is_empty(),|ui| {
                if ui.checkbox(&mut enabled,"Thickness modifier").on_hover_text("Preview a centered shell with closed edges. The sculpting surface stays unchanged until Apply.").changed() {
                    commands.push(Command::SetTemporaryThickness(if enabled {0.05}else{0.}));
                }
                if thickness>0. {
                    if ui.add(egui::DragValue::new(&mut thickness).speed(0.005).range(0.0001..=100.).prefix("Thickness ")).on_hover_text("Total shell thickness in model units, centered on the surface.").changed() {
                        commands.push(Command::SetTemporaryThickness(thickness));
                    }
                    let mut metadata = model.metadata.clone();
                    design::help(ui.add(egui::Slider::new(&mut metadata.thickness_loops,0..=8).text("Rim loops")), "Add loops through the shell wall. Updates the preview immediately; set to 0 to remove them before applying.");
                    design::help(ui.checkbox(&mut metadata.thickness_crease,"Crease rim edges"), "Keep the front/rim and back/rim joins sharp during later smooth subdivision. Updates the preview immediately; turn off to remove these marks before applying.");
                    ui.checkbox(&mut metadata.thickness_groups, "Group front / back / rim");
                    ui.checkbox(&mut metadata.thickness_mask_rim, "Mask rim vertices");
                    ui.checkbox(&mut metadata.thickness_group_mask, "Group masked vertices");
                    if metadata != *model.metadata { commands.push(Command::EditSubmodel(index, model.name.into(), metadata)); }
                    if ui.add_enabled(model.metadata.visible,design::button(ui,Glyph::Mesh,"Apply Thickness modifier")).on_hover_text("Bake the preview into a closed mesh. Resets subdivision levels and converts primitives to a sculpting mesh. Undo restores the original.").customize(Action::ApplyThickness).clicked() {
                        commands.push(Command::ApplyThickness);
                    }
                }
            });
            if !document.active_visible() { ui.weak("Active model is hidden; show it to sculpt or transform."); }
        });
    }
    fn mesh_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if document.primitive().is_some() {
            ui.label("This submodel is parametric. Edit its controls or convert it in Submodels → Primitive.");
            return;
        }
        let settings = document.mesh_settings();
        ui.label(format!(
            "{} vertices • {} faces",
            document.mesh().vertices.len(),
            document.mesh().triangles.len()
        ));
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(!settings.dynamic && !settings.adaptive.enabled,"Subdivision").customize(Action::SubdivisionMode).clicked() { commands.push(Command::SetDynamicMesh(false)); }
                if ui.selectable_label(settings.dynamic,"Dynamic mesh").customize(Action::DynamicMode).clicked() { commands.push(Command::SetDynamicMesh(true)); }
                if ui.selectable_label(settings.adaptive.enabled,"Adaptive sculpt").customize(Action::AdaptiveMode).clicked() { let mut adaptive=settings.adaptive.clone(); adaptive.enabled=true; commands.push(Command::SetAdaptive(adaptive)); }
            });
            if settings.adaptive.enabled {
                ui.label("Add detail or simplify locally as you sculpt.");
                let mut adaptive=settings.adaptive.clone();
                ui.checkbox(&mut adaptive.relative,"Detail relative to brush size");
                let label=if adaptive.relative { "Edge size (% of radius)" } else { "Edge size (px)" };
                ui.add(egui::Slider::new(&mut adaptive.detail,1. ..=32.).text(label));
                ui.checkbox(&mut adaptive.refine,"Refine stretched edges");
                ui.checkbox(&mut adaptive.simplify,"Simplify dense areas");
                ui.add_enabled(adaptive.simplify,egui::Slider::new(&mut adaptive.collapse_ratio,0.1..=0.8).text("Simplify ratio"));
                if adaptive!=settings.adaptive { commands.push(Command::SetAdaptive(adaptive.clone())); }
                ui.label("Smaller edge sizes add detail. Masks are preserved; Mask brush keeps topology fixed.");
                if adaptive.simplify {
                    let unit = if adaptive.relative { "% of brush radius" } else { "px" };
                    ui.label(format!("Smooth removes safe edges shorter than {:.1} {unit} without adding vertices.", adaptive.detail * adaptive.collapse_fraction(Kind::Smooth)));
                    ui.weak("Increase Edge size for more reduction. Masks, sharp features, and mesh boundaries are protected.");
                }
                ui.label("Local adaptation preserves separate parts; use voxel remeshing to fuse them.");
            } else if settings.dynamic {
                ui.label("Voxel remeshing replaces topology and clears masks. Small details below the voxel size may be lost.");
                let mut resolution=settings.resolution;
                let mut auto=settings.auto_remesh;
                ui.add(egui::Slider::new(&mut resolution,16..=96).text("Resolution"));
                ui.label("Voxels along the longest bounding-box axis.");
                ui.checkbox(&mut auto,"Remesh after each stroke");
                if resolution!=settings.resolution || auto!=settings.auto_remesh { commands.push(Command::SetRemeshSettings(resolution,auto)); }
                if ui.add(design::button(ui, Glyph::Restore, "Remesh now")).customize(Action::Remesh).clicked() { commands.push(Command::Remesh); }
            } else {
                ui.label(format!("Level {} of {}",settings.level(),settings.levels()));
                if settings.levels() > 1 {
                let mut level=settings.level();
                ui.add(egui::Slider::new(&mut level,1..=settings.levels()).text("Active level"));
                if level!=settings.level() { commands.push(Command::SetMeshLevel(level)); }
                ui.horizontal(|ui| {
                    if ui.add_enabled(settings.level()>1,design::button(ui, Glyph::Minus, "Lower")).customize(Action::LowerLevel).clicked() { commands.push(Command::SetMeshLevel(settings.level()-1)); }
                    if ui.add_enabled(settings.level()<settings.levels(),design::button(ui, Glyph::Plus, "Higher")).customize(Action::HigherLevel).clicked() { commands.push(Command::SetMeshLevel(settings.level()+1)); }
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(!settings.lower.is_empty(),design::button(ui, Glyph::Delete, "Delete lower")).on_hover_text("Remove all levels below the active level; keep the active surface and higher detail. Undo restores them.").customize(Action::DeleteLowerLevels).clicked() { commands.push(Command::DeleteLowerLevels); }
                    if ui.add_enabled(!settings.upper.is_empty(),design::button(ui, Glyph::Delete, "Delete higher")).on_hover_text("Remove all levels above the active level; keep the active surface and lower levels. Undo restores them.").customize(Action::DeleteHigherLevels).clicked() { commands.push(Command::DeleteHigherLevels); }
                });
                }
                if ui.add_enabled(settings.upper.is_empty() && document.mesh().triangles.len() <= 250_000,design::button(ui, Glyph::Plus, "Subdivide (+ level)")).customize(Action::Subdivide).clicked() { commands.push(Command::Subdivide); }
                ui.weak("Adds an editable level without smoothing.");
                ui.separator();
                self.smooth_subdivision_controls(ui, document, commands);
            }
        });
        ui.separator();
        self.modeling_controls(ui, document, commands);
        self.edge_controls(ui, document, commands);
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

//! Immediate-mode UI. Widgets emit commands; they never perform IO or sculpt edits.
use crate::{
    camera::Camera,
    command::{Command, PendingAction},
    document::Document,
    preferences::{Action, BuiltinMenu, Dock, Hold, Preferences, Shortcut, SidebarSection, Tool},
    sculpt::{Alpha, Axis, Kind},
};
use eframe::egui::{self, Color32, Sense, Vec2};
mod about;
pub(crate) mod alphas;
mod appearance;
mod design;
mod material;
mod operation_status;
pub(crate) mod search;
mod startup;
mod uv;
use design::Glyph;
pub(crate) mod context_menu;
mod customization;
mod edges;
mod files;
mod touch;
mod viewport_controls;
pub(crate) use files::{FileEntry, FilePicker};
mod brush_controls;
mod fusion;
mod icons;
mod instances;
mod masking;
mod menus;
mod mesh_capacity;
mod mirror;
mod modeling;
mod paint;
mod path_thumbnail;
pub(crate) mod paths;
mod primitive;
mod projection;
mod retopology;
pub(crate) use fusion::FusionKey;
pub(crate) use projection::{SurfaceMode, SurfaceReport};
mod references;
mod sidebar;
mod transform;
use customization::ActionResponse;
pub(crate) struct UiState {
    pub(crate) search: search::SearchState,
    pub(crate) zen_mode: bool,
    pub(crate) narrow_panel: Option<Dock>,
    pub(crate) search_blocks_frame: bool,
    pub instances: instances::Controls,
    pub uv: uv::UvControls,
    pub viewport_controls: viewport_controls::ControlsState,
    pub viewport_menu: context_menu::ViewportMenuState,
    icons: icons::Icons,
    pinned_menu_rects: Vec<egui::Rect>,
    pub library_path: String,
    pub(crate) delete_warning: Option<(u64, String)>,
    pub(crate) primitive_warning: Option<(usize, String)>,
    pub(crate) primitive_controls_requested: bool,
    pub(crate) modeling_primitive: Option<(crate::sculpt::Primitive, u64)>,
    pub transform_mode: crate::viewport::TransformMode,
    pub edges: edges::EdgeControls,
    pub projection: projection::ProjectionControls,
    pub(crate) fusion: fusion::FusionControls,
    pub paths_active: bool,
    pub path_brush_active: bool,
    pub paths_history_ready: bool,
    pub paths_history_reason: Option<&'static str>,
    pub paths_preview_active: bool,
    pub guide_paths: bool,
    pub path_starts_only: bool,
    pub modeling: modeling::ModelingControls,
    pub mirror_axis: Axis,
    pub mirror_quad_bridge: bool,
    pub mirror_centreline: bool,
    pub slice_active: bool,
    pub selection_tool: Option<crate::viewport::SelectionTool>,
    pub slice_path_mode: bool,
    pub slice_options: crate::sculpt::SliceOptions,
    pub transform_operation: crate::viewport::TransformOperation,
    pub boolean_enabled: bool,
    pub boolean_ready: bool,
    pub boolean_notice: String,
    pub cage_axis: crate::sculpt::Axis,
    pub respect_transform_mask: bool,
    group_name: String,
    pub selected_vertex_group: Option<(usize, usize)>,
    pub vertex_group_name: String,
    pub vertex_group_rename: String,
    rename_model: Option<(usize, String, String)>,
    pub preferences: Preferences,
    pub(super) customization: customization::Customization,
    pub about_open: bool,
    pub startup_notice_open: bool,
    pub startup_notice_skip: bool,
    pub settings_open: bool,
    pub mesh_capacity_open: bool,
    pub diagnostics_open: bool,
    pub diagnostic_path: String,
    pub recovery_open: bool,
    pub layout_open: bool,
    pub capturing: Option<Action>,
    pub settings_message: String,
    pub mesh_busy: bool,
    pub selected: usize,
    pub(crate) mask_active: bool,
    pub(crate) mask_suppressed: bool,
    smooth_brush: Option<String>,
    brush_editor_focus: Option<(egui::Id, String)>,
    pub(crate) brush_size_preview: Option<(u64, usize)>,
    pub file_picker: Option<FilePicker>,
    pub path: String,
    pub obj_path: String,
    pub reference_path: String,
    pub arrange_references: bool,
    pub references_visible: bool,
    pub selected_reference: usize,
    pub preset_path: String,
    pub alpha_path: String,
    pub alphas: alphas::AlphaControls,
    pub materials: material::MaterialControls,
    pub appearance_brush: Option<String>,
    pub light_controls: crate::lighting::LightControls,
    pub export_busy: bool,
    pub status: String,
    pub pending: Option<PendingAction>,
    pub pending_save_path: String,
    pub wire: bool,
    pub wire_mode: crate::viewport::WireMode,
    pub wire_triangles: bool,
    pub flat_shading: bool,
    pub show_polygroups: bool,
    pub show_backfaces: bool,
    pub backface_opacity: f32,
    pub show_symmetry_guides: bool,
    pub guide_opacity: f32,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            search: Default::default(),
            zen_mode: false,
            narrow_panel: None,
            search_blocks_frame: false,
            instances: Default::default(),
            viewport_controls: Default::default(),
            uv: Default::default(),
            viewport_menu: Default::default(),
            icons: Default::default(),
            pinned_menu_rects: Vec::new(),
            library_path: "brush-library.brushes.json".into(),
            primitive_warning: None,
            delete_warning: None,
            primitive_controls_requested: false,
            modeling_primitive: None,
            transform_mode: crate::viewport::TransformMode::Sculpt,
            edges: Default::default(),
            projection: Default::default(),
            fusion: Default::default(),
            paths_active: false,
            path_brush_active: false,
            paths_history_ready: true,
            paths_history_reason: None,
            paths_preview_active: false,
            guide_paths: true,
            path_starts_only: false,
            modeling: modeling::ModelingControls {
                amount: 0.1,
                snap: 0.04,
                protect_mask: true,
                tap_repeat: true,
                join: true,
                loop_count: 1,
                ..Default::default()
            },
            mirror_axis: Axis::X,
            mirror_quad_bridge: false,
            mirror_centreline: false,
            slice_active: false,
            selection_tool: None,
            slice_path_mode: false,
            slice_options: Default::default(),
            transform_operation: crate::viewport::TransformOperation::Move,
            boolean_enabled: false,
            boolean_ready: false,
            boolean_notice: String::new(),
            cage_axis: crate::sculpt::Axis::Y,
            respect_transform_mask: true,
            group_name: String::new(),
            selected_vertex_group: None,
            vertex_group_name: String::new(),
            vertex_group_rename: String::new(),
            rename_model: None,
            preferences: Preferences::default(),
            customization: Default::default(),
            about_open: false,
            startup_notice_open: false,
            startup_notice_skip: false,
            settings_open: false,
            mesh_capacity_open: false,
            diagnostics_open: false,
            diagnostic_path: "clay-session".into(),
            recovery_open: false,
            layout_open: false,
            capturing: None,
            settings_message: String::new(),
            mesh_busy: false,
            selected: 0,
            mask_active: false,
            mask_suppressed: false,
            smooth_brush: None,
            brush_editor_focus: None,
            brush_size_preview: None,
            file_picker: None,
            path: "sculpt.clay".into(),
            obj_path: "model.obj".into(),
            reference_path: String::new(),
            arrange_references: false,
            references_visible: true,
            selected_reference: 0,
            preset_path: "brush.brush.json".into(),
            alpha_path: String::new(),
            alphas: Default::default(),
            materials: Default::default(),
            appearance_brush: None,
            light_controls: Default::default(),
            export_busy: false,
            status: "Ready • Drag clay to sculpt; Alt-drag empty space to pan; Shift smooths"
                .into(),
            pending: None,
            pending_save_path: "sculpt.clay".into(),
            wire: false,
            wire_mode: Default::default(),
            wire_triangles: false,
            flat_shading: false,
            show_polygroups: false,
            show_backfaces: false,
            backface_opacity: 0.15,
            show_symmetry_guides: true,
            guide_opacity: 0.85,
        }
    }
}
impl UiState {
    pub(crate) fn set_zen_mode(&mut self, enabled: bool) {
        // Navigation uses the current viewport bounds, so saved relative
        // positions follow its edges when workspace panels are hidden.
        self.zen_mode = enabled;
    }

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

    pub(crate) fn selection_brush_active(&self) -> bool {
        self.mask_active || self.viewport_menu.visibility_was_active
    }

    /// Selection holds borrow brush settings without selecting a brush in the UI.
    pub(crate) fn highlighted_brush_index(
        &self,
        document: &Document,
        ctx: &egui::Context,
    ) -> Option<usize> {
        if self.instances.placing || self.selection_brush_active() {
            None
        } else {
            self.brush_index_for_context(document, ctx)
        }
    }

    pub(crate) fn brush_index_for_context(
        &self,
        document: &Document,
        ctx: &egui::Context,
    ) -> Option<usize> {
        if self.selection_brush_active() && !ctx.wants_keyboard_input() {
            return self.preferences.mask_preset.brush_index(document);
        }
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
        self.instances.stop_placing = true;
        self.materials.sampling = false;
        if document
            .brushes()
            .get(index)
            .is_some_and(|b| b.kind == Kind::Paint)
        {
            self.show_polygroups = false;
            self.appearance_brush = Some(document.brushes()[index].identity());
        }
        // Remember both the preset being left and a newly chosen Smooth variation.
        for i in [self.selected, index] {
            if let Some(brush) = document.brushes().get(i)
                && brush.kind == Kind::Smooth
            {
                self.smooth_brush = Some(brush.identity());
            }
        }
        if let Some(brush) = document.brushes().get(index)
            && brush.kind == Kind::Mask
        {
            self.preferences.mask_preset.brush_id = Some(brush.identity());
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
        let narrow = ctx.content_rect().width() < 960.;
        let compact = self.preferences.compact_toolbar || narrow;
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6., 5.);
            // Keep the project and finding tools in the same place in every layout.
            ui.horizontal_wrapped(|ui| {
                ui.strong("CLAY FORM");
                self.project_menu(ui, document, &mut commands);
                if !compact {
                    self.toolbar_action(ui, Glyph::New, "New sphere", Action::New, document, &mut commands);
                    self.toolbar_action(ui, Glyph::Open, "Open", Action::Open, document, &mut commands);
                }
                self.toolbar_action(ui, Glyph::Save, "Save", Action::Save, document, &mut commands);
                let project_path = if document.has_saved_path() { document.current_path() } else { &self.path };
                let project_name = std::path::Path::new(project_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Untitled project");
                let save_state = if !document.has_saved_path() {
                    "Not saved"
                } else if document.is_dirty() {
                    "Modified"
                } else {
                    "Saved"
                };
                ui.add_sized(egui::vec2(130., 32.), egui::Label::new(project_name).truncate())
                    .on_hover_text(project_path);
                ui.weak(save_state);
                ui.separator();
                let search_help = self.preferences.shortcut(&Action::Search)
                    .map_or_else(|| "Find a tool, setting or command".into(), |shortcut| {
                        format!("Find a tool, setting or command · {}", shortcut.label())
                    });
                if design::help(ui.add(design::button(ui, Glyph::Search, "Search…")), search_help)
                    .customize(Action::Search).clicked() {
                    commands.push(Command::Action(Action::Search));
                }
                let workspace_label = if compact {
                    let name = self.workspace_label();
                    let short: String = name.chars().take(20).collect();
                    format!("Workspace: {short}{}", if name.chars().count() > 20 { "…" } else { "" })
                } else {
                    "Workspace…".into()
                };
                if design::help(ui.add(design::button(ui, Glyph::Settings, workspace_label)),
                    "Arrange panels, customise shelves and shortcuts, or recover your layout").clicked() {
                    self.layout_open = true;
                }
                if !compact {
                    self.workspace_selector(ui);
                }
                if narrow {
                    ui.menu_button("Panels", |ui| {
                        ui.weak("Temporary view for this window size");
                        for (label, dock) in [("Sculpting panels", Some(Dock::Left)), ("Scene panels", Some(Dock::Right)), ("Hide panels", None)] {
                            if ui.selectable_value(&mut self.narrow_panel, dock, label).clicked() { ui.close(); }
                        }
                    });
                }
                if design::help(ui.add(design::button(ui, Glyph::View, "Zen mode")),
                    "Hide workspace panels while keeping navigation and search within reach")
                    .customize(Action::ToggleZen).clicked() {
                    commands.push(Command::Action(Action::ToggleZen));
                }
                if self.export_busy {
                    ui.spinner();
                    if ui.button("Cancel export").clicked() {
                        commands.push(Command::CancelExport);
                    }
                }
                if self.mesh_busy {
                    ui.spinner();
                    self.toolbar_action(ui, Glyph::Close, "Cancel mesh operation", Action::CancelMeshOperation, document, &mut commands);
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                self.toolbar_action(ui, Glyph::Undo, "Undo", Action::Undo, document, &mut commands);
                self.toolbar_action(ui, Glyph::Redo, "Redo", Action::Redo, document, &mut commands);
                ui.separator();
                self.builtin_menu(ui, BuiltinMenu::Symmetry, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Edit, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Settings, document, camera, &mut commands);
                self.custom_menu_bar(ui, document, &mut commands);
                if compact {
                    self.builtin_menu(ui, BuiltinMenu::Modeling, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::View, document, camera, &mut commands);
                    ui.menu_button("More tools", |ui| {
                        egui::ScrollArea::vertical()
                            .max_height((ui.ctx().content_rect().height() - 100.).max(120.))
                            .show(ui, |ui| {
                        ui.weak("Appearance");
                        for menu in [BuiltinMenu::Materials, BuiltinMenu::Colours, BuiltinMenu::Textures] {
                            self.builtin_menu(ui, menu, document, camera, &mut commands);
                        }
                        self.toolbar_action(ui, Glyph::Layers, "UV layout", Action::OpenUv, document, &mut commands);
                        ui.separator();
                        ui.weak("Model and view");
                        for menu in [BuiltinMenu::Mesh, BuiltinMenu::Slice, BuiltinMenu::Transform,
                            BuiltinMenu::Primitive, BuiltinMenu::Submodels, BuiltinMenu::Mask,
                            BuiltinMenu::Visibility, BuiltinMenu::Wireframe, BuiltinMenu::References] {
                            self.builtin_menu(ui, menu, document, camera, &mut commands);
                        }
                            });
                    });
                    return;
                }
                self.builtin_menu(ui, BuiltinMenu::Mesh, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Modeling, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Slice, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Transform, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Primitive, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Submodels, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Mask, document, camera, &mut commands);
                self.builtin_menu(ui, BuiltinMenu::Visibility, document, camera, &mut commands);
            });
            if !compact {
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Appearance");
                    self.builtin_menu(ui, BuiltinMenu::Materials, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::Colours, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::Textures, document, camera, &mut commands);
                    self.toolbar_action(ui, Glyph::Layers, "UV layout", Action::OpenUv, document, &mut commands);
                    ui.separator();
                    ui.weak("View");
                    if ui.add(design::button(ui, Glyph::Wire, "Show wireframe").selected(self.wire))
                        .customize(Action::Wireframe).clicked() {
                        self.wire = !self.wire;
                    }
                    self.builtin_menu(ui, BuiltinMenu::Wireframe, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::View, document, camera, &mut commands);
                    self.builtin_menu(ui, BuiltinMenu::References, document, camera, &mut commands);
                    self.toolbar_action(ui, Glyph::Restore, "Reset view", Action::ResetView, document, &mut commands);
                });
            }
            let has_operands = (0..document.model_count()).any(|i| {
                document.model(i).unwrap().metadata.boolean_role != crate::sculpt::BooleanRole::Regular
            });
            if document.texture_paint.is_some() || self.boolean_enabled || has_operands {
                ui.horizontal_wrapped(|ui| {
                    if let Some(channel) = document.texture_paint {
                        ui.label(format!("Paint target: texture {}", channel.label()));
                    }
                    if (self.boolean_enabled || has_operands) && ui.selectable_label(self.boolean_enabled, "Boolean result")
                        .on_hover_text("Switch the main viewport between the evaluated stack and its source operands").clicked() {
                        commands.push(Command::ToggleBooleanResult);
                    }
                });
            }
            if self.preferences.paint_workspace_backup.is_some() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Paint focus");
                    if ui.button("Restore workspace")
                        .on_hover_text("Return to the layout from before Paint focus").clicked() {
                        self.finish_search_reveal(ui.ctx(), false);
                        self.preferences.restore_paint_workspace();
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
        if let Some(tool) = self.selection_tool
            && !self.viewport_menu.visibility_was_active
        {
            egui::TopBottomPanel::top("visibility_selection_controls").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(tool.label());
                    if let crate::viewport::SelectionTool::Visibility(mut settings) = tool {
                        for shape in crate::preferences::VisibilityShape::ALL {
                            ui.selectable_value(&mut settings.shape, shape, shape.label());
                        }
                        ui.selectable_value(&mut settings.hide, true, "Hide")
                            .customize(Action::VisibilityHide);
                        ui.selectable_value(&mut settings.hide, false, "Isolate")
                            .customize(Action::VisibilityIsolate);
                        design::checkbox(ui, &mut settings.flip, "Flip side / region")
                            .customize(Action::FlipVisibilitySide);
                        if settings.tool() != tool {
                            self.preferences.visibility_selection = settings;
                            self.selection_tool = Some(settings.tool());
                        }
                    }
                    if let crate::viewport::SelectionTool::MaskLine {
                        mut erase,
                        mut flip,
                    } = tool
                    {
                        ui.selectable_value(&mut erase, false, "Add mask");
                        ui.selectable_value(&mut erase, true, "Erase mask");
                        design::checkbox(ui, &mut flip, "Flip side")
                            .customize(Action::FlipMaskLine);
                        self.selection_tool =
                            Some(crate::viewport::SelectionTool::MaskLine { erase, flip });
                        if let Some(binding) = self.preferences.shortcut(&Action::FlipMaskLine) {
                            ui.label(format!("Flip: {}", binding.label()));
                        }
                    }
                    ui.separator();
                    ui.label(tool.instructions());
                    ui.separator();
                    if matches!(
                        tool,
                        crate::viewport::SelectionTool::Visibility(
                            crate::preferences::VisibilitySettings {
                                shape: crate::preferences::VisibilityShape::Path,
                                ..
                            }
                        )
                    ) {
                        ui.label("Through all depths · red faces will hide · Esc to cancel");
                    } else {
                        ui.label("Through all depths · Space to move · Esc to cancel");
                    }
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
        let narrow = ctx.content_rect().width() < 960.;
        for dock in [Dock::Left, Dock::Right] {
            if narrow && self.narrow_panel != Some(dock) {
                continue;
            }
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
                .width_range(180. ..=if narrow { 300. } else { 600. })
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
            if narrow {
                // A narrow-window reveal never changes the saved panel widths.
            } else if dock == Dock::Left {
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
                .current_pos(panel.position)
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
                self.search_placement_drag(
                    SidebarSection::Tool(panel.tool),
                    response.response.dragged(),
                );
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
        if self.file_picker.is_some() {
            return None;
        }
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
        search::mark_destination(ui, &format!("panel:{tool:?}"));
        if self.instances.placing && tool == Tool::Brush {
            ui.add_enabled_ui(!self.mesh_busy, |ui| {
                instances::settings(ui, &mut self.instances, document, commands);
            });
            return;
        }
        let focus_before = ui.ctx().memory(|m| m.focused());
        let active = self.brush_index_for_context(document, ui.ctx());
        let fallback = crate::sculpt::Brush::new(if self.selection_brush_active() {
            Kind::Mask
        } else {
            Kind::Smooth
        });
        let original = active.map_or(&fallback, |index| &document.brushes()[index]);
        let selected = active.unwrap_or(self.selected);
        if active.is_none() && matches!(tool, Tool::Brush | Tool::Alpha | Tool::Presets) {
            let kind = if self.selection_brush_active() {
                Kind::Mask
            } else {
                Kind::Smooth
            };
            ui.label(format!(
                "Temporary {kind:?} uses default settings. Add a {kind:?} preset to customize it."
            ));
            if ui.button(format!("Add {kind:?} preset")).clicked() {
                commands.push(Command::AddBrush(kind));
            }
            return;
        }
        if self.selection_brush_active() {
            ui.strong(if self.mask_active {
                "Selection · Mask"
            } else {
                "Selection · Visibility"
            });
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
                let width = ((ui.available_width() - 6. * (columns - 1) as f32) / columns as f32)
                    .floor()
                    .max(32.);
                if self.instances.placing {
                    let action = Action::ObjectBrushModels;
                    let mut button = egui::Button::new("Choose model…");
                    if let Some(shortcut) = self.preferences.shortcut(&action) {
                        button = button.shortcut_text(shortcut.label());
                    }
                    if ui.add(button).clicked() {
                        commands.push(Command::Action(action.clone()));
                    }
                    self.brush_control(ui, &action, "", document, commands);
                    ui.separator();
                }
                // Fixed-size tiles do not need a grid's cached column widths:
                // those widths lag a sidebar drag and force it back open.
                ui.push_id("brush_tiles", |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(6.);
                    ui.horizontal_wrapped(|ui| {
                        for (i, b) in document.brushes().iter().enumerate() {
                            let id = self.brush_texture(ui.ctx(), i, b);
                            if design::brush_tile(
                                ui,
                                id,
                                &b.name,
                                !self.path_brush_active
                                    && self.highlighted_brush_index(document, ui.ctx()) == Some(i),
                                width,
                            )
                            .customize(Action::SelectBrush(b.identity()))
                            .clicked()
                            {
                                if self.paths_active {
                                    commands
                                        .push(Command::Action(Action::SelectBrush(b.identity())));
                                } else {
                                    self.select_brush(i, document);
                                }
                            }
                            if (i + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                        if ui
                            .add_enabled_ui(document.active_visible(), |ui| {
                                design::brush_tile_with_icon(
                                    ui,
                                    "Path Brush",
                                    self.path_brush_active,
                                    width,
                                    |ctx| Some(design::icon(ctx, Glyph::Path)),
                                )
                                .on_hover_text("Draw and edit paths in the viewport")
                                .clicked()
                            })
                            .inner
                        {
                            commands
                                .push(Command::Path(crate::command::PathCommand::ActivateBrush));
                        }
                        instances::library_tiles(
                            ui,
                            &mut self.instances,
                            document,
                            commands,
                            width,
                        );
                    });
                });
                self.search_object_library(ui, document, commands);
                ui.add_space(4.);
                if self.instances.placing || self.path_brush_active {
                    return;
                }
                if active.is_none() {
                    ui.weak(if self.selection_brush_active() {
                        "Selection · default Mask preset"
                    } else {
                        "Temporary Smooth · default settings"
                    });
                    return;
                }
                design::section_header(ui, "Brush library management")
                    .id_salt("brush_library_management")
                    .open(search::requested_section(
                        ui.ctx(),
                        "brush_library_management",
                    ))
                    .default_open(false)
                    .show(ui, |ui| {
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
                    });
            }
            Tool::Brush => {
                ui.strong(format!("Active brush: {}", b.name));
                if b.kind == Kind::Paint {
                    self.paint_controls(ui, b, document, commands);
                    ui.horizontal_wrapped(|ui| {
                    if ui.add(design::button(ui, Glyph::Edit, if document.texture_paint.is_some() { "Fill mapped image" } else { "Fill all vertices" }))
                        .customize(Action::FillAllMaterial)
                        .on_hover_text("Fill the active target at full strength, including masked and hidden regions. One Undo restores the previous paint.")
                        .clicked() {
                        commands.push(Command::FillAllMaterial(b.clone()));
                    }
                    if ui
                        .add(design::button(ui, Glyph::Edit, if document.texture_paint.is_some() { "Fill unmasked image" } else { "Fill unmasked vertices" }))
                        .customize(Action::FillMaterial)
                        .clicked()
                    {
                        commands.push(Command::FillMaterial(b.clone()));
                    }
                    });
                }
                if b.kind == Kind::Decimate {
                    ui.label("Reduce triangles locally without smoothing. Requires Adaptive sculpt; Edge size and Simplify ratio control reduction.");
                }
                ui.add_enabled_ui(!matches!(b.kind, Kind::Decimate | Kind::Relax), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut b.subtract, false, "+ Add")
                            .customize(Action::BrushAdd);
                        ui.selectable_value(&mut b.subtract, true, "− Subtract")
                            .customize(Action::BrushSubtract);
                    });
                });
                let radius = ui
                    .add(egui::Slider::new(&mut b.radius, 1. ..=500.).text("Radius (px)"))
                    .customize(Action::BrushSize);
                self.preview_brush_size(&radius, selected);
                ui.add(egui::Slider::new(&mut b.strength, 0. ..=1.).text("Strength"))
                    .customize(Action::BrushStrength);
                if b.kind == Kind::Relax {
                    ui.add(
                        egui::Slider::new(&mut b.relax_tolerance, 0.000001..=1.)
                            .logarithmic(true)
                            .text("Surface tolerance"),
                    );
                    ui.weak("Redistributes vertices without smoothing the shape. Consecutive Relax strokes share a fixed surface; another mesh edit or reopening captures a new reference. Boundaries, creases and hidden faces stay fixed. Esc cancels the stroke.");
                }
                design::section_header(ui, "Stroke settings")
                    .id_salt("brush_stroke_settings")
                    .open(search::requested_section(ui.ctx(), "brush_stroke_settings"))
                    .default_open(false)
                    .show(ui, |ui| {
                ui.label(if b.kind == Kind::Paint {
                    "Stroke path"
                } else {
                    "Stroke mode"
                });
                let supported = !matches!(b.kind, Kind::Grab | Kind::Snake | Kind::Relax);
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
                    ui.weak(if b.kind == Kind::Relax {
                        "Relax uses Freehand to redistribute the surface continuously."
                    } else {
                        "Grab and Snake use Freehand for continuous displacement."
                    });
                }
                ui.add(egui::Slider::new(&mut b.spacing, 0.01..=1.).text("Spacing"));
                ui.add_enabled_ui(b.supports_stabilization(), |ui| {
                    design::checkbox(ui, &mut b.stabilize, "Stroke stabilisation")
                        .customize(Action::StrokeStabilisation)
                        .on_hover_text("Freehand brush trails the pointer to smooth small movements. Settings apply to the next stroke. Grab and Snake are unaffected.");
                    ui.add_enabled(b.stabilize, egui::Slider::new(&mut b.stabilizer_distance, 1. ..=200.).text("Trailing distance (px)"));
                });
                    });
                design::section_header(ui, "Brush shape and pressure")
                    .id_salt("brush_shape_settings")
                    .open(search::requested_section(ui.ctx(), "brush_shape_settings"))
                    .default_open(false)
                    .show(ui, |ui| {
                let inner = ui.add(egui::Slider::new(&mut b.inner_radius, 0. ..=1.).text("Inner radius (fraction)"))
                    .customize(Action::BrushInnerRadius)
                    .on_hover_text("Full-strength core; falloff fades from here to the outer radius. Applies to Soft and image alphas.");
                self.preview_brush_size(&inner, selected);
                design::checkbox(ui, &mut b.surface_cursor, "Surface footprint and normal")
                    .customize(Action::SurfaceFootprint);
                ui.add(egui::Slider::new(&mut b.falloff, 0.1..=8.).text("Falloff"));
                ui.add(egui::Slider::new(&mut b.rotation, 0. ..=360.).text("Alpha rotation"));
                design::checkbox(ui, &mut b.pressure, "Pressure → strength");
                design::checkbox(ui, &mut b.pressure_radius, "Pressure → radius");
                ui.add(egui::Slider::new(&mut b.pressure_curve, 0.1..=4.).text("Pressure curve"));
                    });
                design::section_header(ui, "Brush preset settings")
                    .id_salt("brush_preset_settings")
                    .open(search::requested_section(ui.ctx(), "brush_preset_settings"))
                    .default_open(false)
                    .show(ui, |ui| {
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
                    let name = ui.add(egui::TextEdit::singleline(&mut b.name).id_salt("brush_preset_name"));
                    #[cfg(test)]
                    ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("brush_preset_name_field"), name.rect));
                    name.on_hover_text("Name this brush preset");
                    egui::ComboBox::from_label("Operation")
                        .selected_text(format!("{:?}", b.kind))
                        .show_ui(ui, |ui| {
                            for k in Kind::ALL {
                                ui.selectable_value(&mut b.kind, k, format!("{k:?}"));
                            }
                        });
                    });
            }
            Tool::Alpha => {
                self.alpha_panel(ui, document, selected, b, commands);
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
                        .add(design::button(ui, Glyph::Open, "Import brush…"))
                        .customize(Action::LoadPreset)
                        .clicked()
                    {
                        commands.push(Command::Action(Action::LoadPreset));
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
                        .add(design::button(ui, Glyph::Open, "Import library…"))
                        .customize(Action::ImportLibrary)
                        .clicked()
                    {
                        commands.push(Command::Action(Action::ImportLibrary));
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
    fn submodel_rows(ui: &mut egui::Ui, document: &Document, commands: &mut Vec<Command>) {
        let groups = std::iter::once(String::new())
            .chain(document.groups().iter().cloned())
            .collect::<Vec<_>>();
        for (section, group) in groups.iter().enumerate() {
            let indices: Vec<_> = (0..document.model_count())
                .filter(|&i| document.model(i).unwrap().metadata.group == *group)
                .collect();
            if section == 0 && indices.is_empty() {
                continue;
            }
            ui.push_id(("submodel_group", group), |ui| {
                let title = if group.is_empty() { "Ungrouped" } else { group };
                let header = egui::CollapsingHeader::new(title)
                    .open(
                        search::requested_section(ui.ctx(), &format!("model-group:{group}"))
                            .or_else(|| {
                                indices
                                    .iter()
                                    .any(|&index| {
                                        search::is_requested(
                                            ui.ctx(),
                                            &format!(
                                                "model:{}",
                                                document.model(index).unwrap().metadata.model_id
                                            ),
                                        )
                                    })
                                    .then_some(true)
                            }),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        if section > 0 {
                            ui.horizontal(|ui| {
                                let all_visible = indices
                                    .iter()
                                    .all(|&i| document.model(i).unwrap().metadata.visible);
                                if visibility_button(ui, all_visible)
                                    .on_hover_text("Show/hide this group")
                                    .clicked()
                                {
                                    commands
                                        .push(Command::GroupVisible(group.clone(), !all_visible));
                                }
                                let index = section - 1;
                                if reorder_button(ui, true, index > 0)
                                    .on_hover_text("Move group up")
                                    .clicked()
                                {
                                    commands.push(Command::ReorderGroup(index, index - 1));
                                }
                                if reorder_button(ui, false, index + 1 < document.groups().len())
                                    .on_hover_text("Move group down")
                                    .clicked()
                                {
                                    commands.push(Command::ReorderGroup(index, index + 1));
                                }
                                if ui.small_button("Ungroup").clicked() {
                                    commands.push(Command::RemoveGroup(group.clone()));
                                }
                            });
                        }
                        for (position, &index) in indices.iter().enumerate() {
                            ui.push_id(index, |ui| {
                                Self::submodel_row(ui, document, &indices, position, commands);
                            });
                        }
                        if indices.is_empty() {
                            ui.weak("Empty group");
                        }
                    });
                search::highlight_response(
                    &header.header_response,
                    &format!("model-group:{group}"),
                );
            });
        }
    }
    fn submodel_row(
        ui: &mut egui::Ui,
        document: &Document,
        siblings: &[usize],
        position: usize,
        commands: &mut Vec<Command>,
    ) {
        let index = siblings[position];
        let model = document.model(index).unwrap();
        let active = index == document.active_model_index();

        let mut select_clicked = false;
        let mut frame = egui::Frame::group(ui.style());
        if active {
            frame = frame
                .fill(ui.visuals().selection.bg_fill)
                .stroke(ui.visuals().selection.stroke);
        }
        let row = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
            frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let eye = visibility_button(ui, model.metadata.visible).on_hover_text(
                        if model.metadata.visible {
                            "Hide model"
                        } else {
                            "Show model"
                        },
                    );
                    if eye.clicked() {
                        let mut metadata = model.metadata.clone();
                        metadata.visible = !metadata.visible;
                        commands.push(Command::EditSubmodel(index, model.name.into(), metadata));
                    }
                    model_thumbnail(ui, model.display_mesh(false), active);
                    ui.vertical(|ui| {
                        let label = ui
                            .selectable_label(active, model.name)
                            .on_hover_text("Select this submodel for sculpting or transforms");
                        search::highlight_response(
                            &label,
                            &format!("model:{}", model.metadata.model_id),
                        );
                        select_clicked |= label.clicked();
                        ui.horizontal(|ui| {
                            let up = reorder_button(ui, true, position > 0)
                                .on_hover_text("Move model up in group");
                            let down = reorder_button(ui, false, position + 1 < siblings.len())
                                .on_hover_text("Move model down in group");
                            if up.clicked() {
                                commands
                                    .push(Command::ReorderSubmodel(index, siblings[position - 1]));
                            }
                            if down.clicked() {
                                commands
                                    .push(Command::ReorderSubmodel(index, siblings[position + 1]));
                            }
                            if active {
                                ui.small("Active");
                            }
                        });
                    });
                });
                let mut role = model.metadata.boolean_role;
                egui::ComboBox::from_id_salt("boolean_role")
                    .selected_text(role.label())
                    .width(94.)
                    .show_ui(ui, |ui| {
                        use crate::sculpt::BooleanRole::*;
                        for choice in [Regular, Add, Subtract, Intersect] {
                            ui.selectable_value(&mut role, choice, choice.label());
                        }
                    });
                if role != model.metadata.boolean_role {
                    let mut metadata = model.metadata.clone();
                    metadata.boolean_role = role;
                    commands.push(Command::EditSubmodel(index, model.name.into(), metadata));
                }
            });
        });
        if select_clicked || row.response.clicked() {
            commands.push(Command::SelectSubmodel(index));
        }
    }

    fn submodel_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            Self::submodel_rows(ui, document, commands)
        });
        ui.separator();
        if ui
            .selectable_label(
                !self.boolean_enabled
                    && self.transform_mode == crate::viewport::TransformMode::World
                    && self.transform_operation == crate::viewport::TransformOperation::Cage,
                "Deformation frame",
            )
            .customize(Action::DeformationFrame)
            .clicked()
        {
            commands.push(Command::TransformOperation(
                crate::viewport::TransformOperation::Cage,
            ));
        }
        if ui
            .selectable_label(self.boolean_enabled, "Boolean result")
            .on_hover_text("Show the evaluated group in the main viewport")
            .clicked()
        {
            commands.push(Command::ToggleBooleanResult);
        }
        if self.boolean_enabled {
            ui.add_enabled_ui(!self.mesh_busy, |ui| {
                ui.weak("Stack runs from bottom to top. Choose a role on each row. The eye bypasses an operand.");
                use crate::sculpt::BooleanMethod;
                let mut method = document.boolean_method();
                egui::ComboBox::from_label("Boolean method")
                    .selected_text(method.label())
                    .show_ui(ui, |ui| {
                        for choice in [BooleanMethod::Voxel, BooleanMethod::Surface] {
                            ui.selectable_value(&mut method, choice, choice.label());
                        }
                    });
                if method != document.boolean_method() {
                    commands.push(Command::BooleanMethod(method));
                }
                if method == BooleanMethod::Voxel {
                    let mut resolution = document.boolean_resolution();
                    egui::ComboBox::from_label("Voxel resolution")
                        .selected_text(resolution.to_string())
                        .show_ui(ui, |ui| {
                            for r in [16, 24, 32, 48, 64, 96] {
                                ui.selectable_value(&mut resolution, r, r.to_string());
                            }
                        });
                    if resolution != document.boolean_resolution() {
                        commands.push(Command::BooleanResolution(resolution));
                    }
                } else {
                    ui.weak("Cut the original surfaces at their intersections. Uncut faces keep their detail.");
                }
                if ui.add_enabled(self.boolean_ready && document.model_count() < 64, egui::Button::new("Bake result"))
                    .on_hover_text("Create a sculptable mesh from this preview and hide its operands. Undo restores the stack.")
                    .clicked()
                {
                    commands.push(Command::BakeBoolean);
                }
                if document.model_count() >= 64 {
                    ui.weak("Free a submodel slot to bake (64 maximum).");
                }
                ui.weak(&self.boolean_notice);
            });
            return;
        }
        if search::requested_section(ui.ctx(), "array_layers").is_some() {
            search::setting_response(ui.label("Select an array's source model, then choose Edit array to inspect its layers. Search keeps your current model and tool."), "Array layer options");
        }
        self.instance_controls(ui, document, commands);
        ui.separator();
        ui.label("Click a model row to select it. The eye toggles visibility.");
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            ui.label("OBJ path");
            ui.add(egui::TextEdit::singleline(&mut self.obj_path).hint_text("/path/to/model.obj"));
            if ui.add_enabled(document.model_count() < 64, design::button(ui, Glyph::Open, "Import OBJ…"))
                .on_hover_text("Add a new submodel at its original position and scale. Keeps this project; one undo step. Geometry and face groups only.")
                .customize(Action::ImportObj).clicked() { commands.push(Command::Action(Action::ImportObj)); }
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
            let active_name = document.model(document.active_model_index()).unwrap().name;
            ui.label(format!("Selected: {active_name}"));
            if document.model_count() == 1 {
                ui.weak("Deleting the last submodel leaves an empty model slot.");
            }
            if design::help(
                ui.add_enabled(self.action_enabled(&Action::DeleteSubmodel, document), design::button(ui, Glyph::Delete, "Delete submodel")),
                Action::DeleteSubmodel.help().unwrap(),
            ).customize(Action::DeleteSubmodel).clicked() {
                commands.push(Command::DeleteSubmodel);
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.group_name).desired_width(120.).hint_text("New submodel group"));
                if ui.add(design::button(ui, Glyph::Layers, "+ Group")).clicked() { commands.push(Command::AddGroup(std::mem::take(&mut self.group_name))); }
            });
            ui.separator();
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
                    if ui.button("Group masked").on_hover_text("Move masked membership into a new group. Keep old groups’ remaining regions and remove fully replaced groups. Supports Undo.").customize(Action::GroupMasked).clicked() { commands.push(Command::CreateVertexGroup(self.vertex_group_name.clone(), false)); }
                    if ui.button("Group all").on_hover_text("Replace all vertex groups on the active model with one group. Includes every subdivision level; Undo restores the previous groups.").customize(Action::GroupAll).clicked() { commands.push(Command::CreateVertexGroup(self.vertex_group_name.clone(), true)); }
                });
                if ui.add(design::button(ui, Glyph::Restore, "Flip visibility")).on_hover_text("Swap visible and hidden faces on the active submodel. Includes partially hidden groups; supports Undo.").customize(Action::InvertVisibility).clicked() {
                    commands.push(Command::InvertVisibility);
                }
                let mut group_visibility = vec![None; model.mesh.vertex_groups.len()];
                for (face, &id) in model.mesh.polygroups.iter().enumerate() {
                    if id != 0 { *group_visibility[id - 1].get_or_insert(false) |= model.mesh.face_visible(face); }
                }
                for (g, group) in model.mesh.vertex_groups.iter().enumerate().filter(|(_, g)| g.purpose.is_selection()) {
                    let color = crate::sculpt::Mesh::group_color(g).map(|c| (c * 255.) as u8);
                    let visible = group_visibility[g].unwrap_or(group.visible);
                    ui.horizontal(|ui| {
                        if visibility_button(ui, visible).on_hover_text("Show/hide faces assigned to this vertex group. Hidden faces cannot be picked or sculpted.").clicked() { commands.push(Command::VertexGroupVisible(g, !visible)); }
                        ui.colored_label(Color32::from_rgb(color[0], color[1], color[2]), "●");
                        let label = ui.selectable_label(self.selected_vertex_group == Some((index, g)), &group.name).on_hover_text(format!("{} weighted vertices", group.weights.iter().filter(|&&w| w > 0.).count()));
                        search::highlight_response(&label, &format!("vertex-group:{}", group.name));
                        if label.clicked() {
                            self.selected_vertex_group = Some((index, g));
                            self.vertex_group_rename = group.name.clone();
                        }
                    });
                }
                if let Some((target, g)) = self.selected_vertex_group
                    && target == index && model.mesh.vertex_groups.get(g).is_some_and(|group| group.purpose.is_selection()) {
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Mask group").customize(Action::MaskGroup).clicked() { commands.push(Command::MaskVertexGroup(g)); }
                            if ui.button("Add masked").on_hover_text("Move masked membership into this group. Keep old groups’ remaining regions and remove fully replaced groups. Supports Undo.").customize(Action::AddMaskedToGroup).clicked() { commands.push(Command::AssignMaskToGroup(g, false)); }
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
                if design::checkbox(ui, &mut enabled,"Thickness modifier").on_hover_text("Preview a centered shell with closed edges. The sculpting surface stays unchanged until Apply.").changed() {
                    commands.push(Command::SetTemporaryThickness(if enabled {0.05}else{0.}));
                }
                if thickness>0. {
                    if ui.add(egui::DragValue::new(&mut thickness).speed(0.005).range(0.0001..=100.).prefix("Thickness ")).on_hover_text("Total shell thickness in model units, centered on the surface.").changed() {
                        commands.push(Command::SetTemporaryThickness(thickness));
                    }
                    let mut metadata = model.metadata.clone();
                    design::help(ui.add(egui::Slider::new(&mut metadata.thickness_loops,0..=8).text("Rim loops")), "Add loops through the shell wall. Updates the preview immediately; set to 0 to remove them before applying.");
                    design::help(design::checkbox(ui, &mut metadata.thickness_crease,"Crease rim edges"), "Keep the front/rim and back/rim joins sharp during later smooth subdivision. Updates the preview immediately; turn off to remove these marks before applying.");
                    design::checkbox(ui, &mut metadata.thickness_groups, "Group front / back / rim");
                    design::checkbox(ui, &mut metadata.thickness_mask_rim, "Mask rim vertices");
                    design::checkbox(ui, &mut metadata.thickness_group_mask, "Group masked vertices");
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
        self.search_saved_paths(ui, document, commands);
        if document.primitive().is_some() {
            ui.label("This submodel is parametric. Edit its controls or convert it in Submodels → Primitive.");
            return;
        }
        self.modeling_controls(ui, document, commands);
        ui.separator();
        if self.modeling.active {
            egui::CollapsingHeader::new("Mesh settings")
                .open(search::requested_any_section(
                    ui.ctx(),
                    &[
                        "mesh_settings",
                        "edge_controls",
                        "edge_support",
                        "edge_cleanup",
                    ],
                ))
                .show(ui, |ui| self.mesh_settings_controls(ui, document, commands));
        } else {
            self.mesh_settings_controls(ui, document, commands);
        }
    }
    fn mesh_settings_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let settings = document.mesh_settings();
        ui.label(format!(
            "{} vertices • {} faces",
            document.mesh().vertices.len(),
            document.mesh().triangles.len() - document.mesh().quads.len()
        ));
        ui.weak(format!(
            "{} quads • {} triangles",
            document.mesh().quads.len(),
            document.mesh().triangles.len() - document.mesh().quads.len() * 2
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
                ui.horizontal_wrapped(|ui| {
                    search::setting_response(ui.label("Adaptive topology"), "Adaptive topology");
                    ui.selectable_value(&mut adaptive.topology, crate::sculpt::Topology::Quads, "Quads");
                    ui.selectable_value(&mut adaptive.topology, crate::sculpt::Topology::Triangles, "Triangles");
                });
                search::setting_response(design::checkbox(ui, &mut adaptive.unmasked_only,"Adapt only unmasked areas"), "Adapt only unmasked areas")
                    .on_hover_text("Keep faces touching any nonzero mask weight unchanged during topology adaptation. Brush deformation still follows mask strength.");
                if adaptive.topology == crate::sculpt::Topology::Quads {
                    ui.weak("Requires an all-quad mesh. Local refinement adds irregular quad junctions; masked or sharply folded areas can limit adaptation.");
                }
                search::setting_response(design::checkbox(ui, &mut adaptive.relative,"Detail relative to brush size"), "Detail relative to brush size");
                let label=if adaptive.relative { "Edge size (% of radius)" } else { "Edge size (px)" };
                search::setting_response(ui.add(egui::Slider::new(&mut adaptive.detail,1. ..=32.).text(label)), "Adaptive edge size");
                search::setting_response(design::checkbox(ui, &mut adaptive.refine,"Refine stretched edges"), "Refine stretched edges");
                search::setting_response(design::checkbox(ui, &mut adaptive.simplify,"Simplify while sculpting"), "Simplify while sculpting");
                search::setting_response(ui.add(egui::Slider::new(&mut adaptive.collapse_ratio,0.1..=0.8).text("Simplify ratio")), "Simplify ratio");
                if adaptive!=settings.adaptive { commands.push(Command::SetAdaptive(adaptive.clone())); }
                ui.label("Smaller edge sizes add detail. Masks are preserved; Mask brush keeps topology fixed.");
                ui.label("Smooth changes shape without changing topology.");
                let unit = if adaptive.relative { "% of brush radius" } else { "px" };
                ui.label(format!("Decimate removes edges up to {:.1} {unit}, preserving surface features.", adaptive.detail * adaptive.collapse_fraction(Kind::Decimate)));
                let decimate = document.brushes().iter().position(|b| b.kind == Kind::Decimate);
                let action = decimate.map_or_else(|| Action::NewBrush("Decimate".into()), |index| Action::SelectBrush(document.brushes()[index].identity()));
                if ui.add(design::button(ui, Glyph::Edit, "Decimate brush")).customize(action).clicked() {
                    if let Some(index) = decimate {
                        self.select_brush(index, document);
                    } else {
                        commands.push(Command::AddBrush(Kind::Decimate));
                    }
                }
                ui.weak("Smooth first, then paint with Decimate to reduce faces. Increase Edge size for more reduction. Decimate also works with automatic simplification off.");
                ui.label("Local adaptation preserves separate parts; use voxel remeshing to fuse them.");
            } else if settings.dynamic {
                ui.label("Voxel remeshing replaces topology and clears masks. Small details below the voxel size may be lost.");
                let mut topology = settings.remesh_topology;
                ui.horizontal_wrapped(|ui| {
                    search::setting_response(ui.label("Remesh topology"), "Remesh topology");
                    ui.selectable_value(&mut topology, crate::sculpt::Topology::Quads, "Quads");
                    ui.selectable_value(&mut topology, crate::sculpt::Topology::Triangles, "Triangles");
                });
                if topology != settings.remesh_topology { commands.push(Command::SetRemeshTopology(topology)); }
                if topology == crate::sculpt::Topology::Quads {
                    let mut algorithm = settings.quad_remesh_algorithm;
                    ui.horizontal_wrapped(|ui| {
                        search::setting_response(ui.label("Quad algorithm"), "Quad algorithm");
                        ui.selectable_value(&mut algorithm, crate::sculpt::QuadRemeshAlgorithm::Fast, "Fast quads");
                        ui.selectable_value(&mut algorithm, crate::sculpt::QuadRemeshAlgorithm::Optimized, "Optimized quads");
                    });
                    if algorithm != settings.quad_remesh_algorithm { commands.push(Command::SetQuadRemeshAlgorithm(algorithm)); }
                    ui.weak(match algorithm {
                        crate::sculpt::QuadRemeshAlgorithm::Fast => "Quickly fits grid-based quads to the sculpt. Retains grid junctions; small features may soften.",
                        crate::sculpt::QuadRemeshAlgorithm::Optimized => "Rebuilds quad connections to reduce crowded junctions. Slower; face count may change and small features may soften. Can be cancelled.",
                    });
                    ui.weak("Increase resolution for small features. Unresolved surface crossings are reported without changing the model.");
                }
                let mut resolution=settings.resolution;
                search::setting_response(ui.add(egui::Slider::new(&mut resolution,16..=96).text("Resolution")), "Remesh resolution");
                ui.label("Voxels along the longest bounding-box axis.");
                if resolution!=settings.resolution { commands.push(Command::SetRemeshResolution(resolution)); }
                if ui.add(design::button(ui, Glyph::Restore, "Remesh now")).customize(Action::Remesh).clicked() { commands.push(Command::Remesh); }
            } else {
                ui.label(format!("Level {} of {}",settings.level(),settings.levels()));
                if settings.levels() > 1 {
                let mut level=settings.level();
                search::setting_response(ui.add(egui::Slider::new(&mut level,1..=settings.levels()).text("Active level")), "Active subdivision level");
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
        self.projection_buttons(ui, document, commands);
        if design::help(ui.add_enabled(!self.mesh_busy, design::button(ui, Glyph::Mesh, "Join meshes…")), "Fuse overlapping solids locally or connect selected holes. Keeps source surfaces and subdivision levels recoverable.").customize(Action::JoinMeshes).clicked() {
            commands.push(Command::OpenFusion);
        }
        search::mark_destination(ui, "setting:Path editing options");
        if ui
            .add_enabled(
                !self.mesh_busy,
                design::button(ui, Glyph::Edit, "Editable paths"),
            )
            .customize(Action::EditPaths)
            .clicked()
        {
            commands.push(Command::Path(crate::command::PathCommand::Open));
        }
        let mut guide_paths = self.guide_paths;
        if design::checkbox(ui, &mut guide_paths, "Guide brush strokes with paths")
            .on_hover_text("Start a stroke near a saved surface path to follow only the section you draw. Turn off for freehand strokes near paths.")
            .customize(Action::TogglePathGuides).changed() {
            commands.push(Command::Action(Action::TogglePathGuides));
        }
        let mut path_starts_only = self.path_starts_only;
        if ui
            .add_enabled(
                self.guide_paths,
                egui::Checkbox::new(&mut path_starts_only, "Only start strokes on paths"),
            )
            .on_hover_text(Action::TogglePathOnlyStrokes.help().unwrap())
            .customize(Action::TogglePathOnlyStrokes)
            .changed()
        {
            commands.push(Command::Action(Action::TogglePathOnlyStrokes));
        }
        if ui
            .add_enabled(
                self.action_enabled(&Action::StrokeEntirePath, document),
                egui::Button::new("Stroke entire path"),
            )
            .on_hover_text(Action::StrokeEntirePath.help().unwrap())
            .customize(Action::StrokeEntirePath)
            .clicked()
        {
            commands.push(Command::Action(Action::StrokeEntirePath));
        }
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

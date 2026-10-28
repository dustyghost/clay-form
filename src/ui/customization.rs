//! User-owned menus, shelves and key bindings share the same action catalog.
use super::*;
use crate::preferences::{Appearance, CustomItem, CustomMenu, Placement};

#[derive(Default)]
pub(crate) struct Customization {
    selected_menu: Option<u64>,
    search: String,
    key_search: String,
    chosen: Option<Action>,
    workspace_name: String,
    pub popup_menu: Option<u64>,
    pub popup_position: Option<egui::Pos2>,
    pub popup_blocks_viewport: bool,
}
#[derive(Clone)]
enum Request {
    Add(Action),
    AddViewport(Action),
    Bind(Action),
    NewPanel(Placement),
    EditMenu(u64),
    Workspaces,
    RemoveItem(u64, usize),
    DeleteMenu(u64),
    DockMenu(u64, Placement),
}
fn request_id() -> egui::Id {
    egui::Id::new("customize_button_request")
}
/// Attach to source buttons without coupling their normal behavior to the editor.
pub(super) trait ActionResponse {
    fn customize(self, action: Action) -> Self;
    fn customize_in(self, action: Action, placement: Placement) -> Self;
    fn custom_item(self, action: Option<Action>, menu: u64, index: usize) -> Self;
}
impl ActionResponse for egui::Response {
    fn customize(self, action: Action) -> Self {
        self.customize_in(action, Placement::Menu)
    }
    fn customize_in(self, action: Action, placement: Placement) -> Self {
        self.context_menu(|ui| {
            if placement != Placement::Menu {
                sidebar_options(ui, placement);
                ui.separator();
            }
            action_options(ui, &action);
        });
        self
    }
    fn custom_item(self, action: Option<Action>, menu: u64, index: usize) -> Self {
        self.context_menu(|ui| {
            if ui.button("Remove item from this menu").clicked() {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(request_id(), Request::RemoveItem(menu, index)));
                ui.close();
            }
            if ui.button("Edit this menu…").clicked() {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(request_id(), Request::EditMenu(menu)));
                ui.close();
            }
            if let Some(action) = &action {
                ui.separator();
                action_options(ui, action);
            }
        });
        self
    }
}
pub(super) fn action_options(ui: &mut egui::Ui, action: &Action) {
    if ui.button("Add to viewport").clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(request_id(), Request::AddViewport(action.clone())));
        ui.close();
    }
    if ui.button("Add to custom menu / shelf…").clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(request_id(), Request::Add(action.clone())));
        ui.close();
    }
    if ui.button("Assign shortcut…").clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(request_id(), Request::Bind(action.clone())));
        ui.close();
    }
}

pub(super) fn sidebar_options(ui: &mut egui::Ui, placement: Placement) {
    if ui.button("New collapsible menu here…").clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(request_id(), Request::NewPanel(placement)));
        ui.close();
    }
    if ui.button("Workspaces and colours…").clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(request_id(), Request::Workspaces));
        ui.close();
    }
}
fn custom_dock_options(ui: &mut egui::Ui, menu: &CustomMenu) {
    ui.horizontal(|ui| {
        for (glyph, label, placement) in [
            (
                if menu.placement == Placement::Floating {
                    super::Glyph::Unpin
                } else {
                    super::Glyph::Pin
                },
                if menu.placement == Placement::Floating {
                    "Unpin menu"
                } else {
                    "Pin menu open"
                },
                if menu.placement == Placement::Floating {
                    Placement::Menu
                } else {
                    Placement::Floating
                },
            ),
            (
                super::Glyph::DockLeft,
                "Make collapsible on left",
                Placement::LeftPanel,
            ),
            (
                super::Glyph::DockRight,
                "Make collapsible on right",
                Placement::RightPanel,
            ),
        ] {
            if super::design::icon_button(ui, glyph, label).clicked() {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(request_id(), Request::DockMenu(menu.id, placement))
                });
                ui.close_kind(egui::UiKind::Menu);
            }
        }
    });
}
pub(super) fn menu_options(ui: &mut egui::Ui, menu: &CustomMenu) {
    if !matches!(menu.placement, Placement::LeftPanel | Placement::RightPanel) {
        custom_dock_options(ui, menu);
        ui.separator();
    }
    if ui.button("Edit this menu…").clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(request_id(), Request::EditMenu(menu.id)));
        ui.close();
    }
    if ui.button("Delete this custom menu").clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(request_id(), Request::DeleteMenu(menu.id)));
        ui.close();
    }
    ui.separator();
    sidebar_options(ui, menu.placement);
}
pub(super) fn sidebar_context(response: egui::Response, placement: Placement) {
    response.context_menu(|ui| sidebar_options(ui, placement));
}
impl UiState {
    pub fn action_label(&self, action: &Action, document: &Document) -> String {
        match action {
            Action::SelectBrush(id) => document
                .brushes()
                .iter()
                .find(|b| b.identity() == *id)
                .map_or_else(
                    || "Unavailable brush".into(),
                    |b| format!("Brush: {}", b.name),
                ),
            Action::ShowPanel(tool) => format!("Show {}", tool.label()),
            Action::OpenMenu(id) => self
                .preferences
                .menus
                .iter()
                .find(|m| m.id == *id)
                .map_or_else(|| "Unavailable menu".into(), |m| format!("Open {}", m.name)),
            Action::ModelPreset(id) => self
                .preferences
                .modeling_presets
                .iter()
                .find(|p| p.id == *id)
                .map_or_else(
                    || "Unavailable modeling preset".into(),
                    |p| format!("Modeling: {}", p.name),
                ),
            Action::NewBrush(name) => format!("Add {name} brush"),
            _ => action.label().into(),
        }
    }
    pub(super) fn action_catalog(&self, document: &Document) -> Vec<Action> {
        let mut actions = Action::ALL.to_vec();
        actions.extend(
            self.preferences
                .modeling_presets
                .iter()
                .map(|p| Action::ModelPreset(p.id)),
        );
        actions.extend(
            document
                .brushes()
                .iter()
                .map(|b| Action::SelectBrush(b.identity())),
        );
        actions.extend(Kind::ALL.iter().map(|k| Action::NewBrush(format!("{k:?}"))));
        actions.push(Action::NewBrush("Alpha Drag".into()));
        actions.extend(Tool::ALL.into_iter().map(Action::ShowPanel));
        actions.extend(
            self.preferences
                .menus
                .iter()
                .map(|m| Action::OpenMenu(m.id)),
        );
        // Keep stale bindings visible so they can be cleared after switching projects/layouts.
        for (action, _) in &self.preferences.bindings {
            if !actions.contains(action) {
                actions.push(action.clone());
            }
        }
        actions
    }
    pub fn action_enabled(&self, action: &Action, document: &Document) -> bool {
        crate::app::policy::EditorPolicy::new(self, document)
            .action(action)
            .is_ok()
    }
    pub(super) fn action_selected(
        &self,
        action: &Action,
        document: &Document,
        ctx: &egui::Context,
    ) -> bool {
        let active = self.brush_index_for_context(document, ctx);
        let brush = active.map(|index| &document.brushes()[index]);
        match action {
            Action::ToggleStencil => document.stencil().is_some_and(|s| s.enabled),
            Action::SelectBrush(id) => self
                .highlighted_brush_index(document, ctx)
                .is_some_and(|index| document.brushes()[index].identity() == *id),
            Action::ModelTool(tool) => self.modeling.active && self.modeling.tool == *tool,
            Action::ModelTarget(span) => self.modeling.active && self.modeling.span == *span,
            Action::ModelAxis(axis) => self.modeling.active && self.modeling.axis == *axis,
            Action::ModelInteraction(mode) => {
                self.modeling.active && self.modeling.interaction == *mode
            }
            Action::ModelProtectMask => self.modeling.protect_mask,
            Action::ModelSmoothPreview => self.modeling.smooth_preview,
            Action::ModelTapRepeat => self.modeling.tap_repeat,
            Action::ModelPreset(id) => self.modeling.active && self.modeling.preset == Some(*id),
            Action::PenModeling => self.modeling.active,
            Action::ModelSelectSingle => {
                self.modeling.active
                    && self.modeling.tool == crate::viewport::ModelTool::Select
                    && self.modeling.span == crate::viewport::SelectionSpan::Single
            }
            Action::ModelSelectLoops => {
                self.modeling.active
                    && self.modeling.tool == crate::viewport::ModelTool::Select
                    && self.modeling.span == crate::viewport::SelectionSpan::Loop
            }
            Action::ModelSelectRing => {
                self.modeling.active
                    && self.modeling.tool == crate::viewport::ModelTool::Select
                    && self.modeling.span == crate::viewport::SelectionSpan::Ring
            }

            Action::ModelFaces => {
                self.modeling.active && self.modeling.mode == crate::viewport::ComponentMode::Face
            }
            Action::ModelEdges => {
                self.modeling.active && self.modeling.mode == crate::viewport::ComponentMode::Edge
            }
            Action::ModelVertices => {
                self.modeling.active && self.modeling.mode == crate::viewport::ComponentMode::Vertex
            }
            Action::ModelSelect => {
                self.modeling.active && self.modeling.tool == crate::viewport::ModelTool::Select
            }
            Action::ModelExtrude => {
                self.modeling.active && self.modeling.tool == crate::viewport::ModelTool::Extrude
            }
            Action::ModelLoopCut => {
                self.modeling.active && self.modeling.tool == crate::viewport::ModelTool::LoopCut
            }
            Action::ModelReplace => {
                self.modeling.active
                    && self.modeling.change == crate::viewport::SelectionChange::Replace
            }
            Action::ModelAdd => {
                self.modeling.active
                    && self.modeling.change == crate::viewport::SelectionChange::Add
            }
            Action::ModelRemove => {
                self.modeling.active
                    && self.modeling.change == crate::viewport::SelectionChange::Remove
            }
            Action::ModelJoin => self.modeling.active && self.modeling.join,

            Action::SelectEdges => self.edges.selecting,
            Action::Wireframe => self.wire,
            Action::Symmetry => document.symmetry().enabled,
            Action::Guides => self.show_symmetry_guides,
            Action::TogglePathGuides => self.guide_paths,
            Action::TogglePathOnlyStrokes => self.path_starts_only,
            Action::PolygroupColors => self.show_polygroups,
            Action::Backfaces => self.show_backfaces,
            Action::ArrangeReferences => self.arrange_references,
            Action::ShowReferences => self.references_visible,
            Action::WireTriangles => self.wire_triangles,
            Action::SelectVisibilityLine => matches!(
                self.selection_tool,
                Some(crate::viewport::SelectionTool::Visibility(
                    crate::preferences::VisibilitySettings {
                        shape: crate::preferences::VisibilityShape::Line,
                        ..
                    }
                ))
            ),
            Action::SelectVisibilityPath => matches!(
                self.selection_tool,
                Some(crate::viewport::SelectionTool::Visibility(
                    crate::preferences::VisibilitySettings {
                        shape: crate::preferences::VisibilityShape::Path,
                        ..
                    }
                ))
            ),
            Action::VisibilityHide => self.preferences.visibility_selection.hide,
            Action::VisibilityIsolate => !self.preferences.visibility_selection.hide,
            Action::FlipVisibilitySide => self.preferences.visibility_selection.flip,
            Action::SelectRectangle => {
                self.selection_tool == Some(crate::viewport::SelectionTool::Rectangle)
            }
            Action::SelectLasso => {
                self.selection_tool == Some(crate::viewport::SelectionTool::Lasso)
            }
            Action::MaskLasso => {
                self.selection_tool == Some(crate::viewport::SelectionTool::MaskLasso)
            }
            Action::MaskLine => matches!(
                self.selection_tool,
                Some(crate::viewport::SelectionTool::MaskLine { .. })
            ),
            Action::FlipMaskLine => matches!(
                self.selection_tool,
                Some(crate::viewport::SelectionTool::MaskLine { flip: true, .. })
            ),
            Action::Slice => self.slice_active,
            Action::Sculpt => self.transform_mode == crate::viewport::TransformMode::Sculpt,
            Action::Transform => self.transform_mode == crate::viewport::TransformMode::World,
            Action::UniversalTransform => {
                self.transform_mode == crate::viewport::TransformMode::World
                    && self.transform_operation == crate::viewport::TransformOperation::Universal
            }
            Action::LineTransform => {
                self.transform_mode == crate::viewport::TransformMode::ActionLine
            }
            Action::Move
            | Action::Scale
            | Action::Rotate
            | Action::PlaceLine
            | Action::PlacePivot => {
                self.transform_mode != crate::viewport::TransformMode::Sculpt
                    && Action::from(self.transform_operation) == *action
            }
            Action::RespectMask => self.respect_transform_mask,
            Action::BrushAdd => brush.is_some_and(|b| !b.subtract),
            Action::BrushSubtract => brush.is_some_and(|b| b.subtract),
            Action::StrokeStabilisation => brush.is_some_and(|b| b.stabilize),
            _ => false,
        }
    }
    fn custom_item_widget(
        &mut self,
        ui: &mut egui::Ui,
        menu: &CustomMenu,
        index: usize,
        document: &Document,
        commands: &mut Vec<Command>,
        vertical: bool,
    ) {
        let item = &menu.items[index];
        if let Some(action) = &item.action {
            self.action_widget(ui, item, document, commands, vertical)
                .custom_item(Some(action.clone()), menu.id, index);
        } else {
            let response = ui.separator();
            ui.interact(
                response.rect.expand(3.),
                response.id.with("edit_separator"),
                egui::Sense::click(),
            )
            .custom_item(None, menu.id, index);
        }
    }

    /// The same controls serve shelves and individually positioned viewport widgets.
    pub(super) fn action_widget(
        &mut self,
        ui: &mut egui::Ui,
        item: &CustomItem,
        document: &Document,
        commands: &mut Vec<Command>,
        close_menu: bool,
    ) -> egui::Response {
        let action = item.action.as_ref().expect("action widget");
        if super::brush_controls::is_control(action) {
            return self.brush_control(ui, action, &item.label, document, commands);
        }
        let label = if item.label.trim().is_empty() {
            self.action_label(action, document)
        } else {
            item.label.clone()
        };
        let shortcut = self
            .preferences
            .shortcut(action)
            .map_or(String::new(), |s| s.label());
        let button = egui::Button::new(label)
            .wrap()
            .selected(self.action_selected(action, document, ui.ctx()))
            .shortcut_text(shortcut);
        let available = self.action_enabled(action, document);
        let response = ui.add_enabled(available, button);
        let response = if available {
            response
        } else {
            ui.interact(
                response.rect,
                response.id.with("edit_disabled_item"),
                egui::Sense::click(),
            )
        };
        let response = self.edge_help(response, action, document);
        if response.clicked() && available {
            commands.push(Command::Action(action.clone()));
            if close_menu {
                ui.close();
            }
        }
        response
    }
    fn custom_items(
        &mut self,
        ui: &mut egui::Ui,
        menu: &CustomMenu,
        document: &Document,
        commands: &mut Vec<Command>,
        vertical: bool,
    ) {
        // Live controls have different natural widths. A grid shares each column's
        // width across rows, leaving large gaps and forcing narrow sidebars wider
        // than their painted bounds. Pack these groups independently instead.
        if !vertical
            && menu.items.iter().any(|i| {
                i.action
                    .as_ref()
                    .is_some_and(super::brush_controls::is_control)
            })
        {
            let available_width = ui.available_width();
            let max_columns = if matches!(menu.placement, Placement::Top | Placement::Bottom) {
                usize::MAX
            } else {
                menu.columns.max(1)
            };
            ui.horizontal_wrapped(|ui| {
                let mut columns = 0;
                let mut used_width = 0.;
                for index in 0..menu.items.len() {
                    let item = &menu.items[index];
                    let width = item
                        .action
                        .as_ref()
                        .map_or(12., |action| {
                            self.brush_control_width(ui, action, &item.label, document)
                        })
                        .min(available_width);
                    if columns > 0
                        && (columns == max_columns || used_width + width > available_width)
                    {
                        ui.end_row();
                        columns = 0;
                        used_width = 0.;
                    }
                    let response = ui.allocate_ui_with_layout(
                        egui::vec2(width, 32.),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.push_id(("custom_control", menu.id, index), |ui| {
                                self.custom_item_widget(ui, menu, index, document, commands, false);
                            });
                        },
                    );
                    columns += 1;
                    used_width += response.response.rect.width() + ui.spacing().item_spacing.x;
                }
            });
            return;
        }
        let columns = if vertical {
            1
        } else {
            menu.columns
                .min((ui.available_width() / 140.).floor().max(1.) as usize)
        };
        let cell_width = ((ui.available_width() - 8. * columns.saturating_sub(1) as f32)
            / columns as f32)
            .max(80.);
        egui::Grid::new(("custom_items", menu.id))
            .num_columns(columns)
            .show(ui, |ui| {
                let mut column = 0;
                for (index, item) in menu.items.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        ui.set_max_width(cell_width);
                        self.custom_item_widget(ui, menu, index, document, commands, vertical);
                    });
                    column += 1;
                    if column == columns || item.action.is_none() {
                        ui.end_row();
                        column = 0;
                    }
                }
                if menu.items.is_empty() {
                    ui.weak("Add buttons in Customize workspace");
                }
            });
    }
    pub fn custom_menu_bar(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        for menu in self.preferences.menus.clone() {
            if menu.placement == Placement::Menu {
                ui.push_id(menu.id, |ui| {
                    let response = ui.menu_button(&menu.name, |ui| {
                        custom_dock_options(ui, &menu);
                        ui.separator();
                        self.custom_items(ui, &menu, document, commands, true)
                    });
                    // The context menu needs a different popup ID from the primary menu.
                    egui::Popup::context_menu(&response.response)
                        .id(response.response.id.with("custom_menu_options"))
                        .show(|ui| menu_options(ui, &menu));
                });
            }
        }
    }
    pub fn custom_surfaces(&mut self, ctx: &egui::Context, document: &Document) -> Vec<Command> {
        let mut commands = Vec::new();
        self.customization.popup_blocks_viewport = self.customization.popup_menu.is_some();
        // Edge shelves are laid out before built-in side panels and the central viewport.
        for (dock_index, placement) in [
            Placement::Top,
            Placement::Bottom,
            Placement::Left,
            Placement::Right,
        ]
        .into_iter()
        .enumerate()
        {
            let menus: Vec<_> = self
                .preferences
                .menus
                .iter()
                .filter(|m| m.placement == placement)
                .cloned()
                .collect();
            if menus.is_empty() {
                continue;
            }
            let id = egui::Id::new(("custom_dock", format!("{placement:?}")));
            let size = self.preferences.shelf_sizes[dock_index];
            let fit_controls = matches!(placement, Placement::Top | Placement::Bottom)
                && menus.iter().any(|menu| {
                    menu.items.iter().any(|item| {
                        item.action
                            .as_ref()
                            .is_some_and(super::brush_controls::is_control)
                    })
                });
            let mut contents = |ui: &mut egui::Ui| {
                if fit_controls {
                    for menu in &menus {
                        self.custom_section(ui, menu, document, &mut commands);
                    }
                    return;
                }
                let height = ui.available_height();
                egui::ScrollArea::both().max_height(height).show(ui, |ui| {
                    for menu in &menus {
                        self.custom_section(ui, menu, document, &mut commands);
                    }
                });
            };
            let response = match placement {
                Placement::Top if fit_controls => egui::TopBottomPanel::top(id)
                    .resizable(false)
                    .show(ctx, &mut contents),
                Placement::Bottom if fit_controls => egui::TopBottomPanel::bottom(id)
                    .resizable(false)
                    .show(ctx, &mut contents),
                Placement::Top => egui::TopBottomPanel::top(id)
                    .resizable(true)
                    .default_height(size)
                    .height_range(24. ..=800.)
                    .show(ctx, &mut contents),
                Placement::Bottom => egui::TopBottomPanel::bottom(id)
                    .resizable(true)
                    .default_height(size)
                    .height_range(24. ..=800.)
                    .show(ctx, &mut contents),
                Placement::Left => egui::SidePanel::left(id)
                    .default_width(size)
                    .width_range(100. ..=800.)
                    .show(ctx, &mut contents),
                Placement::Right => egui::SidePanel::right(id)
                    .default_width(size)
                    .width_range(100. ..=800.)
                    .show(ctx, &mut contents),
                _ => unreachable!(),
            };
            self.preferences.shelf_sizes[dock_index] =
                if matches!(placement, Placement::Top | Placement::Bottom) {
                    response.response.rect.height()
                } else {
                    response.response.rect.width()
                };
        }

        for menu in self.preferences.menus.clone() {
            if menu.placement != Placement::Floating {
                continue;
            }
            let mut open = true;
            let response = egui::Window::new(&menu.name)
                .id(egui::Id::new(("custom_float", menu.id)))
                .default_pos(menu.position)
                .open(&mut open)
                .vscroll(true)
                .show(ctx, |ui| {
                    custom_dock_options(ui, &menu);
                    ui.separator();
                    self.custom_items(ui, &menu, document, &mut commands, false);
                });
            if let Some(current) = self.preferences.menus.iter_mut().find(|m| m.id == menu.id) {
                if let Some(response) = response {
                    current.position = [response.response.rect.min.x, response.response.rect.min.y];
                }
                if !open {
                    current.placement = Placement::Hidden;
                }
            }
        }
        if let Some(menu) = self
            .customization
            .popup_menu
            .and_then(|id| self.preferences.menus.iter().find(|m| m.id == id))
            .cloned()
        {
            let mut open = !ctx.input(|i| i.key_pressed(egui::Key::Escape));
            let before = commands.len();
            let response = egui::Window::new(&menu.name)
                .id(egui::Id::new("custom_shortcut_popup"))
                .collapsible(false)
                .resizable(false)
                .auto_sized()
                .default_width(310.)
                .max_height((ctx.content_rect().height() - 32.).max(100.))
                .open(&mut open)
                .current_pos(
                    self.customization
                        .popup_position
                        .unwrap_or(egui::pos2(350., 160.)),
                )
                .vscroll(true)
                .show(ctx, |ui| {
                    custom_dock_options(ui, &menu);
                    ui.separator();
                    self.custom_items(ui, &menu, document, &mut commands, true)
                });
            let chose_action = commands[before..]
                .iter()
                .any(|c| !matches!(c, Command::UpdateBrush(..)));
            let outside = response.is_some_and(|r| r.response.clicked_elsewhere())
                && !egui::Popup::is_any_open(ctx)
                && before == commands.len();
            if !open || chose_action || outside {
                self.customization.popup_menu = None;
            }
        }
        commands
    }
    pub fn settings(&mut self, ctx: &egui::Context, document: &Document) {
        if let Some(request) = ctx.data_mut(|d| {
            let request = d.get_temp::<Request>(request_id());
            d.remove::<Request>(request_id());
            request
        }) {
            match request {
                Request::AddViewport(action) => {
                    self.add_viewport_control(crate::preferences::ViewportControlKind::Action(
                        action,
                    ));
                }
                Request::RemoveItem(id, index) => {
                    if let Some(menu) = self.preferences.menus.iter_mut().find(|m| m.id == id)
                        && index < menu.items.len()
                    {
                        menu.items.remove(index);
                    }
                }
                Request::DeleteMenu(id) => self.delete_custom_menu(id),
                Request::DockMenu(id, placement) => {
                    if let Some(menu) = self.preferences.menus.iter_mut().find(|m| m.id == id) {
                        menu.placement = placement;
                        menu.collapsible = true;
                        menu.expanded = true;
                    }
                    self.preferences
                        .sidebar_order
                        .retain(|s| *s != SidebarSection::Custom(id));
                    self.preferences
                        .sidebar_order
                        .insert(0, SidebarSection::Custom(id));
                }
                Request::NewPanel(placement) => {
                    let id = self.preferences.add_menu();
                    let menu = self
                        .preferences
                        .menus
                        .iter_mut()
                        .find(|m| m.id == id)
                        .unwrap();
                    menu.placement = placement;
                    menu.columns = 1;
                    self.customization.selected_menu = Some(id);
                    self.customization.chosen = None;
                    self.customization.search.clear();
                    self.layout_open = true;
                }
                Request::EditMenu(id) => {
                    self.customization.selected_menu = Some(id);
                    self.layout_open = true;
                }
                Request::Workspaces => {
                    self.layout_open = true;
                }

                Request::Add(action) => {
                    self.layout_open = true;
                    self.customization.chosen = Some(action);
                }
                Request::Bind(action) => {
                    self.settings_open = true;
                    self.customization.key_search = self.action_label(&action, document);
                    self.capturing = Some(action);
                }
            }
        }
        if !self.settings_open {
            self.capturing = None;
        }
        if let Some(action) = self.capturing.clone() {
            let event = ctx.input(|i| {
                i.events
                    .iter()
                    .filter(|e| matches!(e, egui::Event::Key { repeat: false, .. }))
                    .find_map(Shortcut::from_event)
            });
            if let Some(shortcut) = event {
                if shortcut.key == "Escape" {
                    self.capturing = None;
                } else {
                    match self.preferences.bind(action.clone(), shortcut) {
                        Ok(()) => {
                            self.settings_message =
                                format!("Updated {}", self.action_label(&action, document));
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
        let mut open = self.settings_open;
        egui::Window::new("Key settings").open(&mut open).default_size([540., 700.]).default_pos([350., 80.]).max_height((ctx.content_rect().height()-100.).max(160.)).vscroll(true).show(ctx, |ui| {
            ui.label("Click a binding and press the new keys. Escape cancels.");
            ui.label("1–0 and Shift+1–0 are separate bindings. Changes save automatically.");
            ui.label("Shortcuts pause during text entry and settings editing.");
            self.touch_settings(ui, document);
            ui.add(egui::TextEdit::singleline(&mut self.customization.key_search).hint_text("Search actions, brushes and menus"));
            if let Some(action) = &self.capturing { ui.strong(format!("Press keys for {}…", self.action_label(action, document))); }
            ui.label(&self.settings_message);
            egui::ScrollArea::vertical().id_salt("key_rows").max_height(350.).show(ui, |ui| {
                egui::Grid::new("bindings").striped(true).show(ui, |ui| {
                    for action in self.action_catalog(document) {
                        let label = self.action_label(&action, document);
                        if !label.to_lowercase().contains(&self.customization.key_search.to_lowercase()) { continue; }
                        ui.label(label);
                            let binding = if self.capturing.as_ref() == Some(&action) { "Press keys…".into() }
                                else { self.preferences.shortcut(&action).map_or("Unbound".into(), |b| b.label()) };
                            if ui.button(binding).clicked() { self.capturing = Some(action.clone()); self.settings_message.clear(); }
                            if ui.small_button("Clear").clicked() { self.preferences.clear_binding(&action); if self.capturing.as_ref() == Some(&action) { self.capturing = None; } }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            for (label, hold) in [("Hold to smooth", &mut self.preferences.smooth), ("Hold to reverse", &mut self.preferences.reverse), ("Hold to zoom while dragging", &mut self.preferences.zoom_hold), ("Hold to pan off the model", &mut self.preferences.pan_hold)] {
                egui::ComboBox::from_label(label).selected_text(format!("{hold:?}")).show_ui(ui, |ui| {
                    for value in Hold::ALL { ui.selectable_value(hold, value, format!("{value:?}")); }
                });
            }
            super::context_menu::hold_binding_editor(ui, "Dedicated mask hold (optional)", &mut self.preferences.mask_hold);
            for conflict in self.mask_binding_conflicts() { ui.colored_label(egui::Color32::YELLOW, conflict); }
            ui.weak("Alt reverses masking and visibility without changing the saved operation. The shared selection hold follows the X menu and wins overlapping holds. Right/middle drag and wheel remain available.");
            super::context_menu::hold_binding_editor(ui, "Hold for selection (X menu)", &mut self.preferences.visibility_hold);
            if self.preferences.visibility_hold.enabled() {
                let h = &self.preferences.visibility_hold;
                for (label, hold) in [("Smooth", self.preferences.smooth), ("Reverse", self.preferences.reverse), ("Zoom", self.preferences.zoom_hold), ("Pan", self.preferences.pan_hold)] {
                    let overlaps = match hold { Hold::Ctrl => h.ctrl || h.command, Hold::Shift => h.shift, Hold::Alt => h.alt, Hold::Disabled => false };
                    if overlaps { ui.weak(format!("Selection overrides {label} while its full chord is held.")); }
                }
                if h.ctrl || h.command { ui.weak("Selection overrides Ctrl/Cmd component-selection subtraction while its full chord is held."); }
                for (action,binding) in &self.preferences.bindings {
                    if let Some(binding) = binding && self.preferences.visibility_hold.key.as_ref() == Some(&binding.key) { ui.colored_label(egui::Color32::YELLOW, format!("Selection hold shares a key with {}. Clear or change one binding.", action.label())); }
                }
            }
            ui.weak("Temporary selection takes priority over tool modifiers. Middle/right-button navigation remains available. Zoom takes priority over pan with shared modifiers. Pan starts off the model; on the model, brush modifiers apply. Choose Mask or Visibility in the X menu; the hold uses that mode and its remembered shape.");
            if ui.button("Reset keys to defaults").clicked() {
                let defaults = Preferences::default();
                self.preferences.visibility_hold = defaults.visibility_hold;
                self.preferences.mask_hold = defaults.mask_hold;
                self.preferences.reset_key_bindings(); self.preferences.smooth = defaults.smooth;
                self.preferences.reverse = defaults.reverse; self.preferences.zoom_hold = defaults.zoom_hold; self.preferences.pan_hold = defaults.pan_hold;
                self.capturing = None; self.settings_message.clear();
            }
        });
        self.settings_open = open;
        let mut open = self.layout_open;
        egui::Window::new("Customize workspace").open(&mut open).default_size([760., 750.]).default_pos([300., 80.]).max_height((ctx.content_rect().height()-100.).max(160.)).vscroll(true).show(ctx, |ui| {
            ui.label("Build menus and button shelves. Right-click an action button to add it or assign a key.");
            ui.label("Changes save automatically. Drag floating shelves by their titles.");
            self.workspace_editor(ui);
            ui.separator();
            if ui.button("Configure viewport controls…").clicked() {
                self.viewport_controls.editor_open = true;
            }
            ui.separator();
            self.custom_editor(ui, document);
            ui.separator();
            egui::CollapsingHeader::new("Built-in panels and toolbar").show(ui, |ui| {
                design::checkbox(ui, &mut self.preferences.compact_toolbar, "Compact toolbar (keep Settings and custom menus)");
                for index in 0..self.preferences.panels.len() {
                    ui.push_id(index, |ui| { ui.horizontal(|ui| {
                        let panel = &mut self.preferences.panels[index];
                        ui.label(panel.tool.label());
                        egui::ComboBox::from_id_salt("dock").selected_text(format!("{:?}", panel.dock)).show_ui(ui, |ui| {
                            for dock in Dock::ALL { ui.selectable_value(&mut panel.dock, dock, format!("{dock:?}")); }
                        });
                        design::checkbox(ui, &mut panel.open, "Expanded");
                    }); });
                }
                self.sidebar_order_editor(ui);
                let left_changed = ui.add(egui::Slider::new(&mut self.preferences.left_width,180. ..=600.).text("Left width")).changed();
                let right_changed = ui.add(egui::Slider::new(&mut self.preferences.right_width,180. ..=600.).text("Right width")).changed();
                if left_changed || right_changed { reset_panel_memory(ctx); }
            });
            ui.separator();
        });
        self.layout_open = open && !self.viewport_controls.editor_open;
        super::design::appearance(ctx, &self.preferences.appearance);
    }
    fn custom_editor(&mut self, ui: &mut egui::Ui, document: &Document) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("+ New menu / shelf").clicked() {
                self.customization.selected_menu = Some(self.preferences.add_menu());
            }
            let mut reorder = None;
            for (index, menu) in self.preferences.menus.iter().enumerate() {
                if ui
                    .selectable_label(
                        self.customization.selected_menu == Some(menu.id),
                        &menu.name,
                    )
                    .clicked()
                {
                    self.customization.selected_menu = Some(menu.id);
                }
                if self.customization.selected_menu == Some(menu.id)
                    && index > 0
                    && ui
                        .small_button("←")
                        .on_hover_text("Move before previous menu / shelf")
                        .clicked()
                {
                    reorder = Some((index, index - 1));
                }
            }
            if let Some((a, b)) = reorder {
                self.preferences.menus.swap(a, b);
            }
        });
        if self.customization.selected_menu.is_none() {
            self.customization.selected_menu = self.preferences.menus.first().map(|m| m.id);
        }
        let Some(index) = self
            .preferences
            .menus
            .iter()
            .position(|m| Some(m.id) == self.customization.selected_menu)
        else {
            ui.weak("Create a menu or shelf, then choose buttons from the action catalog below.");
            return;
        };
        let mut menu = self.preferences.menus[index].clone();
        let mut delete = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut menu.name)
                    .desired_width(150.)
                    .hint_text("Menu name"),
            );
            egui::ComboBox::from_id_salt("custom_placement")
                .selected_text(menu.placement.label())
                .show_ui(ui, |ui| {
                    for placement in Placement::ALL {
                        ui.selectable_value(&mut menu.placement, placement, placement.label());
                    }
                });
            ui.add(
                egui::DragValue::new(&mut menu.columns)
                    .range(1..=8)
                    .prefix("Columns: "),
            );
            delete = ui.button("Delete menu").clicked();
        });
        design::checkbox(ui, &mut menu.collapsible, "Collapsible section");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.customization.search)
                    .desired_width(200.)
                    .hint_text("Search available actions"),
            );
            if ui.button("+ Separator").clicked() {
                menu.items.push(CustomItem {
                    action: None,
                    label: String::new(),
                });
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("action_catalog")
            .max_height(140.)
            .show(ui, |ui| {
                for action in self.action_catalog(document) {
                    let label = self.action_label(&action, document);
                    if !label
                        .to_lowercase()
                        .contains(&self.customization.search.to_lowercase())
                    {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        let response = ui.selectable_label(
                            self.customization.chosen.as_ref() == Some(&action),
                            label,
                        );
                        if response.clicked() {
                            self.customization.chosen = Some(action.clone());
                        }
                        if ui.small_button("Add").clicked() {
                            menu.items.push(CustomItem::action(action));
                        }
                    });
                }
            });
        if let Some(action) = &self.customization.chosen
            && ui
                .button(format!(
                    "Add selected: {}",
                    self.action_label(action, document)
                ))
                .clicked()
        {
            menu.items.push(CustomItem::action(action.clone()));
        }
        ui.label("Menu contents · drag the grip or use arrows to reorder · optional custom labels");
        let mut reorder = None;
        let mut remove = None;
        let count = menu.items.len();
        egui::ScrollArea::vertical()
            .id_salt("custom_contents")
            .max_height(230.)
            .show(ui, |ui| {
                for (i, item) in menu.items.iter_mut().enumerate() {
                    let row = ui
                        .push_id((menu.id, i), |ui| {
                            ui.horizontal(|ui| {
                                let grip =
                                    ui.add(egui::Label::new("::").sense(egui::Sense::drag()));
                                grip.dnd_set_drag_payload((menu.id, i));
                                ui.label(
                                    item.action.as_ref().map_or("— Separator —".into(), |a| {
                                        self.action_label(a, document)
                                    }),
                                );
                                if item.action.is_some() {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut item.label)
                                            .desired_width(110.)
                                            .hint_text("Custom label"),
                                    );
                                }
                                if ui.add_enabled(i > 0, egui::Button::new("Up")).clicked() {
                                    reorder = Some((i, i - 1));
                                }
                                if ui
                                    .add_enabled(i + 1 < count, egui::Button::new("Down"))
                                    .clicked()
                                {
                                    reorder = Some((i, i + 1));
                                }
                                if ui
                                    .small_button("×")
                                    .on_hover_text("Remove button")
                                    .clicked()
                                {
                                    remove = Some(i);
                                }
                            })
                        })
                        .inner
                        .response;
                    if let Some(payload) = row.dnd_release_payload::<(u64, usize)>()
                        && payload.0 == menu.id
                        && payload.1 < count
                    {
                        reorder = Some((payload.1, i));
                    }
                }
            });
        if let Some(i) = remove {
            menu.items.remove(i);
        } else if let Some((a, b)) = reorder {
            let item = menu.items.remove(a);
            menu.items.insert(b, item);
        }
        if delete {
            self.delete_custom_menu(menu.id);
        } else {
            self.preferences.menus[index] = menu;
        }
    }
}
fn reset_panel_memory(ctx: &egui::Context) {
    ctx.memory_mut(|m| {
        m.reset_areas();
        for placement in [
            Placement::Top,
            Placement::Bottom,
            Placement::Left,
            Placement::Right,
        ] {
            m.data
                .remove::<egui::containers::panel::PanelState>(egui::Id::new((
                    "custom_dock",
                    format!("{placement:?}"),
                )));
        }
        for id in ["tools_left", "tools_right"] {
            m.data
                .remove::<egui::containers::panel::PanelState>(egui::Id::new(id));
        }
    });
}

#[cfg(test)]
mod tests;

impl UiState {
    pub(super) fn custom_section(
        &mut self,
        ui: &mut egui::Ui,
        menu: &CustomMenu,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let response = if menu.collapsible {
            let header = egui::CollapsingHeader::new(&menu.name)
                .id_salt(("custom_section", menu.id))
                .open(Some(menu.expanded))
                .show(ui, |ui| {
                    self.custom_items(ui, menu, document, commands, false)
                });
            if header.header_response.clicked()
                && let Some(current) = self.preferences.menus.iter_mut().find(|m| m.id == menu.id)
            {
                current.expanded = !menu.expanded;
            }
            header.header_response
        } else {
            let response = ui.add(
                egui::Label::new(egui::RichText::new(&menu.name).strong())
                    .sense(egui::Sense::click()),
            );
            self.custom_items(ui, menu, document, commands, false);
            response
        };
        if matches!(menu.placement, Placement::LeftPanel | Placement::RightPanel) {
            self.section_context(response, SidebarSection::Custom(menu.id));
        } else {
            response.context_menu(|ui| menu_options(ui, menu));
        }
    }
    fn delete_custom_menu(&mut self, id: u64) {
        self.preferences.menus.retain(|m| m.id != id);
        self.preferences
            .sidebar_order
            .retain(|s| *s != SidebarSection::Custom(id));
        if self.customization.selected_menu == Some(id) {
            self.customization.selected_menu = None;
        }
        if self.customization.popup_menu == Some(id) {
            self.customization.popup_menu = None;
        }
        // Bindings are global: another workspace can still contain this menu.
    }
    fn activate_workspace(&mut self, ctx: &egui::Context, index: usize) {
        self.preferences.switch_workspace(index);
        self.customization.selected_menu = None;
        self.customization.popup_menu = None;
        reset_panel_memory(ctx);
        super::design::appearance(ctx, &self.preferences.appearance);
    }
    pub(super) fn workspace_selector(&mut self, ui: &mut egui::Ui) {
        let name = self
            .preferences
            .active_workspace
            .and_then(|i| self.preferences.workspaces.get(i))
            .map_or("Custom", |w| w.name.as_str())
            .to_owned();
        egui::ComboBox::from_id_salt("active_workspace")
            .selected_text(format!("Workspace: {name}"))
            .show_ui(ui, |ui| {
                let mut selected = None;
                for (index, workspace) in self.preferences.workspaces.iter().enumerate() {
                    if ui
                        .selectable_label(
                            self.preferences.active_workspace == Some(index),
                            &workspace.name,
                        )
                        .clicked()
                    {
                        selected = Some(index);
                    }
                }
                if let Some(index) = selected {
                    self.activate_workspace(ui.ctx(), index);
                }
                ui.separator();
                if ui.button("Save / manage workspaces…").clicked() {
                    self.layout_open = true;
                    ui.close();
                }
            });
    }
    fn workspace_editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            self.workspace_selector(ui);
            ui.add(
                egui::TextEdit::singleline(&mut self.customization.workspace_name)
                    .desired_width(160.)
                    .hint_text("New workspace name"),
            );
            let name = self.customization.workspace_name.trim();
            let valid = !name.is_empty()
                && name.len() <= 128
                && !self.preferences.workspaces.iter().any(|w| w.name == name);
            if ui
                .add_enabled(valid, egui::Button::new("Save as workspace"))
                .on_hover_text("Use a unique workspace name")
                .clicked()
                && self
                    .preferences
                    .save_workspace_as(&self.customization.workspace_name)
                    .is_ok()
            {
                self.customization.workspace_name.clear();
            }
        });
        ui.weak("Named workspaces automatically remember menus, pinned menu positions, sidebar order, panel sizes, collapsed sections and colours. Keys are shared across workspaces.");
        egui::CollapsingHeader::new("Workspace colours and style").show(ui, |ui| {
            let appearance = &mut self.preferences.appearance;
            ui.horizontal(|ui| {
                ui.label("Presets:");
                if ui.button("Dark").clicked() {
                    *appearance = Appearance::default();
                }
                if ui.button("Light").clicked() {
                    *appearance = Appearance::light();
                }
                if ui.button("Warm").clicked() {
                    *appearance = Appearance::warm();
                }
            });
            ui.horizontal_wrapped(|ui| {
                for (name, colour) in [
                    ("Panels", &mut appearance.panels),
                    ("Buttons", &mut appearance.buttons),
                    ("Accent", &mut appearance.accent),
                    ("Text", &mut appearance.text),
                    ("Model view", &mut appearance.viewport),
                ] {
                    ui.color_edit_button_srgb(colour);
                    ui.label(name);
                }
            });
            ui.add(egui::Slider::new(&mut appearance.spacing, 2..=16).text("Control spacing"));
            ui.add(egui::Slider::new(&mut appearance.rounding, 0..=16).text("Corner rounding"));
        });
        egui::CollapsingHeader::new("Manage saved workspaces").show(ui, |ui| {
            let mut remove = None;
            for (index, workspace) in self.preferences.workspaces.clone().iter().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(&workspace.name);
                        if ui.button("Switch").clicked() {
                            self.activate_workspace(ui.ctx(), index);
                        }
                        if ui.button("Delete workspace").clicked() {
                            remove = Some(index);
                        }
                    });
                });
            }
            if let Some(index) = remove {
                self.preferences.delete_workspace(index);
            }
            if ui.button("Start a default workspace").clicked() {
                self.preferences.sync_active_workspace();
                self.preferences.active_workspace = None;
                // Keep the brush-menu shortcut identity across named workspaces.
                let brush_id = self
                    .preferences
                    .menus
                    .iter()
                    .find(|m| m.items.iter().any(|i| i.action == Some(Action::BrushSize)))
                    .map(|m| m.id)
                    .unwrap_or_else(|| self.preferences.add_menu());
                let mut defaults = Preferences::default().workspace(String::new());
                defaults.menus[0].id = brush_id;
                self.preferences.apply_workspace(&defaults);
                if !self
                    .preferences
                    .bindings
                    .iter()
                    .any(|(a, _)| *a == Action::OpenMenu(brush_id))
                {
                    let _ = self.preferences.bind(
                        Action::OpenMenu(brush_id),
                        Shortcut::new(egui::Key::S, egui::Modifiers::NONE),
                    );
                }
                self.customization.selected_menu = None;
                self.customization.popup_menu = None;
                reset_panel_memory(ui.ctx());
            }
        });
    }
}

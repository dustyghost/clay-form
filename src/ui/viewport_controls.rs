//! Independent viewport widgets reuse the action catalog and workspace persistence.
use super::*;
use crate::preferences::{
    CustomItem, ViewportControl, ViewportControlIcon as Icon, ViewportControlKind as Kind,
    ViewportControlStyle as Style,
};

pub(crate) struct ControlsState {
    pub editor_open: bool,
    pub arranging: bool,
    search: String,
    new_style: Style,
    gap: f32,
    pending_alignment: Option<bool>,
    reset_confirmation: bool,
    feedback: String,
}

impl Default for ControlsState {
    fn default() -> Self {
        Self {
            editor_open: false,
            arranging: false,
            search: String::new(),
            new_style: Style::Icon,
            gap: 8.,
            pending_alignment: None,
            reset_confirmation: false,
            feedback: String::new(),
        }
    }
}

pub(crate) struct ControlsOutput {
    pub commands: Vec<Command>,
    pub navigation: Vec<(Kind, Vec2)>,
    pub blocks_viewport: bool,
}

/// The stock column shares the right edge with the axis gizmo. On short
/// canvases, use a temporary grid without rewriting the artist's preferences.
struct CompactStockLayout {
    positions: Vec<egui::Pos2>,
    launcher: egui::Pos2,
}
fn compact_stock_layout(rect: egui::Rect, count: usize) -> Option<CompactStockLayout> {
    if rect.height() >= 600. || count == 0 {
        return None;
    }
    let button = 40.;
    let step = 48.;
    // Prefer a row below the 134px axis gizmo. If the viewport is especially
    // short, fit a grid beside it, leaving room above for the editor launcher.
    let regions = [
        egui::Rect::from_min_max(
            rect.min + egui::vec2(24., 158.),
            rect.max - egui::vec2(24., 24.),
        ),
        egui::Rect::from_min_max(
            rect.min + egui::vec2(24., 84.),
            rect.max - egui::vec2(158., 24.),
        ),
    ];
    for region in regions {
        let columns = (((region.width() + 8.) / step).floor().max(0.) as usize).min(count);
        if columns == 0 {
            continue;
        }
        let rows = count.div_ceil(columns);
        let height = button + (rows - 1) as f32 * step;
        if height > region.height() {
            continue;
        }
        let width = button + (columns - 1) as f32 * step;
        let start = egui::pos2(region.center().x - width * 0.5, region.bottom() - height);
        return Some(CompactStockLayout {
            positions: (0..count)
                .map(|index| {
                    start
                        + egui::vec2(
                            (index % columns) as f32 * step,
                            (index / columns) as f32 * step,
                        )
                })
                .collect(),
            launcher: rect.min + egui::vec2(24., 44.),
        });
    }
    None
}

impl UiState {
    pub(crate) fn align_viewport_controls(&mut self, ctx: &egui::Context, vertical: bool) {
        self.preferences.viewport_controls_auto_layout = false;
        self.viewport_controls.pending_alignment = Some(vertical);
        self.apply_viewport_alignment(ctx, vertical);
    }

    /// An explicit placement edit starts from the currently visible stock grid.
    /// Ordinary responsive rendering never mutates saved controls.
    fn materialise_viewport_controls(&mut self, rect: Option<egui::Rect>) {
        let defaults = crate::preferences::default_viewport_controls();
        let rect = rect
            .or(self.viewport_menu.viewport_rect)
            .map(|rect| self.navigation_rect(rect));
        if self.preferences.viewport_controls_auto_layout
            && self.preferences.viewport_controls == defaults
            && let Some(rect) = rect
            && let Some(layout) = compact_stock_layout(rect, defaults.len())
        {
            let bounds = rect.shrink(24_f32.min(rect.width().min(rect.height()).max(0.) * 0.25));
            let travel = (bounds.size() - Vec2::splat(40.)).max(Vec2::ZERO);
            for (control, position) in self
                .preferences
                .viewport_controls
                .iter_mut()
                .zip(layout.positions)
            {
                for axis in 0..2 {
                    control.position[axis] = if travel[axis] > 0. {
                        ((position[axis] - bounds.min[axis]) / travel[axis]).clamp(0., 1.)
                    } else {
                        0.
                    };
                    control.offset[axis] =
                        position[axis] - bounds.min[axis] - control.position[axis] * travel[axis];
                }
            }
        }
        self.preferences.viewport_controls_auto_layout = false;
    }

    fn apply_viewport_alignment(&mut self, ctx: &egui::Context, vertical: bool) {
        let axis = usize::from(vertical);
        let sizes: Vec<f32> = self
            .preferences
            .viewport_controls
            .iter()
            .enumerate()
            .map(|(index, control)| {
                if control.style == Style::Icon {
                    return 40.;
                }
                ctx.data(|d| {
                    d.get_temp::<Vec2>(egui::Id::new(("viewport_control", index)).with("size"))
                })
                .unwrap_or(egui::vec2(140., 40.))[axis]
            })
            .collect();
        let gap = self.viewport_controls.gap;
        let total = sizes.iter().sum::<f32>() + gap * sizes.len().saturating_sub(1) as f32;
        let mut cursor = -total * 0.5;
        for (control, size) in self.preferences.viewport_controls.iter_mut().zip(sizes) {
            control.position = if vertical { [1., 0.56] } else { [0.5, 1.] };
            control.offset = [0.; 2];
            control.offset[axis] = cursor + size * 0.5;
            cursor += size + gap;
        }
    }

    fn viewport_control_widget(
        &mut self,
        ui: &mut egui::Ui,
        control: &ViewportControl,
        document: &Document,
        commands: &mut Vec<Command>,
        arranging: bool,
    ) -> egui::Response {
        let label = self.viewport_control_label(control, document);
        let action = match &control.kind {
            Kind::Action(action) => Some(action),
            _ => None,
        };
        if !arranging
            && control.style == Style::Text
            && let Some(action) = action
        {
            return self.action_widget(
                ui,
                &CustomItem {
                    action: Some(action.clone()),
                    label: control.label.clone(),
                },
                document,
                commands,
                false,
            );
        }
        let selected =
            action.is_some_and(|action| self.action_selected(action, document, ui.ctx()));
        let available =
            arranging || action.is_none_or(|action| self.action_enabled(action, document));
        let drag = arranging || action.is_none();
        let button = design::floating_button(
            ui,
            (control.style != Style::Text).then(|| control_glyph(control)),
            (control.style != Style::Icon).then_some(label.as_str()),
        )
        .selected(selected && !arranging)
        .sense(if drag {
            Sense::click_and_drag()
        } else {
            Sense::click()
        });
        let response = ui.add_enabled(available, button);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                available && ui.is_enabled(),
                &label,
            )
        });
        let hint = if arranging {
            "Drag to position this control. Right-click for style and removal options."
        } else {
            match control.kind {
                Kind::Pan => "Drag left, right, up or down to pan",
                Kind::Zoom => "Drag up to zoom in; down to zoom out",
                Kind::Orbit => "Drag to rotate the camera",
                Kind::Roll => {
                    "Drag right to rotate clockwise in screen space; left for counterclockwise"
                }
                Kind::Action(_) => "Right-click to change style, move or remove",
            }
        };
        let shortcut = action
            .and_then(|a| self.preferences.shortcut(a))
            .map(|s| format!(" · {}", s.label()))
            .unwrap_or_default();
        let response = response.on_hover_text(format!("{label}{shortcut}\n{hint}"));
        super::search::highlight_response(&response, &format!("navigation:{:?}", control.kind));
        if let Some(action) = action {
            super::search::highlight_response(&response, &format!("action:{action:?}"));
        }
        if !arranging && let Some(action) = action {
            if super::brush_controls::is_control(action) {
                egui::Popup::menu(&response)
                    .id(response.id.with("viewport_brush_popup"))
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        self.brush_control(ui, action, &control.label, document, commands);
                    });
            } else if response.clicked() && available {
                commands.push(Command::Action(action.clone()));
            }
        }
        response
    }

    pub(crate) fn reveal_viewport_control(&mut self, kind: Kind) {
        self.layout_open = false;
        self.viewport_controls.editor_open = true;
        self.viewport_controls.search = match kind {
            Kind::Pan => "Pan view".into(),
            Kind::Zoom => "Zoom view".into(),
            Kind::Orbit => "Rotate view".into(),
            Kind::Roll => "Roll view".into(),
            Kind::Action(action) => action.label().into(),
        };
    }

    pub(super) fn add_viewport_control(&mut self, kind: Kind) {
        if self.preferences.viewport_controls.len() >= 128 {
            self.status = "The viewport holds up to 128 controls".into();
            return;
        }
        self.materialise_viewport_controls(None);
        let offset = (self.preferences.viewport_controls.len() % 8) as f32 * 0.04;
        self.preferences.viewport_controls.push(ViewportControl {
            kind,
            style: self.viewport_controls.new_style,
            icon: Icon::Auto,
            label: String::new(),
            position: [0.35 + offset, 0.35 + offset],
            offset: [0.; 2],
        });
        self.viewport_controls.arranging = true;
        self.viewport_controls.feedback = "Added to the viewport. Choose Place controls, drag the new control, then Done placing controls.".into();
    }

    fn viewport_control_label(&self, control: &ViewportControl, document: &Document) -> String {
        if !control.label.trim().is_empty() {
            return control.label.clone();
        }
        match &control.kind {
            Kind::Pan => "Pan view".into(),
            Kind::Zoom => "Zoom view".into(),
            Kind::Orbit => "Rotate view".into(),
            Kind::Roll => "Roll view".into(),
            Kind::Action(action) => self.action_label(action, document),
        }
    }

    pub fn viewport_controls(
        &mut self,
        ctx: &egui::Context,
        rect: egui::Rect,
        document: &Document,
        enabled: bool,
    ) -> ControlsOutput {
        let enabled = enabled && !self.viewport_controls.editor_open && ctx.input(|i| i.focused);
        let mut output = ControlsOutput {
            commands: Vec::new(),
            navigation: Vec::new(),
            blocks_viewport: false,
        };
        if self.viewport_controls.editor_open {
            output.blocks_viewport = true;
            return output;
        }
        let defaults = crate::preferences::default_viewport_controls();
        let compact = (self.preferences.viewport_controls_auto_layout
            && self.preferences.viewport_controls == defaults)
            .then(|| compact_stock_layout(rect, defaults.len()))
            .flatten();
        let launcher_y = if self
            .preferences
            .viewport_controls
            .iter()
            .any(|c| c.position[1] >= 0.95)
        {
            rect.top() + 52.
        } else {
            rect.bottom() - 56.
        };
        // Keep a route back to the editor even when all user controls have been removed.
        let launcher = egui::Area::new(egui::Id::new("viewport_controls_launcher"))
            .order(egui::Order::Foreground)
            .default_size(egui::vec2(240., 40.))
            .movable(false)
            .fixed_pos(
                compact
                    .as_ref()
                    .map_or(egui::pos2(rect.left() + 24., launcher_y), |layout| {
                        layout.launcher
                    }),
            )
            .constrain_to(rect)
            .show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        if self.viewport_controls.arranging
                            && ui.button("Done placing controls").clicked()
                        {
                            self.viewport_controls.arranging = false;
                        }
                        if ui.button("Viewport controls…").clicked() {
                            self.viewport_controls.editor_open = true;
                        }
                    });
                });
            });
        #[cfg(test)]
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("viewport_controls_launcher_response"),
                launcher.response.rect,
            )
        });
        output.blocks_viewport |= enabled && launcher.response.contains_pointer();
        let arranging = self.viewport_controls.arranging;
        let controls = self.preferences.viewport_controls.clone();
        let mut remove = None;
        for (index, control) in controls.iter().enumerate() {
            let id = egui::Id::new(("viewport_control", index));
            // Remember the measured size so right/bottom anchors stay inside the drawing area.
            let size = ctx.data(|d| d.get_temp::<Vec2>(id.with("size"))).unwrap_or(
                if control.style == Style::Icon {
                    Vec2::splat(40.)
                } else {
                    egui::vec2(140., 40.)
                },
            );
            let bounds = rect.shrink(24_f32.min(rect.width().min(rect.height()).max(0.) * 0.25));
            let travel = (bounds.size() - size).max(Vec2::ZERO);
            let position = compact
                .as_ref()
                .map(|layout| layout.positions[index])
                .unwrap_or_else(|| {
                    bounds.min
                        + egui::vec2(
                            control.position[0] * travel.x,
                            control.position[1] * travel.y,
                        )
                        + egui::vec2(control.offset[0], control.offset[1])
                });
            let area = egui::Area::new(id)
                .order(egui::Order::Foreground)
                // Avoid the default Area estimate covering the model before
                // the first measured frame, swallowing an early stroke press.
                .default_size(size)
                .movable(false)
                .fixed_pos(position)
                .constrain_to(rect)
                .show(ctx, |ui| {
                    ui.set_max_width(220.);
                    egui::Frame::NONE.show(ui, |ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            let response = self.viewport_control_widget(
                                ui,
                                control,
                                document,
                                &mut output.commands,
                                arranging,
                            );
                            #[cfg(test)]
                            ctx.data_mut(|d| {
                                d.insert_temp(
                                    egui::Id::new(("viewport_control_response", index)),
                                    response.rect,
                                )
                            });
                            if response.drag_started_by(egui::PointerButton::Primary) && arranging {
                                self.materialise_viewport_controls(Some(rect));
                                let saved = &mut self.preferences.viewport_controls[index];
                                for axis in 0..2 {
                                    if travel[axis] > 0. {
                                        saved.position[axis] =
                                            ((position[axis] - bounds.min[axis]) / travel[axis])
                                                .clamp(0., 1.);
                                    }
                                }
                                saved.offset = [0.; 2];
                            }
                            if response.dragged_by(egui::PointerButton::Primary) {
                                output.blocks_viewport = true;
                                let delta = ctx.input(|i| i.pointer.delta());
                                if arranging {
                                    let control = &mut self.preferences.viewport_controls[index];
                                    for axis in 0..2 {
                                        if travel[axis] > 0. {
                                            control.position[axis] = (control.position[axis]
                                                + delta[axis] / travel[axis])
                                                .clamp(0., 1.);
                                        }
                                    }
                                } else if !matches!(control.kind, Kind::Action(_)) {
                                    output.navigation.push((control.kind.clone(), delta));
                                }
                            }
                            output.blocks_viewport |=
                                response.is_pointer_button_down_on() || response.drag_stopped();
                            response.context_menu(|ui| {
                                ui.label("Button style");
                                for style in Style::ALL {
                                    if ui
                                        .selectable_label(
                                            self.preferences.viewport_controls[index].style
                                                == style,
                                            style.label(),
                                        )
                                        .clicked()
                                    {
                                        self.materialise_viewport_controls(Some(rect));
                                        self.preferences.viewport_controls[index].style = style;
                                        ui.close();
                                    }
                                }
                                ui.separator();

                                if ui.button("Move viewport controls").clicked() {
                                    self.materialise_viewport_controls(Some(rect));
                                    self.viewport_controls.arranging = true;
                                    ui.close();
                                }
                                if ui.button("Configure viewport controls…").clicked() {
                                    self.viewport_controls.editor_open = true;
                                    ui.close();
                                }
                                if ui.button("Remove from viewport").clicked() {
                                    remove = Some(index);
                                    ui.close();
                                }
                            });
                        });
                    });
                });
            ctx.data_mut(|d| d.insert_temp(id.with("size"), area.response.rect.size()));
            // Respect foreground overlays such as the light controls. A button
            // behind them must not disable their input merely by overlapping.
            output.blocks_viewport |= enabled && area.response.contains_pointer();
        }
        if let Some(index) = remove {
            self.materialise_viewport_controls(Some(rect));
            self.preferences.viewport_controls.remove(index);
        }
        // Reapply with the actual widget measurements after leaving the editor.
        if let Some(vertical) = self.viewport_controls.pending_alignment.take() {
            self.apply_viewport_alignment(ctx, vertical);
            ctx.request_repaint();
        }
        output.blocks_viewport |=
            arranging || self.viewport_controls.editor_open || egui::Popup::is_any_open(ctx);
        output
    }

    pub fn viewport_controls_editor(&mut self, ctx: &egui::Context, document: &Document) {
        let mut open = self.viewport_controls.editor_open;
        let mut place = false;
        let mut return_to_workspace = false;
        egui::Window::new("Viewport controls").open(&mut open).default_size([620., 740.])
            .max_width((ctx.content_rect().width() - 32.).max(260.))
            .max_height((ctx.content_rect().height() - 100.).max(160.)).vscroll(true).show(ctx, |ui| {
                super::search::mark_destination(ui, "viewport_controls");
                ui.label("Add controls, then drag them anywhere in the drawing area. Layouts save automatically with your workspace.");
                ui.weak("Style, custom labels, removal and placement apply only to these viewport copies. The original tools and their shortcuts stay available.");
                if ui.button("Back to workspace").clicked() { return_to_workspace = true; }
                if !self.viewport_controls.feedback.is_empty() { ui.weak(&self.viewport_controls.feedback); }
                self.touch_settings(ui, document);
                ui.horizontal(|ui| {
                    place = ui.button("Place controls").clicked();
                    if ui.button("Restore default controls…").clicked() { self.viewport_controls.reset_confirmation = true; }
                });
                if self.viewport_controls.reset_confirmation {
                    ui.group(|ui| {
                        ui.label("Replace this workspace's viewport controls with the defaults? Panels, shelves and keyboard bindings stay unchanged.");
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Use default controls").clicked() {
                                self.preferences.viewport_controls = crate::preferences::default_viewport_controls();
                                self.preferences.viewport_controls_auto_layout = true;
                                self.viewport_controls.reset_confirmation = false;
                                self.viewport_controls.feedback = "Restored default viewport controls for this workspace.".into();
                            }
                            if ui.button("Keep these controls").clicked() { self.viewport_controls.reset_confirmation = false; }
                        });
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("New button style");
                    style_picker(ui, "new_style", &mut self.viewport_controls.new_style);
                    if ui.button("Apply style to all").clicked() {
                        self.materialise_viewport_controls(None);
                        for control in &mut self.preferences.viewport_controls { control.style = self.viewport_controls.new_style; }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut self.viewport_controls.gap).range(4. ..=32.).prefix("Spacing: ").suffix(" px"));
                    if ui.button("Align right").clicked() { self.align_viewport_controls(ctx, true); }
                    if ui.button("Align bottom").clicked() { self.align_viewport_controls(ctx, false); }
                });
                ui.separator();
                let mut remove = None;
                for (index, control) in self.preferences.viewport_controls.clone().iter().enumerate() {
                    ui.push_id(index, |ui| { ui.horizontal_wrapped(|ui| {
                        let response = ui.add_sized([130., 32.], egui::Label::new(self.viewport_control_label(control, document)).truncate());
                        super::search::highlight_response(&response, &format!("navigation:{:?}", control.kind));
                        let mut style = control.style;
                        if style_picker(ui, "style", &mut style) {
                            self.materialise_viewport_controls(None);
                            self.preferences.viewport_controls[index].style = style;
                        }
                        let glyph = control_glyph(control);
                        egui::ComboBox::from_id_salt("icon").width(80.).selected_text(control.icon.label()).show_ui(ui, |ui| {
                            for icon in Icon::ALL {
                                if ui.add(design::button(ui, icon_glyph(icon).unwrap_or(glyph), icon.label()).selected(control.icon == icon)).clicked() {
                                    self.materialise_viewport_controls(None);
                                    self.preferences.viewport_controls[index].icon = icon;
                                    ui.close();
                                }
                            }
                        });
                        let mut label = control.label.clone();
                        if ui.add(egui::TextEdit::singleline(&mut label).desired_width(90.).char_limit(128).hint_text("Custom label")).changed() {
                            self.materialise_viewport_controls(None);
                            self.preferences.viewport_controls[index].label = label;
                        }
                        if ui.button("Remove").clicked() { remove = Some(index); }
                    }); });
                }
                if let Some(index) = remove {
                    self.materialise_viewport_controls(None);
                    self.preferences.viewport_controls.remove(index);
                }
                ui.separator();
                ui.heading("Add a control");
                ui.add(egui::TextEdit::singleline(&mut self.viewport_controls.search).hint_text("Search controls, brushes and menus"));
                let search = self.viewport_controls.search.to_lowercase();
                let kinds = [Kind::Pan, Kind::Zoom, Kind::Orbit, Kind::Roll].into_iter()
                    .chain(self.action_catalog(document).into_iter().map(Kind::Action));
                egui::ScrollArea::vertical().id_salt("viewport_control_catalog").max_height(240.).show(ui, |ui| {
                    for kind in kinds {
                        let label = self.viewport_control_label(&ViewportControl { kind: kind.clone(), style: self.viewport_controls.new_style, icon: Icon::Auto, label: String::new(), position: [0.; 2], offset: [0.; 2] }, document);
                        if label.to_lowercase().contains(&search) {
                            ui.horizontal(|ui| {
                                if ui.add_enabled(self.preferences.viewport_controls.len() < 128, egui::Button::new("Add")).clicked() { self.add_viewport_control(kind.clone()); }
                                let response = ui.label(label);
                                super::search::highlight_response(&response, &format!("navigation:{kind:?}"));
                            });
                        }
                    }
                });
            });
        if place {
            self.materialise_viewport_controls(None);
            open = false;
            self.viewport_controls.arranging = true;
        }
        if return_to_workspace {
            open = false;
            self.viewport_controls.arranging = false;
            self.layout_open = true;
        }
        self.viewport_controls.editor_open = open;
    }
}

fn style_picker(ui: &mut egui::Ui, id: &str, style: &mut Style) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .width(90.)
        .selected_text(style.label())
        .show_ui(ui, |ui| {
            for option in Style::ALL {
                changed |= ui.selectable_value(style, option, option.label()).changed();
            }
        });
    changed
}

fn icon_glyph(icon: Icon) -> Option<Glyph> {
    Some(match icon {
        Icon::Auto => return None,
        Icon::Pan => Glyph::Pan,
        Icon::Zoom => Glyph::Zoom,
        Icon::Orbit => Glyph::Orbit,
        Icon::Roll => Glyph::Roll,
        Icon::Reset => Glyph::Restore,
        Icon::Groups => Glyph::Groups,
        Icon::Wireframe => Glyph::Wire,
        Icon::Brush => Glyph::Edit,
        Icon::Eye => Glyph::View,
        Icon::Undo => Glyph::Undo,
        Icon::Redo => Glyph::Redo,
        Icon::Plus => Glyph::Plus,
        Icon::Minus => Glyph::Minus,
        Icon::Settings => Glyph::Settings,
    })
}

fn control_glyph(control: &ViewportControl) -> Glyph {
    if let Some(glyph) = icon_glyph(control.icon) {
        return glyph;
    }
    match &control.kind {
        Kind::Pan => Glyph::Pan,
        Kind::Zoom => Glyph::Zoom,
        Kind::Orbit => Glyph::Orbit,
        Kind::Roll => Glyph::Roll,
        Kind::Action(action) => match action {
            Action::ResetView | Action::ResetZoom => Glyph::Restore,
            Action::PolygroupColors | Action::GroupAll | Action::GroupMasked => Glyph::Groups,
            Action::Wireframe | Action::WireTriangles => Glyph::Wire,
            Action::Undo => Glyph::Undo,
            Action::Redo => Glyph::Redo,
            Action::ZoomIn | Action::Larger | Action::HigherLevel => Glyph::Plus,
            Action::ZoomOut | Action::Smaller | Action::LowerLevel => Glyph::Minus,
            Action::New => Glyph::New,
            Action::Save | Action::SaveAs => Glyph::Save,
            Action::Open => Glyph::Open,
            Action::Export => Glyph::Export,
            Action::Perspective | Action::Backfaces | Action::FlatShading => Glyph::View,
            Action::Symmetry => Glyph::Symmetry,
            Action::CustomizeWorkspace | Action::KeySettings => Glyph::Settings,
            Action::Subdivide | Action::Remesh => Glyph::Mesh,
            Action::Move | Action::Scale | Action::Rotate => Glyph::Transform,
            _ => Glyph::Edit,
        },
    }
}

#[cfg(test)]
mod compact_layout_tests {
    use super::*;

    fn render(state: &mut UiState, ctx: &egui::Context, rect: egui::Rect) {
        state.viewport_menu.viewport_rect = Some(rect);
        let document = Document::default();
        for _ in 0..3 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    state.viewport_controls(ctx, rect, &document, true);
                },
            );
        }
    }
    fn control_rect(ctx: &egui::Context, index: usize) -> egui::Rect {
        ctx.data(|data| {
            data.get_temp::<egui::Rect>(egui::Id::new(("viewport_control_response", index)))
        })
        .unwrap()
    }

    #[test]
    fn compact_stock_controls_fit_without_overlapping_gizmo_launcher_or_each_other() {
        for size in [
            egui::vec2(624., 229.),
            egui::vec2(480., 229.),
            egui::vec2(624., 180.),
        ] {
            let mut state = UiState::default();
            let before = state.preferences.clone();
            let ctx = egui::Context::default();
            let rect = egui::Rect::from_min_size(egui::pos2(8., 166.), size);
            render(&mut state, &ctx, rect);
            let gizmo = egui::Rect::from_min_size(
                egui::pos2(rect.right() - 146., rect.top() + 12.),
                Vec2::splat(134.),
            );
            let launcher = ctx
                .data(|data| {
                    data.get_temp::<egui::Rect>(egui::Id::new(
                        "viewport_controls_launcher_response",
                    ))
                })
                .unwrap();
            for index in 0..state.preferences.viewport_controls.len() {
                let control = control_rect(&ctx, index);
                assert!(
                    rect.contains_rect(control),
                    "{index}: {control:?} outside {rect:?}"
                );
                assert!(
                    !control.intersects(gizmo),
                    "{index} overlaps the axis gizmo"
                );
                assert!(
                    !control.intersects(launcher),
                    "{index} overlaps the editor launcher"
                );
                for previous in 0..index {
                    assert!(
                        !control.intersects(control_rect(&ctx, previous)),
                        "{index} overlaps {previous}"
                    );
                }
            }
            assert_eq!(
                state.preferences, before,
                "responsive placement must not rewrite preferences"
            );
            let positions: Vec<_> = (0..7).map(|index| control_rect(&ctx, index)).collect();
            state.viewport_menu.viewport_rect = Some(rect);
            state.set_zen_mode(true);
            let zen_rect = state.navigation_rect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            ));
            render(&mut state, &ctx, zen_rect);
            assert_eq!(
                (0..7)
                    .map(|index| control_rect(&ctx, index))
                    .collect::<Vec<_>>(),
                positions
            );
        }
    }

    #[test]
    fn compact_layout_keeps_custom_control_positions() {
        let mut state = UiState::default();
        state.preferences.viewport_controls[0].position = [0.2, 0.5];
        state.preferences.viewport_controls[0].offset = [4., -6.];
        let before = state.preferences.viewport_controls.clone();
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::pos2(8., 20.), egui::vec2(624., 500.));
        render(&mut state, &ctx, rect);
        let bounds = rect.shrink(24.);
        for (index, control) in before.iter().enumerate() {
            let actual = control_rect(&ctx, index);
            let expected = bounds.min
                + (bounds.size() - actual.size())
                    * egui::vec2(control.position[0], control.position[1])
                + egui::vec2(control.offset[0], control.offset[1]);
            assert!(
                (actual.min - expected).length() < 1.,
                "mixed custom layout moved control {index}"
            );
        }
        assert_eq!(state.preferences.viewport_controls, before);
    }

    #[test]
    fn manual_alignment_survives_workspace_switch_and_serialization() {
        let mut state = UiState::default();
        let ctx = egui::Context::default();
        state.preferences.save_workspace_as("Adaptive").unwrap();
        state
            .preferences
            .save_workspace_as("Manual column")
            .unwrap();
        state.align_viewport_controls(&ctx, true);
        assert!(!state.preferences.viewport_controls_auto_layout);
        state.preferences.sync_active_workspace();
        state.preferences.switch_workspace(0);
        assert!(state.preferences.viewport_controls_auto_layout);
        state.preferences.switch_workspace(1);
        assert!(!state.preferences.viewport_controls_auto_layout);
        let restored: Preferences =
            serde_json::from_str(&serde_json::to_string(&state.preferences).unwrap()).unwrap();
        restored.validate().unwrap();
        state.preferences = restored;
        let rect = egui::Rect::from_min_size(egui::pos2(8., 20.), egui::vec2(624., 500.));
        render(&mut state, &ctx, rect);
        for index in 1..7 {
            let previous = control_rect(&ctx, index - 1);
            let next = control_rect(&ctx, index);
            assert!((previous.right() - next.right()).abs() < 1.);
            assert!((next.top() - previous.bottom() - 8.).abs() < 1.);
        }
    }

    #[test]
    fn adding_or_placing_controls_keeps_the_visible_compact_arrangement() {
        for add in [false, true] {
            let mut state = UiState::default();
            let ctx = egui::Context::default();
            let rect = egui::Rect::from_min_size(egui::pos2(8., 166.), egui::vec2(624., 229.));
            render(&mut state, &ctx, rect);
            let before: Vec<_> = (0..7).map(|index| control_rect(&ctx, index)).collect();
            if add {
                state.add_viewport_control(Kind::Action(Action::Undo));
            } else {
                state.materialise_viewport_controls(Some(rect));
                state.viewport_controls.arranging = true;
            }
            assert!(!state.preferences.viewport_controls_auto_layout);
            render(&mut state, &ctx, rect);
            for (index, old) in before.iter().enumerate() {
                assert!(
                    (control_rect(&ctx, index).min - old.min).length() < 1.,
                    "control {index} jumped during explicit edit"
                );
            }
            state.preferences.validate().unwrap();
        }
    }

    #[test]
    fn old_viewport_preferences_get_safe_auto_layout_default() {
        let mut preferences = Preferences::default();
        preferences.save_workspace_as("Legacy").unwrap();
        let mut old = serde_json::to_value(preferences).unwrap();
        old.as_object_mut()
            .unwrap()
            .remove("viewport_controls_auto_layout");
        old["workspaces"][0]
            .as_object_mut()
            .unwrap()
            .remove("viewport_controls_auto_layout");
        let restored: Preferences = serde_json::from_value(old).unwrap();
        assert!(restored.viewport_controls_auto_layout);
        assert!(restored.workspaces[0].viewport_controls_auto_layout);
        restored.validate().unwrap();
    }
}

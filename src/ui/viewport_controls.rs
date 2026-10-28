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
        }
    }
}

pub(crate) struct ControlsOutput {
    pub commands: Vec<Command>,
    pub navigation: Vec<(Kind, Vec2)>,
    pub blocks_viewport: bool,
}

impl UiState {
    pub(crate) fn align_viewport_controls(&mut self, ctx: &egui::Context, vertical: bool) {
        self.viewport_controls.pending_alignment = Some(vertical);
        self.apply_viewport_alignment(ctx, vertical);
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
                Kind::Action(_) => "Right-click to change style, move or remove",
            }
        };
        let shortcut = action
            .and_then(|a| self.preferences.shortcut(a))
            .map(|s| format!(" · {}", s.label()))
            .unwrap_or_default();
        let response = response.on_hover_text(format!("{label}{shortcut}\n{hint}"));
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

    pub(super) fn add_viewport_control(&mut self, kind: Kind) {
        if self.preferences.viewport_controls.len() >= 128 {
            self.status = "The viewport holds up to 128 controls".into();
            return;
        }
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
    }

    fn viewport_control_label(&self, control: &ViewportControl, document: &Document) -> String {
        if !control.label.trim().is_empty() {
            return control.label.clone();
        }
        match &control.kind {
            Kind::Pan => "Pan view".into(),
            Kind::Zoom => "Zoom view".into(),
            Kind::Orbit => "Rotate view".into(),
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
            .fixed_pos(egui::pos2(rect.left() + 24., launcher_y))
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
            let position = bounds.min
                + egui::vec2(
                    control.position[0] * travel.x,
                    control.position[1] * travel.y,
                )
                + egui::vec2(control.offset[0], control.offset[1]);
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
                                let control = &mut self.preferences.viewport_controls[index];
                                ui.label("Button style");
                                for style in Style::ALL {
                                    if ui
                                        .selectable_value(&mut control.style, style, style.label())
                                        .clicked()
                                    {
                                        ui.close();
                                    }
                                }
                                ui.separator();

                                if ui.button("Move viewport controls").clicked() {
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
        egui::Window::new("Viewport controls").open(&mut open).default_size([620., 740.])
            .max_height((ctx.content_rect().height() - 100.).max(160.)).vscroll(true).show(ctx, |ui| {
                ui.label("Add controls, then drag them anywhere in the drawing area. Layouts save automatically with your workspace.");
                ui.horizontal(|ui| {
                    place = ui.button("Place controls").clicked();
                    if ui.button("Restore default controls").clicked() { self.preferences.viewport_controls = crate::preferences::default_viewport_controls(); }
                });
                ui.horizontal(|ui| {
                    ui.label("New button style");
                    style_picker(ui, "new_style", &mut self.viewport_controls.new_style);
                    if ui.button("Apply style to all").clicked() {
                        for control in &mut self.preferences.viewport_controls { control.style = self.viewport_controls.new_style; }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.viewport_controls.gap).range(4. ..=32.).prefix("Spacing: ").suffix(" px"));
                    if ui.button("Align right").clicked() { self.align_viewport_controls(ctx, true); }
                    if ui.button("Align bottom").clicked() { self.align_viewport_controls(ctx, false); }
                });
                ui.separator();
                let mut remove = None;
                for (index, control) in self.preferences.viewport_controls.clone().iter().enumerate() {
                    ui.push_id(index, |ui| { ui.horizontal(|ui| {
                        ui.add_sized([130., 32.], egui::Label::new(self.viewport_control_label(control, document)).truncate());
                        style_picker(ui, "style", &mut self.preferences.viewport_controls[index].style);
                        let glyph = control_glyph(control);
                        egui::ComboBox::from_id_salt("icon").width(80.).selected_text(control.icon.label()).show_ui(ui, |ui| {
                            for icon in Icon::ALL {
                                if ui.add(design::button(ui, icon_glyph(icon).unwrap_or(glyph), icon.label()).selected(control.icon == icon)).clicked() {
                                    self.preferences.viewport_controls[index].icon = icon;
                                    ui.close();
                                }
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut self.preferences.viewport_controls[index].label).desired_width(90.).char_limit(128).hint_text("Custom label"));
                        if ui.button("Remove").clicked() { remove = Some(index); }
                    }); });
                }
                if let Some(index) = remove { self.preferences.viewport_controls.remove(index); }
                ui.separator();
                ui.heading("Add a control");
                ui.add(egui::TextEdit::singleline(&mut self.viewport_controls.search).hint_text("Search controls, brushes and menus"));
                let search = self.viewport_controls.search.to_lowercase();
                let kinds = [Kind::Pan, Kind::Zoom, Kind::Orbit].into_iter()
                    .chain(self.action_catalog(document).into_iter().map(Kind::Action));
                egui::ScrollArea::vertical().id_salt("viewport_control_catalog").max_height(240.).show(ui, |ui| {
                    for kind in kinds {
                        let label = self.viewport_control_label(&ViewportControl { kind: kind.clone(), style: self.viewport_controls.new_style, icon: Icon::Auto, label: String::new(), position: [0.; 2], offset: [0.; 2] }, document);
                        if label.to_lowercase().contains(&search) {
                            ui.horizontal(|ui| {
                                if ui.add_enabled(self.preferences.viewport_controls.len() < 128, egui::Button::new("Add")).clicked() { self.add_viewport_control(kind); }
                                ui.label(label);
                            });
                        }
                    }
                });
            });
        if place {
            open = false;
            self.viewport_controls.arranging = true;
        }
        self.viewport_controls.editor_open = open;
    }
}

fn style_picker(ui: &mut egui::Ui, id: &str, style: &mut Style) {
    egui::ComboBox::from_id_salt(id)
        .width(90.)
        .selected_text(style.label())
        .show_ui(ui, |ui| {
            for option in Style::ALL {
                ui.selectable_value(style, option, option.label());
            }
        });
}

fn icon_glyph(icon: Icon) -> Option<Glyph> {
    Some(match icon {
        Icon::Auto => return None,
        Icon::Pan => Glyph::Pan,
        Icon::Zoom => Glyph::Zoom,
        Icon::Orbit => Glyph::Orbit,
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
            Action::Perspective | Action::Backfaces => Glyph::View,
            Action::Symmetry => Glyph::Symmetry,
            Action::CustomizeWorkspace | Action::KeySettings => Glyph::Settings,
            Action::Subdivide | Action::Remesh => Glyph::Mesh,
            Action::Move | Action::Scale | Action::Rotate => Glyph::Transform,
            _ => Glyph::Edit,
        },
    }
}

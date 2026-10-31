//! Live brush controls shared by custom shelves and shortcut popups.
use super::*;
use crate::sculpt::StrokeMode;

pub(super) fn is_control(action: &Action) -> bool {
    matches!(
        action,
        Action::AppearanceControls
            | Action::BrushIcon
            | Action::BrushSize
            | Action::BrushInnerRadius
            | Action::BrushStrength
            | Action::SurfaceFootprint
            | Action::StrokeMode
            | Action::StrokeStabilisation
    )
}

impl UiState {
    pub(super) fn preview_brush_size(&mut self, response: &egui::Response, index: usize) {
        if response.dragged()
            || response.is_pointer_button_down_on()
            || response.has_focus()
            || response.changed()
        {
            self.brush_size_preview = Some((response.ctx.cumulative_frame_nr(), index));
        }
    }
    pub(super) fn brush_control_width(
        &self,
        ui: &egui::Ui,
        action: &Action,
        label: &str,
        document: &Document,
    ) -> f32 {
        if *action == Action::AppearanceControls {
            return ui.available_width().min(540.);
        }
        let (default, extra) = match action {
            Action::BrushIcon => (
                self.brush_index_for_context(document, ui.ctx())
                    .map(|i| document.brushes()[i].name.as_str())
                    .unwrap_or("No active brush"),
                42.,
            ),
            Action::BrushSize => ("Size", 70.),
            Action::BrushInnerRadius => ("Inner radius", 64.),
            Action::BrushStrength => ("Strength", 64.),
            Action::SurfaceFootprint => ("Surface footprint and normal", 30.),
            Action::StrokeMode => ("Stroke mode", 110.),
            Action::StrokeStabilisation => ("Stroke stabilisation", 94.),
            _ => return 140.,
        };
        let text = if label.is_empty() { default } else { label };
        ui.painter()
            .layout_no_wrap(
                text.into(),
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x
            + extra
    }
    pub(super) fn brush_control(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        label: &str,
        document: &Document,
        commands: &mut Vec<Command>,
    ) -> egui::Response {
        if *action == Action::AppearanceControls {
            return ui
                .add_enabled_ui(!self.mesh_busy, |ui| {
                    ui.horizontal_wrapped(|ui| self.appearance_strip(ui, document, commands));
                })
                .response;
        }
        let Some(index) = self.brush_index_for_context(document, ui.ctx()) else {
            return ui.weak("No active brush");
        };
        // Several controls may edit the same preset in one frame (including a popup).
        let mut brush = commands
            .iter()
            .rev()
            .find_map(|command| match command {
                Command::UpdateBrush(i, b) if *i == index => Some(b.clone()),
                _ => None,
            })
            .unwrap_or_else(|| document.brushes()[index].clone());
        let before = brush.clone();
        let response = ui.add_enabled_ui(self.action_enabled(action, document), |ui| {
            ui.horizontal_wrapped(|ui| match action {
                Action::BrushIcon => {
                    let texture = self.brush_texture(ui.ctx(), index, &brush);
                    ui.add(egui::Image::new((texture, egui::vec2(32., 32.))).sense(Sense::click()))
                        .union(ui.add(egui::Label::new(if label.is_empty() { &brush.name } else { label }).sense(Sense::click())))
                }
                Action::BrushSize => {
                    let label = ui.add(egui::Label::new(if label.is_empty() { "Size" } else { label }).sense(Sense::click()));
                    let value = ui.add(egui::DragValue::new(&mut brush.radius)
                        .speed(1.).range(1. ..=500.).suffix(" px"))
                        .on_hover_text("Drag left/right to change brush radius. Double-click to type a value.");
                    self.preview_brush_size(&value, index);
                    label.union(value)
                }
                Action::BrushStrength => {
                    let label = ui.add(egui::Label::new(if label.is_empty() { "Strength" } else { label }).sense(Sense::click()));
                    label.union(ui.add(egui::DragValue::new(&mut brush.strength)
                        .speed(0.005).range(0. ..=1.)
                        .custom_formatter(|value, _| format!("{:.0}%", value * 100.))
                        .custom_parser(|text| text.trim().trim_end_matches('%').parse::<f64>().ok().map(|v| v / 100.)))
                        .on_hover_text("Drag left/right to change strength. Double-click to enter a percentage."))
                }
                Action::BrushInnerRadius => {
                    let label = ui.add(egui::Label::new(if label.is_empty() { "Inner radius" } else { label }).sense(Sense::click()));
                    let value = ui.add(egui::DragValue::new(&mut brush.inner_radius)
                        .speed(0.005).range(0. ..=1.)
                        .custom_formatter(|value, _| format!("{:.0}%", value * 100.))
                        .custom_parser(|text| text.trim().trim_end_matches('%').parse::<f64>().ok().map(|v| v / 100.)))
                        .on_hover_text("Full-strength core as a percentage of the outer radius. Drag to adjust; double-click to type.");
                    self.preview_brush_size(&value, index);
                    label.union(value)
                }
                Action::SurfaceFootprint => {
                    ui.add(egui::Checkbox::new(&mut brush.surface_cursor,
                        if label.is_empty() { "Surface footprint and normal" } else { label }))
                }
                Action::StrokeMode => {
                    let label = ui.add(egui::Label::new(if label.is_empty() { "Stroke mode" } else { label }).sense(Sense::click()));
                    // The enabled scope cannot advertise the combo's width to
                    // the wrapping layout before it lays out its contents.
                    if ui.available_size_before_wrap().x < 100. {
                        ui.end_row();
                    }
                    label.union(ui.add_enabled_ui(!matches!(brush.kind, Kind::Grab | Kind::Snake | Kind::Relax), |ui| {
                        egui::ComboBox::from_id_salt("stroke_mode").width(90.)
                            .selected_text(brush.effective_stroke_mode().label())
                            .show_ui(ui, |ui| {
                                for mode in StrokeMode::ALL {
                                    ui.selectable_value(&mut brush.stroke_mode, mode, mode.label())
                                        .on_hover_text(mode.description());
                                }
                            }).response
                    }).inner)
                }
                Action::StrokeStabilisation => {
                    ui.add_enabled_ui(brush.supports_stabilization(), |ui| {
                        let checkbox = ui.add(egui::Checkbox::new(&mut brush.stabilize,
                            if label.is_empty() { "Stroke stabilisation" } else { label }));
                        checkbox.union(ui.add_enabled(brush.stabilize,
                            egui::DragValue::new(&mut brush.stabilizer_distance)
                                .speed(1.).range(1. ..=200.).suffix(" px"))
                            .on_hover_text("Trailing distance. Changes apply to the next stroke."))
                    }).inner
                }
                _ => unreachable!(),
            }).inner
        }).inner;
        if matches!(action, Action::BrushSize | Action::BrushInnerRadius) {
            self.preview_brush_size(&response, index);
        }
        if before != brush {
            commands.push(Command::UpdateBrush(index, brush));
        }
        response
    }
}

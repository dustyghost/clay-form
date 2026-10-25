use super::*;
use crate::sculpt::uv::UvMap;
#[derive(Clone, Copy, Default, PartialEq)]
pub(super) enum Tool {
    #[default]
    Move,
    Rotate,
    Scale,
    Mask,
    EraseMask,
}
impl Tool {
    pub(super) fn is_mask(self) -> bool {
        matches!(self, Self::Mask | Self::EraseMask)
    }
}
pub(super) struct Drag {
    base: UvMap,
    pub preview: UvMap,
    start: egui::Pos2,
    last: egui::Pos2,
    pivot: egui::Pos2,
    tool: Tool,
    erase: bool,
}
pub(super) fn pivot(uv: &UvMap, faces: &[usize], rect: egui::Rect) -> Option<egui::Pos2> {
    let mut sum = [0.; 2];
    let mut count = 0;
    for &f in faces {
        if uv.mapped(f) {
            for q in uv.corners[f] {
                sum[0] += q[0];
                sum[1] += q[1];
                count += 1;
            }
        }
    }
    (count > 0).then(|| {
        egui::pos2(
            rect.left() + sum[0] / count as f32 * rect.width(),
            rect.bottom() - sum[1] / count as f32 * rect.height(),
        )
    })
}
pub(super) fn handle(tool: Tool, pivot: egui::Pos2) -> egui::Pos2 {
    match tool {
        Tool::Rotate => pivot + egui::vec2(38., 0.),
        Tool::Scale => pivot + egui::vec2(32., -32.),
        _ => pivot,
    }
}
impl UiState {
    pub(super) fn uv_tools(&mut self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        ui.horizontal_wrapped(|ui| {
            for (tool,label) in [(Tool::Move,"Move"),(Tool::Rotate,"Rotate"),(Tool::Scale,"Scale"),(Tool::Mask,"Paint mask"),(Tool::EraseMask,"Erase mask")] {
                if ui.selectable_value(&mut self.uv.tool,tool,label).changed() { self.uv.drag=None; }
            }
            if ui.button("Clear UV mask").clicked() { commands.push(Command::UvClearMask); }
            if ui.add_enabled(!self.uv.selected.is_empty(),egui::Button::new("Remove islands"))
                .on_hover_text("Remove selected UV mappings; keep the 3D mesh. Undo restores them. Masked islands are protected.").clicked() {
                commands.push(Command::UvRemove(self.uv.selected.clone()));
            }
        });
        if self.uv.tool.is_mask() {
            if self.uv.radius == 0. {
                self.uv.radius = 40.;
            }
            ui.add(egui::Slider::new(&mut self.uv.radius, 4. ..=160.).text("Radius (px)"));
            ui.weak("Purple pins UV points · Alt reverses · packing keeps masked islands fixed");
        }
    }
    pub(super) fn uv_interaction(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        uv: &UvMap,
        commands: &mut Vec<Command>,
    ) -> bool {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.uv.drag = None;
            return true;
        }
        let rect = response.rect;
        let origin = ui.input(|i| i.pointer.press_origin());
        if self.uv.drag.is_none()
            && response.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
            && let Some(start) = origin.filter(|p| rect.contains(*p))
        {
            let center = pivot(uv, &self.uv.selected, rect).unwrap_or(rect.center());
            let active = self.uv.tool.is_mask()
                || (!self.uv.selected.is_empty()
                    && if self.uv.tool == Tool::Rotate {
                        (start.distance(center) - 38.).abs() < 12.
                    } else {
                        start.distance(handle(self.uv.tool, center)) < 16.
                    });
            if active {
                self.uv.drag = Some(Drag {
                    base: uv.clone(),
                    preview: uv.clone(),
                    start,
                    last: start,
                    pivot: center,
                    tool: self.uv.tool,
                    erase: (self.uv.tool == Tool::EraseMask) ^ ui.input(|i| i.modifiers.alt),
                });
            }
        }
        let consumed = self.uv.drag.is_some();
        if let Some(drag) = &mut self.uv.drag {
            if let Some(p) = response.interact_pointer_pos() {
                if drag.tool.is_mask() {
                    // Interpolate all coalesced pointer events so fast strokes do not leave holes.
                    let mut path: Vec<_> = ui.input(|i| {
                        i.events
                            .iter()
                            .filter_map(|e| match e {
                                egui::Event::PointerMoved(p) => Some(*p),
                                _ => None,
                            })
                            .collect()
                    });
                    path.push(p);
                    for end in path {
                        let steps = (drag.last.distance(end) / (self.uv.radius * 0.2).max(1.))
                            .ceil()
                            .clamp(1., 4096.) as usize;
                        for step in 0..=steps {
                            let at = drag.last.lerp(end, step as f32 / steps as f32);
                            if rect.contains(at) {
                                drag.preview.paint_mask(
                                    [
                                        (at.x - rect.left()) / rect.width(),
                                        (rect.bottom() - at.y) / rect.height(),
                                    ],
                                    self.uv.radius / rect.width(),
                                    drag.erase,
                                );
                            }
                        }
                        drag.last = end;
                    }
                } else {
                    drag.preview = drag.base.clone();
                    let delta = p - drag.start;
                    let (offset, scale, angle) = match drag.tool {
                        Tool::Move => ([delta.x / rect.width(), -delta.y / rect.height()], 1., 0.),
                        Tool::Scale => (
                            [0.; 2],
                            (p.distance(drag.pivot) / drag.start.distance(drag.pivot).max(1.))
                                .clamp(0.001, 100.),
                            0.,
                        ),
                        Tool::Rotate => {
                            let a = drag.start - drag.pivot;
                            let b = p - drag.pivot;
                            ([0.; 2], 1., a.angle() - b.angle())
                        }
                        _ => unreachable!(),
                    };
                    if drag
                        .preview
                        .transform(&self.uv.selected, offset, scale, angle)
                        .is_err()
                    {
                        drag.preview = drag.base.clone();
                    }
                }
            }
            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                let drag = self.uv.drag.take().unwrap();
                if drag.preview != drag.base {
                    commands.push(Command::UvReplace(Box::new(drag.preview)));
                }
            } else if !ui.input(|i| i.pointer.primary_down()) {
                self.uv.drag = None;
            }
        }
        consumed
    }
    pub(super) fn uv_paint_handles(&self, ui: &egui::Ui, response: &egui::Response, uv: &UvMap) {
        let painter = ui.painter_at(response.rect);
        let color = Color32::from_rgb(255, 220, 100);
        if self.uv.tool.is_mask() {
            if let Some(p) = response
                .hover_pos()
                .or_else(|| response.interact_pointer_pos())
            {
                painter.circle_stroke(p, self.uv.radius, (1.5, Color32::WHITE));
                painter.circle_stroke(
                    p,
                    self.uv.radius * 0.5,
                    (1., Color32::from_rgb(190, 100, 240)),
                );
            }
            // Dots make mask resolution explicit on coarse meshes.
            for (f, t) in uv.corners.iter().enumerate().filter(|(f, _)| uv.mapped(*f)) {
                for (c, q) in t.iter().enumerate() {
                    painter.circle_filled(
                        egui::pos2(
                            response.rect.left() + q[0] * response.rect.width(),
                            response.rect.bottom() - q[1] * response.rect.height(),
                        ),
                        2.5,
                        if uv.mask(f, c) > 0. {
                            Color32::from_rgb(210, 120, 255)
                        } else {
                            Color32::WHITE
                        },
                    );
                }
            }
        } else if let Some(center) = pivot(uv, &self.uv.selected, response.rect) {
            let h = handle(self.uv.tool, center);
            match self.uv.tool {
                Tool::Move => {
                    painter.circle_filled(center, 8., color);
                    painter.arrow(center, egui::vec2(36., 0.), (2., Color32::LIGHT_RED));
                    painter.arrow(center, egui::vec2(0., -36.), (2., Color32::LIGHT_GREEN));
                }
                Tool::Rotate => {
                    painter.circle_stroke(center, 38., (2., color));
                }
                Tool::Scale => {
                    painter.line_segment([center, h], (2., color));
                    painter.rect_filled(
                        egui::Rect::from_center_size(h, Vec2::splat(12.)),
                        2.,
                        color,
                    );
                }
                _ => {}
            }
        }
    }
}

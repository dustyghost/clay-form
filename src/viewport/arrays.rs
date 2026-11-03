//! Direct controls for an attached array. A drag owns its ordered pointer events.
use super::*;
use crate::{command::InstanceCommand as C, sculpt::instances::*};

#[derive(Clone, Copy)]
enum Parameter {
    Spacing(usize),
    Radius,
    Arc,
    Origin,
}
struct Handle {
    parameter: Parameter,
    point: Pos2,
    direction: Vec2,
    label: &'static str,
}
struct Drag {
    before: Set,
    layer: usize,
    parameter: Parameter,
    start: Pos2,
    direction: Vec2,
    center: Pos2,
    u: Vec2,
    v: Vec2,
    last_angle: f32,
    arc_angle: f32,
    last_pointer: Pos2,
    arc_tangent: Option<Vec2>,
}
#[derive(Default)]
pub(super) struct Editor {
    drag: Option<Drag>,
}
fn component(v: Vec3, i: usize) -> f32 {
    [v.0, v.1, v.2][i]
}
fn set_component(v: &mut Vec3, i: usize, x: f32) {
    match i {
        0 => v.0 = x,
        1 => v.1 = x,
        _ => v.2 = x,
    }
}
fn angle(p: Pos2, center: Pos2, u: Vec2, v: Vec2) -> f32 {
    let d = p - center;
    let det = u.x * v.y - u.y * v.x;
    if det.abs() < 0.01 {
        return 0.;
    }
    ((u.x * d.y - u.y * d.x) / det).atan2((d.x * v.y - d.y * v.x) / det)
}
impl Editor {
    pub fn cancel(&mut self, document: &mut Document) {
        if self.drag.take().is_some() {
            document.cancel_instance_preview();
        }
    }
    fn update(&mut self, p: Pos2, scene: &Scene, document: &mut Document) -> Result<(), String> {
        let Some(d) = &mut self.drag else {
            return Ok(());
        };
        let mut set = d.before.clone();
        let Layout::Array(a) = &mut set.layout else {
            return Ok(());
        };
        let output = a.output;
        let layer = a.layer_mut(d.layer).ok_or("Array layer no longer exists")?;
        let delta = p - d.start;
        let distance = delta.dot(d.direction) / d.direction.length_sq().max(0.0001);
        match d.parameter {
            Parameter::Spacing(i) => {
                let value = component(layer.spacing, i) + distance;
                set_component(&mut layer.spacing, i, value);
            }
            Parameter::Radius => layer.radius = (layer.radius + distance).clamp(0.001, 1e5),
            Parameter::Arc => {
                let step = if let Some(tangent) = d.arc_tangent {
                    (p - d.last_pointer).dot(tangent)
                } else if p.distance_sq(d.center) >= 16. {
                    let now = angle(p, d.center, d.u, d.v);
                    let step = (now - d.last_angle + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    d.last_angle = now;
                    step
                } else {
                    0.
                };
                // Accumulate each motion, rather than choosing the shortest
                // angle from mouse-down. Clamp each step so reversal at either
                // endpoint responds immediately without unwinding overshoot.
                d.arc_angle = (d.arc_angle + step.to_degrees()).clamp(0.01, 360.);
                d.last_pointer = p;
                layer.angle = d.arc_angle;
            }
            Parameter::Origin => {
                let world = scene.at_depth(p, output.point(layer.center))
                    - scene.at_depth(d.start, output.point(layer.center));
                let local = output.linear().inverse_vector(world) * (1. / output.scale);
                layer.center = layer.center + local;
            }
        }
        document.preview_instance_set(set)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        document: &mut Document,
        options: ViewportOptions,
        layer_index: Option<usize>,
        command: &mut Option<C>,
    ) -> bool {
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("array_spacing_popup_visible"), false));
        let Some(index) = layer_index else {
            self.cancel(document);
            return false;
        };
        let Some(mut set) = document
            .attached_array(document.active_model_index())
            .cloned()
        else {
            self.cancel(document);
            return false;
        };
        if !set.visible || document.editing_array_source() || document.selected_instance.is_some() {
            self.cancel(document);
            return false;
        }
        if !options.inline_controls_enabled || !ui.input(|i| i.focused) {
            self.cancel(document);
            return false;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && self.drag.is_some() {
            self.cancel(document);
            return true;
        }
        let before = set.clone();
        let id = set.id;
        let rect = navigation::inner(response.rect).shrink(8.);
        if rect.width() < 150. || rect.height() < 100. {
            return false;
        }
        let mut widgets = Vec::new();
        egui::Area::new(egui::Id::new(("array_inline", id)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min + egui::vec2(0., 45.))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width((rect.width() - 155.).clamp(140., 650.));
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(format!("Array · Layer {}", index + 1));
                        if let Layout::Array(a) = &mut set.layout
                            && let Some(layer) = a.layer_mut(index)
                        {
                            let old_kind = layer.kind;
                            for (kind, name) in [
                                (ArrayKind::Linear, "Linear"),
                                (ArrayKind::Radial, "Radial"),
                                (ArrayKind::Grid, "Grid"),
                            ] {
                                ui.selectable_value(&mut layer.kind, kind, name);
                            }
                            if layer.kind == ArrayKind::Grid && old_kind != layer.kind {
                                layer.count = layer.count.div_ceil(layer.columns) * layer.columns;
                                if layer.spacing_mode == ArraySpacing::Span {
                                    layer.spacing_mode = ArraySpacing::Distance;
                                }
                                if layer.spacing.1 == 0. {
                                    layer.spacing.1 = 1.;
                                }
                            }
                            if layer.kind == ArrayKind::Grid {
                                let mut rows = layer.count.div_ceil(layer.columns);
                                let old = (rows, layer.columns);
                                let r = ui.add(
                                    egui::DragValue::new(&mut layer.columns)
                                        .range(1..=256)
                                        .prefix("Columns "),
                                );
                                widgets.push(("Columns", r.rect));
                                let r = ui.add(
                                    egui::DragValue::new(&mut rows)
                                        .range(1..=256)
                                        .prefix("Rows "),
                                );
                                widgets.push(("Rows", r.rect));
                                if old != (rows, layer.columns) {
                                    layer.count = rows * layer.columns;
                                }
                            } else {
                                let r = ui.add(
                                    egui::DragValue::new(&mut layer.count)
                                        .range(1..=256)
                                        .prefix("Count "),
                                );
                                widgets.push(("Count", r.rect));
                            }
                            if layer.kind == ArrayKind::Radial {
                                ui.add(
                                    egui::DragValue::new(&mut layer.radius)
                                        .range(0.001..=1e5)
                                        .speed(0.01)
                                        .prefix("Radius "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut layer.angle)
                                        .range(0.01..=360.)
                                        .prefix("Arc ")
                                        .suffix("°"),
                                );
                            } else {
                                let combo = egui::ComboBox::from_id_salt("array_spacing_mode")
                                    .selected_text(match layer.spacing_mode {
                                        ArraySpacing::Relative => "Relative",
                                        ArraySpacing::Distance => "Distance",
                                        ArraySpacing::Span => "Fit span",
                                    })
                                    .show_ui(ui, |ui| {
                                        for (mode, label, key) in [
                                            (ArraySpacing::Relative, "Relative", "Relative option"),
                                            (ArraySpacing::Distance, "Distance", "Distance option"),
                                            (ArraySpacing::Span, "Fit span", "Fit span option"),
                                        ] {
                                            if mode == ArraySpacing::Span
                                                && layer.kind != ArrayKind::Linear
                                            {
                                                continue;
                                            }
                                            let response = ui.selectable_value(
                                                &mut layer.spacing_mode,
                                                mode,
                                                label,
                                            );
                                            widgets.push((key, response.rect));
                                        }
                                    });
                                widgets.push(("Spacing mode", combo.response.rect));
                                #[cfg(test)]
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(
                                        egui::Id::new("array_spacing_popup_visible"),
                                        combo.inner.is_some(),
                                    )
                                });
                                let r = ui.add(
                                    egui::DragValue::new(&mut layer.spacing.0)
                                        .speed(0.01)
                                        .prefix("X "),
                                );
                                widgets.push(("Spacing", r.rect));
                                if layer.kind == ArrayKind::Grid {
                                    ui.add(
                                        egui::DragValue::new(&mut layer.spacing.1)
                                            .speed(0.01)
                                            .prefix("Y "),
                                    );
                                }
                            }
                        }
                        if ui.small_button("Edit source").clicked() {
                            *command = Some(C::ArrayMode(true));
                        }
                        if ui.small_button("Done").clicked() {
                            *command = Some(C::ArrayDone);
                        }
                        if ui.small_button("Frame").clicked() {
                            *command = Some(C::FrameArray);
                        }
                    });
                });
            });
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("array_inline_widgets"), widgets));
        if set != before {
            if let Err(e) = document.preview_instance_set(set.clone()) {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new("array_inline_error"), e));
            }
            ui.ctx().request_repaint();
        }
        let Some(set) = document
            .attached_array(document.active_model_index())
            .cloned()
        else {
            return false;
        };
        let Layout::Array(a) = &set.layout else {
            return false;
        };
        let Some(layer) = a.layer(index) else {
            return false;
        };
        if options.transform_mode != TransformMode::Sculpt {
            return false;
        }
        let mut center = a.center;
        for l in a.layers.iter().take(index) {
            center = center + l.center;
        }
        let world = a.output.point(center);
        let at = scene.project(world);
        let basis = Basis::step(layer.rotation, layer.size);
        let vector = |v: Vec3| a.output.vector(basis.vector(v)) * a.output.scale;
        let projected = |v: Vec3| scene.project(world + vector(v)) - at;
        let mut handles = vec![Handle {
            parameter: Parameter::Origin,
            point: at,
            direction: Vec2::ZERO,
            label: "Array origin",
        }];
        let (u, v) = if layer.kind == ArrayKind::Radial {
            let x = axis((layer.radial_axis + 1) % 3);
            let y = axis((layer.radial_axis + 2) % 3);
            let u = projected(x);
            let v = projected(y);
            let radius = layer.radius;
            let points: Vec<_> = (0..=64)
                .map(|i| {
                    let t = (layer.angle * i as f32 / 64.).to_radians();
                    scene.project(world + vector((x * t.cos() + y * t.sin()) * radius))
                })
                .collect();
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(1_f32, instances::SET_COLOR),
            ));
            let t = layer.angle.to_radians();
            let radial_direction = (u + v) * std::f32::consts::FRAC_1_SQRT_2 * 1.15;
            handles.push(Handle {
                parameter: Parameter::Radius,
                point: at + radial_direction * radius,
                direction: radial_direction,
                label: "Radius",
            });
            handles.push(Handle {
                parameter: Parameter::Arc,
                point: at + (u * t.cos() + v * t.sin()) * radius * 0.88,
                direction: Vec2::ZERO,
                label: "Arc",
            });
            (u, v)
        } else {
            let extent = document
                .array_layer_extent(&set, index)
                .unwrap_or(Vec3(1., 1., 1.));
            for i in 0..if layer.kind == ArrayKind::Grid { 2 } else { 3 } {
                let factor = match layer.spacing_mode {
                    ArraySpacing::Relative => component(extent, i),
                    ArraySpacing::Span => 1. / layer.count.saturating_sub(1).max(1) as f32,
                    ArraySpacing::Distance => 1.,
                };
                let direction = projected(axis(i) * factor);
                if direction.length() < 0.01 {
                    continue;
                }
                let step = component(layer.spacing, i);
                let p = if step.abs() * direction.length() > 28. {
                    at + direction * step
                } else {
                    at + direction.normalized() * 65.
                };
                handles.push(Handle {
                    parameter: Parameter::Spacing(i),
                    point: p,
                    direction,
                    label: if i == 0 {
                        "X spacing"
                    } else if i == 1 {
                        "Y spacing"
                    } else {
                        "Z spacing"
                    },
                });
            }
            (Vec2::ZERO, Vec2::ZERO)
        };
        let pointer = ui.input(|i| i.pointer.hover_pos());
        for h in &handles {
            let hovered = pointer.is_some_and(|p| p.distance(h.point) < controls::HIT_RADIUS);
            painter.line_segment(
                [at, h.point],
                egui::Stroke::new(1_f32, instances::SET_COLOR),
            );
            controls::marker(
                painter,
                h.point,
                controls::Marker::Diamond,
                instances::SET_COLOR,
                hovered,
            );
            controls::label(
                painter,
                h.point + egui::vec2(8., 7.),
                h.label,
                instances::SET_COLOR,
            );
        }
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                egui::Id::new("array_inline_handles"),
                handles
                    .iter()
                    .map(|h| (h.label, h.point))
                    .collect::<Vec<_>>(),
            )
        });
        let mut consumed = self.drag.is_some();
        for e in ui.input(|i| i.events.clone()) {
            match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                    ..
                } if self.drag.is_none()
                    && options.enabled
                    && !options.pan_hold.active(modifiers)
                    && !options.zoom_hold.active(modifiers)
                    && response.rect.contains(pos)
                    && ui.ctx().layer_id_at(pos) == Some(response.layer_id) =>
                {
                    if let Some(h) = handles
                        .iter()
                        .rev()
                        .find(|h| h.point.distance(pos) < controls::HIT_RADIUS)
                    {
                        ui.ctx()
                            .data_mut(|d| d.remove::<String>(egui::Id::new("array_inline_error")));
                        self.drag = Some(Drag {
                            before: set.clone(),
                            layer: index,
                            parameter: h.parameter,
                            start: pos,
                            direction: h.direction,
                            center: at,
                            u,
                            v,
                            last_angle: angle(pos, at, u, v),
                            arc_angle: layer.angle,
                            last_pointer: pos,
                            arc_tangent: {
                                let major = u.length().max(v.length());
                                let determinant = (u.x * v.y - u.y * v.x).abs();
                                (determinant < major * major * 0.15).then(|| {
                                    let t = layer.angle.to_radians();
                                    let tangent = -u * t.sin() + v * t.cos();
                                    let direction = if tangent.length_sq() > 0.01 {
                                        tangent
                                    } else if u.length_sq() > v.length_sq() {
                                        u
                                    } else {
                                        v
                                    };
                                    direction.normalized() / (major * layer.radius * 0.88).max(32.)
                                })
                            },
                        });
                        consumed = true;
                    }
                }
                egui::Event::PointerMoved(p) if self.drag.is_some() => {
                    if let Err(e) = self.update(p, scene, document) {
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(egui::Id::new("array_inline_error"), e));
                    }
                    consumed = true;
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } if self.drag.is_some() => {
                    let _ = self.update(pos, scene, document);
                    self.drag = None;
                    if let Err(e) = document.apply_instance_preview() {
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(egui::Id::new("array_inline_error"), e));
                    }
                    consumed = true;
                }
                _ => {}
            }
        }
        if let Some(error) = ui
            .ctx()
            .data(|d| d.get_temp::<String>(egui::Id::new("array_inline_error")))
        {
            controls::label(
                painter,
                rect.left_bottom() - egui::vec2(0., 24.),
                &error,
                controls::REVERSED,
            );
        }
        consumed
    }
}

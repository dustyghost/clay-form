//! Converts viewport gestures into camera movement and document sculpt dabs.
mod gizmo;
mod guides;
mod scene;
use crate::{
    camera::Camera,
    document::Document,
    sculpt::{Dab, Kind},
};
use eframe::egui::{self, Color32, Pos2, Sense, Vec2};
use gizmo::draw_gizmo;
use scene::Scene;
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Gesture {
    Sculpt,
    Orbit,
    Gizmo,
}
#[derive(Default)]
pub(crate) struct Viewport {
    pub(crate) gesture: Option<Gesture>,
    gizmo_dragged: bool,
    stroke: bool,
    last: Option<Pos2>,
}
#[derive(Clone, Copy)]
pub(crate) struct ViewportOptions {
    pub selected: usize,
    pub pressure: f32,
    pub wire: bool,
    pub show_guides: bool,
    pub guide_opacity: f32,
    pub enabled: bool,
}
impl Viewport {
    pub(crate) fn cancel(&mut self, document: &mut Document) {
        *self = Self::default();
        document.end_stroke();
    }
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        document: &mut Document,
        camera: &mut Camera,
        options: ViewportOptions,
    ) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;
        let gizmo_rect = egui::Rect::from_min_size(
            Pos2::new(rect.right() - 146., rect.top() + 12.),
            Vec2::splat(134.),
        );
        let scene = Scene::paint(&painter, rect, document.mesh(), camera, options.wire);
        let (right, up, _) = camera.basis();
        let scale = scene.scale;
        let pointer = response
            .hover_pos()
            .or_else(|| response.interact_pointer_pos());
        let modifiers = ui.input(|i| i.modifiers);
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            camera.zoom_by(scroll);
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            let d = ui.input(|i| i.pointer.delta());
            camera.orbit(d);
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            camera.pan += ui.input(|i| i.pointer.delta());
        }
        let primary =
            response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        if primary
            && self.gesture.is_none()
            && options.enabled
            && let Some(pos) = pointer
        {
            self.gesture = Some(if gizmo_rect.contains(pos) {
                Gesture::Gizmo
            } else if scene.pick(document.mesh(), pos).is_some() {
                Gesture::Sculpt
            } else {
                Gesture::Orbit
            });
            self.gizmo_dragged = false;
        }
        if primary
            && matches!(self.gesture, Some(Gesture::Orbit | Gesture::Gizmo))
            && options.enabled
        {
            let delta = ui.input(|i| i.pointer.delta());
            // The press frame's pointer movement is not part of the drag.
            if !ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                camera.orbit(delta);
                self.gizmo_dragged |= delta.length_sq() > 0.;
            }
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            if self.gesture == Some(Gesture::Gizmo)
                && !self.gizmo_dragged
                && let Some(pos) = ui.input(|i| i.pointer.hover_pos())
                && let Some(axis) = gizmo::axes(camera, gizmo_rect.center())
                    .into_iter()
                    .rev()
                    .find(|axis| axis.position.distance(pos) < 14.)
            {
                camera.yaw = axis.yaw;
                camera.pitch = axis.pitch;
            }
            self.gesture = None;
        }
        let drawing = primary
            && self.gesture == Some(Gesture::Sculpt)
            && !modifiers.ctrl
            && !modifiers.command
            && options.enabled;
        if drawing && !self.stroke {
            document.begin_stroke();
            self.stroke = true;
            self.last = None;
        }
        if !drawing {
            document.end_stroke();
            self.stroke = false;
            self.last = None;
        }
        if let Some(pos) = pointer.filter(|p| {
            !gizmo_rect.contains(*p)
                && self.gesture != Some(Gesture::Orbit)
                && self.gesture != Some(Gesture::Gizmo)
        }) {
            let b = document.brushes()[options.selected].clone();
            let radius = b.radius
                * if b.pressure_radius {
                    options.pressure.powf(b.pressure_curve).max(0.05)
                } else {
                    1.
                };
            painter.circle_stroke(
                pos,
                radius,
                egui::Stroke::new(
                    1.5,
                    if b.subtract ^ modifiers.alt {
                        Color32::from_rgb(255, 140, 100)
                    } else {
                        Color32::from_rgb(125, 210, 245)
                    },
                ),
            );
            if drawing {
                let previous = self.last.unwrap_or(pos);
                let distance = pos.distance(previous);
                let spacing = (radius * b.spacing).max(1.);
                if self.last.is_none() || distance >= spacing || b.kind == Kind::Grab {
                    let steps = if b.kind == Kind::Grab {
                        1
                    } else {
                        (distance / spacing).ceil().clamp(1., 128.) as usize
                    };
                    for step in 1..=steps {
                        let point = previous + (pos - previous) * (step as f32 / steps as f32);
                        let hit = scene.pick(document.mesh(), point);
                        if let Some((p, n)) = hit {
                            let mut brush = b.clone();
                            if modifiers.shift {
                                brush.kind = Kind::Smooth;
                            }
                            let d = (right * ((pos.x - previous.x) / scale)
                                + up * ((previous.y - pos.y) / scale))
                                * (1. / steps as f32);
                            document.apply_dab(
                                &brush,
                                Dab {
                                    center: p,
                                    normal: n,
                                    right,
                                    up,
                                    radius: radius / scale,
                                    pressure: options.pressure,
                                    delta: d,
                                    invert: modifiers.alt,
                                },
                            );
                        }
                    }
                    self.last = Some(pos);
                }
            }
        }
        if options.show_guides {
            guides::paint(
                &scene,
                &painter,
                document.mesh(),
                document.symmetry(),
                options.guide_opacity,
            );
        }
        draw_gizmo(camera, &painter, gizmo_rect, pointer);
        painter.text(
            rect.left_top() + Vec2::splat(16.),
            egui::Align2::LEFT_TOP,
            format!(
                "{} vertices • {} faces",
                document.mesh().vertices.len(),
                document.mesh().triangles.len()
            ),
            egui::FontId::monospace(12.),
            Color32::GRAY,
        );
    }
}

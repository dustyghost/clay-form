//! Converts viewport gestures into camera movement and document sculpt dabs.
mod controls;
mod gizmo;
mod guides;
mod primitive;
mod scene;
mod transform;
use crate::{
    camera::Camera,
    document::Document,
    preferences::Hold,
    sculpt::{Dab, Kind, Vec3},
};
use eframe::egui::{self, Color32, Pos2, Sense, Vec2};
use gizmo::draw_gizmo;
use scene::Scene;
pub(crate) use transform::{TransformMode, TransformOperation};
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Gesture {
    Sculpt,
    Orbit,
    Gizmo,
    Transform,
    Primitive,
}
struct SnakeStroke {
    center: Vec3,
    normal: Vec3,
    mobility: f32,
}
#[derive(Default)]
pub(crate) struct Viewport {
    pub(crate) gesture: Option<Gesture>,
    pub(crate) completed_stroke: bool,
    gizmo_dragged: bool,
    blocked_until_release: bool,
    stroke: bool,
    last: Option<Pos2>,
    snake: Option<SnakeStroke>,
    transform: transform::TransformControls,
    primitive: primitive::PrimitiveControls,
}
#[derive(Clone, Copy)]
pub(crate) struct ViewportOptions {
    pub transform_mode: TransformMode,
    pub transform_operation: TransformOperation,
    pub respect_mask: bool,
    pub selected: usize,
    pub pressure: f32,
    pub wire: bool,
    pub wire_mode: WireMode,
    pub wire_triangles: bool,
    pub show_guides: bool,
    pub guide_opacity: f32,
    pub enabled: bool,
    pub smooth: Hold,
    pub reverse: Hold,
    pub zoom_hold: Hold,
}
impl Viewport {
    pub(crate) fn cancel(&mut self, document: &mut Document) {
        *self = Self::default();
        document.end_stroke();
    }
    pub(crate) fn finish_transform_drag(&mut self, document: &mut Document) {
        let line = self.transform.line;
        let dragging = self.gesture.is_some();
        self.cancel(document);
        self.transform.line = line;
        if dragging {
            self.wait_for_release();
        }
    }
    pub(crate) fn wait_for_release(&mut self) {
        self.blocked_until_release = true;
    }
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        document: &mut Document,
        camera: &mut Camera,
        options: ViewportOptions,
    ) {
        self.completed_stroke = false;
        document.begin_sculpt_frame();
        if !options.enabled && ui.input(|i| i.pointer.primary_down()) {
            self.blocked_until_release = true;
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            self.blocked_until_release = false;
        }
        let options = ViewportOptions {
            enabled: options.enabled && !self.blocked_until_release,
            ..options
        };
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;
        let gizmo_rect = egui::Rect::from_min_size(
            Pos2::new(rect.right() - 146., rect.top() + 12.),
            Vec2::splat(134.),
        );
        let scene = Scene::paint(
            &painter,
            rect,
            document,
            camera,
            options.wire,
            options.wire_mode,
            options.wire_triangles,
        );
        if options.show_guides && document.active_visible() {
            guides::paint(
                &scene,
                &painter,
                document.mesh(),
                document.symmetry(),
                options.guide_opacity,
            );
        }
        let (right, up, _) = camera.basis();
        let scale = scene.scale;
        let pointer = response
            .hover_pos()
            .or_else(|| response.interact_pointer_pos());
        let modifiers = ui.input(|i| i.modifiers);
        let zoom_held = options.zoom_hold.active(modifiers);
        if options.enabled && response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            camera.zoom_by(scroll);
        }
        if options.enabled && response.dragged_by(egui::PointerButton::Secondary) {
            let d = ui.input(|i| i.pointer.delta());
            if zoom_held {
                camera.zoom_drag(d);
            } else {
                camera.orbit(d);
            }
        }
        if options.enabled && response.dragged_by(egui::PointerButton::Middle) {
            camera.pan += ui.input(|i| i.pointer.delta());
        }
        let primary =
            response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        if primary
            && self.gesture.is_none()
            && options.enabled
            && !zoom_held
            && options.transform_mode == TransformMode::Sculpt
            && let Some(point) = pointer.filter(|p| !gizmo_rect.contains(*p))
            && let Some((owner, _, _)) = scene.pick_model(point)
            && owner != document.active_model_index()
        {
            let _ = document.select_submodel(owner);
            self.wait_for_release();
            return;
        }
        if primary
            && self.gesture.is_none()
            && options.enabled
            && let Some(pos) = pointer
        {
            self.gesture = Some(if gizmo_rect.contains(pos) {
                Gesture::Gizmo
            } else if zoom_held {
                Gesture::Orbit
            } else if options.transform_mode != TransformMode::Sculpt
                && self.transform.can_drag(
                    document,
                    &scene,
                    options.transform_mode,
                    options.transform_operation,
                    pos,
                )
            {
                Gesture::Transform
            } else if options.transform_mode == TransformMode::Sculpt
                && self.primitive.can_drag(document, &scene, pos)
            {
                Gesture::Primitive
            } else if options.transform_mode == TransformMode::Sculpt
                && document.primitive().is_none()
                && scene.pick(pos).is_some()
            {
                Gesture::Sculpt
            } else {
                Gesture::Orbit
            });
            self.gizmo_dragged = zoom_held;
        }
        if primary
            && matches!(self.gesture, Some(Gesture::Orbit | Gesture::Gizmo))
            && options.enabled
        {
            let delta = ui.input(|i| i.pointer.delta());
            // The press frame's pointer movement is not part of the drag.
            if !ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                if zoom_held {
                    camera.zoom_drag(delta);
                } else {
                    camera.orbit(delta);
                }
                self.gizmo_dragged |= delta.length_sq() > 0. || zoom_held;
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
        if options.transform_mode != TransformMode::Sculpt {
            let dragging = primary && self.gesture == Some(Gesture::Transform) && options.enabled;
            self.transform.update(
                document,
                &scene,
                options.transform_mode,
                options.transform_operation,
                pointer,
                dragging,
                options.respect_mask,
            );
            if !dragging {
                document.end_stroke();
            }
            self.transform.paint(
                document,
                &scene,
                &painter,
                options.transform_mode,
                options.transform_operation,
            );
            draw_gizmo(camera, &painter, gizmo_rect, pointer);
            return;
        }
        if document.primitive().is_some() {
            let dragging = primary && self.gesture == Some(Gesture::Primitive) && options.enabled;
            self.primitive.update(document, &scene, pointer, dragging);
            self.primitive.paint(document, &scene, &painter, pointer);
            draw_gizmo(camera, &painter, gizmo_rect, pointer);
            painter.text(
                rect.left_bottom() + Vec2::new(16., -16.),
                egui::Align2::LEFT_BOTTOM,
                "Primitive · teal: shape · violet diamonds: spans (drag left / right) · convert to sculpt",
                egui::FontId::proportional(13.),
                controls::NEUTRAL,
            );
            return;
        }
        let drawing = primary && self.gesture == Some(Gesture::Sculpt) && options.enabled;
        if drawing && !self.stroke {
            document.begin_stroke();
            self.stroke = true;
            self.last = None;
            self.snake = None;
        }
        if !drawing {
            self.completed_stroke = document.end_stroke() && self.stroke;
            self.stroke = false;
            self.last = None;
            self.snake = None;
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
                    1.5_f32,
                    if b.subtract ^ options.reverse.active(modifiers) {
                        Color32::from_rgb(255, 140, 100)
                    } else {
                        Color32::from_rgb(125, 210, 245)
                    },
                ),
            );
            if drawing {
                let previous = self.last.unwrap_or(pos);
                let distance = pos.distance(previous);
                let spacing = (radius
                    * if b.kind == Kind::Snake {
                        // Drag displacement must remain small compared with the
                        // support, even when a preset requests widely spaced dabs.
                        b.spacing.min(0.1)
                    } else {
                        b.spacing
                    })
                .max(1.);
                if self.last.is_none()
                    || distance >= spacing
                    || matches!(b.kind, Kind::Grab | Kind::Snake)
                {
                    let steps = if b.kind == Kind::Grab {
                        1
                    } else {
                        (distance / spacing).ceil().clamp(1., 128.) as usize
                    };
                    for step in 1..=steps {
                        let point = previous + (pos - previous) * (step as f32 / steps as f32);
                        let mut brush = b.clone();
                        if options.smooth.active(modifiers) {
                            brush.kind = Kind::Smooth;
                        }
                        let hit = if brush.kind == Kind::Snake {
                            if self.snake.is_none()
                                && let Some((center, normal)) = scene.pick(point)
                            {
                                let seed = document
                                    .mesh()
                                    .vertices
                                    .iter()
                                    .enumerate()
                                    .filter(|(i, _)| document.mesh().mask[*i] < 0.999)
                                    .min_by(|(_, a), (_, b)| {
                                        (**a - center).len().total_cmp(&(**b - center).len())
                                    });
                                if let Some((index, &tip)) =
                                    seed.filter(|(_, v)| (**v - center).len() < radius / scale)
                                {
                                    self.snake = Some(SnakeStroke {
                                        center: tip,
                                        normal,
                                        mobility: 1. - document.mesh().mask[index],
                                    });
                                }
                            }
                            self.snake
                                .as_ref()
                                .map(|snake| (snake.center, snake.normal))
                        } else {
                            scene.pick(point)
                        };
                        if let Some((p, n)) = hit {
                            let d = (right * ((pos.x - previous.x) / scale)
                                + up * ((previous.y - pos.y) / scale))
                                * (1. / steps as f32);
                            let dab = Dab {
                                center: p,
                                normal: n,
                                right,
                                up,
                                radius: radius / scale,
                                pressure: options.pressure,
                                delta: d,
                                invert: options.reverse.active(modifiers),
                            };
                            let deformed = document.apply_dab(&brush, dab);
                            if brush.kind == Kind::Snake
                                && deformed
                                && let Some(snake) = &mut self.snake
                            {
                                // Keep a continuous handle; snapping to a globally nearest
                                // vertex jumps between the shaft and tip as topology changes.
                                snake.center =
                                    snake.center + brush.snake_motion(dab) * snake.mobility;
                            }
                        }
                    }
                    self.last = Some(pos);
                }
            }
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

#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum WireMode {
    #[default]
    Visible,
    Front,
    Transparent,
}

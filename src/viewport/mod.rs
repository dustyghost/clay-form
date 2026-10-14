//! Converts viewport gestures into camera movement and document sculpt dabs.
mod controls;
mod gizmo;
mod gpu;
mod guides;
mod primitive;
mod scene;
mod slice;
mod stabilizer;
mod stroke;
mod transform;
mod visibility;
use crate::{
    camera::Camera,
    document::Document,
    preferences::Hold,
    sculpt::{Dab, Kind, StrokeMode, Vec3},
};
use eframe::egui::{self, Color32, Pos2, Sense, Vec2};
use gizmo::draw_gizmo;
pub(crate) use visibility::SelectionTool;
pub(crate) const AXIS_COLORS: [Color32; 3] = controls::AXES;
use scene::Scene;
pub(crate) use transform::{TransformMode, TransformOperation};
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Gesture {
    Sculpt,
    Orbit,
    Gizmo,
    Transform,
    Primitive,
    Slice,
    Select,
}
struct SnakeStroke {
    center: Vec3,
    normal: Vec3,
    mobility: f32,
}
#[derive(Default)]
pub(crate) struct Viewport {
    renderer: Option<gpu::Renderer>,
    pub(crate) gesture: Option<Gesture>,
    pub(crate) completed_stroke: bool,
    gizmo_dragged: bool,
    gizmo_start: Option<Pos2>,
    gizmo_snap: Option<(f32, f32)>,
    blocked_until_release: bool,
    stroke: bool,
    pending_stroke: Option<stroke::PendingStroke>,
    stabilizer: Option<stabilizer::Stabilizer>,
    last: Option<Pos2>,
    snake: Option<SnakeStroke>,
    transform: transform::TransformControls,
    primitive: primitive::PrimitiveControls,
    slice: Option<slice::SliceDrag>,
    selection: Option<visibility::SelectionDrag>,
    pub(crate) visibility_status: Option<String>,
    slice_path: Option<slice::SlicePathDraft>,
    pub(crate) completed_slice: Option<crate::mesh_job::Operation>,
}
#[derive(Clone, Copy)]
pub(crate) struct ViewportOptions {
    pub slice: Option<crate::sculpt::SliceOptions>,
    pub select: Option<SelectionTool>,
    pub slice_path_mode: bool,
    pub transform_mode: TransformMode,
    pub transform_operation: TransformOperation,
    pub respect_mask: bool,
    pub selected: usize,
    pub pressure: f32,
    pub wire: bool,
    pub wire_mode: WireMode,
    pub wire_triangles: bool,
    pub show_polygroups: bool,
    pub backface_opacity: f32,
    pub show_guides: bool,
    pub guide_opacity: f32,
    pub enabled: bool,
    pub smooth: Hold,
    pub reverse: Hold,
    pub zoom_hold: Hold,
    pub pan_hold: Hold,
}
impl Viewport {
    pub(crate) fn initialize_renderer(&mut self, gl: &eframe::glow::Context) -> Result<(), String> {
        self.renderer = Some(gpu::Renderer::new(gl)?);
        Ok(())
    }
    pub(crate) fn destroy_renderer(&self, gl: &eframe::glow::Context) {
        if let Some(renderer) = &self.renderer {
            renderer.destroy(gl);
        }
    }
    pub(crate) fn cancel(&mut self, document: &mut Document) {
        let renderer = self.renderer.take();
        *self = Self {
            renderer,
            ..Default::default()
        };
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
        self.completed_slice = None;
        document.begin_sculpt_frame();
        if !options.enabled && ui.input(|i| i.pointer.primary_down()) {
            self.blocked_until_release = true;
        }
        let blocked_this_frame = self.blocked_until_release;
        if !ui.input(|i| i.pointer.primary_down()) {
            self.blocked_until_release = false;
        }
        let options = ViewportOptions {
            enabled: options.enabled && !blocked_this_frame,
            ..options
        };
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        let gizmo_rect = egui::Rect::from_min_size(
            Pos2::new(rect.right() - 146., rect.top() + 12.),
            Vec2::splat(134.),
        );
        if let Some(renderer) = &self.renderer {
            renderer.clear_preview();
        }
        let modifiers = ui.input(|i| i.modifiers);
        let previewing = options.enabled
            && ui.input(|i| i.pointer.primary_down() && !i.key_pressed(egui::Key::Escape));
        if previewing && let Some(pending) = self.pending_stroke.as_mut() {
            if let Some(end) = response.interact_pointer_pos() {
                pending.end = end;
            }
            pending.brush.kind = if options.smooth.active(modifiers) {
                Kind::Smooth
            } else {
                pending.source_kind
            };
            pending.pressure = options.pressure;
            pending.invert = options.reverse.active(modifiers);
        }
        let scene = Scene::paint_with_stroke(
            &painter,
            rect,
            document,
            camera,
            options.wire,
            options.wire_mode,
            options.wire_triangles,
            self.renderer.as_mut(),
            self.pending_stroke.as_ref().filter(|_| previewing),
        );
        if let Some(renderer) = &self.renderer {
            renderer.set_backfaces(options.backface_opacity);
            renderer.set_polygroups(options.show_polygroups);
        }
        if options.show_guides && document.active_visible() {
            if let Some(renderer) = &self.renderer {
                renderer.set_guides(document.symmetry(), options.guide_opacity);
            } else {
                guides::paint(
                    &scene,
                    &painter,
                    document.mesh(),
                    document.symmetry(),
                    options.guide_opacity,
                );
            }
        }
        if self.update_visibility(
            ui,
            &response,
            &painter,
            &scene,
            document,
            gizmo_rect,
            options.enabled
                && options.slice.is_none()
                && options.transform_mode == TransformMode::Sculpt,
            options.select,
        ) {
            draw_gizmo(camera, &painter, gizmo_rect, response.hover_pos());
            return;
        }
        let (right, up, _) = camera.basis();
        let scale = scene.scale;
        let pointer = response
            .hover_pos()
            .or_else(|| response.interact_pointer_pos());
        if options.enabled
            && options.select.is_some()
            && response.hovered()
            && pointer.is_some_and(|pos| !gizmo_rect.contains(pos))
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if options.enabled
            && options.select.is_none()
            && response.hovered()
            && let Some(pos) = pointer
            && !gizmo_rect.contains(pos)
            && scene.pick_model(pos).is_some()
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
            let target = ui
                .ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("model_target_cursor"),
                ))
                .with_clip_rect(rect);
            // A dark outline keeps the small red target readable on light groups.
            for (color, width) in [
                (Color32::from_black_alpha(210), 3.),
                (Color32::from_rgb(235, 65, 65), 1.3),
            ] {
                let stroke = egui::Stroke::new(width, color);
                target.circle_stroke(pos, 5., stroke);
                for axis in [Vec2::X, Vec2::Y] {
                    for sign in [-1., 1.] {
                        target.line_segment(
                            [pos + axis * (3. * sign), pos + axis * (8. * sign)],
                            stroke,
                        );
                    }
                }
            }
            target.circle_filled(pos, 1.2, Color32::from_rgb(235, 65, 65));
        }
        let zoom_held = options.zoom_hold.active(modifiers);
        let pan_held = options.pan_hold.active(modifiers);
        if options.enabled
            && self.pending_stroke.is_none()
            && self.slice.is_none()
            && self.slice_path.is_none()
            && response.hovered()
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            camera.zoom_by(scroll);
        }
        if options.enabled
            && self.pending_stroke.is_none()
            && self.slice.is_none()
            && self.slice_path.is_none()
            && response.dragged_by(egui::PointerButton::Secondary)
        {
            let d = ui.input(|i| i.pointer.delta());
            if zoom_held {
                camera.zoom_drag(d);
            } else if pan_held {
                camera.pan += d;
            } else {
                camera.orbit(d);
            }
        }
        if options.enabled
            && self.pending_stroke.is_none()
            && self.slice.is_none()
            && self.slice_path.is_none()
            && response.dragged_by(egui::PointerButton::Middle)
        {
            camera.pan += ui.input(|i| i.pointer.delta());
        }
        let primary =
            response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        if (primary || response.clicked_by(egui::PointerButton::Primary))
            && self.gesture.is_none()
            && options.enabled
            && !zoom_held
            && options.transform_mode == TransformMode::Sculpt
            && options.slice.is_none()
            && options.select.is_none()
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
            && (options.select.is_none() || gizmo_rect.contains(pos))
        {
            self.gesture = Some(if gizmo_rect.contains(pos) {
                Gesture::Gizmo
            } else if zoom_held || (pan_held && scene.pick_model(pos).is_none()) {
                Gesture::Orbit
            } else if options.slice.is_some() && document.primitive().is_none() {
                Gesture::Slice
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
            self.gizmo_dragged = zoom_held || pan_held;
            if self.gesture == Some(Gesture::Gizmo) {
                self.gizmo_start = Some(pos);
                self.gizmo_snap = gizmo::snap_target(camera, gizmo_rect.center(), pos);
            }
        }
        if primary
            && matches!(self.gesture, Some(Gesture::Orbit | Gesture::Gizmo))
            && options.enabled
        {
            let mut delta = ui.input(|i| i.pointer.delta());
            // The press frame's pointer movement is not part of the drag.
            if !ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                if self.gesture == Some(Gesture::Gizmo) && !self.gizmo_dragged {
                    let travel = self
                        .gizmo_start
                        .zip(pointer)
                        .map_or(Vec2::ZERO, |(start, pos)| pos - start);
                    if zoom_held || pan_held || travel.length() > gizmo::DRAG_THRESHOLD {
                        self.gizmo_dragged = true;
                        // Apply accumulated movement once the tap becomes a deliberate drag.
                        if !zoom_held && !pan_held {
                            delta = travel;
                        }
                    } else {
                        delta = Vec2::ZERO;
                    }
                }
                if zoom_held {
                    camera.zoom_drag(delta);
                } else if pan_held {
                    camera.pan += delta;
                } else {
                    camera.orbit(delta);
                }
                self.gizmo_dragged |= delta.length_sq() > 0. || zoom_held || pan_held;
            }
        }
        let previous_gesture = self.gesture;
        if !ui.input(|i| i.pointer.primary_down()) {
            // A fast tap can contain both press and release in the same frame.
            if self.gesture.is_none()
                && options.enabled
                && !zoom_held
                && !pan_held
                && response.clicked_by(egui::PointerButton::Primary)
                && let Some(start) = ui.input(|i| {
                    i.events.iter().rev().find_map(|event| match event {
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            ..
                        } => Some(*pos),
                        _ => None,
                    })
                })
                && gizmo_rect.contains(start)
            {
                self.gesture = Some(Gesture::Gizmo);
                self.gizmo_start = Some(start);
                self.gizmo_snap = gizmo::snap_target(camera, gizmo_rect.center(), start);
                self.gizmo_dragged = false;
            }
            if self.gesture == Some(Gesture::Gizmo)
                && options.enabled
                && !self.gizmo_dragged
                && self
                    .gizmo_start
                    .zip(ui.input(|i| i.pointer.interact_pos()))
                    .is_some_and(|(start, end)| start.distance(end) <= gizmo::DRAG_THRESHOLD)
                && let Some((yaw, pitch)) = self.gizmo_snap
            {
                camera.yaw = yaw;
                camera.pitch = pitch;
            }
            self.gesture = None;
            self.gizmo_start = None;
            self.gizmo_snap = None;
        }
        if let Some(slice_options) = options.slice {
            self.update_slice(
                ui,
                &scene,
                &painter,
                document,
                pointer,
                options.enabled,
                slice_options,
                options.slice_path_mode,
                response.clicked_by(egui::PointerButton::Primary),
            );
            draw_gizmo(camera, &painter, gizmo_rect, pointer);
            return;
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
                "Primitive · teal: shape · axis-colored diamonds: spans (drag left / right) · convert to sculpt",
                egui::FontId::proportional(13.),
                controls::NEUTRAL,
            );
            return;
        }
        let drawing = primary && self.gesture == Some(Gesture::Sculpt) && options.enabled;
        let patterned_click = options.enabled
            && options.select.is_none()
            && !zoom_held
            && !matches!(previous_gesture, Some(Gesture::Orbit | Gesture::Gizmo))
            && response.clicked_by(egui::PointerButton::Primary)
            && document.brushes()[options.selected].effective_stroke_mode() != StrokeMode::Freehand
            && pointer.is_some_and(|pos| !gizmo_rect.contains(pos) && scene.pick(pos).is_some());
        if (drawing || patterned_click) && !self.stroke {
            let mut brush = document.brushes()[options.selected].clone();
            if brush.effective_stroke_mode() == StrokeMode::Freehand {
                document.begin_stroke();
                self.stabilizer = pointer
                    .filter(|_| brush.stabilize && brush.supports_stabilization())
                    .map(|start| stabilizer::Stabilizer::new(start, brush.stabilizer_distance));
            } else if let Some(start) = pointer
                && let Some(anchor) = scene.pick(start)
            {
                let source_kind = brush.kind;
                if options.smooth.active(modifiers) {
                    brush.kind = Kind::Smooth;
                }
                self.pending_stroke = Some(stroke::PendingStroke {
                    start,
                    end: start,
                    brush,
                    source_kind,
                    pressure: options.pressure,
                    invert: options.reverse.active(modifiers),
                    anchor,
                });
            }
            self.stroke = true;
            self.last = None;
            self.snake = None;
        }
        if self.pending_stroke.is_some() {
            if !options.enabled || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.cancel(document);
                self.wait_for_release();
                draw_gizmo(camera, &painter, gizmo_rect, pointer);
                return;
            }
            if drawing {
                let pending = self.pending_stroke.as_mut().unwrap();
                if let Some(end) = pointer {
                    pending.end = end;
                }
                pending.brush.kind = if options.smooth.active(modifiers) {
                    Kind::Smooth
                } else {
                    pending.source_kind
                };
                pending.pressure = options.pressure;
                pending.invert = options.reverse.active(modifiers);
                pending.paint(&painter, &scene, self.renderer.as_ref());
                draw_gizmo(camera, &painter, gizmo_rect, pointer);
                return;
            }
            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                let mut pending = self.pending_stroke.take().unwrap();
                if let Some(end) = ui.input(|i| i.pointer.interact_pos()) {
                    pending.end = end;
                }
                pending.commit(document, &scene);
            } else {
                self.pending_stroke = None;
            }
        }
        if !drawing {
            self.completed_stroke = document.end_stroke() && self.stroke;
            self.stroke = false;
            self.last = None;
            self.snake = None;
            // Never pull the trailing brush to the pointer on release.
            self.stabilizer = None;
        }
        if let Some(pos) = pointer.filter(|p| {
            options.select.is_none()
                && !gizmo_rect.contains(*p)
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
            let color = if b.subtract ^ options.reverse.active(modifiers) {
                Color32::from_rgb(255, 140, 100)
            } else {
                Color32::from_rgb(125, 210, 245)
            };
            let pos = if let Some(stabilizer) = self.stabilizer.as_mut() {
                let trailing = stabilizer.follow(pos);
                stabilizer.paint(&painter, pos, color);
                trailing
            } else {
                pos
            };
            let surface = if b.surface_cursor {
                if let Some(renderer) = &self.renderer {
                    renderer.set_brush(&scene, pos, radius, &b, color);
                    scene.brush_cursor_with_footprint(&painter, pos, radius, &b, color, false)
                } else {
                    scene.brush_cursor(&painter, pos, radius, &b, color)
                }
            } else {
                false
            };
            if !surface {
                for fraction in [1., b.inner_radius] {
                    if fraction > 0. {
                        painter.circle_stroke(
                            pos,
                            radius * fraction,
                            egui::Stroke::new(1.5, color),
                        );
                    }
                }
            }
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
                "{} vertices • {} faces • {} hidden",
                document.mesh().vertices.len(),
                document.mesh().triangles.len(),
                document.mesh().hidden_face_count()
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

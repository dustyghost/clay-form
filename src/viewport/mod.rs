//! Converts viewport gestures into camera movement and document sculpt dabs.
mod controls;
mod edges;
mod modeling;
pub(crate) use modeling::{
    ComponentMode, ModelAxis, ModelConstruction, ModelInteraction, ModelTool, ModelingOptions,
    SelectionChange, SelectionSpan,
};
mod gizmo;
mod gpu;
mod projection;
#[cfg(all(test, target_os = "linux"))]
pub(crate) use gpu::ui_capture::UiCapture;
mod guides;
mod navigation;
mod primitive;
mod references;
mod scene;
mod slice;
mod stabilizer;
mod stencil;
mod stroke;
mod transform;
mod visibility;
use crate::{
    camera::Camera,
    document::Document,
    preferences::Hold,
    sculpt::{Brush, Dab, Kind, StrokeMode, Vec3},
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
    Edges,
    Modeling,
}
struct SnakeStroke {
    center: Vec3,
    normal: Vec3,
    mobility: f32,
}
#[derive(Default)]
pub(crate) struct Viewport {
    pub(crate) attach_stencil: bool,
    stencil_overlay: stencil::Overlay,
    pub(crate) projection_preview: Option<std::sync::Arc<crate::sculpt::Mesh>>,
    pub(crate) projection_mode: bool,
    pub(crate) modeling: modeling::ModelingState,
    edge_selection: Vec<[usize; 2]>,
    edge_revision: u64,
    references: references::ReferenceOverlay,
    renderer: Option<gpu::Renderer>,
    pub(crate) gesture: Option<Gesture>,
    pub(crate) completed_stroke: bool,
    gizmo_dragged: bool,
    gizmo_start: Option<Pos2>,
    gizmo_snap: Option<(f32, f32)>,
    blocked_until_release: bool,
    stroke: bool,
    stroke_brush: Option<String>,
    smooth_removed: Option<(usize, usize)>,
    pending_stroke: Option<stroke::PendingStroke>,
    stabilizer: Option<stabilizer::Stabilizer>,
    last: Option<Pos2>,
    snake: Option<SnakeStroke>,
    transform: transform::TransformControls,
    primitive: primitive::PrimitiveControls,
    slice: Option<slice::SliceDrag>,
    selection: Option<visibility::SelectionDrag>,
    pub(crate) status: Option<String>,
    slice_path: Option<slice::SlicePathDraft>,
    pub(crate) completed_slice: Option<crate::mesh_job::Operation>,
}
#[derive(Clone, Copy)]
pub(crate) struct ViewportOptions {
    pub edge_select: bool,
    pub modeling: Option<ModelingOptions>,
    pub arrange_references: bool,
    pub references_visible: bool,
    pub slice: Option<crate::sculpt::SliceOptions>,
    pub select: Option<SelectionTool>,
    pub slice_path_mode: bool,
    pub transform_mode: TransformMode,
    pub transform_operation: TransformOperation,
    pub respect_mask: bool,
    pub selected: Option<usize>,
    pub pressure: f32,
    pub wire: bool,
    pub wire_mode: WireMode,
    pub wire_triangles: bool,
    pub show_polygroups: bool,
    pub backface_opacity: f32,
    pub show_guides: bool,
    pub guide_opacity: f32,
    pub enabled: bool,
    pub reverse: Hold,
    pub zoom_hold: Hold,
    pub pan_hold: Hold,
}
impl Viewport {
    fn record_cleanup(&mut self, document: &Document, before: (usize, usize)) {
        if let Some((vertices, faces)) = self.smooth_removed.as_mut() {
            *vertices += before.0.saturating_sub(document.mesh().vertices.len());
            *faces += before.1.saturating_sub(document.mesh().triangles.len());
        }
    }
    pub(crate) fn reference_selection(&mut self) -> Option<usize> {
        self.references.selected.take()
    }
    pub(crate) fn cancel_reference_drag(&mut self) {
        if self.references.dragging() {
            self.wait_for_release();
        }
        self.references.cancel_drag();
    }
    pub(crate) fn initialize_renderer(
        &mut self,
        state: &eframe::egui_wgpu::RenderState,
    ) -> Result<(), String> {
        self.renderer = Some(gpu::Renderer::new(state)?);
        Ok(())
    }
    pub(crate) fn destroy_renderer(&self) {
        if let Some(renderer) = &self.renderer {
            renderer.destroy();
        }
    }
    pub(crate) fn cancel(&mut self, document: &mut Document) {
        let renderer = self.renderer.take();
        let edge_selection = std::mem::take(&mut self.edge_selection);
        let mut modeling = std::mem::take(&mut self.modeling);
        modeling.cancel();
        let edge_revision = self.edge_revision;
        let blocked_until_release = self.blocked_until_release
            || self.references.dragging()
            || self.stencil_overlay.controls.capturing();
        let mut references = std::mem::take(&mut self.references);
        references.cancel_drag();
        *self = Self {
            edge_selection,
            modeling,
            edge_revision,
            blocked_until_release,
            references,
            renderer,
            ..Default::default()
        };
        document.end_stroke();
    }
    pub(crate) fn finish_transform_drag(&mut self, document: &mut Document) {
        let mut transform = std::mem::take(&mut self.transform);
        transform.finish_drag();
        let dragging = self.gesture.is_some();
        self.cancel(document);
        self.transform = transform;
        if dragging {
            self.wait_for_release();
        }
    }
    pub(crate) fn reset_transform_pivot(&mut self, document: &Document, mode: TransformMode) {
        self.transform.reset_pivot(document, mode);
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
        if (!options.enabled || !ui.input(|i| i.focused)) && ui.input(|i| i.pointer.primary_down())
        {
            self.blocked_until_release = true;
        }
        let blocked_this_frame = self.blocked_until_release;
        if !ui.input(|i| i.pointer.primary_down()) {
            self.blocked_until_release = false;
        }
        let options = ViewportOptions {
            enabled: options.enabled && !blocked_this_frame && ui.input(|i| i.focused),
            ..options
        };
        if !options.enabled && self.gesture == Some(Gesture::Edges) {
            self.gesture = None;
        }
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
        let brush = options.selected.map_or_else(
            || Brush::new(Kind::Smooth),
            |i| document.brushes()[i].clone(),
        );
        let brush_id = brush.identity();
        if self.stroke_brush.as_ref().is_some_and(|id| *id != brush_id)
            && ui.input(|i| i.pointer.primary_down())
        {
            // A temporary preset starts at the pointer with its own stroke mode
            // and stabilizer. Keep already applied dabs in the same undo step.
            self.pending_stroke = None;
            self.stabilizer = None;
            self.last = None;
            self.snake = None;
            self.stroke = false;
        }
        let previewing = options.enabled
            && !self.projection_mode
            && ui.input(|i| i.pointer.primary_down() && !i.key_pressed(egui::Key::Escape));
        if previewing && let Some(pending) = self.pending_stroke.as_mut() {
            if let Some(end) = response.interact_pointer_pos() {
                pending.end = end;
            }
            pending.pressure = options.pressure;
            pending.invert = options.reverse.active(modifiers);
        }
        self.modeling.update_preview(
            document,
            ui.input(|i| i.pointer.interact_pos()),
            options.modeling,
            options.enabled && !self.projection_mode,
            ui.input(|i| i.key_pressed(egui::Key::Escape)),
        );
        let scene = Scene::paint_with_stroke(
            &painter,
            rect,
            document,
            camera,
            options.wire
                && !options
                    .modeling
                    .is_some_and(|m| m.smooth_preview && !self.modeling.dragging()),
            options.wire_mode,
            options.wire_triangles,
            self.renderer.as_mut(),
            self.pending_stroke.as_ref().filter(|_| previewing),
            self.projection_preview
                .as_deref()
                .or_else(|| self.modeling.preview()),
        );
        if let Some(renderer) = &self.renderer {
            renderer.set_backfaces(options.backface_opacity);
            renderer.set_polygroups(options.show_polygroups);
        }
        if self.projection_mode {
            if options.enabled && response.hovered() {
                camera.zoom_by(ui.input(|i| i.smooth_scroll_delta.y));
            }
            if options.enabled && response.dragged() {
                let delta = ui.input(|i| i.pointer.delta());
                if options.zoom_hold.active(modifiers) {
                    camera.zoom_drag(delta);
                } else if options.pan_hold.active(modifiers)
                    || response.dragged_by(egui::PointerButton::Middle)
                {
                    camera.pan += delta;
                } else {
                    camera.orbit(delta);
                }
            }
            draw_gizmo(camera, &painter, gizmo_rect, response.hover_pos());
            return;
        }
        if self.attach_stencil {
            self.stencil_overlay
                .paint(document.stencil(), &scene, &painter);
            if !options.enabled || ui.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused) {
                self.attach_stencil = false;
                self.wait_for_release();
            } else if response.clicked_by(egui::PointerButton::Primary)
                && let Some(pos) = response.interact_pointer_pos()
            {
                match stencil::attach(document, &scene, pos) {
                    Ok(()) => {
                        self.attach_stencil = false;
                    }
                    Err(error) => self.status = Some(error),
                }
            }
            painter.text(
                rect.center_top() + Vec2::new(0., 24.),
                egui::Align2::CENTER_TOP,
                "Click the model to anchor the stencil · Esc cancels",
                egui::FontId::proportional(14.),
                controls::NEUTRAL,
            );
            return;
        }
        let displayed_stencil = self
            .stencil_overlay
            .controls
            .preview()
            .cloned()
            .or_else(|| document.stencil().cloned());
        self.stencil_overlay
            .paint(displayed_stencil.as_ref(), &scene, &painter);
        let stencil_controls = self.stencil_overlay.controls.show(
            ui,
            document,
            &scene,
            options.enabled
                && !self.stroke
                && self.pending_stroke.is_none()
                && self.gesture.is_none(),
        );
        if stencil_controls {
            draw_gizmo(camera, &painter, gizmo_rect, response.hover_pos());
            draw_mesh_counts(document, &painter, rect);
            return;
        }
        let stencil = document
            .stencil()
            .filter(|s| s.enabled)
            .map(|s| stencil::project(s, &scene));
        self.paint_edges(document, &scene, &painter);
        self.references
            .show(ui, &response, &painter, document, options);
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
        let frame_hovered = response
            .hover_pos()
            .is_some_and(|p| navigation::contains(rect, p));
        navigation::paint(&painter, rect, frame_hovered);
        if frame_hovered {
            response.clone().on_hover_text(format!(
                "Navigation frame · drag to orbit · {:?} + drag to zoom · {:?} + drag to pan",
                options.zoom_hold, options.pan_hold,
            ));
        }
        // Reserve border presses before selection, transforms, and model picking.
        // Latch navigation even when press and release arrive in one frame.
        let frame_start = ui.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some(*pos),
                _ => None,
            })
        });
        let frame_press = options.enabled
            && self.gesture.is_none()
            && frame_start.is_some_and(|p| {
                navigation::contains(rect, p)
                    && !gizmo_rect.contains(p)
                    && ui.ctx().layer_id_at(p) == Some(response.layer_id)
            });
        if frame_press {
            self.gesture = Some(Gesture::Orbit);
        }
        if self.update_visibility(
            ui,
            &response,
            &painter,
            &scene,
            document,
            gizmo_rect,
            options.enabled
                && !options.edge_select
                && options.modeling.is_none()
                && !options.arrange_references
                && options.slice.is_none()
                && options.transform_mode == TransformMode::Sculpt,
            options.select,
        ) {
            draw_gizmo(camera, &painter, gizmo_rect, response.hover_pos());
            return;
        }
        let (right, up, _) = camera.basis();
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
            && !options.arrange_references
            && options.select.is_none()
            && response.hovered()
            && let Some(pos) = pointer
            && !gizmo_rect.contains(pos)
            && !navigation::contains(rect, pos)
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
                (Color32::from_rgb(235, 65, 65), 1.3_f32),
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
        if frame_hovered && options.enabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        let zoom_held = options.zoom_hold.active(modifiers);
        let pan_held = options.pan_hold.active(modifiers);
        if options.enabled
            && self.gesture != Some(Gesture::Transform)
            && self.pending_stroke.is_none()
            && !self.modeling.dragging()
            && self.slice.is_none()
            && self.slice_path.is_none()
            && response.hovered()
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            camera.zoom_by(scroll);
        }
        if options.enabled
            && self.gesture != Some(Gesture::Transform)
            && self.pending_stroke.is_none()
            && !self.modeling.dragging()
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
            && self.gesture != Some(Gesture::Transform)
            && self.pending_stroke.is_none()
            && !self.modeling.dragging()
            && self.slice.is_none()
            && self.slice_path.is_none()
            && response.dragged_by(egui::PointerButton::Middle)
        {
            camera.pan += ui.input(|i| i.pointer.delta());
        }
        if options.arrange_references && self.gesture != Some(Gesture::Orbit) {
            draw_gizmo(camera, &painter, gizmo_rect, response.hover_pos());
            return;
        }
        let primary =
            response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        if self.update_modeling(
            ui, &response, &painter, &scene, document, options, gizmo_rect,
        ) {
            draw_gizmo(camera, &painter, gizmo_rect, pointer);
            return;
        }
        if options.edge_select
            && options.enabled
            && (primary
                || response.clicked_by(egui::PointerButton::Primary)
                || self.gesture == Some(Gesture::Edges))
            && (self.gesture == Some(Gesture::Edges)
                || (self.gesture.is_none()
                    && pointer.is_some_and(|p| {
                        !gizmo_rect.contains(p)
                            && !navigation::contains(rect, p)
                            // Empty-space presses use the normal navigation path.
                            // Include the edge hit tolerance just outside silhouettes.
                            && (scene.pick_model(p).is_some()
                                || Self::edge_at(p, document, &scene).is_some())
                    })))
        {
            if primary {
                self.gesture = Some(Gesture::Edges);
            }
            if ui.input(|i| i.pointer.any_released()) {
                if response.clicked_by(egui::PointerButton::Primary)
                    && let Some(point) = pointer
                {
                    self.pick_edge(point, ui.input(|i| i.modifiers), document, &scene);
                }
                self.gesture = None;
            }
            return;
        }
        if (primary || response.clicked_by(egui::PointerButton::Primary))
            && self.gesture.is_none()
            && options.enabled
            && !zoom_held
            && options.transform_mode == TransformMode::Sculpt
            && options.slice.is_none()
            && options.select.is_none()
            && let Some(point) =
                pointer.filter(|p| !gizmo_rect.contains(*p) && !navigation::contains(rect, *p))
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
            } else if navigation::contains(rect, pos)
                || zoom_held
                || (pan_held && scene.pick_model(pos).is_none())
            {
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
        if previous_gesture == Some(Gesture::Orbit) {
            draw_gizmo(camera, &painter, gizmo_rect, pointer);
            return;
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
            // A complete tap may arrive in one frame. Placement and release
            // coordinates follow the same update path as ordinary drags.
            let fast_press = (options.enabled
                && response.clicked_by(egui::PointerButton::Primary)
                && previous_gesture.is_none()
                && !zoom_held
                && !pan_held)
                .then(|| {
                    ui.input(|i| {
                        i.events.iter().find_map(|event| match event {
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                ..
                            } => Some(*pos),
                            _ => None,
                        })
                    })
                })
                .flatten()
                .filter(|p| {
                    !gizmo_rect.contains(*p)
                        && self.transform.can_drag(
                            document,
                            &scene,
                            options.transform_mode,
                            options.transform_operation,
                            *p,
                        )
                });
            let result = (|| {
                if let Some(start) = fast_press {
                    self.transform.update(
                        document,
                        &scene,
                        options.transform_mode,
                        options.transform_operation,
                        Some(start),
                        true,
                        options.respect_mask,
                    )?;
                }
                let releasing = options.enabled
                    && previous_gesture == Some(Gesture::Transform)
                    && ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
                if dragging || releasing || fast_press.is_some() {
                    self.transform.update(
                        document,
                        &scene,
                        options.transform_mode,
                        options.transform_operation,
                        pointer,
                        true,
                        options.respect_mask,
                    )?;
                }
                Ok::<_, String>(())
            })();
            if !dragging {
                self.transform.finish_drag();
                document.end_stroke();
            }
            if let Err(error) = result {
                self.finish_transform_drag(document);
                self.status = Some(error);
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
                navigation::inner(rect).left_bottom() + Vec2::new(8., -8.),
                egui::Align2::LEFT_BOTTOM,
                "Primitive · teal: shape · axis-colored diamonds: spans (drag left / right) · convert to sculpt",
                egui::FontId::proportional(13.),
                controls::NEUTRAL,
            );
            return;
        }
        if stencil.is_some() && matches!(brush.kind, Kind::Grab | Kind::Snake | Kind::Detail) {
            painter.text(
                rect.center_bottom() - Vec2::new(0., 24.),
                egui::Align2::CENTER_BOTTOM,
                "Disable the stencil to use Grab, Snake or Detail",
                egui::FontId::proportional(14.),
                controls::NEUTRAL,
            );
            document.end_stroke();
            return;
        }
        let drawing = primary && self.gesture == Some(Gesture::Sculpt) && options.enabled;
        let patterned_click = options.enabled
            && options.select.is_none()
            && !zoom_held
            && !matches!(previous_gesture, Some(Gesture::Orbit | Gesture::Gizmo))
            && response.clicked_by(egui::PointerButton::Primary)
            && brush.effective_stroke_mode() != StrokeMode::Freehand
            && pointer.is_some_and(|pos| {
                !gizmo_rect.contains(pos)
                    && !navigation::contains(rect, pos)
                    && scene.pick(pos).is_some()
            });
        if (drawing || patterned_click) && !self.stroke {
            if brush.kind == Kind::Smooth
                && document.mesh_settings().adaptive.enabled
                && document.mesh_settings().adaptive.simplify
            {
                self.smooth_removed.get_or_insert((0, 0));
            }
            let brush = brush.clone();
            if brush.effective_stroke_mode() == StrokeMode::Freehand {
                document.begin_stroke();
                self.stabilizer = pointer
                    .filter(|_| brush.stabilize && brush.supports_stabilization())
                    .map(|start| stabilizer::Stabilizer::new(start, brush.stabilizer_distance));
            } else if let Some(start) = pointer
                && let Some(anchor) = scene.pick(start)
            {
                self.pending_stroke = Some(stroke::PendingStroke {
                    start,
                    end: start,
                    brush,
                    stencil: stencil.clone(),
                    pressure: options.pressure,
                    invert: options.reverse.active(modifiers),
                    anchor,
                });
            }
            self.stroke = true;
            self.stroke_brush = Some(brush_id);
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
                let before = (
                    document.mesh().vertices.len(),
                    document.mesh().triangles.len(),
                );
                let smoothing = pending.brush.kind == Kind::Smooth;
                pending.commit(document, &scene);
                if smoothing {
                    self.record_cleanup(document, before);
                }
            } else {
                self.pending_stroke = None;
            }
        }
        if !drawing {
            if let Some((vertices, faces)) = self.smooth_removed.take() {
                self.status = Some(if vertices > 0 {
                    format!("Smooth cleanup: removed {vertices} vertices and {faces} faces")
                } else {
                    "Smooth cleanup: no removable edges at this size. Increase Edge size for more reduction; protected features stay intact.".into()
                });
            }
            self.completed_stroke = document.end_stroke() && self.stroke;
            self.stroke = false;
            self.stroke_brush = None;
            self.last = None;
            self.snake = None;
            // Never pull the trailing brush to the pointer on release.
            self.stabilizer = None;
        }
        if pointer.is_some_and(|p| navigation::contains(rect, p)) {
            self.last = None;
            self.stabilizer = None;
        }
        if let Some(pos) = pointer.filter(|p| {
            options.select.is_none()
                && (!navigation::contains(rect, *p) || self.pending_stroke.is_some())
                && !gizmo_rect.contains(*p)
                && self.gesture != Some(Gesture::Orbit)
                && self.gesture != Some(Gesture::Gizmo)
        }) {
            let b = brush;
            if drawing
                && self.last.is_none()
                && self.stabilizer.is_none()
                && b.stabilize
                && b.supports_stabilization()
            {
                self.stabilizer = Some(stabilizer::Stabilizer::new(pos, b.stabilizer_distance));
            }
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
                            egui::Stroke::new(1.5_f32, color),
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
                        let brush = b.clone();
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
                                if let Some((index, &tip)) = seed.filter(|(_, v)| {
                                    (**v - center).len() < radius / scene.scale_at(center)
                                }) {
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
                            let scale = scene.scale_at(p);
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
                            let before = (
                                document.mesh().vertices.len(),
                                document.mesh().triangles.len(),
                            );
                            let deformed =
                                document.apply_dab_stenciled(&brush, dab, stencil.as_ref());
                            if brush.kind == Kind::Smooth {
                                self.record_cleanup(document, before);
                            }
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
        draw_mesh_counts(document, &painter, rect);
    }
}

fn draw_mesh_counts(document: &Document, painter: &egui::Painter, rect: egui::Rect) {
    painter.text(
        navigation::inner(rect).left_top() + Vec2::splat(8.),
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

#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum WireMode {
    #[default]
    Visible,
    Front,
    Transparent,
}

//! Surface placement uses the same camera and source snapshot as the viewport.
use super::*;
use crate::sculpt::instances::*;

pub(crate) use crate::sculpt::instances::ObjectOptions as BrushOptions;
pub(super) fn paint_capture_axes(
    painter: &egui::Painter,
    scene: &Scene,
    pivot: Vec3,
    forward: usize,
    up: usize,
) {
    if !scene.visible_depth(pivot) || forward > 2 || up > 2 {
        return;
    }
    let centre = scene.project(pivot);
    if !scene.rect.contains(centre) {
        return;
    }
    for (label, index, offset) in [("Forward", forward, -20.), ("Up", up, 7.)] {
        let direction = scene.axis_direction(pivot, axis(index));
        let delta = direction * 88.;
        let colour = controls::AXES[index];
        let axis_name = ["X", "Y", "Z"][index];
        if delta.length() > 8. {
            painter.arrow(centre, delta, egui::Stroke::new(2.5_f32, colour));
            controls::label(
                painter,
                centre + delta + egui::vec2(6., offset),
                &format!("{label} +{axis_name}"),
                colour,
            );
        } else {
            let facing = if axis(index).dot(scene.view) > 0. {
                "towards view"
            } else {
                "away"
            };
            painter.circle_stroke(centre, 10., egui::Stroke::new(2_f32, colour));
            controls::label(
                painter,
                centre + egui::vec2(14., offset),
                &format!("{label} +{axis_name} · {facing}"),
                colour,
            );
        }
    }
    controls::marker(
        painter,
        centre,
        controls::Marker::Diamond,
        controls::ACTION,
        false,
    );
    controls::label(
        painter,
        centre + egui::vec2(-62., 12.),
        "Contact",
        controls::ACTION,
    );
}
pub(crate) struct ObjectBrush {
    pub piece: Piece,
    pub options: BrushOptions,
    stroke: Option<Set>,
    last_pointer: Option<Pos2>,
    last_surface: Option<Vec3>,
    travelled: f32,
}
impl ObjectBrush {
    pub(crate) fn stroking(&self) -> bool {
        self.stroke.is_some()
    }
    pub fn new(piece: Piece) -> Self {
        Self {
            piece,
            options: Default::default(),
            stroke: None,
            last_pointer: None,
            last_surface: None,
            travelled: 0.,
        }
    }
    pub fn cancel(&mut self, document: &mut Document) {
        if self.stroke.take().is_some() {
            document.cancel_instance_preview();
        }
        self.last_pointer = None;
        self.last_surface = None;
        self.travelled = 0.;
    }
    fn placement(&self, center: Vec3, normal: Vec3, direction: Vec3) -> Placement {
        let forward = direction - normal * direction.dot(normal);
        let forward = if forward.len() > 1e-5 {
            forward.norm()
        } else {
            let fallback = if normal.0.abs() < 0.8 {
                axis(0)
            } else {
                axis(1)
            };
            (fallback - normal * fallback.dot(normal)).norm()
        }
        .rotated(normal, self.options.rotation.to_radians());
        Placement {
            piece: self.piece,
            transform: Transform {
                basis: None,
                origin: center + normal * self.options.offset,
                forward,
                up: normal,
                scale: self.options.scale,
            },
        }
    }
    fn sample(&mut self, point: Pos2, scene: &Scene, document: &Document) -> Result<(), String> {
        let Some((center, normal)) = scene.pick_object_target(point, document) else {
            self.last_surface = None;
            self.travelled = 0.;
            return Ok(());
        };
        let previous = self.last_surface.replace(center);
        let mut hits = Vec::new();
        if let Some(previous) = previous {
            let delta = center - previous;
            let distance = delta.len();
            let mut along = self.options.spacing - self.travelled;
            while along <= distance && hits.len() < MAX_INSTANCES {
                let candidate = previous + delta * (along / distance.max(1e-8));
                if let Some((p, n)) = scene.pick_object_target(scene.project(candidate), document) {
                    hits.push(self.placement(p, n, delta));
                }
                along += self.options.spacing;
            }
            self.travelled = (self.travelled + distance) % self.options.spacing;
        } else {
            hits.push(self.placement(center, normal, scene.right));
        }
        if let Some(Set {
            layout: Layout::Manual(items),
            ..
        }) = &mut self.stroke
        {
            if items.len() + hits.len() > MAX_INSTANCES {
                return Err(
                    "Stroke reached the 256-object limit; release to apply or Escape to cancel"
                        .into(),
                );
            }
            items.extend(hits);
        }
        Ok(())
    }
}
impl Viewport {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_objects(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        document: &mut Document,
        options: ViewportOptions,
        gizmo: egui::Rect,
    ) -> bool {
        let Some(mut brush) = self.object_brush.take() else {
            return false;
        };
        if document.instance_source(brush.piece.source).is_none()
            || options.transform_mode != TransformMode::Sculpt
            || options.modeling.is_some()
            || options.select.is_some()
            || options.slice.is_some()
            || options.edge_select
            || options.arrange_references
            || self.paths.active
            || self.sample_colour
        {
            brush.cancel(document);
            return false;
        }
        let modifiers = ui.input(|i| i.modifiers);
        let navigation = options.zoom_hold.active(modifiers)
            || options.pan_hold.active(modifiers)
            || self.session.gesture.is_some()
            || ui.input(|i| {
                i.pointer.secondary_down()
                    || i.pointer.middle_down()
                    || (!i.pointer.primary_down()
                        && i.smooth_scroll_delta.y != 0.
                        && !brush.stroking())
            });
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escape || !ui.input(|i| i.focused) {
            brush.cancel(document);
            self.object_brush = Some(brush);
            self.wait_for_release();
            return true;
        }
        if !options.enabled {
            if !ui.input(|i| i.pointer.primary_down()) {
                brush.cancel(document);
            }
            self.object_brush = Some(brush);
            return false;
        }
        if navigation {
            if brush.stroke.is_some() {
                brush.cancel(document);
            }
            self.object_brush = Some(brush);
            return false;
        }
        let permitted = |p: Pos2| {
            response.rect.contains(p)
                && !gizmo.contains(p)
                && !navigation::contains(response.rect, p)
                && ui.ctx().layer_id_at(p) == Some(response.layer_id)
        };
        if let Some(point) = response.hover_pos().filter(|p| permitted(*p))
            && let Some((center, normal)) = scene.pick_object_target(point, document)
            && let Some(source) = document.instance_source(brush.piece.source)
        {
            let placement = brush.placement(center, normal, scene.right);
            for tri in source
                .triangles
                .iter()
                .step_by((source.triangles.len() / 180).max(1))
            {
                let p = tri.map(|i| placement.point(source.vertices[i]));
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    if let Some(line) = scene.clip_segment([p[a], p[b]]) {
                        painter.line_segment(
                            line,
                            egui::Stroke::new(1_f32, Color32::from_rgb(80, 225, 205)),
                        );
                    }
                }
            }
        }
        let events = ui.input(|i| i.events.clone());
        let mut changed = false;
        let mut released = false;
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } if permitted(pos) => {
                    if scene.pick_object_target(pos, document).is_some() {
                        match document.next_instance_set_id() {
                            Ok(id) => {
                                brush.stroke = Some(Set {
                                    id,
                                    name: "Object stroke".into(),
                                    visible: true,
                                    layout: Layout::Manual(Vec::new()),
                                    adjustments: Vec::new(),
                                });
                                brush.last_surface = None;
                                brush.last_pointer = Some(pos);
                                brush.travelled = 0.;
                                if let Err(error) = brush.sample(pos, scene, document) {
                                    self.status = Some(error);
                                }
                                changed = true;
                            }
                            Err(error) => self.status = Some(error),
                        }
                    } else if scene.pick_model(pos).is_none() && !brush.stroking() {
                        // Keep ownership of the drag even if the pointer crosses the model.
                        self.session.gesture = Some(Gesture::Orbit);
                        self.object_brush = Some(brush);
                        return false;
                    }
                }
                egui::Event::PointerMoved(pos) if brush.stroke.is_some() => {
                    if let Some(last) = brush.last_pointer.replace(pos) {
                        let steps = (last.distance(pos) / 2.).ceil().clamp(1., 256.) as usize;
                        for i in 1..=steps {
                            let p = last + (pos - last) * (i as f32 / steps as f32);
                            if !permitted(p) {
                                brush.last_surface = None;
                                brush.travelled = 0.;
                                continue;
                            }
                            if let Err(error) = brush.sample(p, scene, document) {
                                self.status = Some(error);
                                break;
                            }
                        }
                        changed = true;
                    }
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } if brush.stroke.is_some() => {
                    if brush.last_pointer != Some(pos) && permitted(pos) {
                        if let Err(error) = brush.sample(pos, scene, document) {
                            self.status = Some(error);
                        }
                        changed = true;
                    }
                    released = true;
                }
                _ => {}
            }
        }
        if changed && let Some(set) = &brush.stroke {
            if let Err(error) = document.preview_instance_set(set.clone()) {
                self.status = Some(error);
                brush.cancel(document);
            }
            ui.ctx().request_repaint();
        }
        if released && let Some(set) = brush.stroke.take() {
            let result = document.finish_object_stroke(set.id, brush.piece.source, brush.options);
            if let Err(error) = result {
                document.cancel_instance_preview();
                self.status = Some(error);
            } else {
                self.status = Some("Object stroke applied · Undo removes the entire stroke".into());
            }
            brush.last_pointer = None;
            brush.last_surface = None;
            self.wait_for_release();
        }
        let consume = brush.stroke.is_some() || response.hover_pos().is_some_and(permitted);
        self.object_brush = Some(brush);
        consume
    }
}

//! Latched visibility gestures, ahead of sculpt/zoom routing.
use super::{Gesture, Viewport, scene::Scene};
use crate::document::Document;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind};

pub(super) struct SelectionDrag {
    start: Pos2,
    end: Pos2,
    last: Pos2,
    group: Option<usize>,
    revision: u64,
    hide: bool,
}
impl Viewport {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_visibility(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        document: &mut Document,
        gizmo: Rect,
        enabled: bool,
        select: bool,
    ) -> bool {
        let modifiers = ui.input(|i| i.modifiers);
        let requested = select || ((modifiers.ctrl || modifiers.command) && modifiers.shift);
        if self.selection.is_none()
            && self.gesture.is_none()
            && enabled
            && requested
            && document.primitive().is_none()
            && document.active_visible()
            && ((response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down()))
                || response.clicked_by(egui::PointerButton::Primary))
            && let Some(pos) = response
                .interact_pointer_pos()
                .filter(|p| !gizmo.contains(*p))
        {
            self.selection = Some(SelectionDrag {
                start: pos,
                end: pos,
                last: pos,
                group: scene.pick_polygroup(pos, document),
                revision: document.display_revision(),
                hide: modifiers.alt,
            });
            self.gesture = Some(Gesture::Select);
        }
        let Some(drag) = self.selection.as_mut() else {
            return false;
        };
        if !enabled
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
            || document.display_revision() != drag.revision
        {
            self.selection = None;
            self.gesture = None;
            self.wait_for_release();
            return true;
        }
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if ui.input(|i| i.key_down(egui::Key::Space)) {
                let delta = pos - drag.last;
                drag.start += delta;
                drag.end += delta;
            } else {
                drag.end = pos;
            }
            drag.last = pos;
        }
        // Alt switches the current rectangle, even after Ctrl/Shift are released.
        drag.hide = modifiers.alt;
        let rect = Rect::from_two_pos(drag.start, drag.end);
        let color = if drag.hide {
            Color32::LIGHT_RED
        } else {
            Color32::LIGHT_GREEN
        };
        painter.rect_filled(rect, 0., color.gamma_multiply(0.13));
        painter.rect_stroke(rect, 0., Stroke::new(1.5, color), StrokeKind::Inside);
        painter.text(
            rect.left_top() - egui::vec2(0., 6.),
            egui::Align2::LEFT_BOTTOM,
            if drag.hide { "Hide" } else { "Isolate" },
            egui::FontId::proportional(13.),
            color,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            let drag = self.selection.take().unwrap();
            self.gesture = None;
            let (result, message) = if drag.start.distance(drag.end) < 4. {
                if let Some(group) = drag.group {
                    (
                        document.select_polygroup(group),
                        "Vertex group visibility updated",
                    )
                } else {
                    (document.show_all_faces(), "All faces shown")
                }
            } else {
                let intersects = document.mesh().triangles.iter().enumerate().any(|(i, t)| {
                    document.mesh().face_visible(i)
                        && triangle_intersects_rect(
                            t.map(|v| scene.project(document.mesh().vertices[v])),
                            rect,
                        )
                });
                if drag.group.is_none() && !intersects {
                    (document.invert_visibility(), "Visibility inverted")
                } else {
                    (
                        document.select_face_region(|p| rect.contains(scene.project(p)), drag.hide),
                        if drag.hide {
                            "Selection hidden"
                        } else {
                            "Selection isolated"
                        },
                    )
                }
            };
            self.visibility_status = Some(result.err().unwrap_or_else(|| message.into()));
        } else if !ui.input(|i| i.pointer.primary_down()) {
            self.selection = None;
            self.gesture = None;
        }
        true
    }
}

// Separating axes: rectangle axes, then each projected triangle edge normal.
fn triangle_intersects_rect(triangle: [Pos2; 3], rect: Rect) -> bool {
    if !Rect::from_points(&triangle).intersects(rect) {
        return false;
    }
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    for i in 0..3 {
        let edge = triangle[(i + 1) % 3] - triangle[i];
        let cross = |p: Pos2| {
            let d = p - triangle[i];
            edge.x * d.y - edge.y * d.x
        };
        let side = cross(triangle[(i + 2) % 3]);
        if side > 0. && corners.iter().all(|&p| cross(p) < 0.)
            || side < 0. && corners.iter().all(|&p| cross(p) > 0.)
        {
            return false;
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangle_overlap_detects_face_interiors_without_vertices() {
        let triangle = [
            egui::pos2(0., 0.),
            egui::pos2(100., 0.),
            egui::pos2(0., 100.),
        ];
        assert!(triangle_intersects_rect(
            triangle,
            Rect::from_min_max(egui::pos2(10., 10.), egui::pos2(20., 20.))
        ));
        assert!(!triangle_intersects_rect(
            triangle,
            Rect::from_min_max(egui::pos2(80., 80.), egui::pos2(90., 90.))
        ));
    }
}

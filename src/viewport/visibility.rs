//! Latched rectangle, lasso, and mask-line gestures ahead of sculpt routing.
use super::{Gesture, Viewport, scene::Scene};
use crate::document::Document;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionTool {
    Rectangle,
    Lasso,
    MaskLasso,
    MaskLassoOperation { erase: bool },
    MaskLine { erase: bool, flip: bool },
}
impl SelectionTool {
    fn is_mask_lasso(self) -> bool {
        matches!(self, Self::MaskLasso | Self::MaskLassoOperation { .. })
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "Select rectangle",
            Self::Lasso => "Select lasso",
            Self::MaskLasso | Self::MaskLassoOperation { .. } => "Mask lasso",
            Self::MaskLine { .. } => "Mask through · Line",
        }
    }
    pub(crate) fn instructions(self) -> &'static str {
        if matches!(self, Self::MaskLine { .. }) {
            "Drag a dividing line, including from empty space · release to apply"
        } else if self.is_mask_lasso() {
            "Drag to mask · Alt to erase · release to apply"
        } else {
            "Drag to isolate · Alt to hide · release to apply"
        }
    }
}

pub(super) struct SelectionDrag {
    start: Pos2,
    end: Pos2,
    last: Pos2,
    points: Vec<Pos2>,
    tool: SelectionTool,
    group: Option<usize>,
    revision: u64,
    projection: (f32, Pos2, crate::sculpt::Vec3, crate::sculpt::Vec3, f32),
    hide: bool,
}
impl SelectionDrag {
    fn move_pointer(&mut self, pos: Pos2, translate: bool) -> bool {
        if !pos.x.is_finite() || !pos.y.is_finite() {
            return false;
        }
        if translate {
            let delta = pos - self.last;
            self.start += delta;
            self.end += delta;
            for point in &mut self.points {
                *point += delta;
            }
        } else {
            self.end = pos;
            if matches!(
                self.tool,
                SelectionTool::Lasso
                    | SelectionTool::MaskLasso
                    | SelectionTool::MaskLassoOperation { .. }
            ) && self.points.last().is_none_or(|p| p.distance(pos) >= 2.)
            {
                // Reject overlong gestures instead of silently changing their shape.
                if self.points.len() >= 4096 {
                    return false;
                }
                self.points.push(pos);
            }
        }
        self.last = pos;
        true
    }
    fn polygon(&self) -> Vec<Pos2> {
        let mut points = self.points.clone();
        if points.last() != Some(&self.end) {
            points.push(self.end);
        }
        points
    }
}

// Evaluate the current view's projected half-plane, including rear-facing vertices.
fn line_distance(start: Pos2, end: Pos2, flip: bool, p: Pos2) -> f32 {
    let edge = end - start;
    let d = p - start;
    (edge.x * d.y - edge.y * d.x) * if flip { -1. } else { 1. }
}
fn line_contains(start: Pos2, end: Pos2, flip: bool, p: Pos2) -> bool {
    line_distance(start, end, flip, p) >= 0.
}
fn paint_mask_line(
    painter: &egui::Painter,
    rect: Rect,
    start: Pos2,
    end: Pos2,
    flip: bool,
    color: Color32,
) {
    let direction = (end - start).normalized();
    if direction.length_sq() == 0. {
        return;
    }
    // Clip the viewport rectangle against exactly the half-plane used on commit.
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let mut polygon = Vec::new();
    let mut a = corners[3];
    let mut da = line_distance(start, end, flip, a);
    for b in corners {
        let db = line_distance(start, end, flip, b);
        if (da >= 0.) != (db >= 0.) {
            polygon.push(a + (b - a) * (da / (da - db)));
        }
        if db >= 0. {
            polygon.push(b);
        }
        a = b;
        da = db;
    }
    if polygon.len() >= 3 {
        painter.add(egui::Shape::convex_polygon(
            polygon,
            color.gamma_multiply(0.10),
            Stroke::NONE,
        ));
    }
    let reach = rect.width() + rect.height();
    painter.line_segment(
        [start - direction * reach, end + direction * reach],
        Stroke::new(1_f32, color.gamma_multiply(0.5)),
    );
    painter.line_segment([start, end], Stroke::new(2_f32, color));
    for point in [start, end] {
        painter.circle_filled(point, 4., color);
    }
    let side = egui::vec2(-direction.y, direction.x) * if flip { -1. } else { 1. };
    painter.arrow(
        start + (end - start) * 0.5,
        side * 36.,
        Stroke::new(2_f32, color),
    );
}

// Even/odd filling supports concave and crossing outlines. Boundary points are
// inside; zero-length edges are harmless. Bounds avoid work for distant vertices.
struct LassoRegion {
    points: Vec<Pos2>,
    bounds: Rect,
}
impl LassoRegion {
    fn new(points: Vec<Pos2>) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let bounds = Rect::from_points(&points);
        let a = points[0];
        let b = *points
            .iter()
            .max_by(|p, q| p.distance_sq(a).total_cmp(&q.distance_sq(a)))?;
        let edge = b - a;
        // A tap, line, or subpixel sliver must never hide the entire model.
        if !points.iter().any(|p| {
            let d = *p - a;
            (edge.x * d.y - edge.y * d.x).abs() > 4.
        }) {
            return None;
        }
        Some(Self { points, bounds })
    }
    fn contains(&self, p: Pos2) -> bool {
        if !self.bounds.contains(p) {
            return false;
        }
        let mut inside = false;
        let mut a = *self.points.last().unwrap();
        for &b in &self.points {
            let edge = b - a;
            let d = p - a;
            let cross = edge.x * d.y - edge.y * d.x;
            if cross.abs() <= 1e-4
                && d.dot(edge) >= 0.
                && d.dot(edge) <= edge.length_sq()
                && edge.length_sq() > 0.
            {
                return true;
            }
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
                inside = !inside;
            }
            a = b;
        }
        inside
    }
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
        select: Option<SelectionTool>,
        navigate: bool,
    ) -> bool {
        let modifiers = ui.input(|i| i.modifiers);
        let requested = select.is_some()
            || ui.input(|i| {
                self.visibility_hold
                    .as_ref()
                    .unwrap_or(&crate::preferences::HoldBinding::visibility_default())
                    .active(i)
            });
        let press = ui.input(|i| {
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
        // egui may report neither clicked nor dragged for a full loop drawn and
        // released in one frame. Accept its press only on this viewport's layer.
        let pressed_here = press.is_some_and(|p| {
            response.rect.contains(p) && ui.ctx().layer_id_at(p) == Some(response.layer_id)
        });
        let mut started = false;
        if self.selection.is_none()
            && press.is_some()
            && self.gesture.is_none()
            && enabled
            && requested
            && !navigate
            && document.primitive().is_none()
            && document.active_visible()
            && ((response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down()))
                || response.clicked_by(egui::PointerButton::Primary)
                || pressed_here)
            && let Some(pos) = press
                .or_else(|| ui.input(|i| i.pointer.press_origin()))
                .or_else(|| response.interact_pointer_pos())
                .filter(|p| !gizmo.contains(*p))
        {
            self.selection = Some(SelectionDrag {
                start: pos,
                end: pos,
                last: pos,
                points: vec![pos],
                tool: select.unwrap_or(SelectionTool::Rectangle),
                group: scene.pick_polygroup(pos, document),
                revision: document.display_revision(),
                projection: (
                    scene.scale,
                    scene.center,
                    scene.right,
                    scene.up,
                    scene.eye_distance,
                ),
                hide: modifiers.alt,
            });
            self.gesture = Some(Gesture::Select);
            started = true;
        }
        let Some(drag) = self.selection.as_mut() else {
            return false;
        };
        // A configurable flip action may update the side during the drag. Other
        // tool switches cancel instead of committing a stale selection on release.
        if matches!(drag.tool, SelectionTool::MaskLine { .. })
            && let Some(tool @ SelectionTool::MaskLine { .. }) = select
        {
            drag.tool = tool;
        }
        if !enabled
            || (matches!(drag.tool, SelectionTool::MaskLine { .. })
                && egui::Popup::is_any_open(ui.ctx()))
            || (select.is_some() && select != Some(drag.tool))
            || ui.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || !i.focused
                    || i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::PointerGone))
            })
            || document.display_revision() != drag.revision
            || drag.projection
                != (
                    scene.scale,
                    scene.center,
                    scene.right,
                    scene.up,
                    scene.eye_distance,
                )
        {
            self.selection = None;
            self.gesture = None;
            self.wait_for_release();
            return true;
        }
        // Preserve all pen/mouse samples delivered in a frame, including a fast
        // press/move/release, but ignore pointer movement before the press.
        let valid = ui.input(|i| {
            let mut collecting = !started;
            for event in &i.events {
                match event {
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        ..
                    } => collecting = true,
                    egui::Event::PointerMoved(pos) if collecting => {
                        if !drag.move_pointer(*pos, i.key_down(egui::Key::Space)) {
                            return false;
                        }
                    }
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } if collecting => {
                        return drag.move_pointer(*pos, i.key_down(egui::Key::Space));
                    }
                    _ => (),
                }
            }
            true
        });
        if !valid {
            self.selection = None;
            self.gesture = None;
            self.wait_for_release();
            self.status = Some(
                "Selection cancelled: outline too long or invalid; draw a shorter lasso".into(),
            );
            return true;
        }
        // Alt switches the operation even after Ctrl/Shift are released.
        drag.hide = match drag.tool {
            SelectionTool::MaskLine { erase, .. } | SelectionTool::MaskLassoOperation { erase } => {
                erase
            }
            _ => modifiers.alt,
        };
        let color = if drag.hide {
            Color32::LIGHT_RED
        } else {
            Color32::LIGHT_GREEN
        };
        let rect = if let SelectionTool::MaskLine { flip, .. } = drag.tool {
            paint_mask_line(painter, scene.rect, drag.start, drag.end, flip, color);
            Rect::from_two_pos(drag.start, drag.end)
        } else if drag.tool == SelectionTool::Rectangle {
            let rect = Rect::from_two_pos(drag.start, drag.end);
            painter.rect_filled(rect, 0., color.gamma_multiply(0.13));
            painter.rect_stroke(rect, 0., Stroke::new(1.5_f32, color), StrokeKind::Inside);
            rect
        } else {
            let points = drag.polygon();
            let rect = Rect::from_points(&points);
            painter.add(egui::Shape::line(points, Stroke::new(2_f32, color)));
            painter.line_segment(
                [drag.end, drag.start],
                Stroke::new(1_f32, color.gamma_multiply(0.6)),
            );
            rect
        };
        let operation = match (drag.tool, drag.hide) {
            (
                SelectionTool::MaskLasso
                | SelectionTool::MaskLassoOperation { .. }
                | SelectionTool::MaskLine { .. },
                false,
            ) => "Add mask",
            (
                SelectionTool::MaskLasso
                | SelectionTool::MaskLassoOperation { .. }
                | SelectionTool::MaskLine { .. },
                true,
            ) => "Erase mask",
            (_, false) => "Isolate",
            (_, true) => "Hide",
        };
        painter.text(
            egui::pos2(
                rect.left().clamp(
                    scene.rect.left() + 8.,
                    (scene.rect.right() - 280.).max(scene.rect.left() + 8.),
                ),
                (rect.top() - 6.).max(scene.rect.top() + 18.),
            ),
            egui::Align2::LEFT_BOTTOM,
            format!("{operation} · through all depths · release to apply"),
            egui::FontId::proportional(13.),
            color,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            let drag = self.selection.take().unwrap();
            self.gesture = None;
            if let SelectionTool::MaskLine { erase, flip } = drag.tool {
                if drag.start.distance(drag.end) < 6. {
                    self.status = Some("Draw a longer dividing line".into());
                    return true;
                }
                let result = document.mask_region_named(
                    |p| {
                        scene.visible_depth(p)
                            && line_contains(drag.start, drag.end, flip, scene.project(p))
                    },
                    erase,
                    if erase {
                        "Erase mask line"
                    } else {
                        "Mask line"
                    },
                );
                self.status = Some(
                    result
                        .err()
                        .unwrap_or_else(|| format!("{operation} through line applied")),
                );
                return true;
            }
            if drag.tool != SelectionTool::Rectangle {
                let Some(region) = LassoRegion::new(drag.polygon()) else {
                    self.status = Some("Draw a lasso enclosing an area".into());
                    return true;
                };
                let contains = |p| scene.visible_depth(p) && region.contains(scene.project(p));
                let result = if drag.tool.is_mask_lasso() {
                    document.mask_region(contains, drag.hide)
                } else {
                    document.select_face_region(contains, drag.hide)
                };
                self.status = Some(
                    result
                        .err()
                        .unwrap_or_else(|| format!("{operation} lasso applied")),
                );
                return true;
            }
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
                        && t.iter()
                            .any(|&v| scene.visible_depth(document.mesh().vertices[v]))
                        && triangle_intersects_rect(
                            t.map(|v| scene.project(document.mesh().vertices[v])),
                            rect,
                        )
                });
                if drag.group.is_none() && !intersects {
                    (document.invert_visibility(), "Visibility inverted")
                } else {
                    (
                        document.select_face_region(
                            |p| scene.visible_depth(p) && rect.contains(scene.project(p)),
                            drag.hide,
                        ),
                        if drag.hide {
                            "Selection hidden"
                        } else {
                            "Selection isolated"
                        },
                    )
                }
            };
            self.status = Some(result.err().unwrap_or_else(|| message.into()));
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
    fn lasso_concavity_crossings_boundaries_and_degenerate_paths() {
        let points = vec![
            egui::pos2(0., 0.),
            egui::pos2(20., 0.),
            egui::pos2(20., 10.),
            egui::pos2(10., 10.),
            egui::pos2(10., 20.),
            egui::pos2(0., 20.),
            egui::pos2(0., 0.),
        ];
        for points in [points.clone(), points.into_iter().rev().collect()] {
            let region = LassoRegion::new(points).unwrap();
            assert!(region.contains(egui::pos2(5., 15.)));
            assert!(region.contains(egui::pos2(15., 5.)));
            assert!(!region.contains(egui::pos2(15., 15.)));
            assert!(region.contains(egui::pos2(10., 10.)));
            assert!(region.contains(egui::pos2(0., 0.)));
            assert!(!region.contains(egui::pos2(-1., 5.)));
        }
        let crossed = LassoRegion::new(vec![
            egui::pos2(0., 0.),
            egui::pos2(20., 20.),
            egui::pos2(0., 20.),
            egui::pos2(20., 0.),
        ])
        .unwrap();
        assert!(crossed.contains(egui::pos2(10., 3.)));
        assert!(crossed.contains(egui::pos2(10., 17.)));
        assert!(!crossed.contains(egui::pos2(3., 10.)));
        assert!(LassoRegion::new(vec![Pos2::ZERO; 3]).is_none());
        assert!(
            LassoRegion::new(vec![Pos2::ZERO, egui::pos2(10., 10.), egui::pos2(20., 20.)])
                .is_none()
        );
    }
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

//! World-axis handles and a user-positioned Action Line.
use super::scene::Scene;
use crate::{
    document::Document,
    sculpt::{Transform, Vec3},
};
use eframe::egui::{self, Color32, Pos2, Vec2};
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) enum TransformMode {
    #[default]
    Sculpt,
    World,
    ActionLine,
}
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) enum TransformOperation {
    #[default]
    Move,
    Scale,
    PlaceLine,
}
#[derive(Clone, Copy)]
enum Handle {
    Axis(Vec3, Vec2),
    Free,
    Uniform,
    LineStart,
    LineEnd,
    LineMove,
    LineScale(Vec3, Vec2),
}
#[derive(Default)]
pub(super) struct TransformControls {
    pub line: Option<[Vec3; 2]>,
    drag: Option<Handle>,
    last: Option<Pos2>,
    pivot: Option<Vec3>,
}
fn bounds_center(document: &Document) -> Vec3 {
    let mut min = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in &document.mesh().vertices {
        min = Vec3(min.0.min(p.0), min.1.min(p.1), min.2.min(p.2));
        max = Vec3(max.0.max(p.0), max.1.max(p.1), max.2.max(p.2));
    }
    (min + max) * 0.5
}
impl TransformControls {
    fn handles(
        &self,
        document: &Document,
        scene: &Scene,
        mode: TransformMode,
        op: TransformOperation,
    ) -> Vec<(Pos2, Handle, Color32, &'static str)> {
        if !document.active_visible() {
            return vec![];
        }
        if mode == TransformMode::World {
            let pivot = self.pivot.unwrap_or_else(|| bounds_center(document));
            let origin = scene.project(pivot);
            let mut handles = vec![(
                origin,
                if op == TransformOperation::Scale {
                    Handle::Uniform
                } else {
                    Handle::Free
                },
                Color32::WHITE,
                if op == TransformOperation::Scale {
                    "ALL"
                } else {
                    "MOVE"
                },
            )];
            for (axis, color, label) in [
                (Vec3(1., 0., 0.), Color32::from_rgb(245, 105, 110), "X"),
                (Vec3(0., 1., 0.), Color32::from_rgb(120, 225, 145), "Y"),
                (Vec3(0., 0., 1.), Color32::from_rgb(110, 165, 250), "Z"),
            ] {
                let projected = Vec2::new(axis.dot(scene.right), -axis.dot(scene.up));
                if projected.length() > 0.1 {
                    handles.push((
                        origin + projected * 85.,
                        Handle::Axis(axis, projected),
                        color,
                        label,
                    ));
                }
            }
            handles
        } else if let Some([a, b]) = self.line {
            let start = scene.project(a);
            let end = scene.project(b);
            let direction = (b - a).norm();
            let projected = Vec2::new(direction.dot(scene.right), -direction.dot(scene.up));
            match op {
                TransformOperation::PlaceLine => vec![
                    (start, Handle::LineStart, Color32::GOLD, "START"),
                    (end, Handle::LineEnd, Color32::GOLD, "END"),
                ],
                TransformOperation::Move => vec![(
                    start + (end - start) * 0.5,
                    Handle::LineMove,
                    Color32::GOLD,
                    "MOVE",
                )],
                TransformOperation::Scale => vec![
                    (
                        end,
                        Handle::LineScale(direction, projected),
                        Color32::GOLD,
                        "STRETCH",
                    ),
                    (
                        start + (end - start) * 0.5,
                        Handle::Uniform,
                        Color32::WHITE,
                        "ALL",
                    ),
                ],
            }
        } else {
            vec![]
        }
    }
    fn hit(
        &self,
        document: &Document,
        scene: &Scene,
        mode: TransformMode,
        op: TransformOperation,
        pointer: Pos2,
    ) -> Option<Handle> {
        self.handles(document, scene, mode, op)
            .into_iter()
            .find(|(p, _, _, _)| p.distance(pointer) <= 15.)
            .map(|(_, h, _, _)| h)
    }
    pub fn can_drag(
        &self,
        document: &Document,
        scene: &Scene,
        mode: TransformMode,
        op: TransformOperation,
        pointer: Pos2,
    ) -> bool {
        document.active_visible()
            && (self.hit(document, scene, mode, op, pointer).is_some()
                || (mode == TransformMode::ActionLine && op == TransformOperation::PlaceLine))
    }
    pub fn update(
        &mut self,
        document: &mut Document,
        scene: &Scene,
        mode: TransformMode,
        op: TransformOperation,
        pointer: Option<Pos2>,
        dragging: bool,
        respect_mask: bool,
    ) {
        if !dragging {
            self.drag = None;
            self.last = None;
            self.pivot = None;
            return;
        }
        let Some(pointer) = pointer else {
            return;
        };
        if self.drag.is_none() {
            self.pivot = Some(if mode == TransformMode::ActionLine {
                self.line
                    .map_or_else(|| bounds_center(document), |line| line[0])
            } else {
                bounds_center(document)
            });
            self.drag = self.hit(document, scene, mode, op, pointer);
            if self.drag.is_none()
                && mode == TransformMode::ActionLine
                && op == TransformOperation::PlaceLine
            {
                let start = scene.pick(pointer).map_or_else(
                    || {
                        let p = self.pivot.unwrap();
                        p + scene.right * ((pointer.x - scene.project(p).x) / scene.scale)
                            + scene.up * ((scene.project(p).y - pointer.y) / scene.scale)
                    },
                    |(p, _)| p,
                );
                self.line = Some([start, start]);
                self.drag = Some(Handle::LineEnd);
            }
            self.last = Some(pointer);
            return;
        }
        let delta = pointer - self.last.unwrap_or(pointer);
        self.last = Some(pointer);
        if delta.length_sq() == 0. {
            return;
        }
        let movement = scene.right * (delta.x / scene.scale) + scene.up * (-delta.y / scene.scale);
        let pivot = self.pivot.unwrap();
        let mut transform = Transform {
            pivot,
            ..Default::default()
        };
        match self.drag.unwrap() {
            Handle::LineStart | Handle::LineEnd => {
                if let Some(line) = &mut self.line {
                    let index = usize::from(matches!(self.drag, Some(Handle::LineEnd)));
                    line[index] = line[index] + movement;
                }
                return;
            }
            Handle::Free | Handle::LineMove => {
                transform.translation = movement;
            }
            Handle::Axis(axis, projected) => {
                let amount = delta.dot(projected) / projected.length_sq().max(0.01);
                if op == TransformOperation::Scale {
                    let factor = (amount * 0.01).clamp(-2., 2.).exp();
                    transform.scale = Vec3(
                        1. + axis.0 * (factor - 1.),
                        1. + axis.1 * (factor - 1.),
                        1. + axis.2 * (factor - 1.),
                    );
                } else {
                    transform.translation = axis * (amount / scene.scale);
                }
            }
            Handle::Uniform => {
                let factor = ((delta.x - delta.y) * 0.008).clamp(-2., 2.).exp();
                transform.scale = Vec3(factor, factor, factor);
            }
            Handle::LineScale(axis, projected) => {
                if axis.len() < 0.9 || projected.length_sq() < 0.01 {
                    return;
                }
                let amount = delta.dot(projected) / projected.length_sq();
                transform.stretch = Some((axis, (amount * 0.01).clamp(-2., 2.).exp()));
            }
        }
        if document
            .apply_transform(transform, respect_mask)
            .is_ok_and(|changed| changed)
        {
            if mode == TransformMode::ActionLine
                && let Some(line) = &mut self.line
            {
                *line = line.map(|p| transform.point(p));
            }
            if mode == TransformMode::World {
                self.pivot = Some(pivot + transform.translation);
            }
        }
    }
    pub fn paint(
        &self,
        document: &Document,
        scene: &Scene,
        painter: &egui::Painter,
        mode: TransformMode,
        op: TransformOperation,
    ) {
        if !document.active_visible() {
            return;
        }
        let origin = scene.project(self.pivot.unwrap_or_else(|| bounds_center(document)));
        if mode == TransformMode::ActionLine
            && let Some(line) = self.line
        {
            painter.line_segment(
                line.map(|p| scene.project(p)),
                egui::Stroke::new(3., Color32::GOLD),
            );
        }
        for (point, _, color, label) in self.handles(document, scene, mode, op) {
            if mode == TransformMode::World {
                painter.line_segment([origin, point], egui::Stroke::new(2.5, color));
            }
            if op == TransformOperation::Scale {
                painter.rect_filled(
                    egui::Rect::from_center_size(point, Vec2::splat(12.)),
                    2.,
                    color,
                );
            } else {
                painter.circle_filled(point, 6., color);
            }
            painter.text(
                point + Vec2::new(9., -9.),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(12.),
                color,
            );
        }
    }
}

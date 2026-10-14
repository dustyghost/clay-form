//! Shared World-axis and Action Line transforms, including session-only pivots.
mod rotation;
use super::{controls, scene::Scene};
use crate::{
    document::Document,
    sculpt::{Transform, Vec3},
};
use eframe::egui::{self, Color32, Pos2, Vec2};
use rotation::{Ring, RotationDrag};
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
    Rotate,
    PlacePivot,
}
impl TransformOperation {
    pub const ALL: [Self; 5] = [
        Self::PlaceLine,
        Self::Move,
        Self::Scale,
        Self::Rotate,
        Self::PlacePivot,
    ];
    pub fn available(self, mode: TransformMode) -> bool {
        mode != TransformMode::Sculpt
            && match self {
                Self::PlaceLine => mode == TransformMode::ActionLine,
                Self::PlacePivot => mode == TransformMode::World,
                _ => true,
            }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Scale => "Scale",
            Self::Rotate => "Rotate",
            Self::PlaceLine => "Set line",
            Self::PlacePivot => "Place pivot",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Self::Move => "Drag an axis or the middle handle to move the model",
            Self::Scale => "Drag an axis/end to stretch, or the middle to scale uniformly",
            Self::Rotate => {
                "Drag a colored ring to rotate about that axis. Action Line rotates around its direction. Edge-on rings use a straight drag along the ring"
            }
            Self::PlaceLine => {
                "Drag to place the pivot and direction; drag either endpoint to adjust"
            }
            Self::PlacePivot => {
                "Click the active surface to place the pivot, or empty space at the current pivot depth; drag to adjust in the view plane"
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Handle {
    Axis(Vec3, Vec2),
    Free,
    Uniform,
    LineStart,
    LineEnd,
    LineMove,
    LineScale(Vec3, Vec2),
    Rotate(Vec3),
    Pivot,
}
#[derive(Default)]
pub(super) struct TransformControls {
    pub line: Option<[Vec3; 2]>,
    drag: Option<Handle>,
    last: Option<Pos2>,
    pivot: Option<Vec3>,
    drag_pivot: Option<Vec3>,
    rotation: Option<RotationDrag>,
}
fn bounds_center(document: &Document) -> Vec3 {
    if document.mesh().vertices.is_empty() {
        return Vec3::default();
    }
    let mut min = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in &document.mesh().vertices {
        min = Vec3(min.0.min(p.0), min.1.min(p.1), min.2.min(p.2));
        max = Vec3(max.0.max(p.0), max.1.max(p.1), max.2.max(p.2));
    }
    (min + max) * 0.5
}
impl TransformControls {
    pub fn finish_drag(&mut self) {
        self.drag = None;
        self.last = None;
        self.drag_pivot = None;
        self.rotation = None;
    }
    pub fn reset_pivot(&mut self, document: &Document, mode: TransformMode) {
        if mode == TransformMode::World {
            self.pivot = None;
        }
        if mode == TransformMode::ActionLine
            && let Some(line) = &mut self.line
        {
            let delta = bounds_center(document) - line[0];
            *line = line.map(|p| p + delta);
        }
    }
    fn origin(&self, document: &Document, mode: TransformMode) -> Vec3 {
        self.drag_pivot.unwrap_or_else(|| {
            if mode == TransformMode::ActionLine {
                self.line
                    .map_or_else(|| bounds_center(document), |line| line[0])
            } else {
                self.pivot.unwrap_or_else(|| bounds_center(document))
            }
        })
    }
    fn rings(&self, document: &Document, mode: TransformMode, op: TransformOperation) -> Vec<Ring> {
        if op != TransformOperation::Rotate
            || !document.active_visible()
            || document.mesh().vertices.is_empty()
        {
            return vec![];
        }
        let pivot = self.origin(document, mode);
        if mode == TransformMode::World {
            [
                (Vec3(1., 0., 0.), controls::AXES[0], "Rotate X"),
                (Vec3(0., 1., 0.), controls::AXES[1], "Rotate Y"),
                (Vec3(0., 0., 1.), controls::AXES[2], "Rotate Z"),
            ]
            .into_iter()
            .map(|(axis, color, label)| Ring {
                axis,
                pivot,
                color,
                label,
            })
            .collect()
        } else if let Some([a, b]) = self.line
            && (b - a).len() > 1e-5
        {
            vec![Ring {
                axis: (b - a).norm(),
                pivot,
                color: controls::ACTION,
                label: "Rotate line",
            }]
        } else {
            vec![]
        }
    }
    fn handles(
        &self,
        document: &Document,
        scene: &Scene,
        mode: TransformMode,
        op: TransformOperation,
    ) -> Vec<(Pos2, Handle, Color32, &'static str)> {
        if !document.active_visible() || document.mesh().vertices.is_empty() {
            return vec![];
        }
        if op == TransformOperation::Rotate {
            return vec![];
        }
        if mode == TransformMode::World {
            let pivot = self.origin(document, mode);
            if op == TransformOperation::PlacePivot {
                return vec![(
                    scene.project(pivot),
                    Handle::Pivot,
                    controls::NEUTRAL,
                    "PIVOT",
                )];
            }
            let origin = scene.project(pivot);
            let mut handles = vec![(
                origin,
                if op == TransformOperation::Scale {
                    Handle::Uniform
                } else {
                    Handle::Free
                },
                controls::NEUTRAL,
                if op == TransformOperation::Scale {
                    "ALL"
                } else {
                    "MOVE"
                },
            )];
            for (axis, color, label) in [
                (Vec3(1., 0., 0.), controls::AXES[0], "X"),
                (Vec3(0., 1., 0.), controls::AXES[1], "Y"),
                (Vec3(0., 0., 1.), controls::AXES[2], "Z"),
            ] {
                let projected = scene.axis_direction(pivot, axis);
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
            let projected = scene.axis_direction(b, direction);
            match op {
                TransformOperation::PlaceLine => vec![
                    (start, Handle::LineStart, controls::ACTION, "START"),
                    (end, Handle::LineEnd, controls::ACTION, "END"),
                ],
                TransformOperation::Move => vec![(
                    start + (end - start) * 0.5,
                    Handle::LineMove,
                    controls::ACTION,
                    "MOVE",
                )],
                TransformOperation::Rotate | TransformOperation::PlacePivot => vec![],
                TransformOperation::Scale => vec![
                    (
                        end,
                        Handle::LineScale(direction, projected),
                        controls::ACTION,
                        "STRETCH",
                    ),
                    (
                        start + (end - start) * 0.5,
                        Handle::Uniform,
                        controls::NEUTRAL,
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
        if op == TransformOperation::Rotate {
            if pointer.distance(scene.project(self.origin(document, mode))) < 16. {
                return None;
            }
            return self
                .rings(document, mode, op)
                .into_iter()
                .map(|ring| (ring.nearest(scene, pointer).0, ring.axis))
                .filter(|(distance, _)| *distance <= controls::HIT_RADIUS)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, axis)| Handle::Rotate(axis));
        }
        self.handles(document, scene, mode, op)
            .into_iter()
            .find(|(p, _, _, _)| p.distance(pointer) <= controls::HIT_RADIUS)
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
            && !document.mesh().vertices.is_empty()
            && (self.hit(document, scene, mode, op, pointer).is_some()
                || (mode == TransformMode::ActionLine && op == TransformOperation::PlaceLine)
                || (mode == TransformMode::World && op == TransformOperation::PlacePivot))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        document: &mut Document,
        scene: &Scene,
        mode: TransformMode,
        op: TransformOperation,
        pointer: Option<Pos2>,
        dragging: bool,
        respect_mask: bool,
    ) -> Result<(), String> {
        if !dragging {
            self.finish_drag();
            return Ok(());
        }
        let Some(pointer) = pointer else {
            return Ok(());
        };
        if self.drag.is_none() {
            self.drag_pivot = Some(self.origin(document, mode));
            self.drag = self.hit(document, scene, mode, op, pointer);
            if self.drag == Some(Handle::Pivot) {
                self.pivot = self.drag_pivot;
            }
            if let Some(Handle::Rotate(axis)) = self.drag {
                self.rotation = self
                    .rings(document, mode, op)
                    .into_iter()
                    .find(|ring| ring.axis == axis)
                    .map(|ring| ring.begin(scene, pointer));
            }
            if self.drag.is_none()
                && matches!(
                    op,
                    TransformOperation::PlaceLine | TransformOperation::PlacePivot
                )
            {
                let pivot = self.drag_pivot.unwrap();
                let start = scene
                    .pick(pointer)
                    .map_or_else(|| scene.at_depth(pointer, pivot), |(p, _)| p);
                if op == TransformOperation::PlaceLine {
                    self.line = Some([start, start]);
                    self.drag = Some(Handle::LineEnd);
                } else {
                    self.pivot = Some(start);
                    self.drag_pivot = Some(start);
                    self.drag = Some(Handle::Pivot);
                }
            }
            self.last = Some(pointer);
            return Ok(());
        }
        let delta = pointer - self.last.unwrap_or(pointer);
        self.last = Some(pointer);
        if delta.length_sq() == 0. {
            return Ok(());
        }
        let pivot = self.drag_pivot.unwrap();
        let anchor = match self.drag {
            Some(Handle::LineStart) => self.line.map_or(pivot, |line| line[0]),
            Some(Handle::LineEnd) => self.line.map_or(pivot, |line| line[1]),
            _ => pivot,
        };
        let scale = scene.scale_at(anchor);
        let movement = scene.right * (delta.x / scale) + scene.up * (-delta.y / scale);
        let mut transform = Transform {
            pivot,
            ..Default::default()
        };
        match self.drag.unwrap() {
            Handle::Pivot => {
                self.pivot = Some(pivot + movement);
                self.drag_pivot = self.pivot;
                return Ok(());
            }
            Handle::Rotate(axis) => {
                let angle = self
                    .rotation
                    .as_mut()
                    .expect("rotation handle has drag state")
                    .advance(scene, pointer);
                transform.rotation = Some((axis, angle));
            }
            Handle::LineStart | Handle::LineEnd => {
                if let Some(line) = &mut self.line {
                    let index = usize::from(matches!(self.drag, Some(Handle::LineEnd)));
                    line[index] = line[index] + movement;
                }
                return Ok(());
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
                    let motion = scene
                        .axis_parameter(pointer, pivot, axis)
                        .zip(scene.axis_parameter(pointer - delta, pivot, axis))
                        .map_or(amount / scale, |(end, start)| end - start);
                    transform.translation = axis * motion;
                }
            }
            Handle::Uniform => {
                let factor = ((delta.x - delta.y) * 0.008).clamp(-2., 2.).exp();
                transform.scale = Vec3(factor, factor, factor);
            }
            Handle::LineScale(axis, projected) => {
                if axis.len() < 0.9 || projected.length_sq() < 0.01 {
                    return Ok(());
                }
                let amount = delta.dot(projected) / projected.length_sq();
                transform.stretch = Some((axis, (amount * 0.01).clamp(-2., 2.).exp()));
            }
        }
        if document.apply_transform(transform, respect_mask)? {
            if mode == TransformMode::ActionLine
                && let Some(line) = &mut self.line
            {
                *line = line.map(|p| transform.point(p));
            }
            if mode == TransformMode::World {
                self.drag_pivot = Some(pivot + transform.translation);
                if self.pivot.is_some() {
                    self.pivot = self.drag_pivot;
                }
            }
        }
        Ok(())
    }
    pub fn paint(
        &self,
        document: &Document,
        scene: &Scene,
        painter: &egui::Painter,
        mode: TransformMode,
        op: TransformOperation,
    ) {
        if !document.active_visible() || document.mesh().vertices.is_empty() {
            return;
        }
        let origin = scene.project(self.origin(document, mode));
        if mode == TransformMode::ActionLine
            && let Some(line) = self.line
        {
            painter.line_segment(
                line.map(|p| scene.project(p)),
                egui::Stroke::new(3_f32, controls::ACTION),
            );
        }
        let pointer = painter.ctx().pointer_hover_pos();
        let hovered = pointer.and_then(|p| self.hit(document, scene, mode, op, p));
        if op == TransformOperation::Rotate {
            controls::marker(
                painter,
                origin,
                controls::Marker::Diamond,
                controls::NEUTRAL,
                false,
            );
            for ring in self.rings(document, mode, op) {
                let highlighted = [self.drag, hovered]
                    .into_iter()
                    .flatten()
                    .any(|h| matches!(h, Handle::Rotate(axis) if (axis - ring.axis).len() < 1e-4));
                ring.paint(scene, painter, highlighted);
            }
            if let Some(rotation) = &self.rotation {
                controls::label(
                    painter,
                    origin + Vec2::new(16., 16.),
                    &format!("{:+.1}°", rotation.angle.to_degrees()),
                    controls::NEUTRAL,
                );
            } else if mode == TransformMode::ActionLine
                && self.line.is_none_or(|[a, b]| (b - a).len() < 1e-5)
            {
                controls::label(
                    painter,
                    origin + Vec2::new(16., 16.),
                    "Set a line with a direction first",
                    controls::ACTION,
                );
            }
        }
        for (point, handle, color, label) in self.handles(document, scene, mode, op) {
            if mode == TransformMode::World {
                painter.line_segment([origin, point], egui::Stroke::new(2_f32, color));
            }
            let highlighted =
                self.drag.is_some_and(|h| h == handle) || hovered.is_some_and(|h| h == handle);
            let marker = if op == TransformOperation::PlacePivot {
                controls::Marker::Diamond
            } else if op == TransformOperation::Scale {
                controls::Marker::Square
            } else {
                controls::Marker::Circle
            };
            controls::marker(painter, point, marker, color, highlighted);
            let caption = if mode == TransformMode::World && matches!(handle, Handle::Axis(..)) {
                format!(
                    "{} {}",
                    if op == TransformOperation::Scale {
                        "Scale"
                    } else {
                        "Move"
                    },
                    label
                )
            } else {
                label.to_owned()
            };
            controls::label(painter, point + Vec2::new(12., -20.), &caption, color);
        }
    }
}

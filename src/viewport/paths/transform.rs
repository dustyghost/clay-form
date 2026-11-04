//! Whole-route transforms reuse the viewport's projected rotation rings.
use crate::sculpt::{
    Vec3,
    paths::{PathMode, SurfacePath},
};
use crate::viewport::{
    controls,
    scene::Scene,
    transform::rotation::{Ring, RotationDrag},
};
use eframe::egui::{self, Pos2, Vec2};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Operation {
    #[default]
    Move,
    Rotate,
    Scale,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Pivot {
    #[default]
    Centre,
    Start,
    Origin,
}
#[derive(Clone, Copy)]
enum Handle {
    Move(Option<Vec3>),
    Scale,
}
enum Gesture {
    Move(Option<Vec3>),
    Rotate(RotationDrag),
    Scale,
}
struct Drag {
    before: SurfacePath,
    pending: f32,
    pivot: Vec3,
    pointer: Pos2,
    gesture: Gesture,
    scale_sections: bool,
}
pub(crate) struct State {
    pub operation: Operation,
    pub pivot: Pivot,
    pub translation: Vec3,
    pub axis: usize,
    pub angle: f32,
    pub scale: f32,
    pub scale_sections: bool,
    pub pending_section_scale: f32,
    pub(super) route_start: Option<Vec3>,
    drag: Option<Drag>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            operation: Operation::Move,
            pivot: Pivot::Centre,
            translation: Vec3::default(),
            axis: 2,
            angle: 0.,
            scale: 1.,
            scale_sections: true,
            pending_section_scale: 1.,
            route_start: None,
            drag: None,
        }
    }
}
impl State {
    pub(crate) fn reset(&mut self) {
        self.translation = Vec3::default();
        self.angle = 0.;
        self.scale = 1.;
        self.pending_section_scale = 1.;
        self.route_start = None;
        self.drag = None;
    }
    fn origin(&self, path: &SurfacePath) -> Vec3 {
        match self.pivot {
            Pivot::Origin => Vec3::default(),
            Pivot::Start => self
                .route_start
                .or_else(|| path.points.get(path.seam).map(|p| p.position))
                .unwrap_or_default(),
            Pivot::Centre => {
                let (mut lo, mut hi) = (
                    Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY),
                    Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
                );
                for p in &path.points {
                    lo = Vec3(
                        lo.0.min(p.position.0),
                        lo.1.min(p.position.1),
                        lo.2.min(p.position.2),
                    );
                    hi = Vec3(
                        hi.0.max(p.position.0),
                        hi.1.max(p.position.1),
                        hi.2.max(p.position.2),
                    );
                }
                (lo + hi) * 0.5
            }
        }
    }
    pub(crate) fn apply_numeric(&mut self, path: &mut SurfacePath) -> Result<(), String> {
        let pivot = self.origin(path);
        let (translation, angle, scale) = match self.operation {
            Operation::Move => (self.translation, 0., 1.),
            Operation::Rotate => (Vec3::default(), self.angle.to_radians(), 1.),
            Operation::Scale => (Vec3::default(), 0., self.scale),
        };
        let next = path.transformed(pivot, translation, axis(self.axis), angle, scale)?;
        if self.operation == Operation::Scale && self.scale_sections {
            self.pending_section_scale *= scale;
        }
        *path = next;
        self.translation = Vec3::default();
        self.angle = 0.;
        self.scale = 1.;
        Ok(())
    }
    fn rings(&self, pivot: Vec3, scene: &Scene) -> Vec<Ring> {
        let mut rings: Vec<_> = (0..3)
            .map(|i| Ring {
                axis: axis(i),
                pivot,
                color: controls::AXES[i],
                label: ["Rotate X", "Rotate Y", "Rotate Z"][i],
                radius: 75.,
            })
            .collect();
        rings.push(Ring {
            axis: scene.view,
            pivot,
            color: controls::NEUTRAL,
            label: "Rotate view",
            radius: 100.,
        });
        rings
    }
    fn handles(
        &self,
        pivot: Vec3,
        scene: &Scene,
    ) -> Vec<(Handle, Pos2, &'static str, egui::Color32)> {
        let center = scene.project(pivot);
        match self.operation {
            Operation::Move => {
                let mut handles =
                    vec![(Handle::Move(None), center, "Move path", controls::NEUTRAL)];
                for i in 0..3 {
                    let direction = scene.axis_direction(pivot, axis(i));
                    if direction.length() > 0.1 {
                        handles.push((
                            Handle::Move(Some(axis(i))),
                            center + direction.normalized() * 85.,
                            ["Move X", "Move Y", "Move Z"][i],
                            controls::AXES[i],
                        ));
                    }
                }
                handles
            }
            Operation::Scale => vec![(
                Handle::Scale,
                center + Vec2::new(72., -72.),
                "Scale path",
                controls::ACTION,
            )],
            Operation::Rotate => Vec::new(),
        }
    }
    pub(super) fn press(&mut self, path: &SurfacePath, scene: &Scene, pointer: Pos2) -> bool {
        if path.mode != PathMode::Free || path.points.len() < 2 {
            return false;
        }
        let pivot = self.origin(path);
        if !scene.visible_depth(pivot) {
            return false;
        }
        let gesture = if self.operation == Operation::Rotate {
            self.rings(pivot, scene)
                .into_iter()
                .filter_map(|ring| {
                    let distance = ring.nearest(scene, pointer).0;
                    (distance <= controls::HIT_RADIUS).then_some((distance, ring))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, ring)| Gesture::Rotate(ring.begin(scene, pointer)))
        } else {
            self.handles(pivot, scene)
                .into_iter()
                .filter(|(_, p, _, _)| p.distance(pointer) <= controls::HIT_RADIUS)
                .min_by(|a, b| {
                    a.1.distance_sq(pointer)
                        .total_cmp(&b.1.distance_sq(pointer))
                })
                .map(|(handle, _, _, _)| match handle {
                    Handle::Move(axis) => Gesture::Move(axis),
                    Handle::Scale => Gesture::Scale,
                })
        };
        if let Some(gesture) = gesture {
            self.drag = Some(Drag {
                before: path.clone(),
                pending: self.pending_section_scale,
                pivot,
                pointer,
                gesture,
                scale_sections: self.scale_sections,
            });
            true
        } else {
            false
        }
    }
    pub(super) fn update(
        &mut self,
        path: &mut SurfacePath,
        scene: &Scene,
        pointer: Pos2,
    ) -> Result<(), String> {
        let Some(drag) = &mut self.drag else {
            return Ok(());
        };
        let (mut translation, mut rotation_axis, mut angle, mut scale) =
            (Vec3::default(), axis(2), 0., 1.);
        match &mut drag.gesture {
            Gesture::Move(None) => {
                translation =
                    scene.at_depth(pointer, drag.pivot) - scene.at_depth(drag.pointer, drag.pivot)
            }
            Gesture::Move(Some(axis)) => {
                let direction = scene.axis_direction(drag.pivot, *axis);
                let amount = scene
                    .axis_parameter(pointer, drag.pivot, *axis)
                    .zip(scene.axis_parameter(drag.pointer, drag.pivot, *axis))
                    .map_or_else(
                        || {
                            (pointer - drag.pointer).dot(direction)
                                / (direction.length_sq() * scene.scale_at(drag.pivot)).max(1e-6)
                        },
                        |(a, b)| a - b,
                    );
                translation = *axis * amount;
            }
            Gesture::Rotate(rotation) => {
                rotation.advance(scene, pointer);
                rotation_axis = rotation.axis;
                angle = rotation.angle;
            }
            Gesture::Scale => {
                scale = (1. + (pointer - drag.pointer).dot(Vec2::new(1., -1.).normalized()) / 100.)
                    .clamp(0.001, 1000.)
            }
        }
        let next = drag
            .before
            .transformed(drag.pivot, translation, rotation_axis, angle, scale)?;
        *path = next;
        self.pending_section_scale = if drag.scale_sections {
            drag.pending * scale
        } else {
            drag.pending
        };
        Ok(())
    }
    pub(super) fn cancel(&mut self, path: &mut SurfacePath) {
        if let Some(drag) = self.drag.take() {
            *path = drag.before;
            self.pending_section_scale = drag.pending;
        }
    }
    pub(super) fn finish(&mut self) {
        self.drag = None;
    }
    pub(super) fn paint(
        &self,
        path: &SurfacePath,
        scene: &Scene,
        painter: &egui::Painter,
        pointer: Option<Pos2>,
    ) {
        if path.mode != PathMode::Free || path.points.len() < 2 {
            return;
        }
        let pivot = self
            .drag
            .as_ref()
            .map_or_else(|| self.origin(path), |d| d.pivot);
        if !scene.visible_depth(pivot) {
            return;
        }
        let center = scene.project(pivot);
        if self.operation == Operation::Rotate {
            for ring in self.rings(pivot, scene) {
                ring.paint(
                    scene,
                    painter,
                    pointer.is_some_and(|p| ring.nearest(scene, p).0 <= controls::HIT_RADIUS),
                );
            }
        } else {
            for (handle, point, label, color) in self.handles(pivot, scene) {
                painter.line_segment([center, point], egui::Stroke::new(1.5_f32, color));
                controls::marker(
                    painter,
                    point,
                    if matches!(handle, Handle::Scale) {
                        controls::Marker::Square
                    } else {
                        controls::Marker::Circle
                    },
                    color,
                    pointer.is_some_and(|p| p.distance(point) <= controls::HIT_RADIUS),
                );
                controls::label(painter, point + Vec2::new(10., -20.), label, color);
            }
        }
        #[cfg(test)]
        painter.ctx().data_mut(|d| {
            d.insert_temp(
                egui::Id::new("whole_path_transform_controls"),
                (
                    center,
                    self.handles(pivot, scene)
                        .into_iter()
                        .map(|(_, p, label, _)| (label.to_owned(), p))
                        .collect::<Vec<_>>(),
                ),
            )
        });
    }
}
fn axis(i: usize) -> Vec3 {
    match i {
        0 => Vec3(1., 0., 0.),
        1 => Vec3(0., 1., 0.),
        _ => Vec3(0., 0., 1.),
    }
}

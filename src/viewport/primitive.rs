//! Direct manipulation of parametric shape controls.
use super::{controls, scene::Scene};
use crate::{
    document::Document,
    sculpt::{Primitive, PrimitiveKind as Kind, Vec3},
};
use eframe::egui::{self, Pos2, Vec2};
#[derive(Clone, Copy, PartialEq)]
enum Control {
    Dimension(usize),
    Uniform,
    Radius,
    TopRadius,
    TubeRadius,
    Resolution(Resolution),
}
#[derive(Clone, Copy, PartialEq)]
enum Resolution {
    Spans(usize),
    Segments,
    Rings,
    Subdivisions,
}
impl Resolution {
    fn value(self, p: &Primitive) -> u32 {
        match self {
            Self::Spans(i) => p.divisions[i],
            Self::Segments => p.segments,
            Self::Rings => p.rings,
            Self::Subdivisions => p.subdivisions,
        }
    }
    fn limits(self, p: &Primitive) -> (u32, u32) {
        match self {
            Self::Spans(_) => (1, 128),
            Self::Segments => (3, 256),
            Self::Rings => (if p.kind == Kind::Torus { 3 } else { 2 }, 128),
            Self::Subdivisions => (1, 6),
        }
    }
    fn set(self, p: &mut Primitive, value: u32) {
        match self {
            Self::Spans(i) => p.divisions[i] = value,
            Self::Segments => p.segments = value,
            Self::Rings => p.rings = value,
            Self::Subdivisions => p.subdivisions = value,
        }
    }
}
fn screen_point(h: Handle, scene: &Scene) -> Pos2 {
    if matches!(h.control, Control::Resolution(_)) {
        let anchor = scene.active_bounds.right_top();
        Pos2::new(
            (anchor.x + 24.).clamp(
                scene.rect.left() + 20.,
                (scene.rect.right() - 155.).max(scene.rect.left() + 20.),
            ),
            (anchor.y + 16. + h.offset.y).clamp(
                scene.rect.top() + 20.,
                (scene.rect.bottom() - 90. + h.offset.y).max(scene.rect.top() + 20.),
            ),
        )
    } else {
        scene.project(h.point) + h.offset
    }
}
#[derive(Clone, Copy)]
struct Handle {
    point: Vec3,
    axis: Vec3,
    control: Control,
    label: &'static str,
    offset: Vec2,
}
#[derive(Default)]
pub(super) struct PrimitiveControls {
    drag: Option<Handle>,
    last: Option<Pos2>,
    span_remainder: f32,
}
fn handles(p: &Primitive) -> Vec<Handle> {
    let mut result = Vec::new();
    let mut add = |local: Vec3, axis: Vec3, control, label, offset| {
        let world_axis = p.basis[0] * axis.0 + p.basis[1] * axis.1 + p.basis[2] * axis.2;
        result.push(Handle {
            point: p.control_point(local),
            axis: world_axis,
            control,
            label,
            offset,
        });
    };
    match p.kind {
        Kind::Circle => add(
            Vec3(p.radius, 0., 0.),
            Vec3(1., 0., 0.),
            Control::Radius,
            "Radius",
            Vec2::ZERO,
        ),
        Kind::Cylinder | Kind::Cone => {
            add(
                Vec3(p.radius, -1., 0.),
                Vec3(1., 0., 0.),
                Control::Radius,
                "Base radius",
                Vec2::ZERO,
            );
            add(
                Vec3(0., 1., 0.),
                Vec3(0., 1., 0.),
                Control::Dimension(1),
                "Height",
                Vec2::new(-16., 0.),
            );
            if p.kind == Kind::Cone {
                add(
                    Vec3(p.top_radius, 1., 0.),
                    Vec3(1., 0., 0.),
                    Control::TopRadius,
                    "Top radius",
                    Vec2::new(16., 0.),
                );
            }
        }
        Kind::Torus => {
            add(
                Vec3(p.radius + p.tube_radius, 0., 0.),
                Vec3(1., 0., 0.),
                Control::Radius,
                "Ring radius",
                Vec2::ZERO,
            );
            add(
                Vec3(p.radius, p.tube_radius, 0.),
                Vec3(0., 1., 0.),
                Control::TubeRadius,
                "Tube",
                Vec2::ZERO,
            );
        }
        _ => {
            add(
                Vec3(-1., 0., 0.),
                Vec3(-1., 0., 0.),
                Control::Dimension(0),
                "Width",
                Vec2::ZERO,
            );
            add(
                Vec3(0., 1., 0.),
                Vec3(0., 1., 0.),
                Control::Dimension(1),
                "Height",
                Vec2::ZERO,
            );
            if !matches!(p.kind, Kind::Plane | Kind::Grid) {
                add(
                    Vec3(0., 0., 1.),
                    Vec3(0., 0., 1.),
                    Control::Dimension(2),
                    "Depth",
                    Vec2::ZERO,
                );
            }
            if matches!(p.kind, Kind::UvSphere | Kind::Icosphere | Kind::Cube) {
                add(
                    Vec3(1., 1., 0.).norm(),
                    Vec3(1., 1., 0.).norm(),
                    Control::Uniform,
                    "Size",
                    Vec2::ZERO,
                );
            }
        }
    }

    let resolution: Vec<(Resolution, &str)> = match p.kind {
        Kind::Cube => vec![
            (Resolution::Spans(0), "X spans"),
            (Resolution::Spans(1), "Y spans"),
            (Resolution::Spans(2), "Z spans"),
        ],
        Kind::Plane | Kind::Grid => vec![
            (Resolution::Spans(0), "X spans"),
            (Resolution::Spans(1), "Y spans"),
        ],
        Kind::Cylinder | Kind::Cone => vec![
            (Resolution::Segments, "Sides"),
            (Resolution::Spans(1), "Height spans"),
        ],
        Kind::Circle => vec![(Resolution::Segments, "Vertices")],
        Kind::UvSphere => vec![
            (Resolution::Segments, "Segments"),
            (Resolution::Rings, "Rings"),
        ],
        Kind::Torus => vec![
            (Resolution::Segments, "Ring spans"),
            (Resolution::Rings, "Tube spans"),
        ],
        Kind::Icosphere => vec![(Resolution::Subdivisions, "Subdivisions")],
    };
    for (row, (control, label)) in resolution.into_iter().enumerate() {
        add(
            Vec3(1., 1., 0.),
            Vec3(1., 0., 0.),
            Control::Resolution(control),
            label,
            Vec2::new(0., row as f32 * 28.),
        );
    }
    result
}
impl PrimitiveControls {
    fn hit(&self, document: &Document, scene: &Scene, point: Pos2) -> Option<Handle> {
        handles(document.primitive()?)
            .into_iter()
            .filter(|h| screen_point(*h, scene).distance(point) <= controls::HIT_RADIUS)
            .min_by(|a, b| {
                screen_point(*a, scene)
                    .distance(point)
                    .total_cmp(&screen_point(*b, scene).distance(point))
            })
    }
    pub fn can_drag(&self, document: &Document, scene: &Scene, point: Pos2) -> bool {
        document.active_visible() && self.hit(document, scene, point).is_some()
    }
    pub fn update(
        &mut self,
        document: &mut Document,
        scene: &Scene,
        pointer: Option<Pos2>,
        dragging: bool,
    ) {
        if !dragging {
            if self.drag.take().is_some() {
                document.end_primitive_edit();
            }
            self.last = None;
            self.span_remainder = 0.;
            return;
        }
        let Some(point) = pointer else {
            return;
        };
        if self.drag.is_none() {
            self.drag = self.hit(document, scene, point);
            self.last = Some(point);
            return;
        }
        let h = self.drag.unwrap();
        let delta = point - self.last.unwrap_or(point);
        self.last = Some(point);
        if delta.length_sq() == 0. {
            return;
        }
        let Some(mut p) = document.primitive().cloned() else {
            return;
        };
        let length = h.axis.len().max(0.001);
        let axis = h.axis.norm();
        let projected = scene.axis_direction(h.point, axis);
        let amount = if projected.length_sq() > 0.04 {
            delta.dot(projected) / projected.length_sq()
        } else {
            delta.x
        };
        let world = amount / scene.scale_at(h.point) / length;
        match h.control {
            Control::Resolution(control) => {
                self.span_remainder += delta.x / 12.;
                let steps = self.span_remainder.trunc() as i32;
                if steps == 0 {
                    return;
                }
                self.span_remainder -= steps as f32;
                let (min, max) = control.limits(&p);
                let value = (control.value(&p) as i32 + steps).clamp(min as i32, max as i32) as u32;
                control.set(&mut p, value);
            }
            Control::Dimension(index) => {
                let value = match index {
                    0 => &mut p.size.0,
                    1 => &mut p.size.1,
                    _ => &mut p.size.2,
                };
                *value = (*value + world * 2.).clamp(0.05, 100.);
            }
            Control::Uniform => {
                p.size = p.size * (amount * 0.01).clamp(-0.5, 0.5).exp();
                p.size = Vec3(
                    p.size.0.clamp(0.05, 100.),
                    p.size.1.clamp(0.05, 100.),
                    p.size.2.clamp(0.05, 100.),
                );
            }
            Control::Radius => {
                p.radius = (p.radius + world).clamp(0.05, 50.);
                if p.kind == Kind::Torus {
                    p.tube_radius = p.tube_radius.min(p.radius - 0.01);
                }
            }
            Control::TopRadius => p.top_radius = (p.top_radius + world).clamp(0., 50.),
            Control::TubeRadius => {
                p.tube_radius = (p.tube_radius + world).clamp(0.01, (p.radius - 0.01).min(25.))
            }
        }
        let _ = document.update_primitive(p);
    }
    pub fn paint(
        &self,
        document: &Document,
        scene: &Scene,
        painter: &egui::Painter,
        pointer: Option<Pos2>,
    ) {
        let Some(params) = document.primitive().filter(|_| document.active_visible()) else {
            return;
        };
        let hovered = pointer.and_then(|p| self.hit(document, scene, p));
        for h in handles(params) {
            let anchor = scene.project(h.point);
            let point = screen_point(h, scene);
            let hot = hovered.is_some_and(|hover| hover.control == h.control);
            let active = self.drag.is_some_and(|drag| drag.control == h.control);
            let (color, marker) = match h.control {
                Control::Resolution(Resolution::Spans(axis)) => {
                    (controls::AXES[axis], controls::Marker::Diamond)
                }
                Control::Dimension(axis) => (controls::AXES[axis], controls::Marker::Square),
                Control::Resolution(_) => (controls::TOPOLOGY, controls::Marker::Diamond),
                Control::Uniform => (controls::SHAPE, controls::Marker::Square),
                _ => (controls::SHAPE, controls::Marker::Circle),
            };
            if h.offset != Vec2::ZERO && !matches!(h.control, Control::Resolution(_)) {
                painter.line_segment(
                    [anchor, point],
                    egui::Stroke::new(1_f32, color.gamma_multiply(0.6)),
                );
            }
            controls::marker(painter, point, marker, color, hot || active);
            if let Control::Resolution(control) = h.control {
                controls::label(
                    painter,
                    point + Vec2::new(14., -7.),
                    &format!("{} · {}", h.label, control.value(params)),
                    color,
                );
            }
            if hot || active {
                let axis = h.axis.norm();
                let projected = if matches!(h.control, Control::Resolution(_)) {
                    Vec2::X
                } else {
                    scene.axis_direction(h.point, axis)
                };
                let direction = if projected.length_sq() > 0.04 {
                    projected.normalized()
                } else {
                    Vec2::X
                };
                painter.line_segment(
                    [point - direction * 28., point - direction * 12.],
                    egui::Stroke::new(1_f32, color),
                );
                if !matches!(h.control, Control::Resolution(_)) {
                    painter.line_segment(
                        [point + direction * 12., point + direction * 28.],
                        egui::Stroke::new(1_f32, color),
                    );
                }
                let caption = if matches!(h.control, Control::Resolution(_)) {
                    "Drag left / right · 12 px per span".to_owned()
                } else {
                    h.label.to_owned()
                };
                let caption_point = if matches!(h.control, Control::Resolution(_)) {
                    point + Vec2::new(14., -30. - h.offset.y)
                } else {
                    point + Vec2::new(14., -30.)
                };
                controls::label(painter, caption_point, &caption, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;
    fn scene(document: &Document) -> Scene {
        let ctx = egui::Context::default();
        let mut result = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            result = Some(Scene::paint(
                &painter,
                egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(800.)),
                document,
                &Camera::default(),
                false,
                crate::viewport::WireMode::Visible,
                false,
            ));
        });
        result.unwrap()
    }
    #[test]
    fn every_primitive_has_resolution_handles_that_preserve_shape_and_undo() {
        for kind in Kind::ALL {
            let mut params = Primitive::for_kind(kind);
            params.segments = 24;
            params.rings = 16;
            let expected = match kind {
                Kind::Cube => 3,
                Kind::Circle | Kind::Icosphere => 1,
                _ => 2,
            };
            let resolution: Vec<_> = handles(&params)
                .into_iter()
                .filter(|h| matches!(h.control, Control::Resolution(_)))
                .collect();
            assert_eq!(resolution.len(), expected, "{}", kind.label());
            for handle in resolution {
                let Control::Resolution(control) = handle.control else {
                    unreachable!()
                };
                let mut document = Document::default();
                document.initialize_primitive().unwrap();
                document.update_primitive(params.clone()).unwrap();
                document.end_primitive_edit();
                let before = document.geometry_snapshot();
                let history = document.history_counts().0;
                let view = scene(&document);
                let point = screen_point(handle, &view);
                let mut controls = PrimitiveControls::default();
                assert!(controls.can_drag(&document, &view, point));
                assert!(
                    controls
                        .hit(&document, &view, point)
                        .is_some_and(|h| h.control == handle.control)
                );
                controls.update(&mut document, &view, Some(point), true);
                for step in 1..=6 {
                    controls.update(
                        &mut document,
                        &view,
                        Some(point + Vec2::new(step as f32 * 4., 0.)),
                        true,
                    );
                }
                controls.update(&mut document, &view, None, false);
                let mut expected = params.clone();
                let (_, max) = control.limits(&params);
                control.set(&mut expected, (control.value(&params) + 2).min(max));
                assert_eq!(
                    document.primitive(),
                    Some(&expected),
                    "{} {}",
                    kind.label(),
                    handle.label
                );
                assert_eq!(document.history_counts().0, history + 1);
                document.mesh_settings().validate(document.mesh()).unwrap();
                document.undo();
                assert_eq!(document.geometry_snapshot(), before);
            }
        }
    }
    #[test]
    fn spans_accumulate_small_drags_reverse_and_clamp_without_empty_undo() {
        let mut params = Primitive::for_kind(Kind::Plane);
        params.divisions = [2; 3];
        let mut document = Document::default();
        document.initialize_primitive().unwrap();
        document.update_primitive(params).unwrap();
        document.end_primitive_edit();
        let view = scene(&document);
        let handle = handles(document.primitive().unwrap())
            .into_iter()
            .find(|h| h.control == Control::Resolution(Resolution::Spans(0)))
            .unwrap();
        let point = screen_point(handle, &view);
        let mut controls = PrimitiveControls::default();
        let history = document.history_counts().0;
        controls.update(&mut document, &view, Some(point), true);
        controls.update(&mut document, &view, Some(point + Vec2::new(4., 0.)), true);
        controls.update(&mut document, &view, None, false);
        assert_eq!(document.history_counts().0, history);
        controls.update(&mut document, &view, Some(point), true);
        controls.update(&mut document, &view, Some(point - Vec2::new(24., 0.)), true);
        assert_eq!(document.primitive().unwrap().divisions[0], 1);
        controls.update(&mut document, &view, Some(point - Vec2::new(12., 0.)), true);
        assert_eq!(document.primitive().unwrap().divisions[0], 2);
        controls.update(
            &mut document,
            &view,
            Some(point + Vec2::new(4000., 0.)),
            true,
        );
        assert_eq!(document.primitive().unwrap().divisions[0], 128);
        controls.update(&mut document, &view, None, false);
        assert_eq!(document.history_counts().0, history + 1);
        document.undo();
        assert_eq!(document.primitive().unwrap().divisions[0], 2);
    }
}

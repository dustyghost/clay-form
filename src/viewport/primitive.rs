//! Direct manipulation of parametric shape and facial controls.
use super::scene::Scene;
use crate::{
    document::Document,
    sculpt::{Primitive, PrimitiveKind as Kind, Vec3},
};
use eframe::egui::{self, Color32, Pos2, Vec2};
#[derive(Clone, Copy)]
enum Control {
    Dimension(usize),
    Uniform,
    Radius,
    TopRadius,
    TubeRadius,
    Face(usize),
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
}
fn handles(p: &Primitive) -> Vec<Handle> {
    let mut result = Vec::new();
    let mut add = |local: Vec3, axis: Vec3, control, label, offset| {
        let world_axis = p.basis[0] * axis.0 + p.basis[1] * axis.1 + p.basis[2] * axis.2;
        result.push(Handle {
            point: if matches!(control, Control::Face(_)) && p.head.anatomy_version > 0 {
                if matches!(control,Control::Face(index) if index>=17) {
                    p.center
                        + p.basis[0] * (local.0 * p.size.0 * 0.5)
                        + p.basis[1] * (local.1 * p.size.1 * 0.5)
                        + p.basis[2] * (local.2 * p.size.2 * 0.5)
                } else {
                    p.facial_control_point(local.0, local.1)
                }
            } else {
                p.control_point(local)
            },
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
            if !matches!(p.kind, Kind::Plane | Kind::Grid | Kind::HumanHead) {
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
    if p.kind == Kind::HumanHead {
        add(
            Vec3(0., -0.75, -0.65).norm(),
            Vec3(0., 0., 1.),
            Control::Dimension(2),
            "Head depth",
            Vec2::new(24., 24.),
        );
        let new = p.head.anatomy_version > 0;
        let ex = if new {
            0.29 + (p.head.eye_spacing - 0.5) * 0.11
        } else {
            0.26 + p.head.eye_spacing * 0.16
        };
        let ey = if new {
            0.035 + (p.head.eye_height - 0.5) * 0.12
        } else {
            0.12 + p.head.eye_height * 0.12
        };
        let my = if p.head.anatomy_version >= 2 {
            -0.54
        } else if new {
            -0.48
        } else {
            -0.36
        };
        for (point, axis, index, label, offset) in [
            (
                Vec3(-0.55, -0.65, 1.),
                Vec3(-1., 0., 0.),
                0,
                "Jaw width",
                Vec2::ZERO,
            ),
            (
                Vec3(0.43, -0.80, 1.),
                Vec3(0., -1., 0.),
                1,
                "Jaw length",
                Vec2::new(15., 15.),
            ),
            (Vec3(0., -0.78, 1.), Vec3(0., 0., 1.), 2, "Chin", Vec2::ZERO),
            (
                Vec3(0.49, -0.14, 1.),
                Vec3(1., 0., 0.),
                3,
                "Cheeks",
                Vec2::ZERO,
            ),
            (
                Vec3(0., if new { -0.205 } else { 0.04 }, 1.),
                Vec3(0., 0., 1.),
                4,
                "Nose",
                Vec2::new(-10., 0.),
            ),
            (
                Vec3(0.10, if new { -0.245 } else { 0.04 }, 1.),
                Vec3(1., 0., 0.),
                5,
                "Nose width",
                Vec2::new(14., 0.),
            ),
            (
                Vec3(ex, ey, 1.),
                Vec3(1., 0., 0.),
                6,
                "Eye spacing",
                Vec2::new(18., 0.),
            ),
            (
                Vec3(-ex, ey, 1.),
                Vec3(0., 0., 1.),
                7,
                "Eye depth",
                Vec2::new(-18., 0.),
            ),
            (
                Vec3(-ex, ey + 0.115, 1.),
                Vec3(0., 1., 0.),
                8,
                "Brow",
                Vec2::ZERO,
            ),
            (
                Vec3(0.84, -0.1, 1.),
                Vec3(1., 0., 0.),
                9,
                "Ears",
                Vec2::ZERO,
            ),
            (
                Vec3(ex + 0.12, ey, 1.),
                Vec3(1., 0., 0.),
                10,
                "Eye size",
                Vec2::new(22., -12.),
            ),
            (
                Vec3(-ex, ey, 1.),
                Vec3(0., 1., 0.),
                11,
                "Eye height",
                Vec2::new(-20., 18.),
            ),
            (
                Vec3(0.23, my, 1.),
                Vec3(1., 0., 0.),
                12,
                "Mouth width",
                Vec2::new(12., 0.),
            ),
            (
                Vec3(0., my - 0.055, 1.),
                Vec3(0., -1., 0.),
                13,
                "Open mouth",
                Vec2::new(0., 15.),
            ),
            (
                Vec3(-0.12, my + 0.035, 1.),
                Vec3(0., 0., 1.),
                14,
                "Lip fullness",
                Vec2::new(-12., 0.),
            ),
            (
                Vec3(ex, ey + 0.058 * (1. - p.head.upper_lid), 1.),
                Vec3(0., -1., 0.),
                15,
                "Upper eyelid",
                Vec2::new(0., -10.),
            ),
            (
                Vec3(ex, ey - 0.038 * (1. - p.head.lower_lid), 1.),
                Vec3(0., 1., 0.),
                16,
                "Lower eyelid",
                Vec2::new(0., 10.),
            ),
        ] {
            add(point, axis, Control::Face(index), label, offset);
        }
        if p.head.anatomy_version >= 2 && p.head.neck {
            add(
                Vec3(0., -0.91 - 0.40 - p.head.neck_length * 0.32, -0.24),
                Vec3(0., -1., 0.),
                Control::Face(17),
                "Neck length",
                Vec2::ZERO,
            );
            add(
                Vec3(0.44 + p.head.neck_width * 0.16, -1.20, -0.18),
                Vec3(1., 0., 0.),
                Control::Face(18),
                "Neck width",
                Vec2::ZERO,
            );
            add(
                Vec3(0.28, -1.18, 0.08),
                Vec3(1., 0., 0.),
                Control::Face(19),
                "Neck muscle",
                Vec2::new(15., 0.),
            );
            add(
                Vec3(0., -1.17, 0.13),
                Vec3(0., 0., 1.),
                Control::Face(20),
                "Throat",
                Vec2::new(-15., 0.),
            );
        }
    }
    result
}
impl PrimitiveControls {
    fn hit(&self, document: &Document, scene: &Scene, point: Pos2) -> Option<Handle> {
        handles(document.primitive()?)
            .into_iter()
            .filter(|h| (scene.project(h.point) + h.offset).distance(point) <= 10.)
            .min_by(|a, b| {
                (scene.project(a.point) + a.offset)
                    .distance(point)
                    .total_cmp(&(scene.project(b.point) + b.offset).distance(point))
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
        let projected = Vec2::new(axis.dot(scene.right), -axis.dot(scene.up));
        let amount = if projected.length_sq() > 0.04 {
            delta.dot(projected) / projected.length_sq()
        } else {
            delta.x
        };
        let world = amount / scene.scale / length;
        match h.control {
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
            Control::Face(index) => {
                let values = [
                    &mut p.head.jaw_width,
                    &mut p.head.jaw_height,
                    &mut p.head.chin,
                    &mut p.head.cheeks,
                    &mut p.head.nose_length,
                    &mut p.head.nose_width,
                    &mut p.head.eye_spacing,
                    &mut p.head.eye_depth,
                    &mut p.head.brow,
                    &mut p.head.ears,
                    &mut p.head.eye_size,
                    &mut p.head.eye_height,
                    &mut p.head.mouth_width,
                    &mut p.head.mouth_open,
                    &mut p.head.lip_size,
                    &mut p.head.upper_lid,
                    &mut p.head.lower_lid,
                    &mut p.head.neck_length,
                    &mut p.head.neck_width,
                    &mut p.head.neck_muscle,
                    &mut p.head.throat,
                ];
                let value = values.into_iter().nth(index).unwrap();
                *value = (*value + amount * 0.008)
                    .clamp(0., if (15..=16).contains(&index) { 1. } else { 2. });
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
        for h in handles(params) {
            let anchor = scene.project(h.point);
            let point = anchor + h.offset;
            let hovered = pointer.is_some_and(|p| p.distance(point) <= 10.);
            let active = self.drag.is_some_and(|drag| drag.label == h.label);
            let color = if hovered || active {
                Color32::WHITE
            } else {
                Color32::from_rgb(130, 235, 185)
            };
            if h.offset != Vec2::ZERO {
                painter.line_segment([anchor, point], egui::Stroke::new(1., color));
            }
            painter.circle_filled(point, 6., Color32::from_rgb(22, 43, 35));
            painter.circle_stroke(point, 6., egui::Stroke::new(2., color));
            if hovered || active {
                let axis = h.axis.norm();
                let projected = Vec2::new(axis.dot(scene.right), -axis.dot(scene.up));
                let direction = if projected.length_sq() > 0.04 {
                    projected.normalized()
                } else {
                    Vec2::X
                };
                painter.line_segment(
                    [point - direction * 28., point + direction * 28.],
                    egui::Stroke::new(1., color),
                );
                painter.text(
                    point + Vec2::new(12., -12.),
                    egui::Align2::LEFT_BOTTOM,
                    h.label,
                    egui::FontId::proportional(13.),
                    color,
                );
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
            ));
        });
        result.unwrap()
    }
    #[test]
    fn mouth_and_eyelid_handles_drag_and_restore_with_undo() {
        for (label, index) in [
            ("Open mouth", 13),
            ("Upper eyelid", 15),
            ("Lower eyelid", 16),
        ] {
            let mut document = Document::default();
            document.initialize_primitive().unwrap();
            document
                .update_primitive(Primitive::for_kind(Kind::HumanHead))
                .unwrap();
            document.end_primitive_edit();
            let before = document.geometry_snapshot();
            let view = scene(&document);
            let h = handles(document.primitive().unwrap())
                .into_iter()
                .find(|h| h.label == label)
                .unwrap();
            let point = view.project(h.point) + h.offset;
            let mut controls = PrimitiveControls::default();
            controls.update(&mut document, &view, Some(point), true);
            let direction = if index == 16 { -1. } else { 1. };
            controls.update(
                &mut document,
                &view,
                Some(point + Vec2::new(0., direction * 45.)),
                true,
            );
            controls.update(&mut document, &view, None, false);
            let head = &document.primitive().unwrap().head;
            let value = match index {
                13 => head.mouth_open,
                15 => head.upper_lid,
                _ => head.lower_lid,
            };
            assert!(value > 0., "{label} updates its parameter");
            document.undo();
            assert_eq!(document.geometry_snapshot(), before);
        }
    }
    #[test]
    fn facial_handle_drags_update_a_head_recipe_and_coalesce_history() {
        let mut document = Document::default();
        document.initialize_primitive().unwrap();
        document
            .update_primitive(Primitive::for_kind(Kind::HumanHead))
            .unwrap();
        document.end_primitive_edit();
        let before = document.geometry_snapshot();
        let count = document.history_counts();
        let view = scene(&document);
        let handle = handles(document.primitive().unwrap())
            .into_iter()
            .find(|h| h.label == "Nose")
            .unwrap();
        let point = view.project(handle.point) + handle.offset;
        let mut controls = PrimitiveControls::default();
        assert!(controls.can_drag(&document, &view, point));
        controls.update(&mut document, &view, Some(point), true);
        controls.update(&mut document, &view, Some(point + Vec2::new(25., 0.)), true);
        controls.update(
            &mut document,
            &view,
            Some(point + Vec2::new(140., 0.)),
            true,
        );
        controls.update(&mut document, &view, None, false);
        assert!(document.primitive().unwrap().head.nose_length > 1.);
        assert_eq!(document.history_counts().0, count.0 + 1);
        document.mesh_settings().validate(document.mesh()).unwrap();
        document.undo();
        assert_eq!(document.geometry_snapshot(), before);
    }
}

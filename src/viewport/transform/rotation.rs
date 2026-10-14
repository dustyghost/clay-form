//! Projected rotation rings shared by World axes and Action Line.
use super::{Scene, Vec3, controls};
use eframe::egui::{self, Color32, Pos2, Vec2};
use std::f32::consts::TAU;

pub(super) const RADIUS: f32 = 85.;
const SEGMENTS: usize = 96;

pub(super) struct Ring {
    pub axis: Vec3,
    pub pivot: Vec3,
    pub color: Color32,
    pub label: &'static str,
}
impl Ring {
    fn basis(&self) -> (Vec3, Vec3) {
        let helper = if self.axis.2.abs() < 0.9 {
            Vec3(0., 0., 1.)
        } else {
            Vec3(0., 1., 0.)
        };
        let u = helper.cross(self.axis).norm();
        (u, self.axis.cross(u))
    }
    fn samples(&self, scene: &Scene) -> Vec<(Pos2, Vec3)> {
        let (u, v) = self.basis();
        (0..=SEGMENTS)
            .map(|i| {
                let (s, c) = (i as f32 * TAU / SEGMENTS as f32).sin_cos();
                let radial = u * c + v * s;
                (
                    scene.project(self.pivot + radial * (RADIUS / scene.scale)),
                    radial,
                )
            })
            .collect()
    }
    /// Closest segment; frontmost wins where an edge-on ring overlaps itself.
    pub fn nearest(&self, scene: &Scene, pointer: Pos2) -> (f32, Vec3) {
        self.samples(scene)
            .windows(2)
            .map(|pair| {
                let (a, b) = (pair[0].0, pair[1].0);
                let ab = b - a;
                let t = ((pointer - a).dot(ab) / ab.length_sq().max(1e-8)).clamp(0., 1.);
                let radial = (pair[0].1 * (1. - t) + pair[1].1 * t).norm();
                (pointer.distance(a + ab * t), radial)
            })
            .min_by(|a, b| {
                // Near-coincident segments otherwise pick an arbitrary back half.
                if (a.0 - b.0).abs() < 0.01 {
                    b.1.dot(scene.view).total_cmp(&a.1.dot(scene.view))
                } else {
                    a.0.total_cmp(&b.0)
                }
            })
            .unwrap()
    }
    pub fn paint(&self, scene: &Scene, painter: &egui::Painter, highlighted: bool) {
        painter.add(egui::Shape::line(
            self.samples(scene).into_iter().map(|(p, _)| p).collect(),
            egui::Stroke::new(if highlighted { 3.5 } else { 2_f32 }, self.color),
        ));
        let (u, v) = self.basis();
        let radial = if self.axis.2.abs() > 0.9 {
            (u + v).norm()
        } else {
            u
        };
        let point = scene.project(self.pivot + radial * (RADIUS / scene.scale));
        controls::label(
            painter,
            point + Vec2::new(10., -18.),
            self.label,
            self.color,
        );
    }
    pub fn begin(&self, scene: &Scene, pointer: Pos2) -> RotationDrag {
        let radial = self.nearest(scene, pointer).1;
        let tangent = (self.axis.dot(scene.view).abs() < 0.18).then(|| {
            let t = self.axis.cross(radial);
            let projected = Vec2::new(t.dot(scene.right), -t.dot(scene.up));
            if projected.length_sq() > 0.01 {
                projected.normalized()
            } else {
                let projected_axis =
                    Vec2::new(self.axis.dot(scene.right), -self.axis.dot(scene.up));
                Vec2::new(-projected_axis.y, projected_axis.x).normalized()
            }
        });
        let mut drag = RotationDrag {
            axis: self.axis,
            pivot: self.pivot,
            radial,
            tangent,
            last: pointer,
            angle: 0.,
        };
        if tangent.is_none()
            && let Some(radial) = drag.projected_radial(scene, pointer)
        {
            drag.radial = radial;
        }
        drag
    }
}
pub(super) struct RotationDrag {
    pub axis: Vec3,
    pivot: Vec3,
    radial: Vec3,
    tangent: Option<Vec2>,
    last: Pos2,
    pub angle: f32,
}
impl RotationDrag {
    fn projected_radial(&self, scene: &Scene, pointer: Pos2) -> Option<Vec3> {
        let delta = pointer - scene.project(self.pivot);
        if delta.length_sq() < 16. {
            return None;
        }
        let p = scene.right * (delta.x / scene.scale) + scene.up * (-delta.y / scene.scale);
        let p = p - scene.view * (p.dot(self.axis) / scene.view.dot(self.axis));
        (p.len() > 1e-6).then(|| p.norm())
    }
    pub fn advance(&mut self, scene: &Scene, pointer: Pos2) -> f32 {
        let angle = if let Some(tangent) = self.tangent {
            // An edge-on ring has no stable ray/plane intersection. Use the
            // latched screen tangent so it remains usable from snapped views.
            (pointer - self.last).dot(tangent) / RADIUS
        } else if let Some(radial) = self.projected_radial(scene, pointer) {
            let angle = self
                .axis
                .dot(self.radial.cross(radial))
                .atan2(self.radial.dot(radial));
            self.radial = radial;
            angle
        } else {
            0.
        };
        self.last = pointer;
        self.angle += angle;
        angle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        camera::Camera,
        document::Document,
        viewport::{WireMode, scene::Scene},
    };
    fn scene(camera: Camera) -> Scene {
        let ctx = egui::Context::default();
        let mut scene = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            scene = Some(Scene::paint(
                &ctx.layer_painter(egui::LayerId::background()),
                egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(600.)),
                &Document::default(),
                &camera,
                false,
                WireMode::Visible,
                false,
            ));
        });
        scene.unwrap()
    }
    #[test]
    fn rotation_rings_track_signed_angles_across_wraps_for_all_axes_and_views() {
        for camera in [
            Camera {
                yaw: 0.7,
                pitch: 0.4,
                ..Default::default()
            },
            Camera {
                yaw: -0.9,
                pitch: -0.7,
                zoom: 1.7,
                pan: Vec2::new(25., -17.),
            },
        ] {
            let scene = scene(camera);
            for axis in [Vec3(1., 0., 0.), Vec3(0., 1., 0.), Vec3(0., 0., 1.)] {
                let ring = Ring {
                    axis,
                    pivot: Vec3(0.2, -0.1, 0.3),
                    color: Color32::WHITE,
                    label: "test",
                };
                let (u, v) = ring.basis();
                let point = |angle: f32| {
                    scene.project(
                        ring.pivot + (u * angle.cos() + v * angle.sin()) * (RADIUS / scene.scale),
                    )
                };
                let mut drag = ring.begin(&scene, point(0.3));
                assert!(drag.tangent.is_none());
                assert!(ring.nearest(&scene, point(0.3)).0 < 0.1);
                for i in 1..=48 {
                    drag.advance(&scene, point(0.3 + i as f32 * TAU / 24.));
                }
                assert!((drag.angle - 2. * TAU).abs() < 1e-4);
                for i in (0..48).rev() {
                    drag.advance(&scene, point(0.3 + i as f32 * TAU / 24.));
                }
                assert!(drag.angle.abs() < 1e-4);
            }
        }
    }
    #[test]
    fn edge_on_rotation_uses_a_stable_tangent_and_can_reverse() {
        let scene = scene(Camera::default());
        for axis in [Vec3(1., 0., 0.), Vec3(0., 1., 0.)] {
            let ring = Ring {
                axis,
                pivot: Vec3::default(),
                color: Color32::WHITE,
                label: "test",
            };
            let point = ring.samples(&scene)[10].0;
            let mut drag = ring.begin(&scene, point);
            let tangent = drag.tangent.unwrap();
            let step = drag.advance(&scene, point + tangent * RADIUS);
            assert!((step - 1.).abs() < 1e-5);
            drag.advance(&scene, point);
            assert!(drag.angle.abs() < 1e-5);
        }
    }
}

//! Orientation overlay and axis hit targets, projected from camera orientation.
use crate::{camera::Camera, sculpt::Vec3};
use eframe::egui::{self, Color32, Pos2, Vec2};
pub(super) fn draw_gizmo(
    camera: &Camera,
    painter: &egui::Painter,
    rect: egui::Rect,
    pointer: Option<Pos2>,
) {
    let center = rect.center();
    painter.rect_filled(rect, 14., Color32::from_rgb(32, 38, 47));
    painter.circle_stroke(
        center,
        54.,
        egui::Stroke::new(1_f32, Color32::from_gray(80)),
    );
    for Axis {
        position: pos,
        depth,
        label,
        color,
        ..
    } in axes(camera, center)
    {
        let color = if depth < -0.01 {
            color.gamma_multiply(0.5)
        } else {
            color
        };
        painter.line_segment([center, pos], egui::Stroke::new(2_f32, color));
        let hovered = pointer.is_some_and(|p| p.distance(pos) < 13.);
        painter.circle_filled(pos, if hovered { 14. } else { 12. }, color);
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.),
            Color32::from_rgb(15, 20, 25),
        );
    }
    painter.text(
        rect.center_bottom() - Vec2::new(0., 4.),
        egui::Align2::CENTER_BOTTOM,
        "Drag · click axis",
        egui::FontId::proportional(10.),
        Color32::LIGHT_GRAY,
    );
}

pub(super) struct Axis {
    pub position: Pos2,
    pub depth: f32,
    pub label: &'static str,
    pub color: Color32,
    pub yaw: f32,
    pub pitch: f32,
}
pub(super) fn axes(camera: &Camera, center: Pos2) -> Vec<Axis> {
    let (right, up, view) = camera.basis();
    let pi = std::f32::consts::PI;
    let half = std::f32::consts::FRAC_PI_2;
    let mut axes: Vec<_> = [
        (
            Vec3(1., 0., 0.),
            "X",
            Color32::from_rgb(240, 100, 100),
            half,
            0.,
        ),
        (
            Vec3(-1., 0., 0.),
            "−X",
            Color32::from_rgb(240, 100, 100),
            -half,
            0.,
        ),
        (
            Vec3(0., 1., 0.),
            "Y",
            Color32::from_rgb(130, 220, 130),
            0.,
            -half,
        ),
        (
            Vec3(0., -1., 0.),
            "−Y",
            Color32::from_rgb(130, 220, 130),
            0.,
            half,
        ),
        (
            Vec3(0., 0., 1.),
            "Z",
            Color32::from_rgb(100, 165, 250),
            0.,
            0.,
        ),
        (
            Vec3(0., 0., -1.),
            "−Z",
            Color32::from_rgb(100, 165, 250),
            pi,
            0.,
        ),
    ]
    .into_iter()
    .map(|(axis, label, color, yaw, pitch)| Axis {
        position: center + Vec2::new(axis.dot(right), -axis.dot(up)) * 42.,
        depth: axis.dot(view),
        label,
        color,
        yaw,
        pitch,
    })
    .collect();
    axes.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    axes
}

//! Shared visual language for controls drawn over the model.
use eframe::egui::{self, Color32, Pos2, Vec2};
pub(super) const SHAPE: Color32 = Color32::from_rgb(91, 218, 196);
pub(super) const TOPOLOGY: Color32 = Color32::from_rgb(194, 161, 255);
pub(super) const ACTION: Color32 = Color32::from_rgb(247, 193, 95);
pub(super) const NEUTRAL: Color32 = Color32::from_rgb(223, 230, 239);
pub(super) const AXES: [Color32; 3] = [
    Color32::from_rgb(245, 113, 127),
    Color32::from_rgb(135, 220, 151),
    Color32::from_rgb(113, 177, 255),
];
pub(super) const HIT_RADIUS: f32 = 12.;
const BACKGROUND: Color32 = Color32::from_rgb(22, 28, 37);
#[derive(Clone, Copy)]
pub(super) enum Marker {
    Circle,
    Square,
    Diamond,
}
pub(super) fn marker(
    painter: &egui::Painter,
    point: Pos2,
    kind: Marker,
    color: Color32,
    highlighted: bool,
) {
    let radius = if highlighted { 8. } else { 6. };
    if highlighted {
        painter.circle_stroke(point, 11., egui::Stroke::new(1.5_f32, Color32::WHITE));
    }
    let stroke = egui::Stroke::new(2_f32, color);
    match kind {
        Marker::Circle => {
            painter.circle_filled(point, radius, BACKGROUND);
            painter.circle_stroke(point, radius, stroke);
        }
        Marker::Square => {
            let rect = egui::Rect::from_center_size(point, Vec2::splat(radius * 2.));
            painter.rect_filled(rect, 2., BACKGROUND);
            painter.rect_stroke(rect, 2., stroke, egui::StrokeKind::Middle);
        }
        Marker::Diamond => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    point + Vec2::new(0., -radius - 1.),
                    point + Vec2::new(radius + 1., 0.),
                    point + Vec2::new(0., radius + 1.),
                    point + Vec2::new(-radius - 1., 0.),
                ],
                BACKGROUND,
                stroke,
            ));
        }
    }
}
pub(super) fn label(painter: &egui::Painter, point: Pos2, text: &str, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.), color);
    let mut rect = egui::Rect::from_min_size(point, galley.size()).expand2(Vec2::new(5., 3.));
    let clip = painter.clip_rect().shrink(5.);
    let shift = Vec2::new(
        (clip.left() - rect.left()).max(0.) - (rect.right() - clip.right()).max(0.),
        (clip.top() - rect.top()).max(0.) - (rect.bottom() - clip.bottom()).max(0.),
    );
    rect = rect.translate(shift);
    painter.rect_filled(rect, 3., BACKGROUND);
    painter.galley(point + shift, galley, color);
}

//! A reserved strip inside the viewport remains reachable when clay fills the view.
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind};

const WIDTH: f32 = 20.;

pub(super) fn inner(rect: Rect) -> Rect {
    rect.shrink(WIDTH.min(rect.width().min(rect.height()) * 0.25))
}
pub(super) fn contains(rect: Rect, point: Pos2) -> bool {
    rect.contains(point) && !inner(rect).contains(point)
}
pub(super) fn paint(painter: &egui::Painter, rect: Rect, hovered: bool) {
    let inside = inner(rect);
    let fill = Color32::from_black_alpha(if hovered { 65 } else { 35 });
    for band in [
        Rect::from_min_max(rect.min, egui::pos2(rect.right(), inside.top())),
        Rect::from_min_max(egui::pos2(rect.left(), inside.bottom()), rect.max),
        Rect::from_min_max(egui::pos2(rect.left(), inside.top()), inside.left_bottom()),
        Rect::from_min_max(
            inside.right_top(),
            egui::pos2(rect.right(), inside.bottom()),
        ),
    ] {
        painter.rect_filled(band, 0., fill);
    }
    painter.rect_stroke(
        inside,
        0.,
        Stroke::new(
            1_f32,
            if hovered {
                Color32::from_rgb(125, 210, 245)
            } else {
                Color32::from_white_alpha(65)
            },
        ),
        StrokeKind::Outside,
    );
}

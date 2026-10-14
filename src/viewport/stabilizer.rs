//! Screen-space trailing brush. Distance, rather than time, controls smoothing.
use eframe::egui::{self, Color32, Painter, Pos2};

pub(super) struct Stabilizer {
    position: Pos2,
    distance: f32,
}
impl Stabilizer {
    pub fn new(position: Pos2, distance: f32) -> Self {
        Self { position, distance }
    }
    pub fn follow(&mut self, pointer: Pos2) -> Pos2 {
        let delta = pointer - self.position;
        let length = delta.length();
        if length > self.distance {
            self.position += delta * ((length - self.distance) / length);
        }
        self.position
    }
    pub fn paint(&self, painter: &Painter, pointer: Pos2, color: Color32) {
        painter.line_segment([self.position, pointer], egui::Stroke::new(1_f32, color));
        painter.circle_filled(self.position, 2.5, color);
        painter.circle_stroke(pointer, 3., egui::Stroke::new(1_f32, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_distance_absorbs_jitter_and_does_not_catch_up_while_stationary() {
        let mut stabilizer = Stabilizer::new(Pos2::ZERO, 30.);
        for point in [Pos2::new(8., 5.), Pos2::new(-4., 12.), Pos2::ZERO] {
            assert_eq!(stabilizer.follow(point), Pos2::ZERO);
        }
        let end = Pos2::new(100., 0.);
        assert_eq!(stabilizer.follow(end), Pos2::new(70., 0.));
        for _ in 0..100 {
            assert_eq!(stabilizer.follow(end), Pos2::new(70., 0.));
        }
        // A reversal takes up the slack instead of making a sharp hook.
        assert_eq!(stabilizer.follow(Pos2::new(50., 0.)), Pos2::new(70., 0.));
        assert_eq!(stabilizer.follow(Pos2::new(20., 0.)), Pos2::new(50., 0.));
        let point = stabilizer.follow(Pos2::new(50., 60.));
        assert_eq!(point, Pos2::new(50., 30.));
    }
}

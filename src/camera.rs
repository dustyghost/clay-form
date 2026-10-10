//! Session-only camera state. Camera changes never modify the sculpt document.
use crate::sculpt::Vec3;
use eframe::egui::Vec2;
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: Vec2,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.,
            pitch: 0.,
            zoom: 1.,
            pan: Vec2::ZERO,
        }
    }
}
impl Camera {
    pub fn zoom_by(&mut self, scroll: f32) {
        if scroll.is_finite() {
            self.set_zoom(self.zoom * (scroll * 0.002).clamp(-20., 20.).exp());
        }
    }
    pub fn set_zoom(&mut self, zoom: f32) {
        if zoom.is_finite() {
            self.zoom = zoom.clamp(0.2, 8.);
        }
    }
    pub fn zoom_step(&mut self, inward: bool) {
        self.set_zoom(self.zoom * if inward { 1.2 } else { 1. / 1.2 });
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let (s, c) = self.yaw.sin_cos();
        let (p, q) = self.pitch.sin_cos();
        (
            Vec3(c, 0., -s),
            Vec3(s * p, q, c * p),
            Vec3(s * q, -p, c * q),
        )
    }
    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * 0.008;
        self.pitch = (self.pitch - delta.y * 0.008)
            .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    }
}

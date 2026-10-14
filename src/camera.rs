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
    pub fn zoom_drag(&mut self, delta: Vec2) {
        self.zoom_by(-delta.y * 3.);
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
        if !delta.is_finite() {
            return;
        }
        // Wrap complete turns for precision, without imposing a pole limit.
        let wrap = |angle: f32| angle % std::f32::consts::TAU;
        self.yaw = wrap(self.yaw - delta.x * 0.008);
        self.pitch = wrap(self.pitch - delta.y * 0.008);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vertical_orbit_continues_over_poles_and_through_full_turns() {
        let mut camera = Camera::default();
        camera.orbit(Vec2::new(0., 250.));
        assert!(camera.pitch.abs() > std::f32::consts::FRAC_PI_2);
        let before = camera.basis().2;
        camera.orbit(Vec2::new(0., 100.));
        assert!((camera.basis().2 - before).len() > 0.5);
        let basis = camera.basis();
        for _ in 0..20 {
            camera.orbit(Vec2::new(
                std::f32::consts::TAU / 0.008,
                std::f32::consts::TAU / 0.008,
            ));
        }
        for (a, b) in [
            (basis.0, camera.basis().0),
            (basis.1, camera.basis().1),
            (basis.2, camera.basis().2),
        ] {
            assert!((a - b).len() < 1e-4);
        }
        assert!(camera.pitch.abs() <= std::f32::consts::TAU);
        assert!(camera.yaw.abs() <= std::f32::consts::TAU);
    }
}

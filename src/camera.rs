//! Session-only camera state. Camera changes never modify the sculpt document.
use crate::sculpt::Vec3;
use eframe::egui::Vec2;
#[derive(Clone, Copy)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    /// Clockwise screen-space rotation, in radians.
    pub roll: f32,
    pub zoom: f32,
    pub pan: Vec2,
    pub perspective: bool,
    pub field_of_view: f32,
    pub perspective_radius: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.,
            pitch: 0.,
            roll: 0.,
            zoom: 1.,
            pan: Vec2::ZERO,
            perspective: false,
            field_of_view: 45.,
            perspective_radius: 1.,
        }
    }
}
impl Camera {
    // Imported meshes retain their original units and can need much less than
    // 20% zoom. Use the same limit for fitting, wheel input and UI controls.
    pub const MIN_ZOOM: f32 = 1e-6;
    /// The same local pixel scale used by surface picking and manual brushes.
    pub fn scale_at(&self, viewport_size: Vec2, point: Vec3) -> f32 {
        Self::projection_scale(
            viewport_size.x.min(viewport_size.y) * 0.35 * self.zoom,
            self.eye_distance(),
            point.dot(self.basis().2),
        )
    }
    pub(crate) fn projection_scale(base: f32, eye_distance: f32, depth: f32) -> f32 {
        if eye_distance == 0. {
            base
        } else {
            base * eye_distance / (eye_distance - depth).max(eye_distance * 0.001)
        }
    }
    pub fn projected_extent(&self, points: impl Iterator<Item = Vec3>) -> f32 {
        let (right, up, view) = self.basis();
        let distance = self.eye_distance();
        points
            .map(|point| {
                let factor = if distance == 0. {
                    1.
                } else {
                    distance / (distance - point.dot(view)).max(distance * 0.001)
                };
                point.dot(right).abs().max(point.dot(up).abs()) * factor
            })
            .fold(0_f32, f32::max)
    }
    pub fn set_field_of_view(&mut self, degrees: f32) {
        if degrees.is_finite() {
            self.field_of_view = degrees.clamp(20., 75.);
        }
    }
    /// Match the orthographic scale at the orbit centre. Zoom enlarges the
    /// image without moving the eye through the sculpt during close-up work.
    pub fn eye_distance(&self) -> f32 {
        if self.perspective {
            self.perspective_radius / (0.7 * (self.field_of_view.to_radians() * 0.5).tan())
        } else {
            0.
        }
    }
    pub fn zoom_by(&mut self, scroll: f32) {
        if scroll.is_finite() {
            self.set_zoom(self.zoom * (scroll * 0.002).clamp(-20., 20.).exp());
        }
    }
    pub fn set_zoom(&mut self, zoom: f32) {
        if zoom.is_finite() {
            self.zoom = zoom.clamp(Self::MIN_ZOOM, 8.);
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
        let right = Vec3(c, 0., -s);
        let up = Vec3(s * p, q, c * p);
        let (sr, cr) = self.roll.sin_cos();
        (
            right * cr + up * sr,
            up * cr - right * sr,
            Vec3(s * q, -p, c * q),
        )
    }
    pub fn roll_by(&mut self, radians: f32) {
        if radians.is_finite() {
            self.roll = (self.roll + radians) % std::f32::consts::TAU;
        }
    }
    pub fn roll_drag(&mut self, delta: Vec2) {
        self.roll_by(delta.x * 0.008);
    }
    pub fn orbit(&mut self, delta: Vec2) {
        if !delta.is_finite() {
            return;
        }
        // Interpret the drag in screen axes even after rolling the view.
        let (s, c) = self.roll.sin_cos();
        let delta = Vec2::new(c * delta.x + s * delta.y, -s * delta.x + c * delta.y);
        // Wrap complete turns for precision, without imposing a pole limit.
        let wrap = |angle: f32| angle % std::f32::consts::TAU;
        // After crossing a pole, world-up points down on screen. Reverse the
        // yaw axis there so horizontal dragging keeps the same screen direction.
        let up = if self.pitch.cos() >= 0. { 1. } else { -1. };
        self.yaw = wrap(self.yaw - delta.x * 0.008 * up);
        self.pitch = wrap(self.pitch - delta.y * 0.008);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn screen_roll_rotates_projection_without_changing_depth_or_view_direction() {
        for (yaw, pitch) in [(0., 0.), (0.7, 0.4), (-1.2, 2.)] {
            let mut camera = Camera {
                yaw,
                pitch,
                ..Default::default()
            };
            let before = camera.basis();
            camera.roll_by(std::f32::consts::FRAC_PI_2);
            let after = camera.basis();
            assert!((after.0 - before.1).len() < 1e-6);
            assert!((after.1 + before.0).len() < 1e-6);
            assert_eq!(after.2, before.2);
            // A point originally on screen-right moves down for a clockwise roll.
            assert!(-before.0.dot(after.1) > 0.99);
            assert!((after.0.cross(after.1) - after.2).len() < 1e-5);
            camera.roll_by(f32::NAN);
            assert_eq!(camera.roll, std::f32::consts::FRAC_PI_2);
            camera.reset();
            assert_eq!(camera.roll, 0.);
        }
    }

    #[test]
    fn screen_roll_keeps_orbit_drags_following_screen_direction() {
        for roll in [0., 0.7, std::f32::consts::FRAC_PI_2, 3., -1.] {
            for pitch in [0., 0.7, 2.] {
                for delta in [Vec2::new(1., 0.), Vec2::new(0., 1.)] {
                    let mut camera = Camera {
                        roll,
                        pitch,
                        ..Default::default()
                    };
                    let point = camera.basis().2;
                    camera.orbit(delta);
                    let (right, up, _) = camera.basis();
                    let motion = Vec2::new(point.dot(right), -point.dot(up));
                    assert!(
                        motion.dot(delta) > 0.,
                        "roll={roll}, pitch={pitch}, delta={delta:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn horizontal_orbit_follows_drag_on_both_sides_of_the_poles() {
        for pitch in [
            0.,
            0.7,
            -0.7,
            1.7,
            -1.7,
            std::f32::consts::PI,
            -std::f32::consts::PI,
            4.6,
            -4.6,
            5.5,
        ] {
            for yaw in [0., 0.8, -1.2] {
                for dx in [-8., 8.] {
                    let mut camera = Camera {
                        yaw,
                        pitch,
                        ..Default::default()
                    };
                    let visible_point = camera.basis().2;
                    camera.orbit(Vec2::new(dx, 0.));
                    let screen_x = visible_point.dot(camera.basis().0);
                    assert!(screen_x * dx > 0., "pitch {pitch}, yaw {yaw}, drag {dx}");
                    camera.orbit(Vec2::new(-dx, 0.));
                    assert!((camera.yaw - yaw).abs() < 1e-6);
                    assert_eq!(camera.pitch, pitch);
                }
            }
        }
    }
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

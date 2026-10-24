//! A viewport light-orbit widget. Its pointer ownership lasts through release,
//! including when the pointer leaves the widget and crosses the sculpt.
use super::{Light, LightKind, Lighting};
use crate::{camera::Camera, sculpt::Vec3};
use eframe::egui::{self, Color32, Rect, Stroke, Vec2};

#[derive(Default)]
pub(crate) struct LightControls {
    drag: Option<(usize, Light, Camera)>,
    holding: bool,
}
impl LightControls {
    /// Returns true while this overlay owns input that the sculpt must not receive.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        viewport: Rect,
        camera: &Camera,
        lighting: &mut Lighting,
        enabled: bool,
    ) -> bool {
        let was_dragging = self.drag.is_some();
        if self.drag.is_some_and(|(index, _, _)| {
            index != lighting.selected || index >= lighting.lights.len()
        }) {
            self.drag = None;
        }
        let cancel = !enabled
            || !lighting.show_controls
            || ctx.input(|i| !i.focused || i.key_pressed(egui::Key::Escape));
        if cancel && let Some((index, start, _)) = self.drag.take() {
            if let Some(light) = lighting.lights.get_mut(index) {
                light.azimuth = start.azimuth;
                light.elevation = start.elevation;
            }
            ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            });
        }
        let mut over = false;
        if lighting.show_controls {
            let response = egui::Area::new(egui::Id::new("viewport_light_controls"))
                .order(egui::Order::Foreground)
                // The default Area estimate is much wider than this widget and
                // constrains its first-frame hit rectangle over the sculpt.
                .default_size(Vec2::new(162., 430.))
                .fixed_pos(viewport.right_top() + Vec2::new(-172., 156.))
                .constrain_to(viewport)
                .movable(false)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_width(148.);
                        ui.add_enabled_ui(enabled, |ui| {
                            if ui.button("Close light controls").clicked() { lighting.show_controls = false; }
                            lighting.selector(ui);
                            let selected = lighting.selected;
                            let Some(light) = lighting.lights.get_mut(selected) else { return; };
                            egui::CollapsingHeader::new("Light")
                                .default_open(true)
                                .show_unindented(ui, |ui| {
                                    let (rect, pad) = ui.allocate_exact_size(Vec2::new(148., 128.), egui::Sense::drag());
                                    let pad = pad.on_hover_cursor(egui::CursorIcon::Grab)
                                        .on_hover_text("Drag to orbit the light around the model. Escape cancels. The hollow marker is behind the model.");
                                    if !cancel && pad.drag_started_by(egui::PointerButton::Primary) {
                                        self.drag = Some((selected, *light, *camera));
                                    }
                                    if let Some((_, start, camera)) = self.drag
                                        && let Some(delta) = pad.total_drag_delta() {
                                        light.orbit_in_view(start, camera, delta);
                                        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                                    }
                                    if pad.drag_stopped_by(egui::PointerButton::Primary) {
                                        self.drag = None;
                                    }
                                    draw_pad(ui.painter(), rect, light.direction_in_view(*camera), pad.hovered());
                                    ui.horizontal(|ui| {
                                        ui.selectable_value(&mut light.kind, LightKind::Directional, "Directional");
                                        ui.selectable_value(&mut light.kind, LightKind::Point, "Point");
                                    });
                                    if light.kind == LightKind::Point {
                                        ui.label("Distance");
                                        ui.spacing_mut().slider_width = 100.;
                                        ui.add(egui::Slider::new(&mut light.distance, 0.1..=50.).logarithmic(true))
                                            .on_hover_text("Move the point light closer to or farther from the model");
                                    }
                                    if ui.button("Front / back").on_hover_text("Move the light to the opposite side of the view plane").clicked() {
                                        let local = light.direction_in_view(*camera);
                                        light.set_view_direction(Vec3(local.0, local.1, -local.2), *camera);
                                    }
                                    ui.checkbox(&mut light.camera_relative, "Follow camera");
                                });
                        });
                    });
                });
            over = enabled
                && ctx.input(|i| {
                    i.pointer
                        .hover_pos()
                        .is_some_and(|p| response.response.rect.contains(p))
                });
            if over && ctx.input(|i| i.pointer.any_pressed()) {
                self.holding = true;
            }
        }
        let blocked = over || self.holding || was_dragging || self.drag.is_some();
        if ctx.input(|i| !i.pointer.any_down()) {
            self.holding = false;
        }
        blocked
    }
}
impl Light {
    fn direction_in_view(self, camera: Camera) -> Vec3 {
        let (right, up, view) = camera.basis();
        let world = self.vector(right, up, view).norm();
        Vec3(world.dot(right), world.dot(up), world.dot(view)).norm()
    }
    fn set_view_direction(&mut self, local: Vec3, camera: Camera) {
        let (right, up, view) = camera.basis();
        let direction = if self.camera_relative {
            local
        } else {
            right * local.0 + up * local.1 + view * local.2
        }
        .norm();
        self.azimuth = direction.0.atan2(direction.2).to_degrees();
        self.elevation = direction
            .1
            .clamp(-1., 1.)
            .asin()
            .to_degrees()
            .clamp(-89., 89.);
    }
    fn orbit_in_view(&mut self, start: Self, camera: Camera, delta: Vec2) {
        let local = start.direction_in_view(camera);
        let azimuth = local.0.atan2(local.2) + delta.x * 0.015;
        let elevation = (local.1.clamp(-1., 1.).asin() - delta.y * 0.015)
            .clamp(-89_f32.to_radians(), 89_f32.to_radians());
        let (s, c) = azimuth.sin_cos();
        let (se, ce) = elevation.sin_cos();
        self.set_view_direction(Vec3(s * ce, se, c * ce), camera);
    }
}
fn draw_pad(painter: &egui::Painter, rect: Rect, light: Vec3, hovered: bool) {
    let center = rect.center() - Vec2::new(0., 10.);
    let radius = 44.;
    let muted = Color32::from_gray(85);
    let accent = Color32::from_rgb(255, 207, 108);
    painter.circle_filled(center, radius, Color32::from_rgb(32, 38, 47));
    painter.circle_stroke(center, radius, Stroke::new(1_f32, muted));
    for horizontal in [true, false] {
        let points = (0..=48)
            .map(|i| {
                let (s, c) = (i as f32 / 48. * std::f32::consts::TAU).sin_cos();
                center
                    + if horizontal {
                        Vec2::new(c, s * 0.3)
                    } else {
                        Vec2::new(c * 0.3, s)
                    } * radius
            })
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(1_f32, muted)));
    }
    let marker = center + Vec2::new(light.0, -light.1) * radius;
    painter.line_segment(
        [center, marker],
        Stroke::new(1.5_f32, accent.gamma_multiply(0.6)),
    );
    painter.circle_filled(center, 3., Color32::LIGHT_GRAY);
    if light.2 >= 0. {
        painter.circle_filled(marker, 7., accent);
    } else {
        painter.circle_filled(marker, 7., Color32::from_rgb(32, 38, 47));
        painter.circle_stroke(marker, 7., Stroke::new(2_f32, accent));
    }
    if hovered {
        painter.circle_stroke(marker, 11., Stroke::new(1_f32, Color32::WHITE));
    }
    for i in 0..8 {
        let (s, c) = (i as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
        let ray = Vec2::new(c, s);
        painter.line_segment(
            [marker + ray * 10., marker + ray * 13.],
            Stroke::new(1_f32, accent),
        );
    }
    painter.text(
        rect.center_bottom() - Vec2::new(0., 4.),
        egui::Align2::CENTER_BOTTOM,
        "Drag light",
        egui::FontId::proportional(12.),
        Color32::LIGHT_GRAY,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lighting_widget_orbits_in_view_for_camera_and_world_lights() {
        let camera = Camera {
            yaw: 0.8,
            pitch: -0.4,
            ..Default::default()
        };
        let mut local = Light::default();
        let mut world = Light {
            camera_relative: false,
            ..local
        };
        world.set_view_direction(local.direction_in_view(camera), camera);
        let delta = Vec2::new(85., -24.);
        local.orbit_in_view(local, camera, delta);
        world.orbit_in_view(world, camera, delta);
        assert!((local.direction_in_view(camera) - world.direction_in_view(camera)).len() < 1e-5);
        for delta in [Vec2::splat(100000.), Vec2::splat(-100000.)] {
            world.orbit_in_view(world, camera, delta);
            world.validate().unwrap();
        }
    }
}

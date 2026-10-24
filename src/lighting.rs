//! Viewport illumination, independent of authored material channels.
mod overlay;
pub(crate) use overlay::LightControls;

use crate::sculpt::Vec3;
use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum LightKind {
    #[default]
    Directional,
    Point,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Lighting {
    pub kind: LightKind,
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub intensity: f32,
    pub fill: f32,
    pub colour: [f32; 3],
    pub camera_relative: bool,
    pub show_controls: bool,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            kind: LightKind::Directional,
            azimuth: (-0.5_f32).atan().to_degrees(),
            elevation: (0.7 / 1.25_f32.sqrt()).atan().to_degrees(),
            distance: 4.,
            intensity: 2.5,
            fill: 1.,
            colour: [1.; 3],
            camera_relative: true,
            show_controls: true,
        }
    }
}
impl Lighting {
    pub fn validate(&self) -> Result<(), String> {
        for (value, min, max) in [
            (self.azimuth, -180., 180.),
            (self.elevation, -89., 89.),
            (self.distance, 0.1, 50.),
            (self.intensity, 0., 20.),
            (self.fill, 0., 4.),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err("Invalid viewport lighting".into());
            }
        }
        if self
            .colour
            .iter()
            .any(|c| !c.is_finite() || !(0. ..=1.).contains(c))
        {
            return Err("Invalid light colour".into());
        }
        Ok(())
    }
    pub fn vector(self, right: Vec3, up: Vec3, view: Vec3) -> Vec3 {
        let (s, c) = self.azimuth.to_radians().sin_cos();
        let (se, ce) = self.elevation.to_radians().sin_cos();
        let local = Vec3(s * ce, se, c * ce);
        let direction = if self.camera_relative {
            right * local.0 + up * local.1 + view * local.2
        } else {
            local
        };
        direction
            * if self.kind == LightKind::Point {
                self.distance
            } else {
                1.
            }
    }
    pub fn at(self, position: Vec3, right: Vec3, up: Vec3, view: Vec3) -> (Vec3, [f32; 3]) {
        let vector = self.vector(right, up, view);
        let (direction, attenuation) = if self.kind == LightKind::Point {
            let delta = vector - position;
            (delta.norm(), 16. / delta.dot(delta).max(0.01))
        } else {
            (vector, 1.)
        };
        (
            direction,
            self.colour.map(|c| c * self.intensity * attenuation),
        )
    }
    pub fn install(self, ctx: &egui::Context) {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("viewport_lighting"), self));
    }
    pub fn current(ctx: &egui::Context) -> Self {
        ctx.data(|d| d.get_temp(egui::Id::new("viewport_lighting")))
            .unwrap_or_default()
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.label("Viewport lighting");
        ui.checkbox(&mut self.show_controls, "Show light controls");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.kind, LightKind::Directional, "Directional light");
            ui.selectable_value(&mut self.kind, LightKind::Point, "Point light");
        });
        let drag = ui
            .add(egui::Button::new("Drag to move light").sense(egui::Sense::drag()))
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text("Drag left/right to orbit the light; up/down changes elevation");
        let id = drag.id.with("start");
        if drag.drag_started() {
            ui.ctx()
                .data_mut(|d| d.insert_temp(id, [self.azimuth, self.elevation]));
        }
        if let Some(delta) = drag.total_drag_delta() {
            let start = ui
                .ctx()
                .data(|d| d.get_temp::<[f32; 2]>(id))
                .unwrap_or([self.azimuth, self.elevation]);
            self.azimuth = (start[0] + delta.x * 0.6 + 180.).rem_euclid(360.) - 180.;
            self.elevation = (start[1] - delta.y * 0.6).clamp(-89., 89.);
        }
        if drag.drag_stopped() {
            ui.ctx().data_mut(|d| d.remove::<[f32; 2]>(id));
        }
        ui.add(
            egui::Slider::new(&mut self.azimuth, -180. ..=180.)
                .text("Light azimuth")
                .suffix("°"),
        );
        ui.add(
            egui::Slider::new(&mut self.elevation, -89. ..=89.)
                .text("Light elevation")
                .suffix("°"),
        );
        if self.kind == LightKind::Point {
            ui.add(
                egui::Slider::new(&mut self.distance, 0.1..=50.)
                    .logarithmic(true)
                    .text("Light distance"),
            );
        }
        ui.add(egui::Slider::new(&mut self.intensity, 0. ..=20.).text("Light intensity"));
        ui.add(egui::Slider::new(&mut self.fill, 0. ..=4.).text("Ambient fill"));
        ui.horizontal(|ui| {
            ui.label("Light colour");
            let mut colour = self.colour;
            if ui.color_edit_button_rgb(&mut colour).changed() {
                self.colour = colour;
            }
        });
        ui.checkbox(&mut self.camera_relative, "Light follows camera");
        if ui.button("Reset studio lighting").clicked() {
            *self = Self::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lighting_defaults_camera_lock_point_falloff_and_validation() {
        let axes = (Vec3(1., 0., 0.), Vec3(0., 1., 0.), Vec3(0., 0., 1.));
        let mut lighting = Lighting::default();
        let expected = Vec3(-0.5, 0.7, 1.).norm();
        assert!((lighting.vector(axes.0, axes.1, axes.2) - expected).len() < 1e-6);
        let rotated = (axes.2 * -1., axes.1, axes.0);
        assert!((lighting.vector(rotated.0, rotated.1, rotated.2) - expected).len() > 0.5);
        lighting.camera_relative = false;
        assert!((lighting.vector(rotated.0, rotated.1, rotated.2) - expected).len() < 1e-6);
        lighting.kind = LightKind::Point;
        let (_, near) = lighting.at(Vec3::default(), axes.0, axes.1, axes.2);
        lighting.distance *= 2.;
        let (_, far) = lighting.at(Vec3::default(), axes.0, axes.1, axes.2);
        assert!((near[0] * 0.25 - far[0]).abs() < 1e-6);
        lighting.validate().unwrap();
        lighting.intensity = f32::NAN;
        assert!(lighting.validate().is_err());
    }
    #[test]
    fn lighting_preferences_preserve_settings_and_load_legacy_defaults() {
        let mut prefs = crate::preferences::Preferences::default();
        prefs.lighting.kind = LightKind::Point;
        prefs.lighting.azimuth = 75.;
        prefs.lighting.colour = [0.2, 0.4, 1.];
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: crate::preferences::Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, prefs);
        let mut value = serde_json::to_value(&prefs).unwrap();
        value.as_object_mut().unwrap().remove("lighting");
        let loaded: crate::preferences::Preferences = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.lighting, Lighting::default());
    }
}

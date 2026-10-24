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
pub(crate) struct Light {
    pub kind: LightKind,
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub intensity: f32,
    pub colour: [f32; 3],
    pub camera_relative: bool,
}
impl Default for Light {
    fn default() -> Self {
        Self {
            kind: LightKind::Directional,
            azimuth: (-0.5_f32).atan().to_degrees(),
            elevation: (0.7 / 1.25_f32.sqrt()).atan().to_degrees(),
            distance: 4.,
            intensity: 2.5,
            colour: [1.; 3],
            camera_relative: true,
        }
    }
}
impl Light {
    pub fn validate(&self) -> Result<(), String> {
        for (value, min, max) in [
            (self.azimuth, -180., 180.),
            (self.elevation, -89., 89.),
            (self.distance, 0.1, 50.),
            (self.intensity, 0., 20.),
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
    pub fn controls(&mut self, ui: &mut egui::Ui) {
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
        ui.horizontal(|ui| {
            ui.label("Light colour");
            let mut colour = self.colour;
            if ui.color_edit_button_rgb(&mut colour).changed() {
                self.colour = colour;
            }
        });
        ui.checkbox(&mut self.camera_relative, "Light follows camera");
    }
}

pub(crate) const MAX_LIGHTS: usize = 8;
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Lighting {
    pub lights: Vec<Light>,
    pub selected: usize,
    pub fill: f32,
    /// Opening the editing widget is a session action, not a saved lighting property.
    #[serde(skip)]
    pub show_controls: bool,
}
fn default_fill() -> f32 {
    1.
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            lights: vec![Light::default()],
            selected: 0,
            fill: default_fill(),
            show_controls: false,
        }
    }
}
impl<'de> Deserialize<'de> for Lighting {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Saved {
            lights: Option<Vec<Light>>,
            #[serde(default)]
            selected: usize,
            #[serde(default = "default_fill")]
            fill: f32,
            #[serde(flatten)]
            legacy: Light,
        }
        let saved = Saved::deserialize(de)?;
        let result = Self {
            lights: saved.lights.unwrap_or_else(|| vec![saved.legacy]),
            selected: saved.selected,
            fill: saved.fill,
            show_controls: false,
        };
        result.validate().map_err(serde::de::Error::custom)?;
        Ok(result)
    }
}
impl Lighting {
    pub fn validate(&self) -> Result<(), String> {
        if self.lights.len() > MAX_LIGHTS
            || self.selected >= self.lights.len().max(1)
            || !self.fill.is_finite()
            || !(0. ..=4.).contains(&self.fill)
        {
            return Err("Invalid viewport lighting rig".into());
        }
        for light in &self.lights {
            light.validate()?;
        }
        Ok(())
    }
    pub fn add_light(&mut self) -> bool {
        if self.lights.len() == MAX_LIGHTS {
            return false;
        }
        let azimuth = self.lights.last().map_or(Light::default().azimuth, |l| {
            (l.azimuth + 120. + 180.).rem_euclid(360.) - 180.
        });
        self.lights.push(Light {
            azimuth,
            intensity: 1.,
            ..Default::default()
        });
        self.selected = self.lights.len() - 1;
        true
    }
    pub fn remove_light(&mut self) -> bool {
        if self.lights.is_empty() {
            return false;
        }
        self.lights.remove(self.selected);
        self.selected = self.selected.min(self.lights.len().saturating_sub(1));
        true
    }
    pub fn selector(&mut self, ui: &mut egui::Ui) {
        if self.lights.is_empty() {
            ui.weak("No lights");
            return;
        }
        egui::ComboBox::from_id_salt("selected_light")
            .selected_text(format!("Light {}", self.selected + 1))
            .width(110.)
            .show_ui(ui, |ui| {
                for i in 0..self.lights.len() {
                    ui.selectable_value(&mut self.selected, i, format!("Light {}", i + 1));
                }
            });
    }
    pub fn samples(
        &self,
        position: Vec3,
        right: Vec3,
        up: Vec3,
        view: Vec3,
    ) -> impl Iterator<Item = (Vec3, [f32; 3])> + Clone + '_ {
        self.lights
            .iter()
            .map(move |light| light.at(position, right, up, view))
    }
    pub fn install(&self, ctx: &egui::Context) {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("viewport_lighting"), self.clone()));
    }
    pub fn current(ctx: &egui::Context) -> Self {
        ctx.data(|d| d.get_temp(egui::Id::new("viewport_lighting")))
            .unwrap_or_default()
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.show_controls, "Show light controls");
        self.selector(ui);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.lights.len() < MAX_LIGHTS,
                    egui::Button::new("Add light"),
                )
                .clicked()
            {
                self.add_light();
            }
            if ui
                .add_enabled(!self.lights.is_empty(), egui::Button::new("Remove light"))
                .clicked()
            {
                self.remove_light();
            }
        });
        ui.weak(format!("{} / {MAX_LIGHTS} lights", self.lights.len()));
        if let Some(light) = self.lights.get_mut(self.selected) {
            ui.push_id(self.selected, |ui| light.controls(ui));
        }
        ui.separator();
        ui.add(egui::Slider::new(&mut self.fill, 0. ..=4.).text("Ambient fill"));
        if ui.button("Reset studio lighting").clicked() {
            *self = Self {
                show_controls: self.show_controls,
                ..Default::default()
            };
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lighting_defaults_camera_lock_point_falloff_and_validation() {
        let axes = (Vec3(1., 0., 0.), Vec3(0., 1., 0.), Vec3(0., 0., 1.));
        let mut lighting = Light::default();
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
    fn lighting_rig_migrates_limits_selection_and_hides_widget_on_load() {
        let legacy = serde_json::json!({"kind":"Point","azimuth":76.,"elevation":15.,"intensity":1.3,"fill":0.4,"distance":5.,"camera_relative":false,"show_controls":true});
        let mut rig: Lighting = serde_json::from_value(legacy).unwrap();
        assert_eq!(rig.lights.len(), 1);
        assert_eq!(rig.lights[0].kind, LightKind::Point);
        assert_eq!(rig.lights[0].azimuth, 76.);
        assert_eq!(rig.fill, 0.4);
        assert!(!rig.show_controls);
        let first = rig.lights[0];
        for _ in 1..MAX_LIGHTS {
            assert!(rig.add_light());
        }
        assert!(!rig.add_light());
        rig.selected = 0;
        assert!(rig.remove_light());
        assert!(!rig.lights.contains(&first));
        rig.show_controls = true;
        let saved = serde_json::to_vec(&rig).unwrap();
        let restored: Lighting = serde_json::from_slice(&saved).unwrap();
        assert_eq!(restored.lights, rig.lights);
        assert_eq!(restored.fill, rig.fill);
        assert!(!restored.show_controls);
        while rig.remove_light() {}
        assert_eq!(rig.selected, 0);
        rig.validate().unwrap();
        assert!(rig.add_light());
        rig.selected = 2;
        assert!(rig.validate().is_err());
    }
    #[test]
    fn lighting_preferences_preserve_settings_and_load_legacy_defaults() {
        let mut prefs = crate::preferences::Preferences::default();
        prefs.lighting.lights[0].kind = LightKind::Point;
        prefs.lighting.lights[0].azimuth = 75.;
        prefs.lighting.lights[0].colour = [0.2, 0.4, 1.];
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: crate::preferences::Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, prefs);
        let mut value = serde_json::to_value(&prefs).unwrap();
        value.as_object_mut().unwrap().remove("lighting");
        let loaded: crate::preferences::Preferences = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.lighting, Lighting::default());
    }
}

use super::{Action, Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TouchDrag {
    Disabled,
    Orbit,
    Pan,
    Zoom,
}
impl TouchDrag {
    pub const ALL: [Self; 4] = [Self::Disabled, Self::Orbit, Self::Pan, Self::Zoom];
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Orbit => "Rotate view",
            Self::Pan => "Pan view",
            Self::Zoom => "Zoom view",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TouchScroll {
    PixelPan,
    AllPan,
    Zoom,
    Disabled,
}
impl TouchScroll {
    pub const ALL: [Self; 4] = [Self::PixelPan, Self::AllPan, Self::Zoom, Self::Disabled];
    pub fn label(self) -> &'static str {
        match self {
            Self::PixelPan => "Pan pixel scroll; zoom wheel",
            Self::AllPan => "Pan all scrolling",
            Self::Zoom => "Zoom all scrolling",
            Self::Disabled => "Disabled",
        }
    }
}

/// Input preferences are global, like keyboard bindings, rather than workspace layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TouchSettings {
    pub enabled: bool,
    pub one_finger: TouchDrag,
    pub two_finger: TouchDrag,
    pub pinch: bool,
    pub twist: bool,
    pub twist_speed: f32,
    pub reverse_twist: bool,
    pub two_tap: Option<Action>,
    pub three_tap: Option<Action>,
    pub scroll: TouchScroll,
    pub button_taps: bool,
    pub drag_speed: f32,
    pub pinch_speed: f32,
    pub reverse_pinch: bool,
    pub tap_seconds: f32,
    pub tap_slop: f32,
}
impl Default for TouchSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            one_finger: TouchDrag::Orbit,
            two_finger: TouchDrag::Pan,
            pinch: true,
            twist: true,
            twist_speed: 1.,
            reverse_twist: false,
            two_tap: Some(Action::Undo),
            three_tap: Some(Action::Redo),
            scroll: TouchScroll::PixelPan,
            button_taps: false,
            drag_speed: 1.,
            pinch_speed: 1.,
            reverse_pinch: false,
            tap_seconds: 0.35,
            tap_slop: 12.,
        }
    }
}
impl TouchSettings {
    pub fn validate(&self) -> Result<(), String> {
        for (value, min, max) in [
            (self.drag_speed, 0.1, 4.),
            (self.pinch_speed, 0.1, 4.),
            (self.twist_speed, 0.1, 4.),
            (self.tap_seconds, 0.1, 0.8),
            (self.tap_slop, 2., 30.),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err("Invalid touch gesture sensitivity or tap tolerance".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::Preferences;

    #[test]
    fn touch_settings_migrate_persist_and_validate() {
        let mut prefs = Preferences::default();
        let mut legacy = serde_json::to_value(&prefs).unwrap();
        legacy.as_object_mut().unwrap().remove("touch");
        let migrated: Preferences = serde_json::from_value(legacy).unwrap();
        assert_eq!(migrated.touch, TouchSettings::default());
        prefs.touch.two_tap = Some(Action::Redo);
        prefs.touch.three_tap = None;
        prefs.touch.two_finger = TouchDrag::Orbit;
        prefs.touch.pinch_speed = 2.;
        prefs.touch.twist = false;
        prefs.touch.twist_speed = 1.7;
        prefs.touch.reverse_twist = true;
        prefs.touch.scroll = TouchScroll::AllPan;
        prefs.touch.button_taps = true;
        let restored: Preferences =
            serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.touch, prefs.touch);
        let mut old_touch = serde_json::to_value(TouchSettings::default()).unwrap();
        for field in ["twist", "twist_speed", "reverse_twist"] {
            old_touch.as_object_mut().unwrap().remove(field);
        }
        let migrated: TouchSettings = serde_json::from_value(old_touch).unwrap();
        assert!(migrated.twist);
        assert_eq!(migrated.twist_speed, 1.);
        assert!(!migrated.reverse_twist);
        for invalid in [f32::NAN, f32::INFINITY, 0., 5.] {
            prefs.touch.pinch_speed = invalid;
            assert!(prefs.validate().is_err());
        }
    }
}

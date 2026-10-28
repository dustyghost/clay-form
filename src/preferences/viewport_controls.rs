use super::{Action, Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum ViewportControlKind {
    Pan,
    Zoom,
    Orbit,
    Action(Action),
}

/// Missing styles preserve existing text controls; new controls offer icons by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ViewportControlStyle {
    #[default]
    Text,
    Icon,
    IconAndText,
}
impl ViewportControlStyle {
    pub const ALL: [Self; 3] = [Self::Icon, Self::IconAndText, Self::Text];
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Icon => "Icon only",
            Self::IconAndText => "Icon + text",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ViewportControlIcon {
    #[default]
    Auto,
    Pan,
    Zoom,
    Orbit,
    Reset,
    Groups,
    Wireframe,
    Brush,
    Eye,
    Undo,
    Redo,
    Plus,
    Minus,
    Settings,
}
impl ViewportControlIcon {
    pub const ALL: [Self; 14] = [
        Self::Auto,
        Self::Pan,
        Self::Zoom,
        Self::Orbit,
        Self::Reset,
        Self::Groups,
        Self::Wireframe,
        Self::Brush,
        Self::Eye,
        Self::Undo,
        Self::Redo,
        Self::Plus,
        Self::Minus,
        Self::Settings,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatic",
            Self::Pan => "Pan",
            Self::Zoom => "Zoom",
            Self::Orbit => "Orbit",
            Self::Reset => "Reset",
            Self::Groups => "Groups",
            Self::Wireframe => "Wireframe",
            Self::Brush => "Brush",
            Self::Eye => "View",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Plus => "Plus",
            Self::Minus => "Minus",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ViewportControl {
    pub kind: ViewportControlKind,
    pub label: String,
    #[serde(default)]
    pub style: ViewportControlStyle,
    #[serde(default)]
    pub icon: ViewportControlIcon,
    /// Fraction of the available travel inside the viewport, independent of window size.
    pub position: [f32; 2],
    /// Logical-pixel offset from the proportional anchor, for evenly spaced groups.
    #[serde(default)]
    pub offset: [f32; 2],
}

pub(crate) fn default_viewport_controls() -> Vec<ViewportControl> {
    use ViewportControlKind::*;
    [
        Pan,
        Zoom,
        Orbit,
        Action(super::Action::ResetView),
        Action(super::Action::PolygroupColors),
        Action(super::Action::Wireframe),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, kind)| ViewportControl {
        kind,
        label: if i == 4 {
            "Vertex groups".into()
        } else {
            String::new()
        },
        style: ViewportControlStyle::Icon,
        icon: ViewportControlIcon::Auto,
        position: [1., 0.56],
        offset: [0., (i as f32 - 2.5) * 48.],
    })
    .collect()
}

pub(super) fn validate(controls: &[ViewportControl]) -> Result<(), String> {
    if controls.len() > 128
        || controls.iter().any(|control| {
            control.label.chars().count() > 128
                || control
                    .offset
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 100_000.)
                || control
                    .position
                    .iter()
                    .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        })
    {
        return Err("Invalid viewport control layout".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::Preferences;

    #[test]
    fn viewport_controls_persist_per_workspace_and_migrate_old_preferences() {
        let mut prefs = Preferences::default();
        prefs.save_workspace_as("Original").unwrap();
        prefs.save_workspace_as("Custom").unwrap();
        prefs.viewport_controls[0].position = [0.2, 0.8];
        prefs.viewport_controls[0].label = "Move camera".into();
        prefs.viewport_controls[0].style = ViewportControlStyle::IconAndText;
        prefs.viewport_controls[0].icon = ViewportControlIcon::Pan;
        prefs.viewport_controls[0].offset = [12., -8.];
        prefs.sync_active_workspace();
        let json = serde_json::to_string(&prefs).unwrap();
        let mut restored: Preferences = serde_json::from_str(&json).unwrap();
        restored.validate().unwrap();
        restored.switch_workspace(0);
        assert_eq!(restored.viewport_controls, default_viewport_controls());
        restored.switch_workspace(1);
        assert_eq!(restored.viewport_controls[0].position, [0.2, 0.8]);
        assert_eq!(restored.viewport_controls[0].label, "Move camera");
        assert_eq!(
            restored.viewport_controls[0].style,
            ViewportControlStyle::IconAndText
        );
        assert_eq!(restored.viewport_controls[0].icon, ViewportControlIcon::Pan);
        assert_eq!(restored.viewport_controls[0].offset, [12., -8.]);
        let mut old_control = serde_json::to_value(&restored.viewport_controls[0]).unwrap();
        for key in ["style", "icon", "offset"] {
            old_control.as_object_mut().unwrap().remove(key);
        }
        let old_control: ViewportControl = serde_json::from_value(old_control).unwrap();
        assert_eq!(old_control.style, ViewportControlStyle::Text);
        assert_eq!(old_control.icon, ViewportControlIcon::Auto);
        assert_eq!(old_control.offset, [0.; 2]);
        // An intentionally empty layout must stay empty after reload.
        restored.viewport_controls.clear();
        let empty: Preferences =
            serde_json::from_str(&serde_json::to_string(&restored).unwrap()).unwrap();
        assert!(empty.viewport_controls.is_empty());
        let mut legacy = serde_json::to_value(&prefs).unwrap();
        legacy.as_object_mut().unwrap().remove("viewport_controls");
        for workspace in legacy["workspaces"].as_array_mut().unwrap() {
            workspace
                .as_object_mut()
                .unwrap()
                .remove("viewport_controls");
        }
        let migrated: Preferences = serde_json::from_value(legacy).unwrap();
        assert_eq!(migrated.viewport_controls, default_viewport_controls());
        assert_eq!(
            migrated.workspaces[0].viewport_controls,
            default_viewport_controls()
        );
        restored.viewport_controls = default_viewport_controls();
        restored.viewport_controls[0].position[0] = f32::NAN;
        assert!(restored.validate().is_err());
    }
}

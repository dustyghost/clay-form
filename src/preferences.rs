//! User preferences are independent of sculpt projects.
use eframe::egui::{self, Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Action {
    Save,
    Open,
    New,
    Undo,
    Redo,
    ResetView,
    Symmetry,
    Wireframe,
    Guides,
    Smaller,
    Larger,
    PreviousBrush,
    NextBrush,
    Subdivide,
    LowerLevel,
    HigherLevel,
    Remesh,
}
impl Action {
    pub const ALL: [Self; 17] = [
        Self::Save,
        Self::Open,
        Self::New,
        Self::Undo,
        Self::Redo,
        Self::ResetView,
        Self::Symmetry,
        Self::Wireframe,
        Self::Guides,
        Self::Smaller,
        Self::Larger,
        Self::PreviousBrush,
        Self::NextBrush,
        Self::Subdivide,
        Self::LowerLevel,
        Self::HigherLevel,
        Self::Remesh,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Save => "Save project",
            Self::Open => "Open project at path",
            Self::New => "New sphere",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::ResetView => "Reset view",
            Self::Symmetry => "Toggle symmetry",
            Self::Wireframe => "Toggle wireframe",
            Self::Guides => "Toggle symmetry guides",
            Self::Smaller => "Smaller brush",
            Self::Larger => "Larger brush",
            Self::PreviousBrush => "Previous brush",
            Self::NextBrush => "Next brush",
            Self::Subdivide => "Subdivide mesh",
            Self::LowerLevel => "Lower subdivision level",
            Self::HigherLevel => "Higher subdivision level",
            Self::Remesh => "Remesh dynamic mesh",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Shortcut {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub command: bool,
}
impl Shortcut {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self {
            key: key.name().into(),
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            alt: modifiers.alt,
            command: modifiers.mac_cmd,
        }
    }
    pub fn matches(&self, key: Key, modifiers: Modifiers) -> bool {
        *self == Self::new(key, modifiers)
    }
    pub fn label(&self) -> String {
        format!(
            "{}{}{}{}{}",
            if self.ctrl { "Ctrl+" } else { "" },
            if self.command { "Cmd+" } else { "" },
            if self.alt { "Alt+" } else { "" },
            if self.shift { "Shift+" } else { "" },
            self.key
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Hold {
    Shift,
    Alt,
    Ctrl,
    Disabled,
}
impl Hold {
    pub const ALL: [Self; 4] = [Self::Shift, Self::Alt, Self::Ctrl, Self::Disabled];
    pub fn active(self, m: Modifiers) -> bool {
        match self {
            Self::Shift => m.shift,
            Self::Alt => m.alt,
            Self::Ctrl => m.ctrl,
            Self::Disabled => false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Tool {
    Library,
    Brush,
    Alpha,
    Presets,
    Tablet,
    Mesh,
}
impl Tool {
    pub const ALL: [Self; 6] = [
        Self::Library,
        Self::Brush,
        Self::Alpha,
        Self::Presets,
        Self::Tablet,
        Self::Mesh,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "Brush library",
            Self::Brush => "Brush settings",
            Self::Alpha => "Alpha",
            Self::Presets => "Brush presets",
            Self::Tablet => "Tablet & navigation",
            Self::Mesh => "Mesh management",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Dock {
    Left,
    Right,
    Floating,
    Hidden,
}
impl Dock {
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Floating, Self::Hidden];
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Panel {
    pub tool: Tool,
    pub dock: Dock,
    pub open: bool,
    pub position: [f32; 2],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    pub version: u32,
    pub bindings: Vec<(Action, Option<Shortcut>)>,
    pub smooth: Hold,
    pub reverse: Hold,
    pub panels: Vec<Panel>,
    pub left_width: f32,
    pub right_width: f32,
}
impl Default for Preferences {
    fn default() -> Self {
        let ctrl = Modifiers::CTRL;
        let none = Modifiers::NONE;
        let keys = [
            (Key::S, ctrl),
            (Key::O, ctrl),
            (Key::N, ctrl),
            (Key::Z, ctrl),
            (Key::Z, ctrl | Modifiers::SHIFT),
            (Key::F, none),
            (Key::X, none),
            (Key::W, none),
            (Key::G, none),
            (Key::OpenBracket, none),
            (Key::CloseBracket, none),
            (Key::Comma, none),
            (Key::Period, none),
            (Key::D, ctrl),
            (Key::D, Modifiers::SHIFT),
            (Key::D, none),
            (Key::R, ctrl),
        ];
        Self {
            version: 1,
            bindings: Action::ALL
                .into_iter()
                .zip(keys)
                .map(|(a, (k, m))| (a, Some(Shortcut::new(k, m))))
                .collect(),
            smooth: Hold::Shift,
            reverse: Hold::Alt,
            panels: Tool::ALL
                .into_iter()
                .enumerate()
                .map(|(i, tool)| Panel {
                    tool,
                    dock: if tool == Tool::Mesh {
                        Dock::Right
                    } else {
                        Dock::Left
                    },
                    open: true,
                    position: [320. + i as f32 * 25., 100. + i as f32 * 25.],
                })
                .collect(),
            left_width: 260.,
            right_width: 260.,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported preferences version".into());
        }
        if self.bindings.len() != Action::ALL.len() || self.panels.len() != Tool::ALL.len() {
            return Err("Incomplete preferences".into());
        }
        for action in Action::ALL {
            if self.bindings.iter().filter(|(a, _)| *a == action).count() != 1 {
                return Err("Duplicate or missing key action".into());
            }
        }
        for tool in Tool::ALL {
            if self.panels.iter().filter(|p| p.tool == tool).count() != 1 {
                return Err("Duplicate or missing tool panel".into());
            }
        }
        for (i, (_, binding)) in self.bindings.iter().enumerate() {
            if let Some(b) = binding {
                if Key::from_name(&b.key).is_none() {
                    return Err("Unknown key".into());
                }
                if self.bindings[..i]
                    .iter()
                    .any(|(_, other)| other.as_ref() == Some(b))
                {
                    return Err("Duplicate keyboard shortcut".into());
                }
            }
        }
        if !self.left_width.is_finite()
            || !self.right_width.is_finite()
            || !(180. ..=600.).contains(&self.left_width)
            || !(180. ..=600.).contains(&self.right_width)
            || self.panels.iter().any(|p| {
                p.position
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 100_000.)
            })
        {
            return Err("Invalid panel geometry".into());
        }
        Ok(())
    }
    pub fn bind(&mut self, action: Action, shortcut: Shortcut) -> Result<(), String> {
        if let Some((other, _)) = self
            .bindings
            .iter()
            .find(|(a, b)| *a != action && b.as_ref() == Some(&shortcut))
        {
            return Err(format!(
                "Already assigned to {}. Clear that binding first.",
                other.label()
            ));
        }
        if let Some((_, b)) = self.bindings.iter_mut().find(|(a, _)| *a == action) {
            *b = Some(shortcut);
        }
        Ok(())
    }
    pub fn actions(&self, ctx: &egui::Context) -> Vec<Action> {
        ctx.input_mut(|input| self.bindings.iter().filter_map(|(a,b)| {
            let b = b.as_ref()?;
            let key = Key::from_name(&b.key)?;
            let found = input.events.iter().any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if b.matches(*k,*modifiers)));
            if found { input.events.retain(|e| !matches!(e,egui::Event::Key { key:k,pressed:true,modifiers,.. } if b.matches(*k,*modifiers))); input.consume_key(Modifiers { ctrl:b.ctrl, shift:b.shift,alt:b.alt,mac_cmd:b.command,command:b.ctrl || b.command },key); Some(*a) } else { None }
        }).collect())
    }
}
pub(crate) fn path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clay-form/preferences.json")
}
pub(crate) fn save(path: &std::path::Path, prefs: &Preferences) -> Result<(), String> {
    prefs.validate()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::persistence::write_json(&path.to_string_lossy(), prefs)
}
pub(crate) fn load(path: &std::path::Path) -> Result<Preferences, String> {
    let mut prefs: Preferences = crate::persistence::read_json(&path.to_string_lossy())?;
    if prefs.bindings.len() == 13
        && [
            Action::Subdivide,
            Action::LowerLevel,
            Action::HigherLevel,
            Action::Remesh,
        ]
        .iter()
        .all(|new| !prefs.bindings.iter().any(|(a, _)| a == new))
    {
        for (action, mut shortcut) in Preferences::default().bindings.into_iter().skip(13) {
            if shortcut.is_some()
                && prefs
                    .bindings
                    .iter()
                    .any(|(_, existing)| existing == &shortcut)
            {
                shortcut = None;
            }
            prefs.bindings.push((action, shortcut));
        }
    }
    // Upgrade the original five-panel workspace without losing its custom layout.
    if prefs.panels.len() == 5 && !prefs.panels.iter().any(|p| p.tool == Tool::Mesh) {
        prefs.panels.push(Panel {
            tool: Tool::Mesh,
            dock: Dock::Right,
            open: true,
            position: [600., 120.],
        });
    }
    prefs.validate()?;
    Ok(prefs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn customized_preferences_survive_disk_and_conflicts_are_rejected() {
        let path =
            std::env::temp_dir().join(format!("clay-preferences-{}.json", std::process::id()));
        let mut prefs = Preferences::default();
        prefs.panels.swap(0, 3);
        prefs.panels[0].dock = Dock::Floating;
        prefs.panels[0].position = [400., 240.];
        prefs.panels[1].dock = Dock::Hidden;
        prefs.smooth = Hold::Ctrl;
        prefs
            .bind(Action::Wireframe, Shortcut::new(Key::F2, Modifiers::ALT))
            .unwrap();
        let before = prefs.clone();
        assert!(
            prefs
                .bind(Action::Save, Shortcut::new(Key::F2, Modifiers::ALT))
                .is_err()
        );
        assert_eq!(prefs, before);
        save(&path, &prefs).unwrap();
        assert_eq!(load(&path).unwrap(), prefs);
        std::fs::remove_file(path).unwrap();
        prefs.panels[1].tool = prefs.panels[0].tool;
        assert!(prefs.validate().is_err());
    }
    #[test]
    fn shortcut_matches_exact_modifiers_and_is_consumed_once() {
        let ctx = egui::Context::default();
        let prefs = Preferences::default();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    egui::Event::Key {
                        key: Key::S,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::CTRL,
                    },
                    egui::Event::Key {
                        key: Key::W,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::SHIFT,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                assert_eq!(prefs.actions(ctx), vec![Action::Save]);
                assert!(prefs.actions(ctx).is_empty());
            },
        );
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn existing_workspace_keeps_customizations_when_mesh_tools_are_added() {
        let path =
            std::env::temp_dir().join(format!("clay-layout-upgrade-{}.json", std::process::id()));
        let mut old = Preferences::default();
        old.panels.retain(|p| p.tool != Tool::Mesh);
        old.bindings.truncate(13);
        old.panels[0].dock = Dock::Floating;
        crate::persistence::write_json(&path.to_string_lossy(), &old).unwrap();
        let upgraded = load(&path).unwrap();
        assert_eq!(&upgraded.panels[..5], &old.panels);
        assert_eq!(upgraded.panels[5].tool, Tool::Mesh);
        assert_eq!(upgraded.panels[5].dock, Dock::Right);
        assert_eq!(&upgraded.bindings[..13], &old.bindings);
        assert_eq!(upgraded.bindings.len(), 17);
        std::fs::remove_file(path).unwrap();
    }
}

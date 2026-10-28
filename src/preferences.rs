//! User preferences are independent of sculpt projects.
use eframe::egui::{self, Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) use crate::action::Action;
mod touch;
mod viewport_controls;
pub(crate) use touch::{TouchDrag, TouchScroll, TouchSettings};
pub(crate) use viewport_controls::{
    ViewportControl, ViewportControlIcon, ViewportControlKind, ViewportControlStyle,
    default_viewport_controls,
};
mod visibility;
pub(crate) use visibility::{VisibilitySettings, VisibilityShape};
mod masking;
mod materials;
pub(crate) use masking::{MaskPreset, MaskShape};
pub(crate) use materials::MaterialLibrary;
#[cfg(test)]
mod brush_controls_tests;
mod hold_binding;
pub(crate) use hold_binding::HoldBinding;
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
    pub fn from_event(event: &egui::Event) -> Option<Self> {
        let egui::Event::Key {
            key,
            physical_key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            return None;
        };
        let key = match physical_key {
            Some(
                k @ (Key::Num0
                | Key::Num1
                | Key::Num2
                | Key::Num3
                | Key::Num4
                | Key::Num5
                | Key::Num6
                | Key::Num7
                | Key::Num8
                | Key::Num9),
            ) => *k,
            _ => *key,
        };
        Some(Self::new(key, *modifiers))
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
    Transform,
    Submodels,
}
impl Tool {
    pub const ALL: [Self; 8] = [
        Self::Library,
        Self::Brush,
        Self::Alpha,
        Self::Presets,
        Self::Tablet,
        Self::Mesh,
        Self::Transform,
        Self::Submodels,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "Brush library",
            Self::Brush => "Brush settings",
            Self::Alpha => "Alpha",
            Self::Presets => "Brush presets",
            Self::Tablet => "Tablet & navigation",
            Self::Mesh => "Mesh management",
            Self::Transform => "Transform",
            Self::Submodels => "Submodels",
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
/// A menu in the menu bar or a shelf docked around the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Placement {
    LeftPanel,
    RightPanel,
    Menu,
    Top,
    Bottom,
    Left,
    Right,
    Floating,
    Hidden,
}
impl Placement {
    pub const ALL: [Self; 9] = [
        Self::LeftPanel,
        Self::RightPanel,
        Self::Menu,
        Self::Top,
        Self::Bottom,
        Self::Left,
        Self::Right,
        Self::Floating,
        Self::Hidden,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::LeftPanel => "Left sidebar",
            Self::RightPanel => "Right sidebar",
            Self::Menu => "Menu bar",
            Self::Top => "Top shelf",
            Self::Bottom => "Bottom shelf",
            Self::Left => "Left shelf",
            Self::Right => "Right shelf",
            Self::Floating => "Floating",
            Self::Hidden => "Hidden",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct CustomItem {
    /// None is a separator. Actions resolve against the current document.
    pub action: Option<Action>,
    pub label: String,
}
impl CustomItem {
    pub fn action(action: Action) -> Self {
        Self {
            action: Some(action),
            label: String::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct CustomMenu {
    pub id: u64,
    pub name: String,
    pub placement: Placement,
    pub columns: usize,
    pub position: [f32; 2],
    pub items: Vec<CustomItem>,
    #[serde(default)]
    pub collapsible: bool,
    #[serde(default = "expanded_default")]
    pub expanded: bool,
}
fn expanded_default() -> bool {
    true
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Appearance {
    pub panels: [u8; 3],
    pub buttons: [u8; 3],
    pub accent: [u8; 3],
    pub text: [u8; 3],
    pub viewport: [u8; 3],
    pub spacing: u8,
    pub rounding: u8,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            panels: [26, 29, 34],
            buttons: [35, 40, 47],
            accent: [88, 195, 215],
            text: [190, 190, 190],
            viewport: [23, 26, 31],
            spacing: 6,
            rounding: 4,
        }
    }
}
impl Appearance {
    pub fn light() -> Self {
        Self {
            panels: [228, 231, 235],
            buttons: [205, 211, 219],
            accent: [25, 100, 155],
            text: [30, 35, 42],
            viewport: [190, 197, 205],
            ..Self::default()
        }
    }
    pub fn warm() -> Self {
        Self {
            panels: [42, 34, 30],
            buttons: [59, 47, 40],
            accent: [220, 158, 92],
            text: [220, 210, 195],
            viewport: [35, 29, 25],
            ..Self::default()
        }
    }
    fn validate(&self) -> Result<(), String> {
        if !(2..=16).contains(&self.spacing) || self.rounding > 16 {
            return Err("Invalid workspace appearance".into());
        }
        Ok(())
    }
}
pub(crate) fn viewport_background(ctx: &egui::Context) -> egui::Color32 {
    ctx.data(|d| d.get_temp::<egui::Color32>(egui::Id::new("workspace_viewport_background")))
        .unwrap_or(egui::Color32::from_rgb(23, 26, 31))
}
/// Stable identities for built-in menus displayed as popups, windows or sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum BuiltinMenu {
    References,
    Brushes,
    Transform,
    Slice,
    Primitive,
    Edit,
    Settings,
    Visibility,
    Mesh,
    Submodels,
    Wireframe,
    Symmetry,
    Mask,
    View,
    Modeling,
}
impl BuiltinMenu {
    pub fn label(self) -> &'static str {
        match self {
            Self::References => "References",
            Self::Brushes => "Add brush",
            Self::Transform => "Transform",
            Self::Slice => "Slice",
            Self::Primitive => "Primitive",
            Self::Edit => "Edit",
            Self::Settings => "Settings",
            Self::Visibility => "Visibility",
            Self::Mesh => "Mesh",
            Self::Submodels => "Submodels",
            Self::Wireframe => "Wire settings",
            Self::Symmetry => "Symmetry",
            Self::Mask => "Mask",
            Self::View => "View",
            Self::Modeling => "Modeling",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PinnedMenu {
    pub menu: BuiltinMenu,
    pub position: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SidebarSection {
    Tool(Tool),
    Custom(u64),
    Builtin(BuiltinMenu),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct MenuSection {
    pub menu: BuiltinMenu,
    pub dock: Dock,
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Workspace {
    pub name: String,
    pub panels: Vec<Panel>,
    pub menus: Vec<CustomMenu>,
    #[serde(default = "default_viewport_controls")]
    pub viewport_controls: Vec<ViewportControl>,
    #[serde(default)]
    pub pinned_menus: Vec<PinnedMenu>,
    #[serde(default)]
    pub menu_sections: Vec<MenuSection>,
    #[serde(default)]
    pub sidebar_order: Vec<SidebarSection>,
    pub left_width: f32,
    pub right_width: f32,
    pub compact_toolbar: bool,
    #[serde(default = "default_shelf_sizes")]
    pub shelf_sizes: [f32; 4],
    #[serde(default)]
    pub appearance: Appearance,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ModelingPreset {
    pub id: u64,
    pub name: String,
    pub options: crate::viewport::ModelingOptions,
}
/// The shared selection hold follows the mode chosen in the X menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SelectionMode {
    Mask,
    #[default]
    Visibility,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    #[serde(default)]
    pub gimp_path: String,
    pub external_paint_app: String,
    pub materials: MaterialLibrary,
    pub paint_inspection: crate::viewport::InspectionMode,
    pub lighting: crate::lighting::Lighting,
    #[serde(default)]
    pub paint_shortcuts_version: u32,
    #[serde(default)]
    pub brush_controls_version: u32,
    pub selection_mode: SelectionMode,
    pub visibility_selection: VisibilitySettings,
    pub modeling_presets: Vec<ModelingPreset>,
    pub next_modeling_preset_id: u64,
    pub recovery: crate::recovery::Settings,
    pub history_retention: crate::diagnostics::Retention,
    pub version: u32,
    pub bindings: Vec<(Action, Option<Shortcut>)>,
    pub mask_hold: HoldBinding,
    pub mask_preset: MaskPreset,
    pub visibility_hold: HoldBinding,
    pub smooth: Hold,
    pub reverse: Hold,
    pub zoom_hold: Hold,
    pub pan_hold: Hold,
    pub touch: TouchSettings,
    pub panels: Vec<Panel>,
    pub left_width: f32,
    pub right_width: f32,
    pub menus: Vec<CustomMenu>,
    #[serde(default = "default_viewport_controls")]
    pub viewport_controls: Vec<ViewportControl>,
    pub pinned_menus: Vec<PinnedMenu>,
    #[serde(default)]
    pub menu_sections: Vec<MenuSection>,
    #[serde(default)]
    pub sidebar_order: Vec<SidebarSection>,
    pub workspaces: Vec<Workspace>,
    pub next_menu_id: u64,
    pub active_workspace: Option<usize>,
    pub paint_workspace_backup: Option<(Workspace, Option<usize>)>,
    pub compact_toolbar: bool,
    #[serde(default = "default_shelf_sizes")]
    pub shelf_sizes: [f32; 4],
    #[serde(default)]
    pub appearance: Appearance,
}
impl Default for Preferences {
    fn default() -> Self {
        let mut prefs = Self {
            gimp_path: String::new(),
            external_paint_app: String::new(),
            materials: MaterialLibrary::default(),
            paint_inspection: Default::default(),
            lighting: Default::default(),
            paint_shortcuts_version: 1,
            brush_controls_version: 2,
            visibility_selection: VisibilitySettings::default(),
            modeling_presets: Vec::new(),
            next_modeling_preset_id: 1,
            recovery: Default::default(),
            history_retention: Default::default(),
            version: 1,
            touch: TouchSettings::default(),
            bindings: Action::ALL
                .iter()
                .cloned()
                .map(|action| {
                    let shortcut = action.default_shortcut();
                    (action, shortcut)
                })
                .collect(),
            selection_mode: SelectionMode::default(),
            mask_hold: HoldBinding::default(),
            mask_preset: MaskPreset::default(),
            visibility_hold: HoldBinding::visibility_default(),
            smooth: Hold::Shift,
            reverse: Hold::Alt,
            zoom_hold: Hold::Ctrl,
            pan_hold: Hold::Alt,
            panels: Tool::ALL
                .into_iter()
                .enumerate()
                .map(|(i, tool)| Panel {
                    tool,
                    dock: if matches!(tool, Tool::Mesh | Tool::Transform | Tool::Submodels) {
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
            menus: Vec::new(),
            viewport_controls: default_viewport_controls(),
            pinned_menus: Vec::new(),
            menu_sections: Vec::new(),
            sidebar_order: Vec::new(),
            workspaces: Vec::new(),
            next_menu_id: 1,
            active_workspace: None,
            paint_workspace_backup: None,
            appearance: Appearance::default(),
            compact_toolbar: false,
            shelf_sizes: default_shelf_sizes(),
        };
        prefs.add_brush_controls();
        prefs
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        self.touch.validate()?;
        self.materials.validate()?;
        self.lighting.validate()?;
        self.mask_hold.validate()?;
        self.visibility_hold.validate()?;
        if self.modeling_presets.len() > 128 || self.next_modeling_preset_id == 0 {
            return Err("Invalid modeling preset library".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for preset in &self.modeling_presets {
            if preset.id == 0
                || preset.id >= self.next_modeling_preset_id
                || !ids.insert(preset.id)
                || preset.name.trim().is_empty()
                || preset.name.len() > 128
            {
                return Err("Invalid modeling preset".into());
            }
            preset.options.validate()?;
        }
        if self.version != 1 {
            return Err("Unsupported preferences version".into());
        }
        if self.bindings.len() < Action::ALL.len() || self.panels.len() != Tool::ALL.len() {
            return Err("Incomplete preferences".into());
        }
        for action in Action::ALL {
            if self.bindings.iter().filter(|(a, _)| a == action).count() != 1 {
                return Err("Duplicate or missing key action".into());
            }
        }
        for tool in Tool::ALL {
            if self.panels.iter().filter(|p| p.tool == tool).count() != 1 {
                return Err("Duplicate or missing tool panel".into());
            }
        }
        self.recovery.validate()?;
        self.history_retention.validate()?;
        self.appearance.validate()?;
        if self
            .active_workspace
            .is_some_and(|index| index >= self.workspaces.len())
        {
            return Err("Unknown active workspace".into());
        }
        validate_menus(&self.menus)?;
        viewport_controls::validate(&self.viewport_controls)?;
        for (index, pinned) in self.pinned_menus.iter().enumerate() {
            if self.pinned_menus[..index]
                .iter()
                .any(|p| p.menu == pinned.menu)
                || pinned
                    .position
                    .iter()
                    .any(|p| !p.is_finite() || p.abs() > 100_000.)
            {
                return Err("Invalid pinned menu position or duplicate menu".into());
            }
        }
        for (index, section) in self.menu_sections.iter().enumerate() {
            if !matches!(section.dock, Dock::Left | Dock::Right)
                || self.menu_sections[..index]
                    .iter()
                    .any(|s| s.menu == section.menu)
                || self.pinned_menus.iter().any(|p| p.menu == section.menu)
            {
                return Err("Invalid or duplicate sidebar menu".into());
            }
        }
        for (index, section) in self.sidebar_order.iter().enumerate() {
            if self.sidebar_order[..index].contains(section) {
                return Err("Duplicate sidebar section order".into());
            }
        }
        for workspace in self.workspaces.iter().chain(
            self.paint_workspace_backup
                .iter()
                .map(|(workspace, _)| workspace),
        ) {
            let prefs = Self {
                panels: workspace.panels.clone(),
                menus: workspace.menus.clone(),
                viewport_controls: workspace.viewport_controls.clone(),
                pinned_menus: workspace.pinned_menus.clone(),
                menu_sections: workspace.menu_sections.clone(),
                sidebar_order: workspace.sidebar_order.clone(),
                left_width: workspace.left_width,
                right_width: workspace.right_width,
                shelf_sizes: workspace.shelf_sizes,
                appearance: workspace.appearance.clone(),
                ..Self::default()
            };
            prefs.validate()?;
        }
        for (i, (action, binding)) in self.bindings.iter().enumerate() {
            if self.bindings[..i].iter().any(|(a, _)| a == action) {
                return Err("Duplicate key action".into());
            }
            if let Some(b) = binding {
                if Key::from_name(&b.key).is_none() || b.key == "Escape" {
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
        if self
            .shelf_sizes
            .iter()
            .any(|v| !v.is_finite() || !(24. ..=800.).contains(v))
        {
            return Err("Invalid shelf size".into());
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
    /// Preserve the original custom-menu-first layout until the user reorders it.
    fn ordered_sections(&self) -> Vec<SidebarSection> {
        let available: Vec<_> = self
            .menu_sections
            .iter()
            .map(|s| SidebarSection::Builtin(s.menu))
            .chain(self.menus.iter().map(|m| SidebarSection::Custom(m.id)))
            .chain(self.panels.iter().map(|p| SidebarSection::Tool(p.tool)))
            .collect();
        let mut order = Vec::new();
        for section in self.sidebar_order.iter().chain(&available) {
            if available.contains(section) && !order.contains(section) {
                order.push(*section);
            }
        }
        order
    }
    pub fn section_dock(&self, section: SidebarSection) -> Option<Dock> {
        match section {
            SidebarSection::Tool(tool) => {
                self.panels.iter().find(|p| p.tool == tool).map(|p| p.dock)
            }
            SidebarSection::Builtin(menu) => self
                .menu_sections
                .iter()
                .find(|s| s.menu == menu)
                .map(|s| s.dock),
            SidebarSection::Custom(id) => {
                self.menus
                    .iter()
                    .find(|m| m.id == id)
                    .and_then(|m| match m.placement {
                        Placement::LeftPanel => Some(Dock::Left),
                        Placement::RightPanel => Some(Dock::Right),
                        _ => None,
                    })
            }
        }
    }
    pub fn sidebar_sections(&self, dock: Dock) -> Vec<SidebarSection> {
        self.ordered_sections()
            .into_iter()
            .filter(|s| self.section_dock(*s) == Some(dock))
            .collect()
    }
    pub fn move_section(&mut self, section: SidebarSection, down: bool) {
        let Some(dock @ (Dock::Left | Dock::Right)) = self.section_dock(section) else {
            return;
        };
        let visible = self.sidebar_sections(dock);
        let Some(index) = visible.iter().position(|s| *s == section) else {
            return;
        };
        let target = if down {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        };
        let Some(other) = target.and_then(|i| visible.get(i)) else {
            return;
        };
        let mut order = self.ordered_sections();
        let a = order.iter().position(|s| *s == section).unwrap();
        let b = order.iter().position(|s| s == other).unwrap();
        order.swap(a, b);
        self.sidebar_order = order;
    }
    pub fn menu_placed(&self, menu: BuiltinMenu) -> bool {
        self.pinned_menus.iter().any(|p| p.menu == menu)
            || self.menu_sections.iter().any(|s| s.menu == menu)
    }
    fn hide_duplicate_tool(&mut self, menu: BuiltinMenu) {
        if menu == BuiltinMenu::Transform
            && let Some(panel) = self.panels.iter_mut().find(|p| p.tool == Tool::Transform)
        {
            panel.dock = Dock::Hidden;
        }
    }
    pub fn pin_menu(&mut self, menu: BuiltinMenu, position: [f32; 2]) {
        self.undock_menu(menu);
        self.pinned_menus.retain(|p| p.menu != menu);
        self.hide_duplicate_tool(menu);
        self.pinned_menus.push(PinnedMenu { menu, position });
    }
    pub fn dock_menu(&mut self, menu: BuiltinMenu, dock: Dock) {
        if !matches!(dock, Dock::Left | Dock::Right) {
            return;
        }
        self.pinned_menus.retain(|p| p.menu != menu);
        self.hide_duplicate_tool(menu);
        if let Some(section) = self.menu_sections.iter_mut().find(|s| s.menu == menu) {
            section.dock = dock;
        } else {
            self.menu_sections.push(MenuSection {
                menu,
                dock,
                open: true,
            });
        }
        let section = SidebarSection::Builtin(menu);
        self.sidebar_order = self.ordered_sections();
        self.sidebar_order.retain(|s| *s != section);
        self.sidebar_order.insert(0, section);
    }
    pub fn undock_menu(&mut self, menu: BuiltinMenu) {
        self.menu_sections.retain(|s| s.menu != menu);
        self.sidebar_order
            .retain(|s| *s != SidebarSection::Builtin(menu));
    }
    pub fn bind(&mut self, action: Action, shortcut: Shortcut) -> Result<(), String> {
        if Key::from_name(&shortcut.key).is_none() || shortcut.key == "Escape" {
            return Err("Choose a valid key; Escape is reserved for cancelling gestures.".into());
        }
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
        } else {
            self.bindings.push((action, Some(shortcut)));
        }
        Ok(())
    }
    pub fn shortcut(&self, action: &Action) -> Option<&Shortcut> {
        self.bindings
            .iter()
            .find(|(a, _)| a == action)
            .and_then(|(_, b)| b.as_ref())
    }
    pub fn clear_binding(&mut self, action: &Action) {
        if let Some((_, binding)) = self.bindings.iter_mut().find(|(a, _)| a == action) {
            *binding = None;
        }
    }
    pub fn reset_key_bindings(&mut self) {
        self.bindings = Action::ALL
            .iter()
            .cloned()
            .map(|action| {
                let shortcut = action.default_shortcut();
                (action, shortcut)
            })
            .collect();
        if let Some(menu) = self
            .menus
            .iter()
            .find(|m| m.items.iter().any(|i| i.action == Some(Action::BrushSize)))
        {
            self.bindings.push((
                Action::OpenMenu(menu.id),
                Some(Shortcut::new(Key::S, Modifiers::NONE)),
            ));
        }
    }
    pub fn actions(&self, ctx: &egui::Context) -> Vec<Action> {
        ctx.input_mut(|input| {
            let mut actions = Vec::new();
            input.events.retain(|event| {
                let Some(shortcut) = Shortcut::from_event(event) else {
                    return true;
                };
                if let Some((action, _)) = self
                    .bindings
                    .iter()
                    .find(|(_, b)| b.as_ref() == Some(&shortcut))
                {
                    if matches!(action, Action::OpenMenu(_))
                        && matches!(event, egui::Event::Key { repeat: true, .. })
                    {
                        return false;
                    }
                    if !actions.contains(action) {
                        actions.push(action.clone());
                    }
                    false
                } else {
                    true
                }
            });
            actions
        })
    }
    pub fn add_menu(&mut self) -> u64 {
        let id = self
            .menus
            .iter()
            .chain(self.workspaces.iter().flat_map(|w| &w.menus))
            .map(|m| m.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(self.next_menu_id);
        self.next_menu_id = id.saturating_add(1);
        self.menus.push(CustomMenu {
            id,
            name: format!("Custom {id}"),
            placement: Placement::Menu,
            columns: 2,
            position: [350., 160.],
            items: Vec::new(),
            collapsible: true,
            expanded: true,
        });
        id
    }
    fn add_brush_controls(&mut self) {
        let id = self.add_menu();
        let menu = self.menus.last_mut().unwrap();
        menu.name = "Brush controls".into();
        menu.placement = Placement::LeftPanel;
        menu.columns = 1;
        menu.collapsible = true;
        menu.items = [
            Action::BrushIcon,
            Action::BrushSize,
            Action::BrushInnerRadius,
            Action::BrushStrength,
            Action::SurfaceFootprint,
            Action::StrokeMode,
            Action::StrokeStabilisation,
        ]
        .into_iter()
        .map(CustomItem::action)
        .collect();
        let shortcut = Shortcut::new(Key::S, Modifiers::NONE);
        if !self
            .bindings
            .iter()
            .any(|(_, b)| b.as_ref() == Some(&shortcut))
        {
            self.bindings.push((Action::OpenMenu(id), Some(shortcut)));
        }
        // Existing named workspaces gain the strip once as well; later edits are respected.
        let menu = self.menus.last().unwrap().clone();
        for workspace in &mut self.workspaces {
            workspace.menus.push(menu.clone());
        }
    }
    /// A reversible layout preset; keep the backup in preferences so a restart
    /// cannot strand the artist without their original workspace.
    pub fn focus_paint_workspace(&mut self) {
        if self.paint_workspace_backup.is_some() {
            return;
        }
        self.sync_active_workspace();
        self.paint_workspace_backup = Some((
            self.workspace("Before Paint focus".into()),
            self.active_workspace,
        ));
        self.active_workspace = None;
        for panel in &mut self.panels {
            panel.dock = if panel.tool == Tool::Brush {
                Dock::Right
            } else {
                Dock::Hidden
            };
            panel.open = panel.tool == Tool::Brush;
        }
        for menu in &mut self.menus {
            if menu.placement != Placement::Hidden {
                menu.placement = Placement::Menu;
            }
        }
        self.pinned_menus.clear();
        self.menu_sections.clear();
        self.sidebar_order.clear();
        self.right_width = 320.;
        self.compact_toolbar = true;
    }
    pub fn restore_paint_workspace(&mut self) {
        if let Some((workspace, active)) = self.paint_workspace_backup.take() {
            self.apply_workspace(&workspace);
            self.active_workspace = active.filter(|index| *index < self.workspaces.len());
        }
    }

    pub fn workspace(&self, name: String) -> Workspace {
        Workspace {
            name,
            panels: self.panels.clone(),
            menus: self.menus.clone(),
            viewport_controls: self.viewport_controls.clone(),
            pinned_menus: self.pinned_menus.clone(),
            menu_sections: self.menu_sections.clone(),
            sidebar_order: self.sidebar_order.clone(),
            left_width: self.left_width,
            right_width: self.right_width,
            compact_toolbar: self.compact_toolbar,
            shelf_sizes: self.shelf_sizes,
            appearance: self.appearance.clone(),
        }
    }
    pub fn apply_workspace(&mut self, workspace: &Workspace) {
        self.panels = workspace.panels.clone();
        self.menus = workspace.menus.clone();
        self.viewport_controls = workspace.viewport_controls.clone();
        self.pinned_menus = workspace.pinned_menus.clone();
        self.menu_sections = workspace.menu_sections.clone();
        self.sidebar_order = workspace.sidebar_order.clone();
        self.left_width = workspace.left_width;
        self.right_width = workspace.right_width;
        self.compact_toolbar = workspace.compact_toolbar;
        self.shelf_sizes = workspace.shelf_sizes;
        self.appearance = workspace.appearance.clone();
    }
    pub fn sync_active_workspace(&mut self) {
        if let Some(index) = self.active_workspace
            && let Some(saved) = self.workspaces.get(index)
        {
            let current = self.workspace(saved.name.clone());
            if self.workspaces[index] != current {
                self.workspaces[index] = current;
            }
        }
    }
    pub fn save_workspace_as(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.len() > 128 {
            return Err("Enter a workspace name (up to 128 bytes).".into());
        }
        if self.workspaces.iter().any(|w| w.name == name) {
            return Err("That workspace name already exists. Choose another name.".into());
        }
        self.sync_active_workspace();
        self.workspaces.push(self.workspace(name.into()));
        self.active_workspace = Some(self.workspaces.len() - 1);
        Ok(())
    }
    pub fn switch_workspace(&mut self, index: usize) {
        if index >= self.workspaces.len() {
            return;
        }
        self.sync_active_workspace();
        let workspace = self.workspaces[index].clone();
        self.apply_workspace(&workspace);
        self.active_workspace = Some(index);
    }
    pub fn delete_workspace(&mut self, index: usize) {
        if index >= self.workspaces.len() {
            return;
        }
        self.workspaces.remove(index);
        self.active_workspace = self.active_workspace.and_then(|active| {
            if active == index {
                None
            } else {
                Some(if active > index { active - 1 } else { active })
            }
        });
    }
}
fn default_shelf_sizes() -> [f32; 4] {
    [84., 84., 220., 220.]
}
fn validate_menus(menus: &[CustomMenu]) -> Result<(), String> {
    for (i, menu) in menus.iter().enumerate() {
        if menu.id == 0
            || menus[..i].iter().any(|m| m.id == menu.id)
            || !(1..=8).contains(&menu.columns)
            || menu
                .position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 100_000.)
        {
            return Err("Invalid custom menu layout".into());
        }
    }
    Ok(())
}
pub(crate) fn path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("AppData/Roaming"))
        });
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    base.unwrap_or_else(|| PathBuf::from("."))
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
    if prefs.paint_shortcuts_version == 0 {
        let shortcut = Action::PolygroupColors.default_shortcut();
        if !prefs.bindings.iter().any(|(_, key)| *key == shortcut)
            && let Some((_, key)) = prefs
                .bindings
                .iter_mut()
                .find(|(a, _)| *a == Action::PolygroupColors)
            && key.is_none()
        {
            *key = shortcut;
        }
        prefs.paint_shortcuts_version = 1;
    }
    // This update introduces T and moves the stock selection-menu key from W to X.
    // Explicit unbindings and non-default assignments remain intact.
    if !prefs
        .bindings
        .iter()
        .any(|(a, _)| *a == Action::UniversalTransform)
    {
        let previous = prefs
            .bindings
            .iter()
            .find(|(a, _)| *a == Action::MaskVisibilityMenu);
        let had_selection_menu = previous.is_some();
        let migrate = previous.is_none()
            || previous.is_some_and(|(_, key)| {
                key.as_ref() == Some(&Shortcut::new(Key::W, Modifiers::NONE))
            });
        if migrate {
            if prefs.shortcut(&Action::Symmetry) == Some(&Shortcut::new(Key::X, Modifiers::NONE)) {
                let replacement = Shortcut::new(Key::X, Modifiers::SHIFT);
                let free = !prefs
                    .bindings
                    .iter()
                    .any(|(_, key)| key.as_ref() == Some(&replacement));
                if let Some((_, key)) = prefs
                    .bindings
                    .iter_mut()
                    .find(|(a, _)| *a == Action::Symmetry)
                {
                    *key = free.then_some(replacement);
                }
            }
            let replacement = Shortcut::new(Key::X, Modifiers::NONE);
            let free = !prefs
                .bindings
                .iter()
                .any(|(_, key)| key.as_ref() == Some(&replacement));
            if free
                && let Some((_, key)) = prefs
                    .bindings
                    .iter_mut()
                    .find(|(a, _)| *a == Action::MaskVisibilityMenu)
            {
                *key = Some(replacement);
            }
        }
        // Restore the displaced stock wireframe key only after W is free.
        let wireframe = Shortcut::new(Key::W, Modifiers::NONE);
        if had_selection_menu
            && prefs.shortcut(&Action::Wireframe) == Some(&Shortcut::new(Key::W, Modifiers::SHIFT))
            && !prefs
                .bindings
                .iter()
                .any(|(_, key)| key.as_ref() == Some(&wireframe))
            && let Some((_, key)) = prefs
                .bindings
                .iter_mut()
                .find(|(a, _)| *a == Action::Wireframe)
        {
            *key = Some(wireframe);
        }
    }
    // Add newly introduced actions without stealing existing shortcuts or clearing explicit unbindings.
    for action in Action::ALL.iter().cloned() {
        let mut shortcut = action.default_shortcut();
        if !prefs.bindings.iter().any(|(a, _)| *a == action) {
            if shortcut.is_some() && prefs.bindings.iter().any(|(_, b)| *b == shortcut) {
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
    for tool in [Tool::Transform, Tool::Submodels] {
        if !prefs.panels.iter().any(|p| p.tool == tool) {
            prefs.panels.push(Panel {
                tool,
                dock: Dock::Right,
                open: true,
                position: [600., 180.],
            });
        }
    }
    if prefs.brush_controls_version == 0 {
        // A file with no menus field has already received the new default menu.
        if !prefs.menus.iter().any(|menu| {
            menu.items
                .iter()
                .any(|item| item.action == Some(Action::BrushSize))
        }) {
            prefs.add_brush_controls();
        } else if let Some(menu) = prefs
            .menus
            .iter()
            .find(|m| m.items.iter().any(|i| i.action == Some(Action::BrushSize)))
        {
            let action = Action::OpenMenu(menu.id);
            let shortcut = Shortcut::new(Key::S, Modifiers::NONE);
            if !prefs
                .bindings
                .iter()
                .any(|(a, b)| *a == action || b.as_ref() == Some(&shortcut))
            {
                prefs.bindings.push((action, Some(shortcut)));
            }
        }
        prefs.brush_controls_version = 1;
    }
    if prefs.brush_controls_version < 2 {
        for menu in prefs
            .menus
            .iter_mut()
            .chain(prefs.workspaces.iter_mut().flat_map(|w| &mut w.menus))
        {
            let stock = [
                Action::BrushIcon,
                Action::BrushSize,
                Action::BrushStrength,
                Action::SurfaceFootprint,
                Action::StrokeMode,
                Action::StrokeStabilisation,
            ];
            if menu.name == "Brush controls"
                && menu.placement == Placement::Top
                && menu.items
                    == stock
                        .into_iter()
                        .map(CustomItem::action)
                        .collect::<Vec<_>>()
            {
                menu.placement = Placement::LeftPanel;
                menu.columns = 1;
                menu.collapsible = true;
            }
            if menu
                .items
                .iter()
                .any(|i| i.action == Some(Action::BrushIcon))
                && !menu
                    .items
                    .iter()
                    .any(|i| i.action == Some(Action::BrushInnerRadius))
                && let Some(index) = menu
                    .items
                    .iter()
                    .position(|i| i.action == Some(Action::BrushSize))
            {
                menu.items
                    .insert(index + 1, CustomItem::action(Action::BrushInnerRadius));
            }
        }
        prefs.brush_controls_version = 2;
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
                        modifiers: Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                assert_eq!(prefs.actions(ctx), vec![Action::Save, Action::Wireframe]);
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
        old.panels
            .retain(|p| !matches!(p.tool, Tool::Mesh | Tool::Transform | Tool::Submodels));
        old.bindings.truncate(13);
        old.panels[0].dock = Dock::Floating;
        crate::persistence::write_json(&path.to_string_lossy(), &old).unwrap();
        let upgraded = load(&path).unwrap();
        assert_eq!(&upgraded.panels[..5], &old.panels);
        assert_eq!(upgraded.panels[5].tool, Tool::Mesh);
        assert_eq!(upgraded.panels[5].dock, Dock::Right);
        assert_eq!(&upgraded.bindings[..13], &old.bindings);
        assert_eq!(upgraded.bindings.len(), Action::ALL.len());
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod zoom_migration_tests {
    use super::*;
    #[test]
    fn zoom_bindings_upgrade_existing_preferences_without_overwriting_conflicts() {
        let path = std::env::temp_dir().join(format!("clay-zoom-keys-{}.json", std::process::id()));
        let mut old = Preferences::default();
        old.bindings.truncate(17);
        old.bind(
            Action::Wireframe,
            Shortcut::new(Key::Equals, Modifiers::NONE),
        )
        .unwrap();
        crate::persistence::write_json(&path.to_string_lossy(), &old).unwrap();
        let upgraded = load(&path).unwrap();
        assert_eq!(&upgraded.bindings[..17], &old.bindings);
        assert!(
            upgraded
                .bindings
                .iter()
                .find(|(a, _)| *a == Action::ZoomIn)
                .unwrap()
                .1
                .is_none()
        );
        assert!(
            upgraded
                .bindings
                .iter()
                .find(|(a, _)| *a == Action::ZoomOut)
                .unwrap()
                .1
                .is_some()
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod zoom_hold_tests {
    use super::*;
    #[test]
    fn older_preferences_get_ctrl_and_custom_hold_is_saved() {
        let path = std::env::temp_dir().join(format!("clay-zoom-hold-{}.json", std::process::id()));
        let mut old = serde_json::to_value(Preferences::default()).unwrap();
        old.as_object_mut().unwrap().remove("zoom_hold");
        crate::persistence::write_json(&path.to_string_lossy(), &old).unwrap();
        let mut upgraded = load(&path).unwrap();
        assert_eq!(upgraded.zoom_hold, Hold::Ctrl);
        upgraded.zoom_hold = Hold::Alt;
        save(&path, &upgraded).unwrap();
        assert_eq!(load(&path).unwrap().zoom_hold, Hold::Alt);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod customization_tests {
    use super::*;
    #[test]
    fn all_twenty_number_row_bindings_are_distinct_and_consumed() {
        let digits = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
            Key::Num0,
        ];
        let mut prefs = Preferences::default();
        for (_, binding) in &mut prefs.bindings {
            *binding = None;
        }
        let mut events = Vec::new();
        for (i, modifiers) in [Modifiers::NONE, Modifiers::SHIFT].into_iter().enumerate() {
            for (j, key) in digits.into_iter().enumerate() {
                prefs
                    .bind(
                        Action::ALL[i * 10 + j].clone(),
                        Shortcut::new(key, modifiers),
                    )
                    .unwrap();
                events.push(egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                });
                events.push(egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: false,
                    repeat: false,
                    modifiers,
                });
            }
        }
        prefs.validate().unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                assert_eq!(prefs.actions(ctx), Action::ALL[..20]);
                assert!(prefs.actions(ctx).is_empty());
            },
        );
        let shifted = egui::Event::Key {
            key: Key::Exclamationmark,
            physical_key: Some(Key::Num1),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        };
        assert_eq!(
            Shortcut::from_event(&shifted),
            Some(Shortcut::new(Key::Num1, Modifiers::SHIFT))
        );
    }
    #[test]
    fn custom_layouts_brush_bindings_and_old_preferences_round_trip() {
        let path =
            std::env::temp_dir().join(format!("clay-custom-interface-{}.json", std::process::id()));
        let mut prefs = Preferences::default();
        prefs.menus.clear();
        prefs
            .bindings
            .retain(|(a, _)| !matches!(a, Action::OpenMenu(_)));
        let id = prefs.add_menu();
        prefs.pan_hold = Hold::Shift;
        prefs.menus[0].name = "My sculpt tools".into();
        prefs.menus[0].placement = Placement::Floating;
        prefs.menus[0].items = vec![
            CustomItem::action(Action::ClearMask),
            CustomItem {
                action: None,
                label: String::new(),
            },
            CustomItem::action(Action::SelectBrush("stable-brush-id".into())),
        ];
        prefs
            .bind(
                Action::SelectBrush("stable-brush-id".into()),
                Shortcut::new(Key::Num1, Modifiers::NONE),
            )
            .unwrap();
        prefs
            .bind(
                Action::OpenMenu(id),
                Shortcut::new(Key::Num1, Modifiers::SHIFT),
            )
            .unwrap();
        prefs.workspaces.push(prefs.workspace("Sculpting".into()));
        save(&path, &prefs).unwrap();
        assert_eq!(load(&path).unwrap(), prefs);
        prefs.menus.clear();
        let next = prefs.add_menu();
        assert_ne!(
            id, next,
            "deleted menu shortcuts must never target a new menu"
        );
        prefs.apply_workspace(&prefs.workspaces[0].clone());
        assert_eq!(prefs.menus[0].id, id);
        let mut old = serde_json::to_value(Preferences::default()).unwrap();
        let object = old.as_object_mut().unwrap();
        for key in [
            "menus",
            "brush_controls_version",
            "workspaces",
            "compact_toolbar",
            "next_menu_id",
            "pan_hold",
            "shelf_sizes",
            "appearance",
            "active_workspace",
        ] {
            object.remove(key);
        }
        object
            .get_mut("bindings")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .truncate(19);
        crate::persistence::write_json(&path.to_string_lossy(), &old).unwrap();
        let migrated = load(&path).unwrap();
        assert_eq!(migrated, Preferences::default());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn workspace_switching_remembers_live_edits_and_colours() {
        let mut prefs = Preferences::default();
        prefs.menus.clear();
        prefs
            .bindings
            .retain(|(a, _)| !matches!(a, Action::OpenMenu(_)));
        prefs.add_menu();
        prefs.menus[0].placement = Placement::LeftPanel;
        prefs.menus[0]
            .items
            .push(CustomItem::action(Action::ClearMask));
        prefs.save_workspace_as(" Sculpt ").unwrap();
        assert_eq!(prefs.workspaces[0].name, "Sculpt");
        prefs.save_workspace_as("Detail").unwrap();
        prefs.appearance = Appearance::light();
        prefs.menus[0].expanded = false;
        prefs.menus[0].placement = Placement::RightPanel;
        prefs.left_width = 310.;
        prefs.switch_workspace(0);
        assert_eq!(prefs.appearance, Appearance::default());
        assert!(prefs.menus[0].expanded);
        assert_eq!(prefs.menus[0].placement, Placement::LeftPanel);
        prefs.appearance = Appearance::warm();
        prefs.switch_workspace(1);
        assert_eq!(prefs.appearance, Appearance::light());
        assert!(!prefs.menus[0].expanded);
        assert_eq!(prefs.menus[0].placement, Placement::RightPanel);
        assert_eq!(prefs.left_width, 310.);
        let path =
            std::env::temp_dir().join(format!("clay-workspaces-{}.json", std::process::id()));
        save(&path, &prefs).unwrap();
        let mut restored = load(&path).unwrap();
        assert_eq!(prefs, restored);
        restored.switch_workspace(0);
        assert_eq!(restored.appearance, Appearance::warm());
        assert!(restored.save_workspace_as("Detail").is_err());
        restored.delete_workspace(1);
        assert_eq!(restored.active_workspace, Some(0));
        restored.delete_workspace(0);
        assert_eq!(restored.active_workspace, None);
        assert_eq!(restored.appearance, Appearance::warm());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn old_menus_and_saved_workspaces_gain_safe_defaults() {
        let mut prefs = Preferences::default();
        prefs.menus.clear();
        prefs
            .bindings
            .retain(|(a, _)| !matches!(a, Action::OpenMenu(_)));
        prefs.add_menu();
        prefs.save_workspace_as("Legacy").unwrap();
        let mut value = serde_json::to_value(prefs).unwrap();
        value["workspaces"][0]
            .as_object_mut()
            .unwrap()
            .remove("appearance");
        {
            let menu = &mut value["menus"][0];
            menu.as_object_mut().unwrap().remove("collapsible");
            menu.as_object_mut().unwrap().remove("expanded");
        }
        let restored: Preferences = serde_json::from_value(value).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.workspaces[0].appearance, Appearance::default());
        assert!(!restored.menus[0].collapsible);
        assert!(restored.menus[0].expanded);
    }
    #[test]
    fn duplicate_custom_bindings_and_invalid_layouts_are_rejected() {
        let mut prefs = Preferences::default();
        prefs.menus.clear();
        prefs
            .bindings
            .retain(|(a, _)| !matches!(a, Action::OpenMenu(_)));
        let id = prefs.add_menu();
        prefs
            .bind(
                Action::OpenMenu(id),
                Shortcut::new(Key::Num0, Modifiers::SHIFT),
            )
            .unwrap();
        assert!(
            prefs
                .bind(
                    Action::ClearMask,
                    Shortcut::new(Key::Num0, Modifiers::SHIFT)
                )
                .is_err()
        );
        prefs.menus[0].columns = 0;
        assert!(prefs.validate().is_err());
        prefs.menus[0].columns = 2;
        prefs.menus.push(prefs.menus[0].clone());
        assert!(prefs.validate().is_err());
    }
}

#[cfg(test)]
mod pinned_menu_tests {
    use super::*;
    #[test]
    fn pinned_menus_roundtrip_migrate_and_validate_saved_workspaces() {
        let mut prefs = Preferences::default();
        prefs.pinned_menus.push(PinnedMenu {
            menu: BuiltinMenu::Mask,
            position: [300., 200.],
        });
        prefs.save_workspace_as("Sculpt").unwrap();
        prefs.save_workspace_as("Detail").unwrap();
        prefs.pinned_menus[0].position = [500., 100.];
        prefs.switch_workspace(0);
        assert_eq!(prefs.pinned_menus[0].position, [300., 200.]);
        prefs.switch_workspace(1);
        assert_eq!(prefs.pinned_menus[0].position, [500., 100.]);
        prefs.validate().unwrap();
        let json = serde_json::to_string(&prefs).unwrap();
        assert_eq!(serde_json::from_str::<Preferences>(&json).unwrap(), prefs);
        let mut old = serde_json::to_value(&prefs).unwrap();
        old.as_object_mut().unwrap().remove("pinned_menus");
        for workspace in old["workspaces"].as_array_mut().unwrap() {
            workspace.as_object_mut().unwrap().remove("pinned_menus");
        }
        let migrated: Preferences = serde_json::from_value(old).unwrap();
        migrated.validate().unwrap();
        assert!(migrated.pinned_menus.is_empty());
        assert!(
            migrated
                .workspaces
                .iter()
                .all(|w| w.pinned_menus.is_empty())
        );
        prefs.workspaces[0].pinned_menus[0].position[0] = f32::NAN;
        assert!(prefs.validate().is_err());
        prefs.workspaces.clear();
        prefs.active_workspace = None;
        prefs.pinned_menus.push(prefs.pinned_menus[0].clone());
        assert!(prefs.validate().is_err());
    }
}

#[cfg(test)]
mod sidebar_tests {
    use super::*;

    #[test]
    fn mixed_sidebar_order_and_collapsed_menus_survive_workspace_switch_and_reload() {
        let mut prefs = Preferences::default();
        prefs.menus.clear();
        prefs
            .bindings
            .retain(|(a, _)| !matches!(a, Action::OpenMenu(_)));
        let id = prefs.add_menu();
        prefs.menus[0].placement = Placement::LeftPanel;
        prefs.save_workspace_as("Original").unwrap();
        prefs.save_workspace_as("Docked").unwrap();
        prefs.pinned_menus.push(PinnedMenu {
            menu: BuiltinMenu::Mask,
            position: [300., 200.],
        });
        prefs.dock_menu(BuiltinMenu::Mask, Dock::Left);
        assert!(prefs.pinned_menus.is_empty());
        prefs.move_section(SidebarSection::Builtin(BuiltinMenu::Mask), true);
        prefs.move_section(SidebarSection::Builtin(BuiltinMenu::Mask), true);
        prefs.menu_sections[0].open = false;
        let order = prefs.sidebar_sections(Dock::Left);
        assert_eq!(
            &order[..4],
            &[
                SidebarSection::Custom(id),
                SidebarSection::Tool(Tool::Library),
                SidebarSection::Builtin(BuiltinMenu::Mask),
                SidebarSection::Tool(Tool::Brush),
            ]
        );
        let right = prefs.sidebar_sections(Dock::Right);
        prefs.move_section(SidebarSection::Custom(id), false);
        assert_eq!(prefs.sidebar_sections(Dock::Left), order);
        assert_eq!(prefs.sidebar_sections(Dock::Right), right);
        prefs.switch_workspace(0);
        assert!(prefs.menu_sections.is_empty());
        prefs.switch_workspace(1);
        assert!(!prefs.menu_sections[0].open);
        assert_eq!(prefs.sidebar_sections(Dock::Left), order);
        prefs.validate().unwrap();
        let restored: Preferences =
            serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
        assert_eq!(restored, prefs);
        prefs.dock_menu(BuiltinMenu::Mask, Dock::Right);
        assert!(
            !prefs
                .sidebar_sections(Dock::Left)
                .contains(&SidebarSection::Builtin(BuiltinMenu::Mask))
        );
        prefs.undock_menu(BuiltinMenu::Mask);
        assert!(prefs.menu_sections.is_empty());
        prefs.validate().unwrap();
    }

    #[test]
    fn old_layouts_keep_their_order_and_invalid_menu_layouts_are_rejected() {
        let mut prefs = Preferences::default();
        prefs.panels.swap(0, 1);
        prefs.add_menu();
        prefs.menus[0].placement = Placement::LeftPanel;
        prefs.save_workspace_as("Legacy").unwrap();
        let expected = prefs.sidebar_sections(Dock::Left);
        let mut old = serde_json::to_value(&prefs).unwrap();
        for key in ["menu_sections", "sidebar_order"] {
            old.as_object_mut().unwrap().remove(key);
            old["workspaces"][0].as_object_mut().unwrap().remove(key);
        }
        let mut restored: Preferences = serde_json::from_value(old).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.sidebar_sections(Dock::Left), expected);
        restored.dock_menu(BuiltinMenu::Symmetry, Dock::Left);
        restored.menu_sections[0].dock = Dock::Hidden;
        assert!(restored.validate().is_err());
        restored.menu_sections[0].dock = Dock::Left;
        restored.pinned_menus.push(PinnedMenu {
            menu: BuiltinMenu::Symmetry,
            position: [0., 0.],
        });
        assert!(restored.validate().is_err());
        restored.pinned_menus.clear();
        restored.sidebar_order.push(restored.sidebar_order[0]);
        assert!(restored.validate().is_err());
    }
}

#[cfg(test)]
mod quick_menu_migration_tests {
    use super::*;
    #[test]
    fn quick_menu_migration_preserves_custom_bindings_and_moves_stock_symmetry() {
        for custom_x in [false, true] {
            let mut prefs = Preferences::default();
            prefs.bindings.retain(|(a, _)| {
                !matches!(
                    a,
                    Action::MaskVisibilityMenu | Action::TransformMenu | Action::UniversalTransform
                )
            });
            prefs
                .bind(
                    if custom_x {
                        Action::Guides
                    } else {
                        Action::Symmetry
                    },
                    Shortcut::new(Key::X, Modifiers::NONE),
                )
                .unwrap();
            let path = std::env::temp_dir().join(format!(
                "clay-quick-menu-{}-{custom_x}.json",
                std::process::id()
            ));
            crate::persistence::write_json(&path.to_string_lossy(), &prefs).unwrap();
            let loaded = load(&path).unwrap();
            loaded.validate().unwrap();
            assert_eq!(
                loaded.shortcut(&Action::TransformMenu),
                Some(&Shortcut::new(Key::E, Modifiers::NONE))
            );
            if custom_x {
                assert!(loaded.shortcut(&Action::MaskVisibilityMenu).is_none());
                assert_eq!(
                    loaded.shortcut(&Action::Guides),
                    prefs.shortcut(&Action::Guides)
                );
            } else {
                assert_eq!(
                    loaded.shortcut(&Action::MaskVisibilityMenu),
                    Some(&Shortcut::new(Key::X, Modifiers::NONE))
                );
                assert_eq!(
                    loaded.shortcut(&Action::Symmetry),
                    Some(&Shortcut::new(Key::X, Modifiers::SHIFT))
                );
            }
            save(&path, &loaded).unwrap();
            assert_eq!(load(&path).unwrap(), loaded);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn universal_key_upgrade_moves_previous_w_and_preserves_custom_t_and_unbinding() {
        for unbound in [false, true] {
            let mut prefs = Preferences::default();
            prefs
                .bindings
                .retain(|(a, _)| *a != Action::UniversalTransform);
            prefs
                .bind(Action::Wireframe, Shortcut::new(Key::W, Modifiers::SHIFT))
                .unwrap();
            prefs
                .bind(
                    Action::MaskVisibilityMenu,
                    Shortcut::new(Key::W, Modifiers::NONE),
                )
                .unwrap();
            prefs
                .bind(Action::Symmetry, Shortcut::new(Key::X, Modifiers::NONE))
                .unwrap();
            prefs
                .bind(Action::Guides, Shortcut::new(Key::T, Modifiers::NONE))
                .unwrap();
            if unbound {
                prefs.clear_binding(&Action::MaskVisibilityMenu);
            }
            let path = std::env::temp_dir().join(format!(
                "clay-universal-keys-{}-{unbound}.json",
                std::process::id()
            ));
            crate::persistence::write_json(&path.to_string_lossy(), &prefs).unwrap();
            let loaded = load(&path).unwrap();
            loaded.validate().unwrap();
            assert_eq!(
                loaded.shortcut(&Action::Wireframe),
                Some(&Shortcut::new(Key::W, Modifiers::NONE))
            );
            assert!(loaded.shortcut(&Action::UniversalTransform).is_none());
            assert_eq!(
                loaded.shortcut(&Action::Guides),
                prefs.shortcut(&Action::Guides)
            );
            if unbound {
                assert!(loaded.shortcut(&Action::MaskVisibilityMenu).is_none());
            } else {
                assert_eq!(
                    loaded.shortcut(&Action::MaskVisibilityMenu),
                    Some(&Shortcut::new(Key::X, Modifiers::NONE))
                );
                assert_eq!(
                    loaded.shortcut(&Action::Symmetry),
                    Some(&Shortcut::new(Key::X, Modifiers::SHIFT))
                );
            }
            save(&path, &loaded).unwrap();
            assert_eq!(load(&path).unwrap(), loaded);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn wireframe_upgrade_preserves_custom_keys_and_occupied_w() {
        for (binding, occupied) in [
            (None, false),
            (Some(Shortcut::new(Key::F2, Modifiers::NONE)), false),
            (Some(Shortcut::new(Key::W, Modifiers::SHIFT)), true),
        ] {
            let mut prefs = Preferences::default();
            prefs
                .bindings
                .retain(|(a, _)| *a != Action::UniversalTransform);
            prefs.clear_binding(&Action::Wireframe);
            if let Some(key) = binding.clone() {
                prefs.bind(Action::Wireframe, key).unwrap();
            }
            if occupied {
                prefs
                    .bind(Action::Guides, Shortcut::new(Key::W, Modifiers::NONE))
                    .unwrap();
            }
            let path = std::env::temp_dir().join(format!(
                "clay-wireframe-upgrade-{}.json",
                std::process::id()
            ));
            crate::persistence::write_json(&path.to_string_lossy(), &prefs).unwrap();
            let loaded = load(&path).unwrap();
            assert_eq!(loaded.shortcut(&Action::Wireframe), binding.as_ref());
            loaded.validate().unwrap();
            std::fs::remove_file(path).unwrap();
        }
    }
}

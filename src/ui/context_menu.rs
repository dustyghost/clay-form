//! Shared viewport tool chooser. Consumers supply actions; this popup never edits geometry.
use super::*;
use crate::viewport::{ModelTool, SelectionTool};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewportMenuContext {
    Modeling,
    Visibility,
    Mask,
}
#[derive(Default)]
pub(crate) struct ViewportMenuState {
    pub context: Option<ViewportMenuContext>,
    pub position: Option<egui::Pos2>,
    pub viewport_rect: Option<egui::Rect>,
    pub selected: usize,
    pub just_opened: bool,
    pub hold: Option<crate::preferences::HoldBinding>,
    pub suppress_visibility: bool,
    pub visibility_was_active: bool,
}
fn consume_menu_key(input: &mut egui::InputState, key: egui::Key) -> bool {
    let pressed = input.key_pressed(key);
    input.events.retain(
        |event| !matches!(event, egui::Event::Key { key: event_key, .. } if *event_key == key),
    );
    pressed
}
pub(crate) struct ViewportMenuEntry {
    pub action: Action,
    pub label: String,
    pub selected: bool,
    pub enabled: bool,
    pub hint: String,
}

/// Modifier-only chords and key-plus-modifier chords share one persisted representation.
pub(crate) fn hold_binding_editor(
    ui: &mut egui::Ui,
    label: &str,
    hold: &mut crate::preferences::HoldBinding,
) {
    ui.push_id(label, |ui| {
        ui.label(format!("{label}: {}", hold.label()));
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut hold.ctrl, "Ctrl");
            ui.checkbox(&mut hold.shift, "Shift");
            ui.checkbox(&mut hold.alt, "Alt");
            ui.checkbox(&mut hold.command, "Cmd");
            egui::ComboBox::from_id_salt("hold_key")
                .selected_text(hold.key.as_deref().unwrap_or("No key"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut hold.key, None, "No key");
                    for key in egui::Key::ALL
                        .iter()
                        .filter(|key| **key != egui::Key::Escape)
                    {
                        ui.selectable_value(&mut hold.key, Some(key.name().to_owned()), key.name());
                    }
                });
            if ui.button("Clear").clicked() {
                *hold = Default::default();
            }
            if ui.button("Reset").clicked() {
                *hold = if label == "Hold for visibility" {
                    crate::preferences::HoldBinding::visibility_default()
                } else {
                    Default::default()
                };
            }
        });
    });
}

impl UiState {
    pub(crate) fn viewport_menu_entries(
        &self,
        context: ViewportMenuContext,
        document: &Document,
    ) -> Vec<ViewportMenuEntry> {
        match context {
            ViewportMenuContext::Modeling => [
                ModelTool::Select,
                ModelTool::Move,
                ModelTool::Rotate,
                ModelTool::Scale,
                ModelTool::Extrude,
                ModelTool::Fuse,
                ModelTool::Inset,
                ModelTool::Bevel,
                ModelTool::LoopCut,
                ModelTool::Slide,
                ModelTool::EdgeExtrude,
                ModelTool::EdgeSplit,
                ModelTool::EdgeCollapse,
                ModelTool::PointExtrude,
                ModelTool::PointSplit,
                ModelTool::FacePoint,
                ModelTool::FaceSplit,
            ]
            .into_iter()
            .filter(|tool| {
                tool.required_mode()
                    .is_none_or(|mode| mode == self.modeling.mode)
            })
            .map(|tool| {
                let action = Action::ModelTool(tool);
                let enabled = self.action_enabled(&action, document);
                ViewportMenuEntry {
                    action,
                    label: tool.label().into(),
                    selected: self.modeling.tool == tool,
                    enabled,
                    hint: if !enabled {
                        "Convert the primitive and select a visible editable model.".into()
                    } else if self.modeling.count == 0 && tool != ModelTool::Select {
                        "Choose this function, then select or target components in the viewport."
                            .into()
                    } else {
                        "Keeps the current component selection. No edit until you use the tool."
                            .into()
                    },
                }
            })
            .collect(),
            ViewportMenuContext::Visibility => [
                (
                    Action::TemporaryVisibilityRectangle,
                    SelectionTool::Rectangle,
                    "Rectangle",
                ),
                (
                    Action::TemporaryVisibilityLasso,
                    SelectionTool::Lasso,
                    "Lasso",
                ),
            ]
            .into_iter()
            .map(|(action, tool, label)| ViewportMenuEntry {
                action,
                label: label.into(),
                selected: self.temporary_visibility_tool == tool,
                enabled: document.primitive().is_none() && document.active_visible(),
                hint: "Drag to isolate; Alt hides. This shape is remembered for the next hold."
                    .into(),
            })
            .collect(),
            ViewportMenuContext::Mask => [
                (Action::MaskLasso, "Lasso"),
                (Action::MaskLine, "Through-depth line"),
            ]
            .into_iter()
            .map(|(action, label)| ViewportMenuEntry {
                enabled: self.action_enabled(&action, document),
                action,
                label: label.into(),
                selected: false,
                hint: "Choose the masking shape without painting.".into(),
            })
            .collect(),
        }
    }

    pub(crate) fn viewport_menu_popup(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
    ) -> (Option<Action>, bool) {
        let Some(context) = self.viewport_menu.context else {
            return (None, false);
        };
        let just_opened = std::mem::take(&mut self.viewport_menu.just_opened);
        let entries = self.viewport_menu_entries(context, document);
        let cancelled = ctx.input(|i| {
            !i.focused
                || i.key_pressed(egui::Key::Escape)
                || self
                    .viewport_menu
                    .hold
                    .as_ref()
                    .is_some_and(|hold| !hold.active(i))
        });
        if cancelled {
            ctx.input_mut(|i| {
                consume_menu_key(i, egui::Key::Escape);
            });
            self.viewport_menu.context = None;
            return (None, true);
        }
        let mut action = None;
        let count = entries.len();
        let mut index = self.viewport_menu.selected.min(count.saturating_sub(1));
        ctx.input_mut(|i| {
            if count > 0 {
                if consume_menu_key(i, egui::Key::ArrowDown) {
                    index = (index + 1) % count;
                }
                if consume_menu_key(i, egui::Key::ArrowUp) {
                    index = (index + count - 1) % count;
                }
                if consume_menu_key(i, egui::Key::Enter) && entries[index].enabled {
                    action = Some(entries[index].action.clone());
                }
            }
        });
        self.viewport_menu.selected = index;
        let pos = self
            .viewport_menu
            .position
            .unwrap_or(ctx.content_rect().center());
        let position = egui::pos2(
            pos.x
                .min(ctx.content_rect().right() - 280.)
                .max(ctx.content_rect().left()),
            pos.y
                .min(ctx.content_rect().bottom() - 420.)
                .max(ctx.content_rect().top()),
        );
        let mut open = true;
        let response = egui::Window::new(match context {
            ViewportMenuContext::Modeling => "Modeling functions",
            ViewportMenuContext::Visibility => "Visibility selection",
            ViewportMenuContext::Mask => "Masking",
        })
        .id(egui::Id::new("viewport_tool_menu"))
        .open(&mut open)
        .fixed_pos(position)
        .resizable(false)
        .collapsible(false)
        .default_width(260.)
        .max_height((ctx.content_rect().height() - 50.).max(120.))
        .vscroll(true)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.weak("Up/Down choose · Enter apply · Escape close");
            for (i, entry) in entries.iter().enumerate() {
                let label = if entry.selected {
                    format!("{} (active)", entry.label)
                } else {
                    entry.label.clone()
                };
                let response = ui
                    .add_enabled(
                        entry.enabled,
                        egui::Button::new(label)
                            .selected(i == index)
                            .min_size(egui::vec2(230., 32.)),
                    )
                    .on_hover_text(&entry.hint);
                if response.clicked() {
                    action = Some(entry.action.clone());
                }
            }
        });
        let outside = !just_opened
            && response.is_some_and(|r| {
                ctx.input(|i| {
                    i.pointer.any_pressed()
                        && i.pointer
                            .interact_pos()
                            .is_some_and(|p| !r.response.rect.contains(p))
                })
            });
        let closed = !open || outside || action.is_some();
        if closed {
            self.viewport_menu.context = None;
        }
        (action, closed)
    }
}

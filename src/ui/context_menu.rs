//! Shared viewport tool chooser. Consumers supply actions; this popup never edits geometry.
use super::*;
use crate::viewport::{ComponentMode, ModelTool, SelectionSpan};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewportMenuContext {
    Modeling,
    Visibility,
    Mask,
    Transform,
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

fn menu_entry(
    ui: &mut egui::Ui,
    entry: &ViewportMenuEntry,
    keyboard_selected: bool,
    compact: bool,
    reveal: bool,
) -> bool {
    let label = if entry.selected {
        format!("{} (active)", entry.label)
    } else {
        entry.label.clone()
    };
    let width = if compact {
        0.
    } else {
        ui.available_width().clamp(0., 230.)
    };
    let response = ui
        .add_enabled(
            entry.enabled,
            egui::Button::new(label)
                .selected(keyboard_selected)
                .min_size(egui::vec2(width, 32.)),
        )
        .on_hover_text(&entry.hint);
    if keyboard_selected && reveal {
        response.scroll_to_me(Some(egui::Align::Center));
    }
    response.clicked()
}

fn modeling_choices(
    ui: &mut egui::Ui,
    entries: &[ViewportMenuEntry],
    keyboard_index: Option<usize>,
    reveal: bool,
) -> Option<(usize, Action)> {
    let mut chosen = None;
    let highlighted =
        |i, entry: &ViewportMenuEntry| keyboard_index.map_or(entry.selected, |index| i == index);
    ui.strong("Component");
    ui.horizontal_wrapped(|ui| {
        for (i, entry) in entries.iter().take(3).enumerate() {
            if menu_entry(ui, entry, highlighted(i, entry), true, reveal) {
                chosen = Some((i, entry.action.clone()));
            }
        }
    });
    ui.strong("Target");
    ui.horizontal_wrapped(|ui| {
        for (i, entry) in entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| matches!(entry.action, Action::ModelTarget(_)))
        {
            if menu_entry(ui, entry, highlighted(i, entry), true, reveal) {
                chosen = Some((i, entry.action.clone()));
            }
        }
    });
    ui.separator();
    ui.strong("Function");
    for (i, entry) in entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| matches!(entry.action, Action::ModelTool(_)))
    {
        if menu_entry(ui, entry, highlighted(i, entry), false, reveal) {
            chosen = Some((i, entry.action.clone()));
        }
    }
    chosen
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
                *hold = if label == "Hold for selection (X menu)" {
                    crate::preferences::HoldBinding::visibility_default()
                } else {
                    Default::default()
                };
            }
        });
    });
}

impl UiState {
    pub(super) fn modeling_function_menu(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
        popup: bool,
    ) {
        let entries = self.viewport_menu_entries(ViewportMenuContext::Modeling, document);
        if let Some((_, action)) = modeling_choices(ui, &entries, None, false) {
            let function = matches!(action, Action::ModelTool(_));
            commands.push(Command::Action(action));
            if popup && function {
                ui.close_kind(egui::UiKind::Menu);
            }
        }
    }

    pub(crate) fn viewport_menu_entries(
        &self,
        context: ViewportMenuContext,
        document: &Document,
    ) -> Vec<ViewportMenuEntry> {
        match context {
            ViewportMenuContext::Modeling => {
                let mut entries: Vec<_> = [
                    (ComponentMode::Face, Action::ModelFaces, "Face"),
                    (ComponentMode::Edge, Action::ModelEdges, "Edge"),
                    (ComponentMode::Vertex, Action::ModelVertices, "Vertex"),
                ]
                .into_iter()
                .map(|(mode, action, label)| ViewportMenuEntry {
                    enabled: self.action_enabled(&action, document),
                    action,
                    label: label.into(),
                    selected: self.modeling.mode == mode,
                    hint: "Switch component mode and convert the current selection, without editing geometry.".into(),
                })
                .collect();
                entries.extend(SelectionSpan::ALL.into_iter()
                    .filter(|span| span.available(self.modeling.mode))
                    .map(|span| {
                        let action = Action::ModelTarget(span);
                        ViewportMenuEntry {
                            enabled: self.action_enabled(&action, document),
                            action,
                            label: span.label().into(),
                            selected: self.modeling.span == span,
                            hint: "Choose what the next Modeling gesture targets. Keeps the current tool and selection.".into(),
                        }
                    }));
                entries.extend([
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
            );
                entries
            }
            ViewportMenuContext::Visibility => {
                use crate::preferences::VisibilityShape;
                let settings = self.preferences.visibility_selection;
                [
                    (Action::TemporaryVisibilityRectangle, "Rectangle", settings.shape == VisibilityShape::Rectangle),
                    (Action::TemporaryVisibilityLasso, "Lasso", settings.shape == VisibilityShape::Lasso),
                    (Action::SelectVisibilityLine, "Line", settings.shape == VisibilityShape::Line),
                    (Action::SelectVisibilityPath, "Closed path", settings.shape == VisibilityShape::Path),
                    (Action::VisibilityHide, "Hide selected region", settings.hide),
                    (Action::VisibilityIsolate, "Isolate selected region", !settings.hide),
                    (Action::FlipVisibilitySide, "Flip side / region", settings.flip),
                ].into_iter().map(|(action,label,selected)| ViewportMenuEntry {
                    action, label: label.into(), selected,
                    enabled: document.primitive().is_none() && document.active_visible(),
                    hint: "Remembered for the next hold. Red faces preview the result; no geometry is cut.".into(),
                }).collect()
            }
            ViewportMenuContext::Mask => self.temporary_mask_menu_entries(document),
            ViewportMenuContext::Transform => self.transform_menu_entries(document),
        }
    }

    fn transform_menu_entries(&self, document: &Document) -> Vec<ViewportMenuEntry> {
        use crate::viewport::{ModelAxis, ModelInteraction, TransformMode, TransformOperation};
        let mut choices = Vec::new();
        if self.modeling.active {
            for tool in [ModelTool::Move, ModelTool::Rotate, ModelTool::Scale] {
                choices.push((
                    Action::ModelTool(tool),
                    tool.label().to_owned(),
                    self.modeling.tool == tool,
                ));
            }
            for mode in [
                ModelInteraction::Free,
                ModelInteraction::Gizmo,
                ModelInteraction::Axis,
            ] {
                choices.push((
                    Action::ModelInteraction(mode),
                    mode.label().to_owned(),
                    self.modeling.interaction == mode,
                ));
            }
            if self.modeling.interaction == ModelInteraction::Axis {
                for axis in ModelAxis::ALL {
                    if axis != ModelAxis::Uniform || self.modeling.tool == ModelTool::Scale {
                        let action = Action::ModelAxis(axis);
                        choices.push((
                            action.clone(),
                            action.label().to_owned(),
                            self.modeling.axis == axis,
                        ));
                    }
                }
            }
        } else {
            for operation in [
                TransformOperation::Move,
                TransformOperation::Rotate,
                TransformOperation::Scale,
                TransformOperation::Universal,
            ] {
                choices.push((
                    operation.into(),
                    operation.label().to_owned(),
                    self.transform_mode != TransformMode::Sculpt
                        && self.transform_operation == operation,
                ));
            }
            for (mode, label) in [
                (TransformMode::World, "World axes"),
                (TransformMode::ActionLine, "Action Line"),
            ] {
                choices.push((mode.into(), label.to_owned(), self.transform_mode == mode));
            }
            for action in [
                Action::PlacePivot,
                Action::PlaceLine,
                Action::ResetPivot,
                Action::RespectMask,
            ] {
                choices.push((
                    action.clone(),
                    action.label().to_owned(),
                    matches!(action, Action::RespectMask) && self.respect_transform_mask,
                ));
            }
        }
        choices
            .into_iter()
            .map(|(action, label, selected)| ViewportMenuEntry {
                enabled: self.action_enabled(&action, document),
                action,
                label,
                selected,
                hint: "Choose transform options. Geometry changes only when you use the tool."
                    .into(),
            })
            .collect()
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
        let mut navigated = false;
        ctx.input_mut(|i| {
            if count > 0 {
                if consume_menu_key(i, egui::Key::ArrowDown) {
                    index = (index + 1) % count;
                    navigated = true;
                }
                if consume_menu_key(i, egui::Key::ArrowUp) {
                    index = (index + count - 1) % count;
                    navigated = true;
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
            ViewportMenuContext::Visibility => "Mask / visibility",
            ViewportMenuContext::Mask => "Mask / visibility",
            ViewportMenuContext::Transform => "Transform options",
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
            if matches!(
                context,
                ViewportMenuContext::Mask | ViewportMenuContext::Visibility
            ) {
                ui.horizontal(|ui| {
                    for (choice, label) in [
                        (ViewportMenuContext::Mask, "Mask"),
                        (ViewportMenuContext::Visibility, "Visibility"),
                    ] {
                        if ui.selectable_label(context == choice, label).clicked() {
                            self.preferences.selection_mode = match choice {
                                ViewportMenuContext::Mask => {
                                    crate::preferences::SelectionMode::Mask
                                }
                                _ => crate::preferences::SelectionMode::Visibility,
                            };
                            self.viewport_menu.context = Some(choice);
                            self.viewport_menu.selected = 0;
                        }
                    }
                });
                let hold = &self.preferences.visibility_hold;
                if hold.enabled() {
                    ui.weak(format!(
                        "Hold {} to use selected tab · Alt reverses",
                        hold.label()
                    ));
                } else {
                    ui.weak("Assign Hold for selection in Key settings · Alt reverses");
                }
                ui.separator();
            } else if context == ViewportMenuContext::Transform {
                ui.strong(if self.modeling.active {
                    "Selected components"
                } else {
                    "Active object"
                });
            }
            if context == ViewportMenuContext::Modeling {
                if let Some((i, chosen)) = modeling_choices(ui, &entries, Some(index), navigated) {
                    self.viewport_menu.selected = i;
                    action = Some(chosen);
                }
            } else {
                for (i, entry) in entries.iter().enumerate() {
                    if menu_entry(ui, entry, i == index, false, navigated) {
                        action = Some(entry.action.clone());
                    }
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
        let selection_option = matches!(
            action,
            Some(
                Action::ModelFaces
                    | Action::ModelEdges
                    | Action::ModelVertices
                    | Action::ModelTarget(_)
                    | Action::ModelInteraction(_)
                    | Action::ModelAxis(_)
                    | Action::Transform
                    | Action::LineTransform
                    | Action::RespectMask
            )
        );
        let closed = !open || outside || (action.is_some() && !selection_option);
        if closed {
            self.viewport_menu.context = None;
        }
        (action, closed)
    }
}

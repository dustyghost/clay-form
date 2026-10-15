use super::*;
use crate::viewport::{ComponentMode, ModelTool, SelectionChange, SelectionSpan};
#[derive(Default)]
pub(crate) struct ModelingControls {
    pub active: bool,
    pub mode: ComponentMode,
    pub tool: ModelTool,
    pub change: SelectionChange,
    pub span: SelectionSpan,
    pub count: usize,
    pub loop_count: u32,
    pub join: bool,
    pub group_name: String,
    pub group: usize,
}
impl UiState {
    pub(super) fn modeling_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let unavailable = self.edge_unavailable(&Action::SelectEdges, document);
        let mut active = self.modeling.active;
        let response = ui
            .add_enabled(
                unavailable.is_none(),
                egui::Checkbox::new(&mut active, "Pen modeling"),
            )
            .customize(Action::PenModeling);
        if self
            .edge_help(response, &Action::PenModeling, document)
            .changed()
        {
            commands.push(Command::Action(Action::PenModeling));
        }
        if !self.modeling.active {
            return;
        }
        ui.label(format!(
            "{:?} · {} · {} selected",
            self.modeling.mode,
            self.modeling.tool.label(),
            self.modeling.count
        ));
        if document.symmetry().enabled {
            ui.weak("Symmetry on · matching geometry and topology required");
        }
        ui.add_enabled_ui(unavailable.is_none(), |ui| {
            ui.horizontal_wrapped(|ui| {
                for (mode, action) in [
                    (ComponentMode::Face, Action::ModelFaces),
                    (ComponentMode::Edge, Action::ModelEdges),
                    (ComponentMode::Vertex, Action::ModelVertices),
                ] {
                    if ui.selectable_label(self.modeling.mode == mode, format!("{mode:?}"))
                        .customize(action.clone()).clicked() {
                        commands.push(Command::Action(action));
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                for (tool, action) in [
                    (ModelTool::Select, Action::ModelSelect),
                    (ModelTool::Extrude, Action::ModelExtrude),
                    (ModelTool::LoopCut, Action::ModelLoopCut),
                ] {
                    if (tool == ModelTool::Extrude && self.modeling.mode != ComponentMode::Face)
                        || (tool == ModelTool::LoopCut && self.modeling.mode != ComponentMode::Edge) {
                        continue;
                    }
                    if ui.selectable_label(self.modeling.tool == tool, tool.label())
                        .customize(action.clone()).clicked() {
                        commands.push(Command::Action(action));
                    }
                }
            });
            match self.modeling.tool {
                ModelTool::Select => {
                    ui.horizontal_wrapped(|ui| {
                        for (change, action) in [
                            (SelectionChange::Replace, Action::ModelReplace),
                            (SelectionChange::Add, Action::ModelAdd),
                            (SelectionChange::Remove, Action::ModelRemove),
                        ] {
                            if ui.selectable_label(self.modeling.change == change, format!("{change:?}"))
                                .customize(action.clone()).clicked() {
                                commands.push(Command::Action(action));
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        for (span,label,action) in [
                            (SelectionSpan::Single,"Individual",Action::ModelSelectSingle),
                            (SelectionSpan::Loop,"Loop",Action::ModelSelectLoops),
                            (SelectionSpan::Ring,"Parallel edges",Action::ModelSelectRing),
                        ] {
                            if (span==SelectionSpan::Ring && self.modeling.mode!=ComponentMode::Edge)
                                || (span==SelectionSpan::Loop && self.modeling.mode==ComponentMode::Vertex) {continue;}
                            if ui.selectable_label(self.modeling.span==span,label).customize(action.clone()).clicked() {
                                commands.push(Command::Action(action));
                            }
                        }
                    });
                    ui.weak("Tap or drag to select · Shift adds · Ctrl removes");
                    match (self.modeling.mode,self.modeling.span) {
                        (ComponentMode::Face,SelectionSpan::Loop) => {ui.weak("Hover near a face edge to choose the loop direction.");}
                        (ComponentMode::Edge,SelectionSpan::Loop) => {ui.weak("Follows connected edges along the loop.");}
                        (ComponentMode::Edge,SelectionSpan::Ring) => {ui.weak("Follows opposite edges across a quad strip.");}
                        _ => {}
                    }
                    ui.weak("Replace clears once per stroke · Esc cancels");
                }
                ModelTool::Extrude => {
                    design::help(
                        ui.checkbox(&mut self.modeling.join, "Join matching faces").customize(Action::ModelJoin),
                        "Snap to an aligned opposite face and remove both caps to open a tunnel. Single-face extrusions only.",
                    );
                    ui.weak("Drag up: out · down: in · release: apply · Esc: cancel");
                    ui.weak("Flat face regions only. Edits replace stored levels.");
                }
                ModelTool::LoopCut => {
                    use crate::sculpt::LoopCut;
                    self.modeling.loop_count = self.modeling.loop_count.clamp(1, LoopCut::MAX_COUNT);
                    ui.horizontal(|ui| {
                        ui.label("Loops");
                        if design::help(ui.button("−").customize(Action::ModelFewerLoops), "Fewer loop cuts (minimum 1)").clicked() {
                            self.modeling.loop_count = self.modeling.loop_count.saturating_sub(1).max(1);
                        }
                        ui.add(egui::DragValue::new(&mut self.modeling.loop_count).range(1..=LoopCut::MAX_COUNT));
                        if design::help(ui.button("+").customize(Action::ModelMoreLoops), "More loop cuts (maximum 16)").clicked() {
                            self.modeling.loop_count = (self.modeling.loop_count+1).min(LoopCut::MAX_COUNT);
                        }
                    });
                    ui.weak("Tap: centered · drag along edge: slide · release: apply");
                    ui.weak("Wheel while dragging: loop count · Esc: cancel");
                    ui.weak("Follows visible quads. Edits replace stored levels.");
                }
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Clear selection").customize(Action::ModelClearSelection).clicked() {
                    commands.push(Command::ClearModelSelection);
                }
                if self.modeling.mode == ComponentMode::Edge
                    && ui.button("Select loop").customize(Action::ModelSelectLoop).clicked() {
                    commands.push(Command::ModelSelectLoop);
                }
            });
            if self.modeling.mode==ComponentMode::Edge {
                let response=ui.add_enabled(self.modeling.count>0,egui::Button::new("Dissolve selected loops"))
                    .customize(Action::ModelDissolveLoops);
                if design::help(response,"Remove complete interior quad loops and merge the surrounding faces. Keeps the surface connected; replaces stored levels. Undo restores the mesh.").clicked() {
                    commands.push(Command::ModelDissolveLoops);
                }
            }
            ui.collapsing("Selection to vertex group", |ui| {
                ui.text_edit_singleline(&mut self.modeling.group_name);
                let enabled = self.modeling.count > 0 && document.mesh_settings().levels() == 1;
                let help = "Use a single subdivision level and select components first. Face selections include all corners; edges include both ends. Masks stay unchanged.";
                if design::help(
                    ui.add_enabled(enabled, egui::Button::new("Create group from selection")).customize(Action::ModelCreateGroup), help,
                ).clicked() {
                    commands.push(Command::GroupModelSelection(None, false));
                }
                let groups = &document.mesh().vertex_groups;
                self.modeling.group = self.modeling.group.min(groups.len().saturating_sub(1));
                if !groups.is_empty() {
                    egui::ComboBox::from_id_salt("modeling_group")
                        .selected_text(&groups[self.modeling.group].name)
                        .show_ui(ui, |ui| {
                            for (i, group) in groups.iter().enumerate() {
                                ui.selectable_value(&mut self.modeling.group, i, &group.name);
                            }
                        });
                    ui.horizontal_wrapped(|ui| {
                        for (label, remove, action) in [
                            ("Add to group", false, Action::ModelAddToGroup),
                            ("Remove from group", true, Action::ModelRemoveFromGroup),
                        ] {
                            if design::help(ui.add_enabled(enabled, egui::Button::new(label)).customize(action), help).clicked() {
                                commands.push(Command::GroupModelSelection(Some(self.modeling.group), remove));
                            }
                        }
                    });
                }
            });
        });
    }
}

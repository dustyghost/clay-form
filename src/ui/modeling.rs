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
    pub axis: crate::viewport::ModelAxis,
    pub amount: f32,
    pub protect_mask: bool,
    pub individual: bool,
    pub snap: f32,
    pub tap_repeat: bool,
    pub smooth_preview: bool,
    pub preset_name: String,
    pub preset: Option<u64>,
    pub can_repeat: bool,
    pub repeat_label: String,
    pub amounts: std::collections::BTreeMap<ModelTool, f32>,
    pub group_name: String,
    pub group: usize,
}
impl ModelingControls {
    pub fn set_tool(&mut self, tool: ModelTool) {
        if tool == self.tool {
            return;
        }
        self.amounts.insert(self.tool, self.amount);
        self.tool = tool;
        self.amount = self.amounts.get(&tool).copied().unwrap_or(match tool {
            ModelTool::Scale => 1.,
            ModelTool::Rotate => 15.,
            _ => 0.1,
        });
    }

    pub fn load_options(&mut self, o: crate::viewport::ModelingOptions) {
        self.mode = o.mode;
        self.tool = o.tool;
        self.change = o.change;
        self.span = o.span;
        self.join = o.join;
        self.loop_count = o.loop_count;
        self.axis = o.axis;
        self.amount = o.amount;
        self.protect_mask = o.protect_mask;
        self.individual = o.individual;
        self.snap = o.snap;
        self.tap_repeat = o.tap_repeat;
        self.smooth_preview = o.smooth_preview;
    }

    pub fn options(&self) -> crate::viewport::ModelingOptions {
        crate::viewport::ModelingOptions {
            mode: self.mode,
            tool: self.tool,
            change: self.change,
            span: self.span,
            join: self.join,
            loop_count: self.loop_count,
            axis: self.axis,
            amount: self.amount,
            protect_mask: self.protect_mask,
            individual: self.individual,
            snap: self.snap,
            tap_repeat: self.tap_repeat,
            smooth_preview: self.smooth_preview,
        }
    }
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
            ui.horizontal_wrapped(|ui| {
                for tool in [ModelTool::Move,ModelTool::Rotate,ModelTool::Scale] {
                    if ui.selectable_label(self.modeling.tool==tool,match tool {ModelTool::Move=>"Move",ModelTool::Rotate=>"Rotate",ModelTool::Scale=>"Scale",_=>tool.label()}).customize(Action::ModelTool(tool)).clicked() { commands.push(Command::Action(Action::ModelTool(tool))); }
                }
            });
            if self.modeling.mode==ComponentMode::Face { ui.horizontal_wrapped(|ui| { for tool in [ModelTool::Inset,ModelTool::Bevel,ModelTool::Fuse] { if ui.selectable_label(self.modeling.tool==tool,match tool {ModelTool::Move=>"Move",ModelTool::Rotate=>"Rotate",ModelTool::Scale=>"Scale",_=>tool.label()}).customize(Action::ModelTool(tool)).clicked() {commands.push(Command::Action(Action::ModelTool(tool)));} } }); }
            if self.modeling.mode==ComponentMode::Edge && ui.selectable_label(self.modeling.tool==ModelTool::Slide,"Slide loop").customize(Action::ModelTool(ModelTool::Slide)).clicked(){commands.push(Command::Action(Action::ModelTool(ModelTool::Slide)));}
            ui.label("Target");
            ui.horizontal_wrapped(|ui| {
                for span in SelectionSpan::ALL.into_iter().filter(|s|s.available(self.modeling.mode)) {
                    if ui.selectable_label(self.modeling.span==span,span.label()).customize(match span { SelectionSpan::Single=>Action::ModelSelectSingle, SelectionSpan::Loop=>Action::ModelSelectLoops,SelectionSpan::Ring=>Action::ModelSelectRing,_=>Action::ModelTarget(span) }).clicked() { commands.push(Command::Action(Action::ModelTarget(span))); }
                }
            });
            ui.checkbox(&mut self.modeling.protect_mask,"Protect mask").customize(Action::ModelProtectMask);
            ui.weak("Hidden faces are protected · topology edits replace stored levels");
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
                    ui.weak("Tap or drag to select · Shift adds · Ctrl removes");
                    match (self.modeling.mode,self.modeling.span) {
                        (ComponentMode::Face,SelectionSpan::Loop) => {ui.weak("Hover near a face edge to choose the loop direction.");}
                        (ComponentMode::Edge,SelectionSpan::Loop) => {ui.weak("Follows connected edges along the loop.");}
                        (ComponentMode::Edge,SelectionSpan::Ring) => {ui.weak("Follows opposite edges across a quad strip.");}
                        _ => {}
                    }
                    ui.weak("Replace clears once per stroke · Esc cancels");
                }
                ModelTool::Extrude | ModelTool::Fuse => {
                    if self.modeling.tool==ModelTool::Fuse {
                        ui.label("Snap range (fraction of local edge)");
                        ui.add(egui::DragValue::new(&mut self.modeling.snap).range(0.0..=0.25).speed(0.005));
                        ui.weak("Push inward to fuse at a compatible side loop or cap.");
                    }
                    if self.modeling.tool==ModelTool::Extrude {ui.checkbox(&mut self.modeling.individual,"Individual faces");}
                    ui.add(egui::DragValue::new(&mut self.modeling.amount).prefix("Distance ").speed(0.01));
                    if ui.add_enabled(self.modeling.count>0,egui::Button::new("Apply amount")).customize(Action::ModelApply).clicked(){commands.push(Command::ModelApply);}

                    design::help(
                        ui.checkbox(&mut self.modeling.join, "Join matching faces").customize(Action::ModelJoin),
                        "Snap to an aligned opposite face and remove both caps to open a tunnel. Single-face extrusions only.",
                    );
                    ui.weak("Drag up: out · down: in · release: apply · Esc: cancel");
                    ui.weak("Connected regions share walls; Individual faces extrude separately.");
                }
                ModelTool::Inset | ModelTool::Bevel => {
                    ui.checkbox(&mut self.modeling.individual,"Individual faces");
                    ui.add(egui::DragValue::new(&mut self.modeling.amount).prefix("Width ").range(0.0..=1000.).speed(0.01));
                    if ui.add_enabled(self.modeling.count>0,egui::Button::new("Apply amount")).customize(Action::ModelApply).clicked(){commands.push(Command::ModelApply);}
                    ui.weak("Drag up to increase width · release applies · Esc cancels");
                    if self.modeling.tool==ModelTool::Bevel {ui.weak("Flat region with one exterior edge at each corner.");}
                }
                ModelTool::Move | ModelTool::Rotate | ModelTool::Scale | ModelTool::Slide => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Axis");
                        for axis in crate::viewport::ModelAxis::ALL {
                            if axis==crate::viewport::ModelAxis::Uniform && self.modeling.tool!=ModelTool::Scale {continue;}
                            ui.selectable_value(&mut self.modeling.axis,axis,format!("{axis:?}"));
                        }
                    });
                    ui.label(match self.modeling.tool { ModelTool::Slide=>"Slide fraction (-1 to 1)", ModelTool::Rotate=>"Angle (degrees)",ModelTool::Scale=>"Scale factor",_=>"Distance" });
                    ui.add(egui::DragValue::new(&mut self.modeling.amount).speed(0.01));
                    if ui.add_enabled(self.modeling.count>0,egui::Button::new("Apply amount")).customize(Action::ModelApply).clicked() { commands.push(Command::ModelApply); }
                    ui.weak("Drag up/down to adjust · release applies · Esc cancels");
                    ui.weak("Rotate and scale around the selection center");
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
            if self.modeling.tool.is_edit() || self.modeling.tool==ModelTool::Extrude {
                ui.checkbox(&mut self.modeling.tap_repeat,"Tap to repeat").customize(Action::ModelTapRepeat)
                    .on_hover_text("Tap a target to apply the displayed amount and options. Drag to set a new amount. Select and centered loop cuts are unchanged.");
                if self.modeling.tap_repeat {ui.weak(format!("Tap applies {:.3} · drag sets a new amount",self.modeling.amount));}
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
            if self.modeling.mode==ComponentMode::Face && ui.button("Select region boundary").customize(Action::ModelBoundary).clicked() { commands.push(Command::ModelBoundary); }
            if self.modeling.mode==ComponentMode::Edge {
                let response=ui.add_enabled(self.modeling.count>0,egui::Button::new("Dissolve selected loops"))
                    .customize(Action::ModelDissolveLoops);
                if design::help(response,"Remove complete interior quad loops and merge the surrounding faces. Keeps the surface connected; replaces stored levels. Undo restores the mesh.").clicked() {
                    commands.push(Command::ModelDissolveLoops);
                }
            }
            ui.horizontal_wrapped(|ui|{
                if ui.add_enabled(self.modeling.can_repeat && self.modeling.count>0,egui::Button::new("Repeat last edit")).customize(Action::ModelRepeat).on_hover_text(&self.modeling.repeat_label).clicked(){commands.push(Command::ModelRepeat);}
                ui.checkbox(&mut self.modeling.smooth_preview,"Smooth preview").customize(Action::ModelSmoothPreview);
            });
            if self.modeling.smooth_preview {ui.weak("One smooth level · cage stays editable · preview pauses during a drag");}
            ui.collapsing("Tool presets",|ui|{
                ui.add(egui::TextEdit::singleline(&mut self.modeling.preset_name).hint_text("Preset name"));
                if ui.button("Save tool preset").customize(Action::ModelSavePreset).clicked(){commands.push(Command::Action(Action::ModelSavePreset));}
                for preset in &self.preferences.modeling_presets {
                    ui.horizontal(|ui|{
                        if ui.selectable_label(self.modeling.preset==Some(preset.id),format!("Use {}",preset.name)).customize(Action::ModelPreset(preset.id)).clicked(){commands.push(Command::Action(Action::ModelPreset(preset.id)));}
                        if ui.small_button("Delete").clicked(){commands.push(Command::Action(Action::ModelDeletePreset(preset.id)));}
                    });
                }
                ui.weak("Saves action, target and options. Right-click a preset to add it to a shelf.");
            });
            ui.collapsing("Topology",|ui|{
                for edit in crate::sculpt::TopologyEdit::ALL {
                    use crate::sculpt::TopologyEdit as T;
                    let mode=match edit {T::DeleteFaces=>ComponentMode::Face,T::Connect|T::Weld=>ComponentMode::Vertex,_=>ComponentMode::Edge};
                    if self.modeling.mode==mode && ui.add_enabled(self.modeling.count>0,egui::Button::new(edit.label())).customize(Action::ModelTopology(edit)).clicked(){commands.push(Command::ModelTopology(edit));}
                }
                ui.weak("Delete opens holes; dissolve keeps the surface. Weld uses the midpoint.");
            });
            ui.collapsing("Selection to vertex group", |ui| {
                ui.text_edit_singleline(&mut self.modeling.group_name);
                let enabled = self.modeling.count > 0 && document.mesh_settings().levels() == 1;
                let help = "Use a single subdivision level and select components first. Face colors use only selected faces; vertex weights include their corners. Edges include both ends. Masks stay unchanged.";
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

use super::*;
use crate::viewport::{ComponentMode, ModelTool, SelectionChange, SelectionSpan};
#[derive(Default)]
pub(crate) struct ModelingControls {
    pub active: bool,
    pub mode: ComponentMode,
    pub tool: ModelTool,
    pub change: SelectionChange,
    pub span: SelectionSpan,
    pub flat_angle: f32,
    pub erase_radius: f32,
    pub count: usize,
    pub loop_count: u32,
    pub join: bool,
    pub axis: crate::viewport::ModelAxis,
    pub interaction: crate::viewport::ModelInteraction,
    pub transform_direction: crate::sculpt::Vec3,
    pub directions: std::collections::BTreeMap<ModelTool, crate::sculpt::Vec3>,
    pub amount: f32,
    pub preview_amount: Option<f32>,
    pub protect_mask: bool,
    pub individual: bool,
    pub snap: f32,
    pub construction: crate::viewport::ModelConstruction,
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
        self.directions.insert(self.tool, self.transform_direction);
        self.transform_direction = self.directions.get(&tool).copied().unwrap_or_default();
        self.tool = tool;
        self.amount = self.amounts.get(&tool).copied().unwrap_or(match tool {
            ModelTool::EdgeSplit | ModelTool::EdgeCollapse => 0.5,
            ModelTool::Scale => 1.,
            ModelTool::Rotate => 15.,
            _ => 0.1,
        });
    }

    pub fn load_options(&mut self, o: crate::viewport::ModelingOptions) {
        self.amounts.insert(self.tool, self.amount);
        self.amounts.insert(o.tool, o.amount);
        self.directions.insert(self.tool, self.transform_direction);
        self.directions.insert(o.tool, o.transform_direction);
        self.interaction = o.interaction;
        self.transform_direction = o.transform_direction;
        self.mode = o.mode;
        self.tool = o.tool;
        self.change = o.change;
        self.span = o.span;
        self.flat_angle = o.flat_angle;
        self.erase_radius = o.erase_radius;
        self.join = o.join;
        self.loop_count = o.loop_count;
        self.axis = o.axis;
        self.amount = o.amount;
        self.protect_mask = o.protect_mask;
        self.individual = o.individual;
        self.snap = o.snap;
        self.construction = o.construction;
        self.tap_repeat = o.tap_repeat;
        self.smooth_preview = o.smooth_preview;
    }

    pub fn options(&self) -> crate::viewport::ModelingOptions {
        crate::viewport::ModelingOptions {
            mode: self.mode,
            tool: self.tool,
            change: self.change,
            span: self.span,
            flat_angle: self.flat_angle,
            erase_radius: self.erase_radius,
            join: self.join,
            loop_count: self.loop_count,
            axis: self.axis,
            interaction: self.interaction,
            transform_direction: self.transform_direction,
            amount: self.amount,
            protect_mask: self.protect_mask,
            individual: self.individual,
            snap: self.snap,
            construction: self.construction,
            tap_repeat: self.tap_repeat,
            smooth_preview: self.smooth_preview,
        }
    }
}
impl UiState {
    pub(super) fn point_split_controls(&mut self, ui: &mut egui::Ui) -> bool {
        ui.separator();
        ui.strong("Split point settings");
        if let Some(amount) = self.modeling.preview_amount {
            ui.label(format!("Radius {amount:.6} (preview)"));
        } else {
            ui.add(
                egui::DragValue::new(&mut self.modeling.amount)
                    .prefix("Radius ")
                    .range(0.000001..=1000.)
                    .speed(0.001)
                    .max_decimals(6),
            );
        }
        design::checkbox(
            ui,
            &mut self.modeling.construction.double_ring,
            "Double ring sides (4 to 8)",
        );
        design::checkbox(
            ui,
            &mut self.modeling.construction.round,
            "Circularize ring",
        );
        design::checkbox(ui, &mut self.modeling.construction.crease, "Crease ring");
        design::checkbox(
            ui,
            &mut self.modeling.construction.triangulate,
            "Triangulate center",
        );
        design::checkbox(ui, &mut self.modeling.tap_repeat, "Tap to repeat")
            .customize(Action::ModelTapRepeat);
        ui.weak("Radius uses model units from the original vertex. Press to preview; drag up/down to resize. Release applies; Esc cancels.");
        ui.weak("Closed point fan · radius below half the shortest edge");
        ui.add_enabled(self.modeling.count > 0, egui::Button::new("Apply amount"))
            .customize(Action::ModelApply)
            .clicked()
    }

    pub(super) fn modeling_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let unavailable = self.edge_unavailable(&Action::SelectEdges, document);
        let mut active = self.modeling.active;
        let response = ui
            .horizontal(|ui| {
                let response = ui
                    .add_enabled(
                        unavailable.is_none(),
                        egui::Checkbox::new(&mut active, "Modeling"),
                    )
                    .customize(Action::PenModeling);
                if ui
                    .small_button("New mesh…")
                    .customize(Action::ModelNewMesh)
                    .on_hover_text(
                        "Start with a low-resolution cube. Review the replacement warning first.",
                    )
                    .clicked()
                {
                    commands.push(Command::RequestModelingPrimitive);
                }
                response
            })
            .inner;
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
            ui.horizontal_wrapped(|ui| {
                for tool in [ModelTool::EdgeExtrude,ModelTool::EdgeSplit,ModelTool::EdgeCollapse,ModelTool::PointExtrude,ModelTool::PointSplit,ModelTool::FacePoint,ModelTool::FaceSplit] {
                    if tool.required_mode()==Some(self.modeling.mode) && ui.selectable_label(self.modeling.tool==tool,tool.label()).customize(Action::ModelTool(tool)).clicked(){commands.push(Command::Action(Action::ModelTool(tool)));}
                }
            });
            ui.label("Target");
            ui.horizontal_wrapped(|ui| {
                for span in SelectionSpan::ALL.into_iter().filter(|s|s.available(self.modeling.mode)) {
                    if ui.selectable_label(self.modeling.span==span,span.label()).customize(match span { SelectionSpan::Single=>Action::ModelSelectSingle, SelectionSpan::Loop=>Action::ModelSelectLoops,SelectionSpan::Ring=>Action::ModelSelectRing,_=>Action::ModelTarget(span) }).clicked() { commands.push(Command::Action(Action::ModelTarget(span))); }
                }
            });
            if self.modeling.span==SelectionSpan::Flat {
                ui.add(egui::Slider::new(&mut self.modeling.flat_angle,0. ..=180.).text("Island angle").suffix("°"));
                ui.weak("Connected faces within this angle of the first face");
            }
            design::checkbox(ui, &mut self.modeling.protect_mask,"Protect mask").customize(Action::ModelProtectMask);
            ui.weak("Hidden faces are protected · topology edits replace stored levels");
            match self.modeling.tool {
                ModelTool::PointSplit => {
                    if self.point_split_controls(ui) { commands.push(Command::ModelApply); }
                }
                ModelTool::EdgeExtrude | ModelTool::EdgeSplit | ModelTool::EdgeCollapse | ModelTool::PointExtrude | ModelTool::FacePoint | ModelTool::FaceSplit => {
                    let tool=self.modeling.tool;
                    if matches!(tool,ModelTool::EdgeSplit|ModelTool::EdgeCollapse) {
                        ui.label("Position from marked endpoint (0–1)");
                        let range=if tool==ModelTool::EdgeSplit {0.001..=0.999}else{0.0..=1.0};
                        ui.add(egui::DragValue::new(&mut self.modeling.amount).range(range).speed(0.01));
                        if tool==ModelTool::EdgeCollapse {ui.horizontal_wrapped(|ui|{ui.selectable_value(&mut self.modeling.amount,0.,"Marked end");ui.selectable_value(&mut self.modeling.amount,0.5,"Midpoint");ui.selectable_value(&mut self.modeling.amount,1.,"Other end");});}
                        ui.weak("Drag along the edge to place the point · Esc cancels");
                    } else if !tool.instant() {
                        ui.label(if tool==ModelTool::PointSplit {"Radius"}else{"Distance"});
                        ui.add(egui::DragValue::new(&mut self.modeling.amount).speed(0.01));
                        ui.weak("Drag up/down to adjust · release applies · Esc cancels");
                    }
                    if tool==ModelTool::EdgeExtrude {
                        design::checkbox(ui, &mut self.modeling.construction.other_side,"Other adjacent side");
                        ui.weak("Border: extend outward · surface: extrude the highlighted side");
                    }
                    if tool==ModelTool::PointExtrude {
                        ui.add(egui::DragValue::new(&mut self.modeling.construction.width).prefix("Base radius ").range(0.0001..=1000.).speed(0.01));
                    }
                    if matches!(tool,ModelTool::PointSplit|ModelTool::PointExtrude) {
                        design::checkbox(ui, &mut self.modeling.construction.round,"Circularize ring");
                        design::checkbox(ui, &mut self.modeling.construction.crease,"Crease ring");
                        ui.weak("Closed point fan · radius below half the shortest edge");
                    }
                    if tool.instant(){ui.weak(if tool==ModelTool::FacePoint {"Tap: connect a center point to each corner"}else{"Tap: split into quads and connect neighboring edges"});}
                    if ui.add_enabled(self.modeling.count>0,egui::Button::new(if tool.instant(){"Apply to selection"}else{"Apply amount"})).customize(Action::ModelApply).clicked(){commands.push(Command::ModelApply);}
                }

                ModelTool::Select => {
                    ui.horizontal_wrapped(|ui| {
                        for (change, action) in [
                            (SelectionChange::Replace, Action::ModelReplace),
                            (SelectionChange::Add, Action::ModelAdd),
                            (SelectionChange::Remove, Action::ModelRemove),
                        ] {
                            if ui.selectable_label(self.modeling.erase_radius == 0. && self.modeling.change == change, format!("{change:?}"))
                                .customize(action.clone()).clicked() {
                                commands.push(Command::Action(action));
                            }
                        }
                    });
                    let mut erase = self.modeling.erase_radius > 0.;
                    if ui.checkbox(&mut erase, "Paint away selection").changed() {
                        self.modeling.erase_radius = if erase { 32. } else { 0. };
                    }
                    if erase {
                        ui.add(egui::Slider::new(&mut self.modeling.erase_radius,4. ..=160.).text("Eraser radius (px)"));
                        ui.weak("Brush visible selected components · Esc restores the selection");
                    } else { ui.weak("Tap or drag to select · Shift adds · Ctrl removes"); }
                    match (self.modeling.mode,self.modeling.span) {
                        (ComponentMode::Face,SelectionSpan::Loop) => {ui.weak("Hover near a face edge to choose the loop direction.");}
                        (ComponentMode::Edge,SelectionSpan::Loop) => {ui.weak("Follows connected edges along the loop.");}
                        (ComponentMode::Edge,SelectionSpan::Ring) => {ui.weak("Follows opposite edges across a quad strip.");}
                        _ => {}
                    }
                    if !erase { ui.weak("Replace clears once per stroke · Esc cancels"); }
                }
                ModelTool::Extrude | ModelTool::Fuse => {
                    if self.modeling.tool==ModelTool::Fuse {
                        ui.label("Snap range (fraction of local edge)");
                        ui.add(egui::DragValue::new(&mut self.modeling.snap).range(0.0..=0.25).speed(0.005));
                        ui.weak("Each separate flat region snaps to its own compatible loop or cap.");
                    }
                    if self.modeling.tool==ModelTool::Extrude {design::checkbox(ui, &mut self.modeling.individual,"Individual faces");}
                    ui.add(egui::DragValue::new(&mut self.modeling.amount).prefix("Distance ").speed(0.01));
                    if ui.add_enabled(self.modeling.count>0,egui::Button::new("Apply amount")).customize(Action::ModelApply).clicked(){commands.push(Command::ModelApply);}

                    design::help(
                        design::checkbox(ui, &mut self.modeling.join, "Join matching faces").customize(Action::ModelJoin),
                        "Snap to an aligned opposite cap with matching face layout and remove both caps to open a tunnel.",
                    );
                    ui.weak("Drag up: out · down: in · release: apply · Esc: cancel");
                    if self.modeling.tool == ModelTool::Fuse {
                        ui.weak("Adjacent faces stay together; separate regions fuse independently.");
                    } else {
                        ui.weak("Connected regions share walls; Individual faces extrude separately.");
                    }
                }
                ModelTool::Inset | ModelTool::Bevel => {
                    design::checkbox(ui, &mut self.modeling.individual,"Individual faces");
                    ui.add(egui::DragValue::new(&mut self.modeling.amount).prefix("Width ").range(0.0..=1000.).speed(0.01));
                    if ui.add_enabled(self.modeling.count>0,egui::Button::new("Apply amount")).customize(Action::ModelApply).clicked(){commands.push(Command::ModelApply);}
                    ui.weak("Drag up to increase width · release applies · Esc cancels");
                    if self.modeling.tool==ModelTool::Bevel {ui.weak("Flat region with one exterior edge at each corner.");}
                }
                ModelTool::Move | ModelTool::Rotate | ModelTool::Scale | ModelTool::Slide => {
                    use crate::viewport::ModelInteraction;
                    let direct = self.modeling.tool.is_transform();
                    if direct {
                        ui.horizontal_wrapped(|ui| {
                            for mode in [ModelInteraction::Free,ModelInteraction::Gizmo,ModelInteraction::Axis] {
                                if ui.selectable_label(self.modeling.interaction==mode,mode.label()).customize(Action::ModelInteraction(mode)).clicked() { commands.push(Command::Action(Action::ModelInteraction(mode))); }
                            }
                        });
                    }
                    if !direct || self.modeling.interaction==ModelInteraction::Axis { ui.horizontal_wrapped(|ui| {
                        ui.label("Axis");
                        for axis in crate::viewport::ModelAxis::ALL {
                            if axis==crate::viewport::ModelAxis::Uniform && self.modeling.tool!=ModelTool::Scale {continue;}
                            ui.selectable_value(&mut self.modeling.axis,axis,format!("{axis:?}"));
                        }
                    });
                    } else {
                        ui.weak(if self.modeling.interaction==ModelInteraction::Gizmo {"Select components, then drag the handles. Center: free / uniform. Colors: X, Y, Z."}else{match self.modeling.tool {ModelTool::Move=>"Drag a target freely in the view plane",ModelTool::Rotate=>"Drag a target to rotate with the trackball",_=>"Drag up/down to scale uniformly"}});
                        let d=self.modeling.transform_direction;
                        if d.len()<1e-6 {
                            ui.weak(if self.modeling.tool==ModelTool::Scale {"Uniform scale"}else{"Drag first to set the direction for Apply amount"});
                        } else { ui.weak(format!("Direction: X {:.2}, Y {:.2}, Z {:.2}",d.0,d.1,d.2)); }
                    }
                    ui.label(match self.modeling.tool { ModelTool::Slide=>"Slide fraction (-1 to 1)", ModelTool::Rotate=>"Angle (degrees)",ModelTool::Scale=>"Scale factor",_=>"Distance" });
                    ui.add(egui::DragValue::new(&mut self.modeling.amount).speed(0.01));
                    let has_direction=!direct || self.modeling.interaction==ModelInteraction::Axis || self.modeling.tool==ModelTool::Scale || self.modeling.transform_direction.len()>1e-6;
                    if ui.add_enabled(self.modeling.count>0 && has_direction,egui::Button::new("Apply amount")).customize(Action::ModelApply).clicked() { commands.push(Command::ModelApply); }
                    ui.weak("Release applies · Esc cancels · one Undo per drag");
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
            if ((self.modeling.tool.is_edit() && !self.modeling.tool.instant()) || self.modeling.tool==ModelTool::Extrude) && !(self.modeling.tool.is_transform() && self.modeling.interaction==crate::viewport::ModelInteraction::Gizmo) {
                design::checkbox(ui, &mut self.modeling.tap_repeat,"Tap to repeat").customize(Action::ModelTapRepeat)
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
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(self.modeling.count>0,egui::Button::new("Mark UV seams")).clicked() { commands.push(Command::UvSeams(true)); }
                    if ui.add_enabled(self.modeling.count>0,egui::Button::new("Clear UV seams")).clicked() { commands.push(Command::UvSeams(false)); }
                });
                ui.weak("Select edges, then mark or clear seams. Unwrap uses the marked seams.");
                let response=ui.add_enabled(self.modeling.count>0,egui::Button::new("Dissolve selected loops"))
                    .customize(Action::ModelDissolveLoops);
                if design::help(response,"Remove complete interior quad loops and merge the surrounding faces. Keeps the surface connected; replaces stored levels. Undo restores the mesh.").clicked() {
                    commands.push(Command::ModelDissolveLoops);
                }
            }
            ui.horizontal_wrapped(|ui|{
                if ui.add_enabled(self.modeling.can_repeat && self.modeling.count>0,egui::Button::new("Repeat last edit")).customize(Action::ModelRepeat).on_hover_text(&self.modeling.repeat_label).clicked(){commands.push(Command::ModelRepeat);}
                design::checkbox(ui, &mut self.modeling.smooth_preview,"Smooth preview").customize(Action::ModelSmoothPreview);
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
                let help = "Use a single subdivision level and select components first. Face colors use only selected faces; vertex weights include their corners. Edges include both ends. Creating or adding replaces previous membership; partial regions remain, fully replaced groups disappear. Masks stay unchanged.";
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

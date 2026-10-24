//! Resolve reusable actions into concrete application commands.
use super::*;
use crate::command::AssetKind;

impl App {
    pub(super) fn execute_action(&mut self, action: Action, ctx: &egui::Context) {
        if !self.ui.action_enabled(&action, &self.document) {
            return;
        }
        self.ui.edges.selected = self.viewport.selected_edges(&self.document).to_vec();
        let active_brush = self.ui.brush_index_for_context(&self.document, ctx);
        let command = match action {
            Action::TemporaryMaskBrush
            | Action::TemporaryMaskLasso
            | Action::TemporaryMaskLine
            | Action::TemporaryMaskAdd
            | Action::TemporaryMaskErase
            | Action::TemporaryMaskFlip
            | Action::TemporaryMaskPreset(_) => {
                self.change_temporary_mask(action);
                None
            }
            Action::ViewportContextMenu => {
                self.open_viewport_menu(ctx);
                None
            }
            Action::MaskVisibilityMenu | Action::TransformMenu => {
                self.open_quick_menu(ctx, action);
                None
            }
            Action::ModelAxis(axis) => {
                self.viewport.cancel(&mut self.document);
                self.ui.modeling.axis = axis;
                None
            }
            Action::TemporaryVisibilityRectangle | Action::TemporaryVisibilityLasso => {
                self.ui.preferences.visibility_selection.shape =
                    if action == Action::TemporaryVisibilityRectangle {
                        crate::preferences::VisibilityShape::Rectangle
                    } else {
                        crate::preferences::VisibilityShape::Lasso
                    };
                if !self.temporary_visibility_active(ctx)
                    && matches!(
                        self.ui.selection_tool,
                        Some(crate::viewport::SelectionTool::Visibility(_))
                    )
                {
                    self.ui.selection_tool = Some(self.ui.preferences.visibility_selection.tool());
                }
                None
            }
            Action::EditPaths => Some(Command::Path(crate::command::PathCommand::Open)),
            Action::StrokeEntirePath => {
                Some(Command::Path(crate::command::PathCommand::StrokeEntirePath))
            }
            Action::TogglePathOnlyStrokes => {
                self.viewport.cancel(&mut self.document);
                self.ui.path_starts_only = !self.ui.path_starts_only;
                self.viewport.session.guided.path_starts_only = self.ui.path_starts_only;
                None
            }
            Action::TogglePathGuides => {
                self.viewport.cancel(&mut self.document);
                self.ui.guide_paths = !self.ui.guide_paths;
                self.viewport.session.guided.enabled = self.ui.guide_paths;
                None
            }
            Action::PenModeling => Some(Command::ModelingMode(!self.ui.modeling.active)),
            Action::ModelFaces
            | Action::ModelEdges
            | Action::ModelVertices
            | Action::ModelSelectSingle
            | Action::ModelSelectLoops
            | Action::ModelSelectRing
            | Action::ModelSelect
            | Action::ModelExtrude
            | Action::ModelLoopCut
            | Action::ModelReplace
            | Action::ModelAdd
            | Action::ModelRemove => {
                use crate::viewport::{
                    ComponentMode as Mode, ModelTool as Tool, SelectionChange as Change,
                    SelectionSpan as Span,
                };
                if !self.ui.modeling.active {
                    self.dispatch(Command::ModelingMode(true), ctx);
                }
                self.viewport.cancel(&mut self.document);
                let mode = match action {
                    Action::ModelFaces | Action::ModelExtrude => Mode::Face,
                    Action::ModelEdges | Action::ModelLoopCut | Action::ModelSelectRing => {
                        Mode::Edge
                    }
                    Action::ModelSelectLoops if self.ui.modeling.mode == Mode::Vertex => Mode::Edge,
                    Action::ModelVertices => Mode::Vertex,
                    _ => self.ui.modeling.mode,
                };
                if mode != self.ui.modeling.mode {
                    self.dispatch(Command::ModelingComponent(mode), ctx);
                }
                let tool = match action {
                    Action::ModelExtrude => Tool::Extrude,
                    Action::ModelLoopCut => Tool::LoopCut,
                    _ => Tool::Select,
                };
                self.ui.modeling.set_tool(tool);
                match action {
                    Action::ModelSelectSingle => self.ui.modeling.span = Span::Single,
                    Action::ModelSelectLoops => self.ui.modeling.span = Span::Loop,
                    Action::ModelSelectRing => self.ui.modeling.span = Span::Ring,
                    Action::ModelReplace => self.ui.modeling.change = Change::Replace,
                    Action::ModelAdd => self.ui.modeling.change = Change::Add,
                    Action::ModelRemove => self.ui.modeling.change = Change::Remove,
                    _ => {}
                }
                None
            }
            Action::ModelTool(tool) => {
                if !self.ui.modeling.active {
                    self.dispatch(Command::ModelingMode(true), ctx);
                }
                self.viewport.cancel(&mut self.document);
                if let Some(mode) = tool.required_mode()
                    && self.ui.modeling.mode != mode
                {
                    self.dispatch(Command::ModelingComponent(mode), ctx);
                }
                self.ui.modeling.set_tool(tool);
                None
            }
            Action::ModelInteraction(mode) => {
                self.viewport.cancel(&mut self.document);
                self.ui.modeling.interaction = mode;
                None
            }
            Action::ModelTarget(span) => {
                if !self.ui.modeling.active {
                    self.dispatch(Command::ModelingMode(true), ctx);
                }
                self.viewport.cancel(&mut self.document);
                self.ui.modeling.span = span;
                None
            }
            Action::ModelNewMesh => Some(Command::RequestModelingPrimitive),
            Action::ModelTopology(edit) => Some(Command::ModelTopology(edit)),
            Action::ModelRepeat => Some(Command::ModelRepeat),
            Action::ModelSmoothPreview => {
                self.ui.modeling.smooth_preview = !self.ui.modeling.smooth_preview;
                None
            }
            Action::ModelTapRepeat => {
                self.ui.modeling.tap_repeat = !self.ui.modeling.tap_repeat;
                None
            }
            Action::ModelSavePreset => {
                let name = self.ui.modeling.preset_name.trim().to_string();
                let options = self.ui.modeling.options();
                if name.is_empty() || name.len() > 128 {
                    self.ui.status = "Enter a preset name (up to 128 bytes)".into();
                } else if let Err(e) = options.validate() {
                    self.ui.status = e;
                } else if self.ui.preferences.modeling_presets.len() >= 128 {
                    self.ui.status = "The modeling library holds up to 128 presets".into();
                } else if let Some(next) =
                    self.ui.preferences.next_modeling_preset_id.checked_add(1)
                {
                    let id = self.ui.preferences.next_modeling_preset_id;
                    self.ui.preferences.next_modeling_preset_id = next;
                    self.ui
                        .preferences
                        .modeling_presets
                        .push(crate::preferences::ModelingPreset { id, name, options });
                    self.ui.modeling.preset = Some(id);
                }
                None
            }
            Action::ModelPreset(id) => {
                if let Some(preset) = self
                    .ui
                    .preferences
                    .modeling_presets
                    .iter()
                    .find(|p| p.id == id)
                    .cloned()
                {
                    if !self.ui.modeling.active {
                        self.dispatch(Command::ModelingMode(true), ctx);
                    }
                    self.dispatch(Command::ModelingComponent(preset.options.mode), ctx);
                    self.ui.modeling.load_options(preset.options);
                    self.ui.modeling.preset = Some(id);
                    self.ui.modeling.preset_name = preset.name;
                }
                None
            }
            Action::ModelDeletePreset(id) => {
                self.ui.preferences.modeling_presets.retain(|p| p.id != id);
                if self.ui.modeling.preset == Some(id) {
                    self.ui.modeling.preset = None;
                }
                None
            }
            Action::ModelApply => Some(Command::ModelApply),
            Action::ModelBoundary => Some(Command::ModelBoundary),
            Action::ModelProtectMask => {
                self.ui.modeling.protect_mask = !self.ui.modeling.protect_mask;
                None
            }
            Action::ModelJoin => {
                self.ui.modeling.join = !self.ui.modeling.join;
                None
            }
            Action::ModelFewerLoops | Action::ModelMoreLoops => {
                self.ui.modeling.loop_count = (self.ui.modeling.loop_count as i32
                    + if action == Action::ModelMoreLoops {
                        1
                    } else {
                        -1
                    })
                .clamp(1, crate::sculpt::LoopCut::MAX_COUNT as i32)
                    as u32;
                None
            }
            Action::ModelClearSelection => Some(Command::ClearModelSelection),
            Action::ModelSelectLoop => Some(Command::ModelSelectLoop),
            Action::ModelDissolveLoops => Some(Command::ModelDissolveLoops),
            Action::ModelCreateGroup => Some(Command::GroupModelSelection(None, false)),
            Action::ModelAddToGroup | Action::ModelRemoveFromGroup => {
                Some(Command::GroupModelSelection(
                    Some(self.ui.modeling.group),
                    action == Action::ModelRemoveFromGroup,
                ))
            }

            Action::SelectEdges => {
                self.ui.edges.selecting = !self.ui.edges.selecting;
                self.ui.modeling.active = false;
                self.viewport.cancel(&mut self.document);
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                self.ui.arrange_references = false;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                if self.ui.edges.selecting {
                    self.ui.wire = true;
                    self.ui.edges.target = crate::sculpt::BorderTarget::Selected;
                }
                None
            }
            Action::SelectCreases
            | Action::SelectBorderEdges
            | Action::ExpandEdgeLoop
            | Action::ClearEdgeSelection => {
                let edges = match action {
                    Action::SelectCreases => self.document.mesh().border_edges(
                        crate::sculpt::BorderTarget::Selected,
                        0.5,
                        &self.document.mesh().creases,
                    ),
                    Action::SelectBorderEdges => self.document.mesh().border_edges(
                        self.ui.edges.target,
                        self.document
                            .model(self.document.active_model_index())
                            .unwrap()
                            .metadata
                            .mask_threshold,
                        &self.ui.edges.selected,
                    ),
                    Action::ExpandEdgeLoop => self
                        .document
                        .mesh()
                        .expand_edge_loop(&self.ui.edges.selected),
                    _ => Ok(Vec::new()),
                };
                match edges {
                    Ok(edges) => {
                        if action != Action::ClearEdgeSelection {
                            // Selecting edges should make the next click refine that
                            // selection, rather than unexpectedly sculpt the surface.
                            self.ui.edges.selecting = true;
                            self.ui.modeling.active = false;
                            self.viewport.cancel(&mut self.document);
                            self.ui.selection_tool = None;
                            self.ui.slice_active = false;
                            self.ui.arrange_references = false;
                            self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                        }
                        self.viewport
                            .set_edge_selection(edges.clone(), &self.document);
                        self.ui.edges.selected = edges;
                        self.ui.edges.target = crate::sculpt::BorderTarget::Selected;
                        self.ui.wire = true;
                    }
                    Err(error) => self.ui.status = error,
                }
                None
            }
            Action::CreaseEdges
            | Action::UncreaseEdges
            | Action::ClearCreases
            | Action::AddSupportLoops
            | Action::RelaxBorders
            | Action::SmoothSubdivide => {
                use crate::sculpt::EdgeEdit;
                let target = self.ui.edges.target;
                let selected = self.ui.edges.selected.clone();
                let edit = match action {
                    Action::CreaseEdges | Action::UncreaseEdges => EdgeEdit::Crease {
                        target,
                        selected,
                        remove: action == Action::UncreaseEdges,
                    },
                    Action::ClearCreases => EdgeEdit::ClearCreases,
                    Action::AddSupportLoops => EdgeEdit::Loops {
                        target,
                        selected,
                        count: self.ui.edges.loops,
                        width: self.ui.edges.width,
                    },
                    Action::RelaxBorders => EdgeEdit::Relax {
                        target,
                        selected,
                        amount: self.ui.edges.relax,
                    },
                    _ => EdgeEdit::SmoothSubdivide(self.ui.edges.smoothness),
                };
                Some(Command::EditEdges(edit))
            }
            Action::ProjectDetails => Some(Command::OpenProjection),
            Action::RelaxMesh => Some(Command::OpenRelaxation),
            Action::KeepProjectionReference => Some(Command::KeepProjectionReference),
            Action::Subdivide => Some(Command::Subdivide),
            Action::Remesh => Some(Command::Remesh),
            Action::LowerLevel => self
                .document
                .mesh_settings()
                .level()
                .checked_sub(1)
                .filter(|&n| n > 0)
                .map(Command::SetMeshLevel),
            Action::HigherLevel => {
                let settings = self.document.mesh_settings();
                (settings.level() < settings.levels())
                    .then(|| Command::SetMeshLevel(settings.level() + 1))
            }
            Action::Save => Some(Command::Save),
            Action::Open => Some(Command::BrowseProject(false)),
            Action::SaveAs => Some(Command::BrowseProject(true)),
            Action::New => Some(Command::Request(PendingAction::New)),
            Action::Undo => Some(Command::Undo),
            Action::Redo => Some(Command::Redo),
            Action::ResetView => Some(Command::ResetView),
            Action::Perspective => Some(Command::SetPerspective(!self.camera.perspective)),
            Action::ZoomIn => Some(Command::ZoomIn),
            Action::ZoomOut => Some(Command::ZoomOut),
            Action::Mirror(axis, positive) => Some(Command::Mirror(axis, positive)),
            Action::Symmetry => {
                let mut symmetry = self.document.symmetry().clone();
                symmetry.enabled = !symmetry.enabled;
                Some(Command::SetSymmetry(symmetry))
            }
            Action::Wireframe => {
                self.ui.wire = !self.ui.wire;
                None
            }
            Action::Guides => {
                self.ui.show_symmetry_guides = !self.ui.show_symmetry_guides;
                None
            }
            Action::Smaller | Action::Larger => {
                let Some(index) = active_brush else { return };
                let mut brush = self.document.brushes()[index].clone();
                brush.radius = (brush.radius * if action == Action::Smaller { 0.9 } else { 1.1 })
                    .clamp(1., 500.);
                Some(Command::UpdateBrush(index, brush))
            }
            Action::PreviousBrush => {
                let index = (self.ui.selected + self.document.brushes().len() - 1)
                    % self.document.brushes().len();
                self.ui.select_brush(index, &self.document);
                None
            }
            Action::NextBrush => {
                let index = (self.ui.selected + 1) % self.document.brushes().len();
                self.ui.select_brush(index, &self.document);
                None
            }

            Action::CancelMeshOperation => Some(Command::CancelMeshJob),
            Action::ImportObj => Some(Command::BrowseAsset(AssetKind::Obj)),
            Action::AddReference => Some(Command::BrowseAsset(AssetKind::Reference)),
            Action::ArrangeReferences => {
                Some(Command::ArrangeReferences(!self.ui.arrange_references))
            }
            Action::ShowReferences => {
                self.ui.references_visible = !self.ui.references_visible;
                Some(Command::ArrangeReferences(false))
            }
            Action::ReferenceSettings => {
                self.ui
                    .preferences
                    .pin_menu(crate::preferences::BuiltinMenu::References, [200., 100.]);
                None
            }
            Action::Export => Some(Command::Export),
            Action::ClearMask => Some(Command::ClearMask),
            Action::FillAllMaterial => Some(Command::FillAllMaterial(
                self.document.brushes()[self.ui.selected].clone(),
            )),
            Action::FillMaterial => Some(Command::FillMaterial(
                self.document.brushes()[self.ui.selected].clone(),
            )),
            Action::InvertMask => Some(Command::InvertMask),
            Action::DeleteMasked => Some(Command::DeleteMasked),
            Action::ExpandMask => Some(Command::GrowMask(true)),
            Action::ContractMask => Some(Command::GrowMask(false)),
            Action::BlurMask => Some(Command::BlurMask),
            Action::SharpenMask => Some(Command::SharpenMask),
            Action::SelectVisibilityLine | Action::SelectVisibilityPath => {
                use crate::preferences::VisibilityShape;
                self.ui.preferences.visibility_selection.shape =
                    if action == Action::SelectVisibilityLine {
                        VisibilityShape::Line
                    } else {
                        VisibilityShape::Path
                    };
                let tool = self.ui.preferences.visibility_selection.tool();
                if self.temporary_visibility_active(ctx)
                    || self.ui.viewport_menu.context
                        == Some(crate::ui::context_menu::ViewportMenuContext::Visibility)
                {
                    if matches!(
                        self.ui.selection_tool,
                        Some(crate::viewport::SelectionTool::Visibility(_))
                    ) {
                        self.ui.selection_tool = Some(tool);
                    }
                    None
                } else {
                    Some(Command::SelectTool(
                        (self.ui.selection_tool != Some(tool)).then_some(tool),
                    ))
                }
            }
            Action::VisibilityHide | Action::VisibilityIsolate | Action::FlipVisibilitySide => {
                let temporary = self.temporary_visibility_active(ctx);
                let settings = &mut self.ui.preferences.visibility_selection;
                match action {
                    Action::VisibilityHide => settings.hide = true,
                    Action::VisibilityIsolate => settings.hide = false,
                    _ => settings.flip = !settings.flip,
                }
                if !temporary
                    && matches!(
                        self.ui.selection_tool,
                        Some(crate::viewport::SelectionTool::Visibility(_))
                    )
                {
                    self.ui.selection_tool = Some(settings.tool());
                }
                None
            }
            Action::ApplyVisibilityPath
            | Action::ClearVisibilityPath
            | Action::DeleteVisibilityPoint => {
                self.viewport.visibility_path_action(action);
                None
            }
            Action::SelectRectangle | Action::SelectLasso | Action::MaskLasso => {
                use crate::viewport::SelectionTool;
                let tool = match action {
                    Action::SelectRectangle => SelectionTool::Rectangle,
                    Action::SelectLasso => SelectionTool::Lasso,
                    _ => SelectionTool::MaskLasso,
                };
                Some(Command::SelectTool(
                    (self.ui.selection_tool != Some(tool)).then_some(tool),
                ))
            }
            Action::MaskLine => {
                use crate::viewport::SelectionTool;
                Some(Command::SelectTool(
                    (!matches!(self.ui.selection_tool, Some(SelectionTool::MaskLine { .. })))
                        .then_some(SelectionTool::MaskLine {
                            erase: false,
                            flip: false,
                        }),
                ))
            }
            Action::FlipMaskLine => {
                if let Some(crate::viewport::SelectionTool::MaskLine { flip, .. }) =
                    &mut self.ui.selection_tool
                {
                    *flip = !*flip;
                }
                None
            }
            Action::ShowAllFaces => Some(Command::ShowAllFaces),
            Action::InvertVisibility => Some(Command::InvertVisibility),
            Action::SplitHidden => Some(Command::SplitHidden),
            Action::Slice => Some(Command::SliceTool(!self.ui.slice_active)),
            Action::Primitive => Some(Command::RequestPrimitive),
            Action::ConvertPrimitive => Some(Command::ConvertPrimitive),
            Action::DuplicateModel => Some(Command::DuplicateModel),
            Action::DeleteSubmodel => Some(Command::DeleteSubmodel),
            Action::MergeDown => Some(Command::MergeDown),
            Action::ExtractMasked => Some(Command::ExtractMasked),
            Action::ApplyThickness => Some(Command::ApplyThickness),
            Action::TemporaryThickness => {
                let model = self
                    .document
                    .model(self.document.active_model_index())
                    .unwrap();
                Some(Command::SetTemporaryThickness(
                    if model.metadata.temporary_thickness > 0. {
                        0.
                    } else {
                        0.05
                    },
                ))
            }
            Action::GroupMasked => Some(Command::CreateVertexGroup(
                self.ui.vertex_group_name.clone(),
                false,
            )),
            Action::GroupAll => Some(Command::CreateVertexGroup(
                self.ui.vertex_group_name.clone(),
                true,
            )),
            Action::MaskGroup
            | Action::AddMaskedToGroup
            | Action::RemoveMaskedFromGroup
            | Action::DeleteGroup
            | Action::RenameGroup => self
                .ui
                .selected_vertex_group
                .filter(|(m, g)| {
                    *m == self.document.active_model_index()
                        && *g < self.document.mesh().vertex_groups.len()
                })
                .map(|(_, g)| match action {
                    Action::MaskGroup => Command::MaskVertexGroup(g),
                    Action::AddMaskedToGroup => Command::AssignMaskToGroup(g, false),
                    Action::RemoveMaskedFromGroup => Command::AssignMaskToGroup(g, true),
                    Action::RenameGroup => {
                        Command::RenameVertexGroup(g, self.ui.vertex_group_rename.clone())
                    }
                    _ => Command::DeleteVertexGroup(g),
                }),
            Action::DeleteLowerLevels => Some(Command::DeleteLowerLevels),
            Action::DeleteHigherLevels => Some(Command::DeleteHigherLevels),
            Action::SubdivisionMode => Some(Command::SetDynamicMesh(false)),
            Action::DynamicMode => Some(Command::SetDynamicMesh(true)),
            Action::AdaptiveMode => {
                let mut settings = self.document.mesh_settings().adaptive.clone();
                settings.enabled = true;
                Some(Command::SetAdaptive(settings))
            }
            Action::UniversalTransform => {
                if self.ui.transform_mode != crate::viewport::TransformMode::Sculpt {
                    Some(Command::TransformMode(
                        crate::viewport::TransformMode::Sculpt,
                    ))
                } else {
                    Some(Command::TransformOperation(
                        crate::viewport::TransformOperation::Universal,
                    ))
                }
            }
            Action::Sculpt => Some(Command::TransformMode(
                crate::viewport::TransformMode::Sculpt,
            )),
            Action::Transform => Some(Command::TransformMode(
                crate::viewport::TransformMode::World,
            )),
            Action::LineTransform => Some(Command::TransformMode(
                crate::viewport::TransformMode::ActionLine,
            )),
            Action::Move => Some(Command::TransformOperation(
                crate::viewport::TransformOperation::Move,
            )),
            Action::Scale => Some(Command::TransformOperation(
                crate::viewport::TransformOperation::Scale,
            )),
            Action::PlaceLine => Some(Command::TransformOperation(
                crate::viewport::TransformOperation::PlaceLine,
            )),
            Action::Rotate => Some(Command::TransformOperation(
                crate::viewport::TransformOperation::Rotate,
            )),
            Action::PlacePivot => Some(Command::TransformOperation(
                crate::viewport::TransformOperation::PlacePivot,
            )),
            Action::ResetPivot => Some(Command::ResetTransformPivot),
            Action::RespectMask => Some(Command::RespectTransformMask(
                !self.ui.respect_transform_mask,
            )),
            Action::PolygroupColors => {
                self.ui.show_polygroups = !self.ui.show_polygroups;
                None
            }
            Action::Backfaces => {
                self.ui.show_backfaces = !self.ui.show_backfaces;
                None
            }
            Action::WireTriangles => {
                self.ui.wire_triangles = !self.ui.wire_triangles;
                None
            }
            Action::WireVisible | Action::WireFront | Action::WireTransparent => {
                self.ui.wire_mode = match action {
                    Action::WireVisible => crate::viewport::WireMode::Visible,
                    Action::WireFront => crate::viewport::WireMode::Front,
                    _ => crate::viewport::WireMode::Transparent,
                };
                None
            }
            Action::ResetZoom => Some(Command::SetZoom(1.)),
            Action::CenterSymmetry => {
                let mut symmetry = self.document.symmetry().clone();
                symmetry.center = Default::default();
                Some(Command::SetSymmetry(symmetry))
            }
            Action::DuplicateBrush => active_brush.map(Command::DuplicateBrush),
            Action::CaptureBrushIcon => active_brush.map(Command::CaptureBrushIcon),
            Action::ToggleStencil => self.document.stencil().cloned().map(|mut s| {
                s.enabled = !s.enabled;
                Command::SetStencil(Some(s))
            }),
            Action::AttachStencil => Some(Command::AttachStencil),
            Action::RemoveStencil => Some(Command::SetStencil(None)),
            Action::LoadAlpha => Some(Command::BrowseAsset(AssetKind::Alpha)),
            Action::SavePreset => active_brush.map(Command::SavePreset),
            Action::LoadPreset => Some(Command::BrowseAsset(AssetKind::Brush)),
            Action::ExportLibrary => Some(Command::ExportBrushLibrary),
            Action::ImportLibrary => Some(Command::BrowseAsset(AssetKind::BrushLibrary)),
            Action::RestoreBrushIcon
            | Action::ResetBrushSettings
            | Action::BrushAdd
            | Action::BrushSubtract
            | Action::StrokeFreehand
            | Action::StrokeLine
            | Action::StrokeDragRect
            | Action::StrokeDragDot
            | Action::StrokeStabilisation
            | Action::AlphaSoft
            | Action::AlphaHard
            | Action::AlphaSquare => {
                let Some(index) = active_brush else { return };
                let mut brush = self.document.brushes()[index].clone();
                use crate::sculpt::{Alpha, StrokeMode};
                match action {
                    Action::RestoreBrushIcon => brush.icon = None,
                    Action::ResetBrushSettings => brush.reset_settings(),
                    Action::BrushAdd => brush.subtract = false,
                    Action::BrushSubtract => brush.subtract = true,
                    Action::StrokeFreehand => brush.stroke_mode = StrokeMode::Freehand,
                    Action::StrokeLine => brush.stroke_mode = StrokeMode::Line,
                    Action::StrokeDragRect => brush.stroke_mode = StrokeMode::DragRect,
                    Action::StrokeDragDot => brush.stroke_mode = StrokeMode::DragDot,
                    Action::StrokeStabilisation => {
                        if !brush.supports_stabilization() {
                            return;
                        }
                        brush.stabilize = !brush.stabilize;
                    }
                    Action::AlphaSoft => brush.alpha = Alpha::Soft,
                    Action::AlphaHard => brush.alpha = Alpha::Hard,
                    Action::AlphaSquare => brush.alpha = Alpha::Square,
                    _ => unreachable!(),
                }
                Some(Command::UpdateBrush(index, brush))
            }
            Action::RecoverySettings => {
                self.open_recovery();
                None
            }
            Action::SaveRecovery => {
                self.save_recovery(ctx);
                None
            }
            Action::KeySettings => {
                self.ui.settings_open = true;
                None
            }
            Action::CustomizeWorkspace => {
                self.ui.layout_open = true;
                None
            }
            Action::SelectBrush(id) => {
                if let Some(index) = self
                    .document
                    .brushes()
                    .iter()
                    .position(|b| b.identity() == id)
                {
                    self.ui.select_brush(index, &self.document);
                }
                None
            }
            Action::ShowPanel(tool) => {
                if tool == crate::preferences::Tool::Transform
                    && self
                        .ui
                        .preferences
                        .menu_placed(crate::preferences::BuiltinMenu::Transform)
                {
                    if let Some(section) = self
                        .ui
                        .preferences
                        .menu_sections
                        .iter_mut()
                        .find(|s| s.menu == crate::preferences::BuiltinMenu::Transform)
                    {
                        section.open = true;
                    }
                    return;
                }
                if let Some(panel) = self
                    .ui
                    .preferences
                    .panels
                    .iter_mut()
                    .find(|p| p.tool == tool)
                {
                    panel.open = true;
                    if panel.dock == crate::preferences::Dock::Hidden {
                        panel.dock = crate::preferences::Dock::Floating;
                    }
                }
                None
            }
            Action::OpenMenu(id) => {
                self.ui.customization.popup_menu = Some(id);
                None
            }
            Action::NewBrush(name) if name == "Alpha Drag" => Some(Command::AddAlphaDragBrush),
            Action::NewBrush(name) => crate::sculpt::Kind::ALL
                .into_iter()
                .find(|k| format!("{k:?}") == name)
                .map(Command::AddBrush),
        };
        if let Some(command) = command {
            self.dispatch(command, ctx);
        }
    }
}

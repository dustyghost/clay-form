//! Resolve reusable actions into concrete application commands.
use super::*;

impl App {
    pub(super) fn execute_action(&mut self, action: Action, ctx: &egui::Context) {
        if !self.ui.action_enabled(&action, &self.document) {
            return;
        }
        let active_brush = self.ui.brush_index_for_context(&self.document, ctx);
        let command = match action {
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
            Action::Open => Some(Command::Request(PendingAction::Open(self.ui.path.clone()))),
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
            Action::ImportObj => Some(Command::ImportObj),
            Action::AddReference => Some(Command::AddReference(egui::DroppedFile {
                path: Some(self.ui.reference_path.clone().into()),
                ..Default::default()
            })),
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
            Action::InvertMask => Some(Command::InvertMask),
            Action::DeleteMasked => Some(Command::DeleteMasked),
            Action::ExpandMask => Some(Command::GrowMask(true)),
            Action::ContractMask => Some(Command::GrowMask(false)),
            Action::BlurMask => Some(Command::BlurMask),
            Action::SharpenMask => Some(Command::SharpenMask),
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
            Action::ShowAllFaces => Some(Command::ShowAllFaces),
            Action::InvertVisibility => Some(Command::InvertVisibility),
            Action::SplitHidden => Some(Command::SplitHidden),
            Action::Slice => Some(Command::SliceTool(!self.ui.slice_active)),
            Action::Primitive => Some(Command::RequestPrimitive),
            Action::ConvertPrimitive => Some(Command::ConvertPrimitive),
            Action::DuplicateModel => Some(Command::DuplicateModel),
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
            Action::LoadAlpha => active_brush.map(Command::LoadAlpha),
            Action::SavePreset => active_brush.map(Command::SavePreset),
            Action::LoadPreset => Some(Command::LoadPreset),
            Action::ExportLibrary => Some(Command::ExportBrushLibrary),
            Action::ImportLibrary => Some(Command::ImportBrushLibrary),
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

//! One availability policy for buttons, shortcuts, and direct commands.
use crate::{action::Action, command::Command, document::Document, sculpt::Kind, ui::UiState};

#[derive(Clone, Copy, PartialEq)]
enum Intent {
    Edit,
    Navigate,
    History,
    Brush,
    PathGuides,
    Path,
    Projection,
    FilePicker,
    Pending,
    Primitive,
    Alpha,
    Icon,
    CancelJob,
    Request,
}

pub(crate) struct EditorPolicy<'a> {
    ui: &'a UiState,
    document: &'a Document,
}
impl<'a> EditorPolicy<'a> {
    pub fn new(ui: &'a UiState, document: &'a Document) -> Self {
        Self { ui, document }
    }
    fn available(&self, intent: Intent) -> Result<(), &'static str> {
        let ui = self.ui;
        if ui.file_picker.is_some() && intent != Intent::FilePicker {
            return Err("Finish choosing a file first");
        }
        if intent == Intent::FilePicker {
            return Ok(());
        }
        if ui.pending.is_some() && intent != Intent::Pending {
            return Err("Resolve unsaved changes first");
        }
        if ui.primitive_warning.is_some() && intent != Intent::Primitive {
            return Err("Confirm or cancel primitive replacement first");
        }
        if (ui.alphas.draft.is_some() || ui.alphas.overwrite.is_some()) && intent != Intent::Alpha {
            return Err("Finish editing the alpha first");
        }
        if ui.icon_capture_open() && intent != Intent::Icon {
            return Err("Finish capturing the brush icon first");
        }
        if (ui.recovery_open || ui.diagnostics_open || ui.settings_open || ui.layout_open)
            && !matches!(intent, Intent::Request | Intent::Pending)
        {
            return Err("Close the open dialog before editing");
        }
        if ui.projection.open
            && !matches!(
                intent,
                Intent::Projection | Intent::Navigate | Intent::Request
            )
        {
            return Err("Apply or cancel the surface preview before editing the scene");
        }
        if ui.paths_active {
            if !matches!(
                intent,
                Intent::Path
                    | Intent::PathGuides
                    | Intent::History
                    | Intent::Brush
                    | Intent::Navigate
                    | Intent::Request
            ) {
                return Err("Finish editing paths before using another tool");
            }
            if intent == Intent::History && !ui.paths_history_ready {
                return Err(ui
                    .paths_history_reason
                    .unwrap_or("Finish editing paths before Undo or Redo"));
            }
            if intent == Intent::Brush && ui.paths_preview_active {
                return Err("Apply or cancel the path preview before changing the brush");
            }
        }
        if ui.mesh_busy
            && !matches!(
                intent,
                Intent::Navigate | Intent::Brush | Intent::CancelJob | Intent::Request
            )
        {
            return Err("Wait for the mesh operation, or cancel it first");
        }
        Ok(())
    }
    pub fn path(&self) -> Result<(), &'static str> {
        self.available(Intent::Path)
    }
    pub fn history(&self) -> Result<(), &'static str> {
        self.available(Intent::History)
    }
    pub fn action(&self, action: &Action) -> Result<(), &'static str> {
        let intent = match action {
            Action::CancelMeshOperation => Intent::CancelJob,
            Action::ResetView | Action::ZoomIn | Action::ZoomOut | Action::Perspective => {
                Intent::Navigate
            }
            Action::Undo | Action::Redo => Intent::History,
            Action::TogglePathGuides | Action::TogglePathOnlyStrokes => Intent::PathGuides,
            _ => Intent::Edit,
        };
        self.available(intent)?;
        if *action == Action::CancelMeshOperation {
            return if self.ui.mesh_busy {
                Ok(())
            } else {
                Err("No mesh operation is running")
            };
        }
        if self.action_supported(action) {
            Ok(())
        } else if *action == Action::Undo {
            Err("Nothing to undo")
        } else if *action == Action::Redo {
            Err("Nothing to redo")
        } else if *action == Action::Remesh && !self.document.mesh_settings().dynamic {
            Err("Switch to dynamic mode first")
        } else {
            Err("Action is unavailable for the current selection")
        }
    }
    fn action_supported(&self, action: &Action) -> bool {
        let document = self.document;
        let sculpt = document.primitive().is_none() && document.active_visible();
        match action {
            Action::UniversalTransform => {
                self.ui.transform_mode != crate::viewport::TransformMode::Sculpt
                    || (document.active_visible() && !document.mesh().vertices.is_empty())
            }
            Action::Move
            | Action::Scale
            | Action::Rotate
            | Action::PlaceLine
            | Action::PlacePivot => {
                document.active_visible() && !document.mesh().vertices.is_empty()
            }
            Action::ResetPivot => {
                self.ui.transform_mode != crate::viewport::TransformMode::Sculpt
                    && document.active_visible()
                    && !document.mesh().vertices.is_empty()
            }
            Action::ToggleStencil | Action::RemoveStencil => document.stencil().is_some(),
            Action::AttachStencil => sculpt && document.stencil().is_some(),
            Action::ImportObj => document.model_count() < 64,
            Action::AddReference => document.references().len() < crate::reference::MAX_REFERENCES,
            Action::ArrangeReferences => !document.references().is_empty(),
            Action::Undo => document.can_undo(),
            Action::Redo => document.can_redo(),
            Action::SelectBrush(id) => document.brushes().iter().any(|b| b.identity() == *id),
            Action::NewBrush(name) => {
                name == "Alpha Drag" || Kind::ALL.iter().any(|k| format!("{k:?}") == *name)
            }
            Action::OpenMenu(id) => self.ui.preferences.menus.iter().any(|m| m.id == *id),

            Action::ModelCreateGroup | Action::ModelAddToGroup | Action::ModelRemoveFromGroup => {
                self.edge_unavailable(action).is_none()
                    && self.ui.modeling.count > 0
                    && document.mesh_settings().levels() == 1
                    && (*action == Action::ModelCreateGroup
                        || self.ui.modeling.group < document.mesh().vertex_groups.len())
            }
            Action::ModelRepeat => {
                self.edge_unavailable(action).is_none()
                    && self.ui.modeling.active
                    && self.ui.modeling.can_repeat
                    && self.ui.modeling.count > 0
            }
            Action::ModelSavePreset => self.ui.modeling.active,
            Action::ModelPreset(id) => {
                self.edge_unavailable(action).is_none()
                    && self
                        .ui
                        .preferences
                        .modeling_presets
                        .iter()
                        .any(|p| p.id == *id)
            }
            Action::ModelDeletePreset(id) => self
                .ui
                .preferences
                .modeling_presets
                .iter()
                .any(|p| p.id == *id),
            Action::ModelSmoothPreview | Action::ModelTapRepeat => {
                self.edge_unavailable(action).is_none()
            }
            Action::ModelTopology(_) => {
                self.edge_unavailable(action).is_none()
                    && self.ui.modeling.active
                    && self.ui.modeling.count > 0
            }
            Action::ModelApply => {
                self.edge_unavailable(action).is_none()
                    && self.ui.modeling.active
                    && self.ui.modeling.count > 0
                    && (self.ui.modeling.tool.is_edit()
                        || self.ui.modeling.tool == crate::viewport::ModelTool::Extrude)
            }
            Action::ModelBoundary => {
                self.edge_unavailable(action).is_none()
                    && self.ui.modeling.active
                    && self.ui.modeling.count > 0
                    && self.ui.modeling.mode == crate::viewport::ComponentMode::Face
            }
            Action::ModelTarget(span) => {
                self.edge_unavailable(action).is_none() && span.available(self.ui.modeling.mode)
            }
            Action::ModelTool(_) | Action::ModelProtectMask => {
                self.edge_unavailable(action).is_none()
            }
            Action::ModelDissolveLoops => {
                self.edge_unavailable(action).is_none()
                    && self.ui.modeling.active
                    && self.ui.modeling.mode == crate::viewport::ComponentMode::Edge
                    && self.ui.modeling.count > 0
            }
            Action::ModelFewerLoops => {
                self.edge_unavailable(action).is_none() && self.ui.modeling.loop_count > 1
            }
            Action::ModelMoreLoops => {
                self.edge_unavailable(action).is_none()
                    && self.ui.modeling.loop_count < crate::sculpt::LoopCut::MAX_COUNT
            }
            Action::PenModeling
            | Action::ModelFaces
            | Action::ModelEdges
            | Action::ModelVertices
            | Action::ModelSelect
            | Action::ModelExtrude
            | Action::ModelLoopCut
            | Action::ModelReplace
            | Action::ModelAdd
            | Action::ModelRemove
            | Action::ModelJoin
            | Action::ModelClearSelection
            | Action::ModelSelectLoop
            | Action::ModelSelectSingle
            | Action::ModelSelectLoops
            | Action::ModelSelectRing
            | Action::SelectEdges
            | Action::SelectCreases
            | Action::SelectBorderEdges
            | Action::ExpandEdgeLoop
            | Action::ClearEdgeSelection
            | Action::CreaseEdges
            | Action::UncreaseEdges
            | Action::ClearCreases
            | Action::AddSupportLoops
            | Action::RelaxBorders
            | Action::SmoothSubdivide => self.edge_unavailable(action).is_none(),
            Action::Mirror(..) => self.mirror_unavailable().is_none(),
            Action::DeleteSubmodel => {
                !self.ui.recovery_open
                    && !self.ui.diagnostics_open
                    && !self.ui.settings_open
                    && !self.ui.layout_open
                    && self.ui.file_picker.is_none()
            }
            Action::MergeDown => document.merge_down_target().is_some(),
            Action::ConvertPrimitive => {
                document.primitive().is_some() && !document.mesh().triangles.is_empty()
            }
            Action::ApplyThickness => {
                document.active_visible()
                    && document
                        .model(document.active_model_index())
                        .unwrap()
                        .metadata
                        .temporary_thickness
                        > 0.
            }
            Action::MaskGroup
            | Action::AddMaskedToGroup
            | Action::RemoveMaskedFromGroup
            | Action::DeleteGroup
            | Action::RenameGroup => {
                sculpt
                    && self.ui.selected_vertex_group.is_some_and(|(m, g)| {
                        m == document.active_model_index()
                            && g < document.mesh().vertex_groups.len()
                    })
            }
            Action::SplitHidden => {
                sculpt
                    && document.mesh().hidden_face_count() > 0
                    && document.mesh().hidden_face_count() < document.mesh().triangles.len()
            }
            Action::LowerLevel | Action::DeleteLowerLevels => {
                sculpt && document.mesh_settings().level() > 1
            }
            Action::HigherLevel | Action::DeleteHigherLevels => {
                sculpt && document.mesh_settings().level() < document.mesh_settings().levels()
            }
            Action::ProjectDetails | Action::RelaxMesh => {
                document.require_projection_target().is_ok()
            }
            Action::EditPaths => document.active_visible(),
            Action::StrokeEntirePath => {
                sculpt
                    && document.active_visible()
                    && document.paths().iter().any(|p| {
                        p.target == document.active_model_id()
                            && p.mode == crate::sculpt::paths::PathMode::Surface
                    })
            }
            Action::KeepProjectionReference => {
                document.model_count() < 64 && document.require_projection_target().is_ok()
            }
            Action::Subdivide => {
                sculpt
                    && !document.mesh_settings().dynamic
                    && !document.mesh_settings().adaptive.enabled
                    && document.mesh_settings().upper.is_empty()
                    && document.mesh().triangles.len() <= 250_000
            }
            Action::Remesh => {
                sculpt
                    && document.mesh_settings().dynamic
                    && document.mesh().hidden_face_count() == 0
            }
            Action::SelectVisibilityLine
            | Action::SelectVisibilityPath
            | Action::VisibilityHide
            | Action::VisibilityIsolate
            | Action::FlipVisibilitySide => sculpt && document.active_visible(),
            Action::ApplyVisibilityPath
            | Action::ClearVisibilityPath
            | Action::DeleteVisibilityPoint => {
                sculpt
                    && ((self.ui.viewport_menu.visibility_was_active
                        && self.ui.preferences.visibility_selection.shape
                            == crate::preferences::VisibilityShape::Path)
                        || matches!(
                            self.ui.selection_tool,
                            Some(crate::viewport::SelectionTool::Visibility(
                                crate::preferences::VisibilitySettings {
                                    shape: crate::preferences::VisibilityShape::Path,
                                    ..
                                }
                            ))
                        ))
            }
            Action::MaskLine => sculpt && document.active_visible(),
            Action::FlipMaskLine => {
                sculpt
                    && matches!(
                        self.ui.selection_tool,
                        Some(crate::viewport::SelectionTool::MaskLine { .. })
                    )
            }
            Action::ClearMask
            | Action::InvertMask
            | Action::DeleteMasked
            | Action::ExpandMask
            | Action::ContractMask
            | Action::BlurMask
            | Action::SharpenMask
            | Action::SelectLasso
            | Action::MaskLasso
            | Action::SelectRectangle
            | Action::ShowAllFaces
            | Action::InvertVisibility
            | Action::Slice
            | Action::ExtractMasked
            | Action::GroupMasked
            | Action::GroupAll
            | Action::SubdivisionMode
            | Action::DynamicMode
            | Action::AdaptiveMode => sculpt,
            _ => true,
        }
    }
    pub fn edge_unavailable(&self, action: &Action) -> Option<&'static str> {
        if let Err(reason) = self.available(Intent::Edit) {
            return Some(reason);
        }
        let document = self.document;
        if document.primitive().is_some() {
            Some("Convert the primitive in Submodels before editing its edges.")
        } else if !document.active_visible() {
            Some("Show the active submodel before editing its edges.")
        } else if document
            .model(document.active_model_index())
            .unwrap()
            .metadata
            .temporary_thickness
            > 0.
        {
            Some("Apply or disable the Thickness modifier in Submodels to edit the base mesh.")
        } else if *action == Action::SmoothSubdivide && document.mesh().has_hidden_faces() {
            Some("Show all faces before smooth subdivision.")
        } else {
            None
        }
    }
    pub fn mirror_unavailable(&self) -> Option<&'static str> {
        if let Err(reason) = self.available(Intent::Edit) {
            return Some(reason);
        }
        let document = self.document;
        if document.primitive().is_some() {
            Some("Convert the primitive to a sculpting mesh first.")
        } else if !document.active_visible() || document.mesh().has_hidden_faces() {
            Some("Show the active model and all of its faces first.")
        } else if document.mesh().triangles.is_empty() {
            Some("The active model has no surface to mirror.")
        } else if document
            .model(document.active_model_index())
            .unwrap()
            .metadata
            .temporary_thickness
            > 0.
        {
            Some("Apply or remove thickness modifier first.")
        } else {
            None
        }
    }
    pub fn command(&self, command: &Command) -> Result<(), &'static str> {
        let intent = match command {
            Command::Action(action) => return self.action(action),
            Command::Path(_) => Intent::Path,
            Command::ResetView
            | Command::ZoomIn
            | Command::ZoomOut
            | Command::SetZoom(_)
            | Command::SetPerspective(_)
            | Command::SetFieldOfView(_) => Intent::Navigate,
            Command::Undo | Command::Redo => Intent::History,
            Command::UpdateBrush(..) => Intent::Brush,
            Command::PreviewProjection
            | Command::ClearProjectionPreview
            | Command::ApplyProjection
            | Command::CancelProjection => Intent::Projection,
            Command::NavigateProjectFiles(_)
            | Command::AcceptProjectFile(_)
            | Command::CancelProjectFile => Intent::FilePicker,
            Command::SaveAndContinue
            | Command::DiscardAndContinue
            | Command::CancelPending
            | Command::BrowseProject(_) => Intent::Pending,
            Command::ConfirmPrimitive | Command::CancelPrimitive => Intent::Primitive,
            Command::SaveAlpha(..) | Command::ExportAlpha(..) => Intent::Alpha,
            Command::ExportIconImage(..) => Intent::Icon,
            Command::CancelMeshJob => Intent::CancelJob,
            Command::Request(_) => Intent::Request,
            _ => Intent::Edit,
        };
        self.available(intent)?;
        let action = match command {
            Command::Undo => Some(Action::Undo),
            Command::Redo => Some(Action::Redo),
            Command::Subdivide => Some(Action::Subdivide),
            Command::Remesh => Some(Action::Remesh),
            Command::DeleteSubmodel => Some(Action::DeleteSubmodel),
            Command::MergeDown => Some(Action::MergeDown),
            Command::SplitHidden => Some(Action::SplitHidden),
            Command::ConvertPrimitive => Some(Action::ConvertPrimitive),
            Command::OpenProjection => Some(Action::ProjectDetails),
            Command::OpenRelaxation => Some(Action::RelaxMesh),
            Command::KeepProjectionReference => Some(Action::KeepProjectionReference),
            Command::ApplyThickness => Some(Action::ApplyThickness),
            _ => None,
        };
        if let Some(action) = action {
            self.action(&action)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    #[test]
    fn buttons_shortcuts_and_direct_edits_share_modal_and_job_policy() {
        for mode in 0..5 {
            let mut app = App::default();
            match mode {
                0 => app.ui.mesh_busy = true,
                1 => app.ui.projection.open = true,
                2 => app.viewport.paths.active = true,
                3 => app.ui.settings_open = true,
                _ => app.ui.pending = Some(crate::command::PendingAction::New),
            }
            app.sync_path_history_state();
            let before = app.document.project();
            let counts = app.document.history_counts();
            let action = Action::InvertMask;
            assert!(!app.ui.action_enabled(&action, &app.document));
            let ctx = eframe::egui::Context::default();
            app.dispatch(Command::Action(action), &ctx);
            app.dispatch(Command::InvertMask, &ctx);
            assert!(app.document.project() == before, "mode {mode}");
            assert_eq!(app.document.history_counts(), counts);
        }
    }
    #[test]
    fn navigation_and_cancellation_remain_available_during_jobs() {
        let mut app = App::default();
        app.ui.mesh_busy = true;
        let policy = EditorPolicy::new(&app.ui, &app.document);
        assert!(policy.action(&Action::ResetView).is_ok());
        assert!(policy.command(&Command::ResetView).is_ok());
        assert!(policy.action(&Action::CancelMeshOperation).is_ok());
        assert!(policy.command(&Command::CancelMeshJob).is_ok());
        assert!(policy.command(&Command::Subdivide).is_err());
    }
}

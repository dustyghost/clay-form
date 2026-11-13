//! One availability policy for buttons, shortcuts, and direct commands.
use crate::{action::Action, command::Command, document::Document, sculpt::Kind, ui::UiState};

#[derive(Clone, Copy, PartialEq)]
enum Intent {
    Instances,
    Edit,
    Navigate,
    History,
    Brush,
    PathGuides,
    Path,
    Projection,
    Fusion,
    FilePicker,
    Pending,
    Primitive,
    DeleteConfirmation,
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
        if self.document.instance_preview_active()
            && !matches!(
                intent,
                Intent::Instances | Intent::Navigate | Intent::History | Intent::Request
            )
        {
            return Err("Apply or cancel the instance preview first");
        }
        if ui.file_picker.is_some() && intent != Intent::FilePicker {
            return Err("Finish choosing a file first");
        }
        if intent == Intent::FilePicker {
            return Ok(());
        }
        if ui.pending.is_some() && intent != Intent::Pending {
            return Err("Resolve unsaved changes first");
        }
        if ui.delete_warning.is_some() && intent != Intent::DeleteConfirmation {
            return Err("Confirm or cancel submodel deletion first");
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
        if (ui.recovery_open
            || ui.diagnostics_open
            || ui.settings_open
            || ui.mesh_capacity_open
            || ui.layout_open
            || ui.viewport_controls.editor_open
            || ui.viewport_controls.arranging)
            && !matches!(intent, Intent::Request | Intent::Pending)
        {
            return Err("Close the open dialog before editing");
        }
        if ui.fusion.open && !matches!(intent, Intent::Fusion | Intent::Navigate | Intent::Request)
        {
            return Err("Apply or cancel the mesh join before editing the scene");
        }
        if ui.projection.open
            && !(intent == Intent::History
                && ui.projection.mode == crate::ui::SurfaceMode::Retopology
                && !ui.projection.busy)
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
            Action::Search | Action::ToggleZen => Intent::Request,
            Action::CreateInstanceArray
            | Action::CreatePathAssembly
            | Action::CaptureObjectBrush => Intent::Instances,
            Action::CancelMeshOperation => Intent::CancelJob,
            Action::ResetView | Action::ZoomIn | Action::ZoomOut | Action::Perspective => {
                Intent::Navigate
            }
            Action::Undo | Action::Redo => Intent::History,
            Action::SelectBrush(_) if self.ui.path_brush_active => Intent::Brush,
            Action::GuidedRetopology if self.ui.path_brush_active => Intent::Path,
            Action::TogglePathGuides | Action::TogglePathOnlyStrokes => Intent::PathGuides,
            _ => Intent::Edit,
        };
        self.available(intent)?;
        if self.ui.projection.open && intent == Intent::History {
            let label = if *action == Action::Undo {
                self.document.undo_label()
            } else {
                self.document.redo_label()
            };
            if label != Some("Retopology density stroke") {
                return Err("Close Guided retopology to undo or redo other edits");
            }
        }
        if *action == Action::CancelMeshOperation {
            return if self.ui.mesh_busy {
                Ok(())
            } else {
                Err("No mesh operation is running")
            };
        }
        if self.action_supported(action) {
            Ok(())
        } else {
            Err(self.action_prerequisite(action))
        }
    }

    /// The same explanation is shown by search, shelves, menus and the dispatcher.
    /// Only called after the shared modal/job checks and the support check fail.
    fn action_prerequisite(&self, action: &Action) -> &'static str {
        let document = self.document;
        match action {
            Action::Undo => return "Nothing to undo",
            Action::Redo => return "Nothing to redo",
            Action::TemporaryMaskPreset(id) if !document.brushes().iter().any(|brush| brush.kind == Kind::Mask && brush.identity() == *id) => return "This Mask preset no longer exists. Choose a Mask brush in Mask / visibility",
            Action::SelectBrush(_) => return "This brush is no longer in the project. Choose a brush from the Brushes panel",
            Action::OpenMenu(_) => return "This custom menu no longer exists. Open Workspace to create or restore one",
            Action::ModelPreset(_) | Action::ModelDeletePreset(_) => {
                if !self.ui.preferences.modeling_presets.iter().any(|p| matches!(action, Action::ModelPreset(id) | Action::ModelDeletePreset(id) if p.id == *id)) {
                    return "This modelling preset no longer exists. Choose a current preset";
                }
            }
            Action::ToggleStencil | Action::RemoveStencil | Action::AttachStencil => {
                if document.stencil().is_none() {
                    return "Load an alpha and enable its stencil first";
                }
            }
            Action::ImportObj | Action::JoinMeshes | Action::KeepProjectionReference | Action::GuidedRetopology if document.model_count() >= 64 => return "Remove an unused submodel to free a model slot first",
            Action::AddReference => return "Remove an unused reference image before adding another",
            Action::ArrangeReferences => return "Add a reference image first",
            Action::MergeDown => return "Choose a model with another model below it in Submodels",
            Action::ConvertPrimitive => return "Select a primitive with a surface to convert",
            Action::ApplyThickness => return "Enable the Thickness modifier on the active model first",
            _ => {}
        }
        if document.primitive().is_some() {
            return "Convert the primitive in Submodels to a sculpting mesh first";
        }
        if !document.active_visible() {
            return "Show the active model in Submodels first";
        }
        if matches!(
            action,
            Action::ModelApply
                | Action::ModelRepeat
                | Action::ModelTopology(_)
                | Action::ModelBoundary
                | Action::ModelDissolveLoops
                | Action::ModelTarget(_)
                | Action::ModelCreateGroup
                | Action::ModelAddToGroup
                | Action::ModelRemoveFromGroup
                | Action::ModelPreset(_)
                | Action::ModelTool(_)
                | Action::ModelProtectMask
                | Action::ModelFewerLoops
                | Action::ModelMoreLoops
                | Action::ModelSmoothPreview
                | Action::ModelTapRepeat
                | Action::PenModeling
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
                | Action::SmoothSubdivide
        ) && let Some(reason) = self.edge_unavailable(action)
        {
            return reason;
        }
        if matches!(action, Action::Mirror(..))
            && let Some(reason) = self.mirror_unavailable()
        {
            return reason;
        }
        if matches!(
            action,
            Action::GuidedRetopology
                | Action::ProjectDetails
                | Action::RelaxMesh
                | Action::KeepProjectionReference
        ) {
            if document
                .attached_array(document.active_model_index())
                .is_some_and(|array| array.visible)
                && !document.editing_array_source()
            {
                return "Choose Edit source for this array, or Make editable before editing its surface";
            }
            if document
                .model(document.active_model_index())
                .is_some_and(|model| model.metadata.temporary_thickness > 0.)
            {
                return "Apply or remove the target's Thickness modifier before editing its surface";
            }
        }
        match action {
            Action::ModelApply if !self.ui.modeling.active => {
                "Enable Modeling and choose a tool first"
            }
            Action::ModelApply
                if !self.ui.modeling.tool.is_edit()
                    && self.ui.modeling.tool != crate::viewport::ModelTool::Extrude =>
            {
                "Choose an editing tool in Modeling before applying an amount"
            }
            Action::ModelRepeat if !self.ui.modeling.can_repeat => {
                "Apply a modelling edit first, then select components to repeat it"
            }
            Action::ModelDissolveLoops
                if self.ui.modeling.mode != crate::viewport::ComponentMode::Edge =>
            {
                "Switch to Edge mode and select a loop first"
            }
            Action::ModelBoundary
                if self.ui.modeling.mode != crate::viewport::ComponentMode::Face =>
            {
                "Switch to Face mode and select faces first"
            }
            Action::ModelApply
            | Action::ModelRepeat
            | Action::ModelTopology(_)
            | Action::ModelBoundary
                if self.ui.modeling.count == 0 =>
            {
                match self.ui.modeling.mode {
                    crate::viewport::ComponentMode::Face => "Select a face in the viewport first",
                    crate::viewport::ComponentMode::Edge => "Select an edge in the viewport first",
                    crate::viewport::ComponentMode::Vertex => {
                        "Select a vertex in the viewport first"
                    }
                }
            }
            Action::ModelDissolveLoops => "Select an edge loop in the viewport first",
            Action::FillMaterial | Action::FillAllMaterial
                if !document
                    .brushes()
                    .get(self.ui.selected)
                    .is_some_and(|brush| brush.kind == Kind::Paint) =>
            {
                "Select a Paint brush before filling material"
            }
            Action::FillMaterial | Action::FillAllMaterial
                if document.texture_paint.is_some()
                    && !document.mesh().uv.current(document.mesh()) =>
            {
                "Generate current UVs in UV / textures before painting the image"
            }
            Action::FillMaterial | Action::FillAllMaterial => {
                "Choose an assigned paint image in Textures first"
            }
            Action::ModelSavePreset => "Enable Modeling before saving its settings as a preset",
            Action::ModelTarget(_) => "Choose a component mode that supports this selection target",
            Action::ModelFewerLoops => "The loop count is already at its minimum",
            Action::ModelMoreLoops => "The loop count is already at its maximum",
            Action::ModelCreateGroup | Action::ModelAddToGroup | Action::ModelRemoveFromGroup
                if document.mesh_settings().levels() > 1 =>
            {
                "Use a mesh with a single subdivision level before editing component groups"
            }
            Action::ModelCreateGroup | Action::ModelAddToGroup | Action::ModelRemoveFromGroup
                if self.ui.modeling.count == 0 =>
            {
                "Select components in the viewport first"
            }
            Action::ModelAddToGroup | Action::ModelRemoveFromGroup => {
                "Choose a selection group in Modeling first"
            }
            Action::MaskGroup
            | Action::AddMaskedToGroup
            | Action::RemoveMaskedFromGroup
            | Action::DeleteGroup
            | Action::RenameGroup => "Choose a selection group on the active model first",
            Action::LowerLevel | Action::DeleteLowerLevels => {
                "The active model is already at its lowest subdivision level"
            }
            Action::HigherLevel | Action::DeleteHigherLevels => {
                "The active model is already at its highest subdivision level"
            }
            Action::Subdivide
                if document.mesh_settings().dynamic
                    || document.mesh_settings().adaptive.enabled =>
            {
                "Switch to Subdivision mode before adding a level"
            }
            Action::Subdivide if !document.mesh_settings().upper.is_empty() => {
                "Switch to the highest subdivision level before subdividing"
            }
            Action::Subdivide => {
                "Reduce the current mesh to 250,000 faces or fewer before subdividing"
            }
            Action::Remesh if !document.mesh_settings().dynamic => "Switch to Dynamic mode first",
            Action::Remesh => "Show all faces before remeshing",
            Action::SplitHidden => {
                "Hide some faces, leaving some visible, before splitting the model"
            }
            Action::StrokeEntirePath => "Create a surface path on the active model first",
            Action::CreatePathAssembly if document.paths().is_empty() => {
                "Create a path before adding a path assembly"
            }
            Action::ApplyVisibilityPath
            | Action::ClearVisibilityPath
            | Action::DeleteVisibilityPoint => "Choose Closed path in Mask / visibility first",
            Action::FlipMaskLine => "Choose the Mask line tool first",
            Action::GuidedRetopology => {
                "Choose an editable surface with valid topology for Guided retopology"
            }
            Action::ResetPivot => "Enable a transform tool before resetting its pivot",
            _ if document.mesh().triangles.is_empty() => {
                "Select a model with a surface in Submodels first"
            }
            _ => "Choose an editable model and the required components in the tool panel",
        }
    }
    fn action_supported(&self, action: &Action) -> bool {
        let document = self.document;
        let sculpt = document.primitive().is_none() && document.active_visible();
        match action {
            Action::TemporaryMaskPreset(id) => {
                sculpt
                    && document
                        .brushes()
                        .iter()
                        .any(|brush| brush.kind == Kind::Mask && brush.identity() == *id)
            }
            Action::TemporaryMaskBrush
            | Action::TemporaryMaskLasso
            | Action::TemporaryMaskLine
            | Action::TemporaryMaskAdd
            | Action::TemporaryMaskErase
            | Action::TemporaryMaskFlip
            | Action::TemporaryVisibilityRectangle
            | Action::TemporaryVisibilityLasso => sculpt,
            Action::FillMaterial | Action::FillAllMaterial => {
                sculpt
                    && document
                        .brushes()
                        .get(self.ui.selected)
                        .is_some_and(|brush| brush.kind == Kind::Paint)
                    && document.texture_paint.is_none_or(|channel| {
                        document.mesh().uv.current(document.mesh())
                            && document
                                .mesh()
                                .uv
                                .textures
                                .iter()
                                .any(|texture| texture.channel == channel)
                    })
            }
            Action::CreateInstanceArray | Action::CaptureObjectBrush => {
                !document.mesh().triangles.is_empty()
            }
            Action::CreatePathAssembly => {
                !document.mesh().triangles.is_empty() && !document.paths().is_empty()
            }
            Action::UniversalTransform => {
                self.ui.transform_mode != crate::viewport::TransformMode::Sculpt
                    || (document.active_visible() && !document.mesh().vertices.is_empty())
            }
            Action::DeformationFrame => sculpt && !document.mesh().vertices.is_empty(),
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
                matches!(name.as_str(), "Alpha Drag" | "Object")
                    || Kind::ALL.iter().any(|k| format!("{k:?}") == *name)
            }
            Action::OpenMenu(id) => self.ui.preferences.menus.iter().any(|m| m.id == *id),

            Action::ModelCreateGroup | Action::ModelAddToGroup | Action::ModelRemoveFromGroup => {
                self.edge_unavailable(action).is_none()
                    && self.ui.modeling.count > 0
                    && document.mesh_settings().levels() == 1
                    && (*action == Action::ModelCreateGroup
                        || document
                            .mesh()
                            .vertex_groups
                            .get(self.ui.modeling.group)
                            .is_some_and(|group| group.purpose.is_selection()))
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
                    && !self.ui.mesh_capacity_open
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
                            && document
                                .mesh()
                                .vertex_groups
                                .get(g)
                                .is_some_and(|group| group.purpose.is_selection())
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
            Action::GuidedRetopology => document.require_retopology_target().is_ok(),
            Action::ProjectDetails | Action::RelaxMesh => {
                document.require_projection_target().is_ok()
            }
            Action::JoinMeshes => document.active_visible() && document.model_count() < 64,
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
            Command::OpenRetopology if self.ui.path_brush_active => Intent::Path,
            Command::Instances(_) => Intent::Instances,
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
            Command::DiscoverFusion
            | Command::PreviewFusion
            | Command::ClearFusion
            | Command::ApplyFusion
            | Command::CancelFusion => Intent::Fusion,
            Command::NavigateProjectFiles(_)
            | Command::AcceptProjectFile(_)
            | Command::CancelProjectFile => Intent::FilePicker,
            Command::SaveAndContinue
            | Command::DiscardAndContinue
            | Command::CancelPending
            | Command::BrowseProject(_) => Intent::Pending,
            Command::ConfirmDeleteSubmodel(_) => Intent::DeleteConfirmation,
            Command::ConfirmPrimitive | Command::CancelPrimitive => Intent::Primitive,
            Command::SaveAlpha(..) | Command::ExportAlpha(..) => Intent::Alpha,
            Command::ExportIconImage(..) => Intent::Icon,
            Command::CancelMeshJob | Command::CancelExport => Intent::CancelJob,
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
            Command::OpenRetopology => Some(Action::GuidedRetopology),
            Command::OpenRelaxation => Some(Action::RelaxMesh),
            Command::OpenFusion => Some(Action::JoinMeshes),
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
        for mode in 0..6 {
            let mut app = App::default();
            match mode {
                0 => app.ui.mesh_busy = true,
                1 => app.ui.projection.open = true,
                2 => app.viewport.paths.active = true,
                3 => app.ui.settings_open = true,
                4 => app.ui.mesh_capacity_open = true,
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
    fn prerequisites_explain_component_selection_and_stale_entries() {
        let mut app = App::default();
        app.ui.modeling.active = true;
        app.ui.modeling.tool = crate::viewport::ModelTool::Move;
        assert_eq!(
            EditorPolicy::new(&app.ui, &app.document).action(&Action::ModelApply),
            Err("Select a face in the viewport first")
        );
        app.ui.modeling.mode = crate::viewport::ComponentMode::Edge;
        assert_eq!(
            EditorPolicy::new(&app.ui, &app.document).action(&Action::ModelApply),
            Err("Select an edge in the viewport first")
        );
        app.ui.modeling.count = 1;
        assert!(
            EditorPolicy::new(&app.ui, &app.document)
                .action(&Action::ModelApply)
                .is_ok()
        );
        assert!(
            EditorPolicy::new(&app.ui, &app.document)
                .action(&Action::SelectBrush("removed-brush".into()))
                .unwrap_err()
                .contains("no longer")
        );
        assert!(
            EditorPolicy::new(&app.ui, &app.document)
                .action(&Action::OpenMenu(u64::MAX))
                .unwrap_err()
                .contains("no longer")
        );
    }

    #[test]
    fn unrelated_thickness_does_not_obscure_subdivision_prerequisites() {
        let mut app = App::default();
        let plane = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
        app.document
            .replace_geometry(crate::sculpt::Geometry::new(plane.mesh().unwrap()))
            .unwrap();
        app.document.set_temporary_thickness(0.02).unwrap();
        assert_eq!(
            EditorPolicy::new(&app.ui, &app.document).action(&Action::LowerLevel),
            Err("The active model is already at its lowest subdivision level")
        );
    }

    #[test]
    fn paint_fill_explains_brush_and_missing_uv_prerequisites() {
        let mut app = App::default();
        assert_eq!(
            EditorPolicy::new(&app.ui, &app.document).action(&Action::FillMaterial),
            Err("Select a Paint brush before filling material")
        );
        app.ui.selected = app
            .document
            .brushes()
            .iter()
            .position(|brush| brush.kind == Kind::Paint)
            .unwrap();
        assert!(
            EditorPolicy::new(&app.ui, &app.document)
                .action(&Action::FillMaterial)
                .is_ok()
        );
        app.document.texture_paint = Some(crate::sculpt::texture::Channel::Color);
        assert_eq!(
            EditorPolicy::new(&app.ui, &app.document).action(&Action::FillMaterial),
            Err("Generate current UVs in UV / textures before painting the image")
        );
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

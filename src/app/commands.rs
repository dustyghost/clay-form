//! Execute UI commands and enforce edit boundaries.
use super::*;

impl App {
    fn set_camera_projection(&mut self, perspective: bool, field_of_view: f32) {
        if self.camera.perspective == perspective
            && self.camera.field_of_view == field_of_view.clamp(20., 75.)
        {
            return;
        }
        let before = self.camera;
        self.camera.perspective = perspective;
        self.camera.set_field_of_view(field_of_view);
        // Set the eye outside the visible scene when changing projection. Keep
        // this distance fixed during strokes, so deformation cannot move the view.
        let mut radius = 1_f32;
        for index in 0..self.document.model_count() {
            let model = self.document.model(index).unwrap();
            if model.metadata.visible {
                for point in &model
                    .display_mesh(index == self.document.active_model_index())
                    .vertices
                {
                    radius = radius.max(point.len());
                }
            }
        }
        self.camera.perspective_radius = radius;
        let (mut old_extent, mut new_extent) = (0_f32, 0_f32);
        for index in 0..self.document.model_count() {
            let model = self.document.model(index).unwrap();
            if !model.metadata.visible {
                continue;
            }
            let points = model
                .display_mesh(index == self.document.active_model_index())
                .vertices
                .iter()
                .copied();
            old_extent = old_extent.max(before.projected_extent(points.clone()));
            new_extent = new_extent.max(self.camera.projected_extent(points));
        }
        if old_extent > 1e-6 && new_extent > 1e-6 {
            self.camera.set_zoom(before.zoom * old_extent / new_extent);
        }
    }
    fn activate_transform(&mut self, mode: crate::viewport::TransformMode) {
        use crate::viewport::{TransformMode as Mode, TransformOperation as Op};
        self.ui.selection_tool = None;
        self.ui.slice_active = false;
        self.viewport.finish_transform_drag(&mut self.document);
        self.ui.transform_mode = mode;
        self.ui.transform_operation = if mode == Mode::ActionLine {
            Op::PlaceLine
        } else {
            Op::Move
        };
        self.ui.status = match mode {
            Mode::Sculpt => "Sculpt mode",
            Mode::World => "World axes: choose Move, Scale, Rotate, or Place pivot",
            Mode::ActionLine => {
                "Action Line: place a pivot and direction, then choose Move, Scale, or Rotate"
            }
        }
        .into();
    }
    pub(crate) fn dispatch(&mut self, command: Command, ctx: &egui::Context) {
        self.capture_diagnostic_context(ctx);
        self.sync_path_history_state();
        if let Command::Action(action) = command {
            self.execute_action(action, ctx);
            return;
        }
        if let Command::Path(command) = command {
            if let Err(error) = self.dispatch_path(command) {
                self.ui.status = error;
            }
            self.sync_path_history_state();
            return;
        }
        if self.viewport.paths.active
            && !matches!(
                command,
                Command::UpdateBrush(..)
                    | Command::Undo
                    | Command::Redo
                    | Command::ResetView
                    | Command::ZoomIn
                    | Command::ZoomOut
                    | Command::SetZoom(_)
                    | Command::SetPerspective(_)
                    | Command::SetFieldOfView(_)
            )
        {
            self.ui.status = "Finish editing paths before using another tool".into();
            return;
        }
        if matches!(command, Command::Undo | Command::Redo)
            && let Err(error) = self.prepare_path_history()
        {
            self.ui.status = error;
            return;
        }
        if self.viewport.paths.active
            && (self.paths.busy() || self.paths.has_preview())
            && matches!(command, Command::UpdateBrush(..))
        {
            self.ui.status = "Apply or cancel the path preview before changing the brush".into();
            return;
        }
        if self.ui.projection.open
            && !matches!(
                command,
                Command::PreviewProjection
                    | Command::ClearProjectionPreview
                    | Command::ApplyProjection
                    | Command::CancelProjection
                    | Command::ResetView
                    | Command::ZoomIn
                    | Command::ZoomOut
                    | Command::SetZoom(_)
                    | Command::SetPerspective(_)
                    | Command::SetFieldOfView(_)
            )
        {
            self.ui.status = "Apply or cancel the projection before editing the scene".into();
            return;
        }
        if self.mesh_job.is_some()
            && matches!(
                command,
                Command::OpenProjection | Command::KeepProjectionReference
            )
        {
            self.ui.status = "Wait for the mesh operation first".into();
            return;
        }
        if self.ui.file_picker.is_some()
            && !matches!(
                command,
                Command::NavigateProjectFiles(_)
                    | Command::AcceptProjectFile(_)
                    | Command::CancelProjectFile
            )
        {
            return;
        }
        if matches!(
            command,
            Command::DeleteSubmodel
                | Command::OpenProjection
                | Command::KeepProjectionReference
                | Command::AddReference(_)
                | Command::BrowseAsset(_)
                | Command::PlaceReference(..)
                | Command::RemoveReference(_)
                | Command::ArrangeReferences(_)
                | Command::Mirror(..)
                | Command::EditEdges(_)
                | Command::RequestModelingPrimitive
                | Command::ModelingMode(_)
                | Command::ModelingComponent(_)
                | Command::ClearModelSelection
                | Command::ModelSelectLoop
                | Command::ModelTopology(_)
                | Command::ModelRepeat
                | Command::ModelApply
                | Command::ModelBoundary
                | Command::ModelDissolveLoops
                | Command::GroupModelSelection(..)
        ) && (self.ui.pending.is_some()
            || self.ui.recovery_open
            || self.ui.diagnostics_open
            || self.ui.settings_open
            || self.ui.layout_open
            || self.ui.icon_capture_open())
        {
            return;
        }
        if self.ui.primitive_warning.is_some()
            && !matches!(
                command,
                Command::ConfirmPrimitive | Command::CancelPrimitive
            )
        {
            self.ui.status = "Confirm or cancel primitive replacement first".into();
            return;
        }
        if self.mesh_job.is_some()
            && matches!(
                command,
                Command::AddReference(_)
                    | Command::PlaceReference(..)
                    | Command::RemoveReference(_)
                    | Command::ArrangeReferences(_)
                    | Command::RequestModelingPrimitive
                    | Command::RequestPrimitive
                    | Command::ConfirmPrimitive
                    | Command::UpdatePrimitive(_)
                    | Command::ConvertPrimitive
                    | Command::Undo
                    | Command::Redo
                    | Command::EditSubmodel(..)
                    | Command::ReorderSubmodel(..)
                    | Command::AddGroup(_)
                    | Command::RemoveGroup(_)
                    | Command::ReorderGroup(..)
                    | Command::GroupVisible(..)
                    | Command::TransformMode(_)
                    | Command::TransformOperation(_)
                    | Command::RespectTransformMask(_)
                    | Command::ResetTransformPivot
                    | Command::SelectSubmodel(_)
                    | Command::DuplicateModel
                    | Command::DeleteSubmodel
                    | Command::MergeDown
                    | Command::ExtractMasked
                    | Command::ClearMask
                    | Command::InvertMask
                    | Command::DeleteMasked
                    | Command::SliceTool(_)
                    | Command::SelectTool(_)
                    | Command::ShowAllFaces
                    | Command::InvertVisibility
                    | Command::SplitHidden
                    | Command::SetTemporaryThickness(_)
                    | Command::ApplyThickness
                    | Command::GrowMask(_)
                    | Command::BlurMask
                    | Command::SharpenMask
                    | Command::MaskIntensity(_)
                    | Command::MaskThreshold(_)
                    | Command::CreateVertexGroup(..)
                    | Command::AssignMaskToGroup(..)
                    | Command::MaskVertexGroup(_)
                    | Command::RenameVertexGroup(..)
                    | Command::DeleteVertexGroup(_)
                    | Command::VertexGroupVisible(..)
                    | Command::SetMeshLevel(_)
                    | Command::DeleteLowerLevels
                    | Command::DeleteHigherLevels
                    | Command::SetDynamicMesh(_)
                    | Command::SetAdaptive(_)
                    | Command::SetRemeshSettings(..)
                    | Command::Subdivide
                    | Command::Remesh
                    | Command::Mirror(..)
                    | Command::EditEdges(_)
                    | Command::ModelingMode(_)
                    | Command::ModelingComponent(_)
                    | Command::ClearModelSelection
                    | Command::ModelSelectLoop
                    | Command::ModelTopology(_)
                    | Command::ModelRepeat
                    | Command::ModelApply
                    | Command::ModelBoundary
                    | Command::ModelDissolveLoops
                    | Command::GroupModelSelection(..)
            )
        {
            self.ui.status = "Wait for the mesh operation, or cancel it first".into();
            return;
        }
        let projection_changed = match command {
            Command::ResetView => true,
            Command::SetPerspective(value) => value != self.camera.perspective,
            Command::SetFieldOfView(value) => {
                value.is_finite() && value.clamp(20., 75.) != self.camera.field_of_view
            }
            _ => false,
        };
        if projection_changed {
            self.viewport.finish_transform_drag(&mut self.document);
        }
        // A new view must not change the projection underneath a latched
        // transform ring/axis. Keep the edit so far and require a fresh press.
        if matches!(
            command,
            Command::ZoomIn | Command::ZoomOut | Command::SetZoom(_) | Command::ResetView
        ) && (self.viewport.gesture == Some(crate::viewport::Gesture::Transform)
            || self.viewport.guided.dragging())
        {
            self.viewport.finish_transform_drag(&mut self.document);
        }
        if matches!(
            command,
            Command::SliceTool(_)
                | Command::SelectTool(_)
                | Command::TransformMode(_)
                | Command::TransformOperation(_)
                | Command::RequestPrimitive
                | Command::Undo
                | Command::Redo
        ) {
            self.ui.arrange_references = false;
            self.ui.edges.selecting = false;
            if !matches!(command, Command::Undo | Command::Redo) {
                self.ui.modeling.active = false;
            }
            self.viewport.cancel_reference_drag();
        }
        self.document.prepare_edit(command.edit_kind());
        let result: Result<(), String> = match command {
            Command::Path(_) => unreachable!(),
            Command::ModelingMode(active) => if active {
                self.document.require_modeling()
            } else {
                Ok(())
            }
            .map(|()| {
                self.viewport.cancel(&mut self.document);
                self.ui.modeling.active = active;
                self.ui.edges.selecting = false;
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                self.ui.arrange_references = false;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                if active {
                    self.ui.wire = true;
                }
            }),
            Command::ModelingComponent(mode) => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.sync(&self.document);
                self.viewport
                    .modeling
                    .convert_selection(self.document.mesh(), mode, false);
                self.viewport.set_edge_selection(Vec::new(), &self.document);
                self.ui.modeling.mode = mode;
                self.ui.modeling.span = crate::viewport::SelectionSpan::Single;
                self.ui
                    .modeling
                    .set_tool(crate::viewport::ModelTool::Select);
                Ok(())
            }
            Command::ClearModelSelection => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.selected.clear();
                self.viewport.set_edge_selection(Vec::new(), &self.document);
                Ok(())
            }
            Command::ModelBoundary => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.sync(&self.document);
                self.viewport.modeling.convert_selection(
                    self.document.mesh(),
                    crate::viewport::ComponentMode::Edge,
                    true,
                );
                self.ui.modeling.mode = crate::viewport::ComponentMode::Edge;
                self.ui
                    .modeling
                    .set_tool(crate::viewport::ModelTool::Select);
                self.ui.modeling.span = crate::viewport::SelectionSpan::Single;
                Ok(())
            }
            Command::ModelTopology(edit) => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.topology_edit(
                    &mut self.document,
                    edit,
                    self.ui.modeling.protect_mask,
                )
            }
            Command::ModelRepeat => {
                self.viewport.cancel(&mut self.document);
                self.viewport
                    .modeling
                    .repeat_last(
                        &mut self.document,
                        self.ui.modeling.mode,
                        self.ui.modeling.protect_mask,
                    )
                    .map(|()| {
                        if let Some(last) = self.viewport.modeling.last_edit {
                            self.ui.modeling.load_options(last);
                        }
                    })
            }
            Command::ModelApply => {
                self.viewport.cancel(&mut self.document);
                let options = self.ui.modeling.options();
                self.viewport
                    .modeling
                    .apply_amount(&mut self.document, options)
            }
            Command::ModelDissolveLoops => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.sync(&self.document);
                let edges = self.viewport.selected_edges(&self.document).to_vec();
                let revision = self.document.display_revision();
                self.document
                    .require_modeling()
                    .and_then(|()| {
                        let symmetry = crate::sculpt::ModelingSymmetry::new(
                            self.document.mesh(),
                            self.document.symmetry(),
                        )?;
                        let mesh = self
                            .document
                            .mesh()
                            .dissolve_loops(&symmetry.edges(&edges))?;
                        symmetry.validate_result(&mesh)?;
                        self.document
                            .mesh()
                            .check_model_protection(&mesh, self.ui.modeling.protect_mask)?;
                        Ok(mesh)
                    })
                    .and_then(|mesh| {
                        self.document
                            .commit_modeling(mesh, "Dissolve selected loops", revision)
                    })
                    .map(|()| {
                        self.viewport.modeling.sync(&self.document);
                        self.viewport.set_edge_selection(Vec::new(), &self.document);
                    })
            }
            Command::ModelSelectLoop => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.sync(&self.document);
                let edges = self.viewport.selected_edges(&self.document).to_vec();
                crate::sculpt::ModelingSymmetry::new(self.document.mesh(), self.document.symmetry())
                    .and_then(|symmetry| {
                        self.document
                            .mesh()
                            .expand_edge_loop(&edges)
                            .map(|edges| symmetry.edges(&edges))
                    })
                    .map(|edges| {
                        self.viewport.set_model_edges(edges, &self.document);
                    })
            }
            Command::GroupModelSelection(existing, remove) => {
                self.viewport.modeling.sync(&self.document);
                let vertices = self
                    .viewport
                    .modeling
                    .selected_vertices(self.document.mesh());
                let faces = (self.ui.modeling.mode == crate::viewport::ComponentMode::Face)
                    .then(|| self.viewport.modeling.selected_faces());
                let edges = self.viewport.selected_edges(&self.document).to_vec();
                self.document
                    .group_model_selection(
                        &vertices,
                        faces.as_deref(),
                        &self.ui.modeling.group_name,
                        existing,
                        remove,
                    )
                    .map(|target| {
                        self.ui.modeling.group = target.unwrap_or(0);
                        self.ui.selected_vertex_group =
                            target.map(|g| (self.document.active_model_index(), g));
                        self.ui.vertex_group_rename = target
                            .map(|g| self.document.mesh().vertex_groups[g].name.clone())
                            .unwrap_or_default();
                        if target.is_some_and(|i| !self.document.mesh().vertex_groups[i].visible) {
                            self.viewport.modeling.sync(&self.document);
                        } else {
                            self.viewport.modeling.revision = self.document.display_revision();
                            self.viewport.set_edge_selection(edges, &self.document);
                        }
                    })
            }
            Command::EditEdges(edit) => self.start_mesh_job(Operation::Edges(edit)),
            Command::Mirror(axis, source_positive) => {
                self.start_mesh_job(Operation::Mirror(crate::sculpt::MirrorOptions {
                    axis,
                    source_positive,
                    offset: self.document.symmetry().center.dot(axis.vector()),
                }))
            }
            Command::Action(_) => unreachable!(),
            Command::AddReference(file) => self.add_reference(file),
            Command::ArrangeReferences(active) => {
                self.arrange_references(active);
                Ok(())
            }
            Command::PlaceReference(index, placement) => {
                self.viewport.cancel_reference_drag();
                self.document.place_reference(index, placement)
            }
            Command::RemoveReference(index) => {
                self.viewport.cancel_reference_drag();
                let result = self.document.remove_reference(index);
                if self.document.references().is_empty() {
                    self.arrange_references(false);
                }
                result
            }
            Command::SelectTool(active) => {
                self.viewport.finish_transform_drag(&mut self.document);
                self.ui.selection_tool = active;
                self.ui.slice_active = false;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                Ok(())
            }
            Command::ShowAllFaces => {
                self.viewport.cancel(&mut self.document);
                self.document.show_all_faces()
            }
            Command::InvertVisibility => {
                self.viewport.cancel(&mut self.document);
                self.document.invert_visibility()
            }
            Command::SplitHidden => {
                self.viewport.cancel(&mut self.document);
                self.document.split_hidden().map(|()| {
                    self.ui.status =
                        "Hidden faces moved into a new submodel; subdivision levels reset".into()
                })
            }
            Command::SliceTool(active) => {
                self.ui.selection_tool = None;
                self.viewport.finish_transform_drag(&mut self.document);
                self.ui.slice_active = active;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                self.ui.status = if active {
                    "Slice: draw a line across the active model; release to apply; Esc exits Slice"
                } else {
                    "Sculpt mode"
                }
                .into();
                Ok(())
            }
            Command::Request(action) => {
                self.request(action, ctx);
                Ok(())
            }
            Command::BrowseProject(save) => {
                if self.ui.pending.is_none() {
                    self.browse_project(save, false);
                }
                Ok(())
            }
            Command::BrowseAsset(asset) => {
                self.browse_asset(asset);
                Ok(())
            }
            Command::NavigateProjectFiles(path) => {
                self.navigate_project_files(&path);
                Ok(())
            }
            Command::AcceptProjectFile(overwrite) => {
                self.accept_project_file(overwrite, ctx);
                Ok(())
            }
            Command::CancelProjectFile => {
                self.ui.file_picker = None;
                self.viewport.wait_for_release();
                Ok(())
            }
            Command::Save => {
                if !self.document.has_saved_path()
                    || !persistence::same_path(&self.ui.path, self.document.current_path())
                {
                    self.browse_project(true, false);
                } else {
                    let path = self.ui.path.clone();
                    self.save_to(&path);
                }
                Ok(())
            }
            Command::Export => {
                let path = std::path::Path::new(&self.ui.path).with_extension("obj");
                persistence::export_obj(&path.to_string_lossy(), self.document.mesh())
                    .map(|()| self.ui.status = format!("Exported {}", path.display()))
            }
            Command::Undo => {
                let dragging = self.viewport.gesture.is_some();
                self.viewport.cancel(&mut self.document);
                if dragging {
                    self.viewport.wait_for_release();
                }
                let label = self.document.undo_label();
                self.document.undo();
                self.refresh_path_after_history();
                self.ui.status =
                    label.map_or("Nothing to undo".into(), |label| format!("Undid: {label}"));
                Ok(())
            }
            Command::Redo => {
                let dragging = self.viewport.gesture.is_some();
                self.viewport.cancel(&mut self.document);
                if dragging {
                    self.viewport.wait_for_release();
                }
                let label = self.document.redo_label();
                self.document.redo();
                self.refresh_path_after_history();
                self.ui.status =
                    label.map_or("Nothing to redo".into(), |label| format!("Redid: {label}"));
                Ok(())
            }
            Command::RequestModelingPrimitive => {
                self.viewport.cancel(&mut self.document);
                let mut primitive =
                    crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
                primitive.divisions = [3; 3];
                self.ui.modeling_primitive = Some((primitive, self.document.display_revision()));
                self.ui.primitive_warning = Some((
                    self.document.active_model_index(),
                    self.document
                        .model(self.document.active_model_index())
                        .unwrap()
                        .name
                        .into(),
                ));
                Ok(())
            }
            Command::RequestPrimitive => {
                self.ui.modeling_primitive = None;
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                self.viewport.cancel(&mut self.document);
                if self.document.primitive().is_some() {
                    self.ui.primitive_controls_requested = true;
                } else {
                    self.ui.primitive_warning = Some((
                        self.document.active_model_index(),
                        self.document
                            .model(self.document.active_model_index())
                            .unwrap()
                            .name
                            .into(),
                    ));
                }
                Ok(())
            }
            Command::CancelPrimitive => {
                self.ui.primitive_warning = None;
                self.ui.modeling_primitive = None;
                Ok(())
            }
            Command::ConfirmPrimitive => {
                let modeling = self.ui.modeling_primitive.take();
                if let Some((index, name)) = self.ui.primitive_warning.take() {
                    if modeling
                        .as_ref()
                        .is_some_and(|(_, revision)| *revision != self.document.display_revision())
                        || self.document.active_model_index() != index
                        || self.document.model(index).is_none_or(|m| m.name != name)
                    {
                        Err("Submodel changed; request primitive replacement again".into())
                    } else if let Some((primitive, _)) = modeling {
                        self.document
                            .initialize_modeling_primitive(primitive)
                            .map(|()| {
                                self.ui.primitive_controls_requested = false;
                                self.dispatch(Command::ModelingMode(true), ctx);
                                self.dispatch(
                                    Command::ModelingComponent(
                                        crate::viewport::ComponentMode::Face,
                                    ),
                                    ctx,
                                );
                                self.ui.status =
                                    "Modeling cube ready · Undo restores the previous model".into();
                            })
                    } else {
                        self.document.initialize_primitive().map(|()| {self.ui.primitive_controls_requested=true;self.ui.transform_mode=crate::viewport::TransformMode::Sculpt;self.ui.status="Parametric primitive: drag shape handles or edit parameters; convert when ready to sculpt".into();})
                    }
                } else {
                    Ok(())
                }
            }
            Command::UpdatePrimitive(primitive) => self.document.update_primitive(primitive),
            Command::EndPrimitiveEdit => {
                self.document.end_primitive_edit();
                Ok(())
            }
            Command::ConvertPrimitive => {
                self.viewport.cancel(&mut self.document);
                self.document.convert_primitive().map(|()| {
                    self.ui.primitive_controls_requested = false;
                    self.ui.status =
                        "Primitive converted: geometry is now editable with sculpt brushes".into();
                })
            }
            Command::TransformMode(mode) => {
                self.activate_transform(mode);
                Ok(())
            }
            Command::TransformOperation(operation) => {
                use crate::viewport::{TransformMode as Mode, TransformOperation as Op};
                let mode = match operation {
                    Op::PlaceLine => Mode::ActionLine,
                    Op::PlacePivot => Mode::World,
                    _ if self.ui.transform_mode == Mode::Sculpt => Mode::World,
                    _ => self.ui.transform_mode,
                };
                self.activate_transform(mode);
                self.ui.transform_operation = operation;
                self.ui.status = operation.hint().into();
                Ok(())
            }
            Command::ResetTransformPivot => {
                self.viewport.finish_transform_drag(&mut self.document);
                self.viewport
                    .reset_transform_pivot(&self.document, self.ui.transform_mode);
                self.ui.status = "Pivot reset to active model center".into();
                Ok(())
            }
            Command::RespectTransformMask(respect) => {
                self.viewport.finish_transform_drag(&mut self.document);
                self.ui.respect_transform_mask = respect;
                Ok(())
            }
            Command::EditSubmodel(index, name, metadata) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_model_metadata(index, name, metadata)
            }
            Command::ReorderSubmodel(from, to) => {
                self.viewport.cancel(&mut self.document);
                self.document.reorder_model(from, to)
            }
            Command::AddGroup(name) => {
                self.viewport.cancel(&mut self.document);
                self.document.add_group(name)
            }
            Command::RemoveGroup(name) => {
                self.viewport.cancel(&mut self.document);
                self.document.remove_group(&name)
            }
            Command::ReorderGroup(from, to) => {
                self.viewport.cancel(&mut self.document);
                self.document.reorder_group(from, to)
            }
            Command::GroupVisible(name, visible) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_group_visible(&name, visible)
            }
            Command::SelectSubmodel(index) => {
                self.viewport.cancel(&mut self.document);
                self.document.select_submodel(index)
            }
            Command::DeleteSubmodel => {
                self.viewport.cancel(&mut self.document);
                let name = self
                    .document
                    .model(self.document.active_model_index())
                    .unwrap()
                    .name
                    .to_owned();
                if self.document.delete_submodel() {
                    self.ui.selected_vertex_group = None;
                    self.ui.projection.source = None;
                    self.viewport.set_edge_selection(Vec::new(), &self.document);
                    self.viewport.modeling.sync(&self.document);
                    self.viewport
                        .reset_transform_pivot(&self.document, self.ui.transform_mode);
                    self.viewport.wait_for_release();
                    self.ui.status = format!("Deleted {name}. Undo restores the submodel.");
                } else {
                    self.ui.status = "The last model slot is already empty".into();
                }
                Ok(())
            }
            Command::MergeDown => {
                self.viewport.cancel(&mut self.document);
                self.document.merge_down().map(|()| {
                    self.ui.selected_vertex_group = None;
                    self.ui.status =
                        "Submodels merged; current surfaces kept, subdivision levels reset".into();
                })
            }
            Command::DuplicateModel | Command::ExtractMasked => {
                self.viewport.cancel(&mut self.document);
                let extract = matches!(command, Command::ExtractMasked);
                self.document.duplicate_model(extract).map(|()| {
                    self.ui.status = if extract {
                        "Masked surface extracted; source kept".into()
                    } else {
                        "Model duplicated".into()
                    }
                })
            }
            Command::GrowMask(grow) => {
                self.viewport.cancel(&mut self.document);
                self.document.grow_mask(grow)
            }
            Command::MaskIntensity(amount) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_mask_intensity(amount)
            }
            Command::BlurMask | Command::SharpenMask => {
                self.viewport.cancel(&mut self.document);
                self.viewport.wait_for_release();
                if matches!(command, Command::BlurMask) {
                    self.document.blur_mask()
                } else {
                    self.document.sharpen_mask()
                }
            }
            Command::MaskThreshold(amount) => self.document.set_mask_threshold(amount),
            Command::CreateVertexGroup(name, all) => {
                self.viewport.cancel(&mut self.document);
                self.document.create_vertex_group(name, all).map(|index| {
                    self.ui.selected_vertex_group =
                        Some((self.document.active_model_index(), index));
                    self.ui.vertex_group_rename =
                        self.document.mesh().vertex_groups[index].name.clone();
                    self.ui.vertex_group_name.clear();
                    self.ui.modeling.group = index;
                    self.ui.status = "Vertex group created".into();
                })
            }
            Command::AssignMaskToGroup(index, remove) => {
                self.viewport.cancel(&mut self.document);
                self.document
                    .assign_mask_to_group(index, remove)
                    .map(|target| {
                        self.ui.selected_vertex_group =
                            target.map(|g| (self.document.active_model_index(), g));
                        self.ui.vertex_group_rename = target
                            .map(|g| self.document.mesh().vertex_groups[g].name.clone())
                            .unwrap_or_default();
                        self.ui.modeling.group = target.unwrap_or(0);
                    })
            }
            Command::MaskVertexGroup(index) => {
                self.viewport.cancel(&mut self.document);
                self.document.mask_from_group(index)
            }
            Command::RenameVertexGroup(index, name) => {
                self.document.rename_vertex_group(index, name)
            }
            Command::VertexGroupVisible(index, visible) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_vertex_group_visible(index, visible)
            }
            Command::DeleteVertexGroup(index) => {
                self.ui.selected_vertex_group = None;
                self.document.remove_vertex_group(index)
            }
            Command::SetTemporaryThickness(value) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_temporary_thickness(value)
            }
            Command::ApplyThickness => {
                self.viewport.cancel(&mut self.document);
                self.document
                    .apply_thickness()
                    .map(|()| self.ui.status = "Thickness applied; closed shell created".into())
            }
            Command::InvertMask => {
                self.viewport.cancel(&mut self.document);
                self.document
                    .invert_mask()
                    .map(|()| self.ui.status = "Mask inverted".into())
            }
            Command::DeleteMasked => {
                self.viewport.cancel(&mut self.document);
                self.document.delete_masked().map(|changed| {
                    self.ui.status = if changed {
                        "Masked surface deleted; subdivision levels reset".into()
                    } else {
                        "No complete faces meet the mask threshold".into()
                    };
                })
            }
            Command::ClearMask => {
                self.viewport.cancel(&mut self.document);
                self.document.clear_mask();
                Ok(())
            }
            Command::OpenProjection => self.open_projection(),
            Command::KeepProjectionReference => {
                self.viewport.cancel(&mut self.document);
                self.document.preserve_projection_reference().map(|index| {
                    self.ui.projection.source = Some(crate::document::ProjectionSource::Submodel(index));
                    self.ui.status = "Reference copy saved below the active model. Remesh or sculpt the target, then choose Project details.".into();
                })
            }
            Command::PreviewProjection => self.start_projection(),
            Command::ClearProjectionPreview => {
                self.clear_projection_preview();
                Ok(())
            }
            Command::ApplyProjection => self.apply_projection(),
            Command::CancelProjection => {
                self.cancel_projection();
                Ok(())
            }
            Command::Subdivide => self.start_mesh_job(Operation::Subdivide),
            Command::Remesh => self.start_mesh_job(Operation::Remesh),
            Command::CancelMeshJob => {
                self.cancel_mesh_job();
                Ok(())
            }
            Command::SetMeshLevel(level) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_mesh_level(level)
            }
            Command::DeleteLowerLevels | Command::DeleteHigherLevels => {
                let lower = matches!(command, Command::DeleteLowerLevels);
                self.viewport.cancel(&mut self.document);
                self.document.delete_mesh_levels(lower).map(|()| {
                    self.ui.status = if lower {
                        "Lower subdivision levels deleted"
                    } else {
                        "Higher subdivision levels deleted"
                    }
                    .into();
                })
            }
            Command::SetAdaptive(settings) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_adaptive(settings)
            }
            Command::SetDynamicMesh(enabled) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_dynamic_mesh(enabled)
            }
            Command::SetRemeshSettings(resolution, auto) => {
                self.document.set_remesh_settings(resolution, auto)
            }
            Command::ZoomIn | Command::ZoomOut => {
                self.camera.zoom_step(matches!(command, Command::ZoomIn));
                Ok(())
            }
            Command::SetZoom(zoom) => {
                self.camera.set_zoom(zoom);
                Ok(())
            }
            Command::ResetView => {
                self.camera.reset();
                Ok(())
            }
            Command::SetPerspective(enabled) => {
                self.set_camera_projection(enabled, self.camera.field_of_view);
                Ok(())
            }
            Command::SetFieldOfView(degrees) => {
                if degrees.is_finite() {
                    self.set_camera_projection(self.camera.perspective, degrees);
                }
                Ok(())
            }
            Command::SetSymmetry(symmetry) => {
                let modeling_drag = self.viewport.modeling.dragging();
                self.viewport.cancel(&mut self.document);
                if modeling_drag {
                    self.viewport.wait_for_release();
                }
                self.document.set_symmetry(symmetry)
            }
            Command::AddBrush(kind) => self
                .document
                .append_brush(crate::sculpt::Brush::new(kind))
                .map(|index| self.ui.select_brush(index, &self.document)),
            Command::AddAlphaDragBrush => self
                .document
                .append_brush(crate::sculpt::Brush::alpha_drag())
                .map(|index| self.ui.select_brush(index, &self.document)),
            Command::DuplicateBrush(index) => self
                .document
                .duplicate_brush(index)
                .map(|index| self.ui.select_brush(index, &self.document)),
            Command::UpdateBrush(index, brush) => self.document.update_brush(index, brush),
            Command::SaveAlpha(index, asset) => self.document.save_alpha(index, asset).map(|i| {
                self.ui.alphas.selected = Some(crate::ui::alphas::Selection::Project(i));
                self.ui.alphas.draft = None;
                self.ui.status =
                    "Alpha saved in project library; choose Use on brush or Place stencil".into();
            }),
            Command::RemoveAlpha(index) => self
                .document
                .remove_alpha(index)
                .map(|()| self.ui.alphas.selected = None),
            Command::UseAlpha(index, brush) => self
                .document
                .alphas()
                .get(index)
                .cloned()
                .ok_or("Alpha no longer exists".to_string())
                .and_then(|asset| self.document.set_alpha(brush, asset.image)),
            Command::ExportAlpha(index, replace) => {
                let path = self.ui.alphas.export_path.clone();
                if std::path::Path::new(&path).exists() && !replace {
                    self.ui.alphas.overwrite = Some((index, path));
                    Ok(())
                } else {
                    self.document
                        .alphas()
                        .get(index)
                        .ok_or("Alpha no longer exists".to_string())
                        .and_then(|asset| persistence::save_alpha(&path, &asset.image))
                        .map(|()| {
                            self.ui.alphas.overwrite = None;
                            self.ui.status = format!("Exported 16-bit alpha: {path}");
                        })
                }
            }
            Command::SetStencil(stencil) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_stencil(stencil)
            }
            Command::AttachStencil => {
                self.viewport.cancel(&mut self.document);
                self.viewport.attach_stencil = true;
                Ok(())
            }
            Command::SavePreset(index) => {
                persistence::save_brush(&self.ui.preset_path, &self.document.brushes()[index])
                    .map(|()| self.ui.status = "Brush saved".into())
            }
            Command::CaptureBrushIcon(index) => {
                self.viewport.cancel(&mut self.document);
                crate::brush_icon::capture(self.document.mesh(), &self.camera).and_then(|icon| {
                    let brush = self
                        .document
                        .brushes()
                        .get(index)
                        .ok_or("Brush no longer exists")?;
                    self.ui.set_icon_shot(ctx, index, brush, icon);
                    Ok(())
                })
            }
            Command::ExportIconImage(path, icon) => image::save_buffer_with_format(
                &path,
                &icon.rgba,
                icon.size as u32,
                icon.size as u32,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .map_err(|e| e.to_string())
            .map(|()| self.ui.status = format!("Saved square capture: {path}")),
            Command::ExportBrushLibrary => {
                persistence::save_brush_library(&self.ui.library_path, self.document.brushes())
                    .map(|()| self.ui.status = "Brush library exported".into())
            }
            Command::SaveAndContinue => {
                if let Some(action) = self.ui.pending.clone() {
                    let path = self.ui.pending_save_path.clone();
                    if matches!(&action,PendingAction::Open(target) if persistence::same_path(&path,target))
                    {
                        self.ui.status="Choose a different save path to avoid overwriting the project you are opening".into();
                    } else if !self.document.has_saved_path()
                        || !persistence::same_path(&path, self.document.current_path())
                    {
                        self.browse_project(true, true);
                    } else if self.save_to(&path) {
                        self.ui.pending = None;
                        self.execute_pending(action, ctx);
                    }
                }
                Ok(())
            }
            Command::DiscardAndContinue => {
                if let Some(action) = self.ui.pending.take() {
                    self.execute_pending(action, ctx);
                }
                Ok(())
            }
            Command::CancelPending => {
                self.ui.pending = None;
                Ok(())
            }
        };
        if let Err(error) = result {
            self.diagnostic_warning("An application command failed; detailed error text withheld to exclude personal paths");
            self.ui.status = error;
        }
    }
}

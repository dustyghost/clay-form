//! Execute UI commands and enforce edit boundaries.
use super::*;
mod brushes;
mod mesh;
mod modeling;
mod scene;

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
        if let Err(reason) =
            super::policy::EditorPolicy::new(&self.ui, &self.document).command(&command)
        {
            self.ui.status = reason.into();
            return;
        }
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
        if matches!(command, Command::Undo | Command::Redo)
            && let Err(error) = self.prepare_path_history()
        {
            self.ui.status = error;
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
        ) && (self.viewport.session.gesture == Some(crate::viewport::Gesture::Transform)
            || self.viewport.session.guided.dragging())
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
        let uv_attributes = matches!(
            command,
            Command::UvPack(_)
                | Command::UvPackSelected(..)
                | Command::UvTransform(..)
                | Command::UvReplace(_)
                | Command::UvRemove(_)
                | Command::UvClearMask
                | Command::UvSeams(_)
        );
        let result: Result<(), String> = match command {
            Command::Texture(command) => self.texture_command(command),
            Command::UvUnwrap(automatic, padding) => {
                self.start_mesh_job(Operation::UvUnwrap(automatic, padding))
            }
            Command::UvPack(padding) => self
                .document
                .edit_uv("Auto arrange UV islands", |uv| uv.pack(padding)),
            Command::UvPackSelected(faces, padding) => {
                self.document.edit_uv("Arrange selected UV islands", |uv| {
                    uv.pack_selected(&faces, padding)
                })
            }
            Command::UvTransform(faces, offset, scale, angle) => {
                self.document.edit_uv("Transform UV islands", |uv| {
                    uv.transform(&faces, offset, scale, angle)
                })
            }
            Command::UvReplace(next) => self.document.edit_uv("Edit UV islands", |uv| {
                if uv.topology != next.topology {
                    return Err("UV edit is out of date".into());
                }
                *uv = *next;
                Ok(())
            }),
            Command::UvRemove(faces) => self
                .document
                .edit_uv("Remove UV islands", |uv| uv.remove_islands(&faces)),
            Command::UvClearMask => self.document.edit_uv("Clear UV mask", |uv| {
                uv.masks.clear();
                Ok(())
            }),
            Command::UvSeams(mark) => {
                let edges = self
                    .viewport
                    .modeling
                    .selected
                    .iter()
                    .filter_map(|c| {
                        if let crate::viewport::Component::Edge(e) = c {
                            Some(*e)
                        } else {
                            None
                        }
                    })
                    .collect();
                self.document.mark_uv_seams(edges, mark).map(|()| {
                    self.ui.wire = true;
                    self.viewport.modeling.revision = self.document.display_revision();
                })
            }
            Command::UvSelect(faces) => {
                let selected: std::collections::BTreeSet<_> = faces.into_iter().collect();
                self.viewport.modeling.selected =
                    crate::sculpt::uv::polygon_triangles(self.document.mesh())
                        .iter()
                        .enumerate()
                        .filter(|(_, fs)| fs.iter().any(|f| selected.contains(f)))
                        .map(|(i, _)| crate::viewport::Component::Face(i))
                        .collect();
                self.viewport.modeling.revision = self.document.display_revision();
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                self.ui.slice_active = false;
                self.ui.selection_tool = None;
                self.ui.edges.selecting = false;
                self.ui.modeling.active = true;
                self.ui.modeling.mode = crate::viewport::ComponentMode::Face;
                self.ui.modeling.tool = crate::viewport::ModelTool::Select;
                Ok(())
            }
            Command::Path(_) => unreachable!(),
            command @ (Command::ModelingMode(..)
            | Command::ModelingComponent(..)
            | Command::ClearModelSelection
            | Command::ModelBoundary
            | Command::ModelTopology(..)
            | Command::ModelRepeat
            | Command::ModelApply
            | Command::ModelDissolveLoops
            | Command::ModelSelectLoop
            | Command::GroupModelSelection(..)) => self.dispatch_modeling(command),
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
            Command::SaveMaterial(asset) => self.document.save_material(asset),
            Command::SaveMaterialToLibrary(asset) => {
                self.ui.preferences.materials.save_asset(&asset).map(|()| {
                    self.ui.status =
                        "Saved to personal library. Project Undo does not change library presets."
                            .into();
                })
            }
            Command::RemoveMaterial(id) => self.document.remove_material(id).map(|()| {
                self.ui.status =
                    "Removed material; existing paint is preserved. Undo restores the material."
                        .into();
            }),
            Command::FillAllMaterial(brush) => self.document.fill_all_material(&brush),
            Command::FillMaterial(brush) => self.document.fill_material(&brush),
            Command::BrowseExport(format) => {
                self.browse_export(format);
                Ok(())
            }
            Command::CancelExport => {
                if let Some(job) = &self.export_job {
                    job.cancel();
                }
                Ok(())
            }
            Command::Export => {
                let path = std::path::Path::new(&self.ui.path).with_extension("obj");
                persistence::export_obj(&path.to_string_lossy(), self.document.mesh())
                    .map(|()| self.ui.status = format!("Exported {}", path.display()))
            }
            Command::Undo => {
                let dragging = self.viewport.session.gesture.is_some();
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
                let dragging = self.viewport.session.gesture.is_some();
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
                    Op::PlacePivot | Op::Universal => Mode::World,
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
            command @ (Command::EditSubmodel(..)
            | Command::ReorderSubmodel(..)
            | Command::AddGroup(..)
            | Command::RemoveGroup(..)
            | Command::ReorderGroup(..)
            | Command::GroupVisible(..)
            | Command::SelectSubmodel(..)
            | Command::DeleteSubmodel
            | Command::MergeDown
            | Command::DuplicateModel
            | Command::ExtractMasked) => self.dispatch_scene(command),
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
            Command::OpenRelaxation => self.open_relaxation(),
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
            command @ (Command::Subdivide
            | Command::Remesh
            | Command::CancelMeshJob
            | Command::SetMeshLevel(..)
            | Command::DeleteLowerLevels
            | Command::DeleteHigherLevels
            | Command::SetAdaptive(..)
            | Command::SetDynamicMesh(..)
            | Command::SetRemeshResolution(..)
            | Command::SetRemeshTopology(..)
            | Command::SetQuadRemeshAlgorithm(..)
            | Command::SetVoxelSkinProjection(..)
            | Command::ZoomIn
            | Command::ZoomOut
            | Command::SetZoom(..)
            | Command::ResetView
            | Command::SetPerspective(..)
            | Command::SetFieldOfView(..)
            | Command::SetSymmetry(..)) => self.dispatch_mesh(command),
            command @ (Command::AddBrush(..)
            | Command::AddAlphaDragBrush
            | Command::DuplicateBrush(..)
            | Command::UpdateBrush(..)
            | Command::SaveAlpha(..)
            | Command::RemoveAlpha(..)
            | Command::UseAlpha(..)
            | Command::ExportAlpha(..)
            | Command::SetStencil(..)
            | Command::AttachStencil
            | Command::SavePreset(..)
            | Command::CaptureBrushIcon(..)
            | Command::ExportIconImage(..)
            | Command::ExportBrushLibrary) => self.dispatch_brushes(command, ctx),
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
        if result.is_ok() && uv_attributes {
            self.viewport
                .modeling
                .accept_attribute_revision(&self.document);
        }
        if let Err(error) = result {
            self.diagnostic_warning("An application command failed; detailed error text withheld to exclude personal paths");
            self.ui.status = error;
        }
    }
}

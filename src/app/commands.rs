//! Execute UI commands and enforce edit boundaries.
use super::*;

impl App {
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
        if let Command::Action(action) = command {
            self.execute_action(action, ctx);
            return;
        }
        if matches!(
            command,
            Command::AddReference(_)
                | Command::PlaceReference(..)
                | Command::RemoveReference(_)
                | Command::ArrangeReferences(_)
        ) && (self.ui.pending.is_some()
            || self.ui.recovery_open
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
                    | Command::ImportObj
            )
        {
            self.ui.status = "Wait for the mesh operation, or cancel it first".into();
            return;
        }
        // A new view must not change the projection underneath a latched
        // transform ring/axis. Keep the edit so far and require a fresh press.
        if matches!(
            command,
            Command::ZoomIn | Command::ZoomOut | Command::SetZoom(_) | Command::ResetView
        ) && self.viewport.gesture == Some(crate::viewport::Gesture::Transform)
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
            self.viewport.cancel_reference_drag();
        }
        self.document.prepare_edit(command.edit_kind());
        let result: Result<(), String> = match command {
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
            Command::Save => {
                let path = self.ui.path.clone();
                self.save_to(&path);
                Ok(())
            }
            Command::ImportObj => {
                self.start_mesh_job(Operation::ImportObj(self.ui.obj_path.clone()))
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
                self.ui.status =
                    label.map_or("Nothing to redo".into(), |label| format!("Redid: {label}"));
                Ok(())
            }
            Command::RequestPrimitive => {
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
                Ok(())
            }
            Command::ConfirmPrimitive => {
                if let Some((index, name)) = self.ui.primitive_warning.take() {
                    if self.document.active_model_index() != index
                        || self.document.model(index).is_none_or(|m| m.name != name)
                    {
                        Err("Submodel changed; request primitive replacement again".into())
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
                    self.ui.status = "Vertex group created".into();
                })
            }
            Command::AssignMaskToGroup(index, remove) => {
                self.viewport.cancel(&mut self.document);
                self.document.assign_mask_to_group(index, remove)
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
            Command::SetSymmetry(symmetry) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_symmetry(symmetry)
            }
            Command::AddBrush(kind) => self
                .document
                .append_brush(crate::sculpt::Brush::new(kind))
                .map(|index| self.ui.select_brush(index, &self.document)),
            Command::DuplicateBrush(index) => self
                .document
                .duplicate_brush(index)
                .map(|index| self.ui.select_brush(index, &self.document)),
            Command::UpdateBrush(index, brush) => self.document.update_brush(index, brush),
            Command::LoadAlpha(index) => persistence::load_alpha(&self.ui.alpha_path)
                .and_then(|alpha| self.document.set_alpha(index, alpha))
                .map(|()| self.ui.status = "Alpha loaded (white applies; black protects)".into()),
            Command::SavePreset(index) => {
                persistence::save_brush(&self.ui.preset_path, &self.document.brushes()[index])
                    .map(|()| self.ui.status = "Brush saved".into())
            }
            Command::LoadPreset => persistence::load_brush(&self.ui.preset_path)
                .and_then(|brush| self.document.import_brushes(vec![brush]))
                .map(|(added, updated, index)| {
                    if let Some(index) = index {
                        self.ui.select_brush(index, &self.document);
                    }
                    self.ui.status = format!("Imported brush: {added} added, {updated} updated");
                }),
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
            Command::ImportBrushLibrary => persistence::load_brush_library(&self.ui.library_path)
                .and_then(|brushes| self.document.import_brushes(brushes))
                .map(|(added, updated, _)| {
                    self.ui.status = format!("Imported library: {added} added, {updated} updated")
                }),
            Command::SaveAndContinue => {
                if let Some(action) = self.ui.pending.clone() {
                    let path = self.ui.pending_save_path.clone();
                    if matches!(&action,PendingAction::Open(target) if persistence::same_path(&path,target))
                    {
                        self.ui.status="Choose a different save path to avoid overwriting the project you are opening".into();
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
            self.ui.status = error;
        }
    }
}

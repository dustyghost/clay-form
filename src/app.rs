//! Application controller: coordinates UI commands, document changes, and IO adapters.
use crate::{
    camera::Camera,
    command::{Command, PendingAction},
    document::Document,
    mesh_job::{MeshJob, Operation},
    persistence,
    preferences::{self, Action, Preferences},
    tablet::Tablet,
    ui::UiState,
    viewport::{Viewport, ViewportOptions},
};
use eframe::egui;

pub struct App {
    pub(crate) document: Document,
    pub(crate) camera: Camera,
    pub(crate) viewport: Viewport,
    ui: UiState,
    tablet: Tablet,
    pressure: f32,
    closing: bool,
    mesh_job: Option<MeshJob>,
    preferences_path: Option<std::path::PathBuf>,
    saved_preferences: Preferences,
    preferences_changed: Option<std::time::Instant>,
}
impl Default for App {
    fn default() -> Self {
        Self {
            document: Document::default(),
            camera: Camera::default(),
            viewport: Viewport::default(),
            ui: UiState::default(),
            tablet: Tablet::new(),
            pressure: 1.,
            closing: false,
            mesh_job: None,
            preferences_path: None,
            saved_preferences: Preferences::default(),
            preferences_changed: None,
        }
    }
}
impl App {
    pub fn with_gl(gl: &eframe::glow::Context) -> Result<Self, String> {
        let mut app = Self::new();
        app.viewport.initialize_renderer(gl)?;
        Ok(app)
    }
    pub fn new() -> Self {
        let mut app = Self::default();
        let path = preferences::path();
        if path.exists() {
            match preferences::load(&path) {
                Ok(prefs) => {
                    app.ui.preferences = prefs.clone();
                    app.saved_preferences = prefs;
                }
                Err(error) => app.ui.status = format!("Could not load preferences: {error}"),
            }
        }
        app.preferences_path = Some(path);
        app
    }
    fn save_preferences(&mut self) {
        if self.ui.preferences == self.saved_preferences {
            return;
        }
        if let Some(path) = &self.preferences_path {
            match preferences::save(path, &self.ui.preferences) {
                Ok(()) => {
                    self.saved_preferences = self.ui.preferences.clone();
                    self.preferences_changed = None;
                }
                Err(error) => {
                    self.ui.status = format!("Could not save preferences: {error}");
                    self.preferences_changed = Some(std::time::Instant::now());
                }
            }
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input()
            || self.ui.primitive_warning.is_some()
            || self.ui.pending.is_some()
            || self.ui.settings_open
            || self.ui.layout_open
            || self.ui.icon_capture_open()
        {
            return;
        }
        for action in self.ui.preferences.actions(ctx) {
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
                Action::ZoomIn => Some(Command::ZoomIn),
                Action::ZoomOut => Some(Command::ZoomOut),
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
                    let mut brush = self.document.brushes()[self.ui.selected].clone();
                    brush.radius = (brush.radius
                        * if action == Action::Smaller { 0.9 } else { 1.1 })
                    .clamp(1., 500.);
                    Some(Command::UpdateBrush(self.ui.selected, brush))
                }
                Action::PreviousBrush => {
                    self.ui.selected = (self.ui.selected + self.document.brushes().len() - 1)
                        % self.document.brushes().len();
                    None
                }
                Action::NextBrush => {
                    self.ui.selected = (self.ui.selected + 1) % self.document.brushes().len();
                    None
                }
            };
            if let Some(command) = command {
                self.dispatch(command, ctx);
            }
            if self.ui.pending.is_some() {
                break;
            }
        }
    }

    fn start_mesh_job(&mut self, operation: Operation) -> Result<(), String> {
        self.document.require_sculpt_mesh()?;
        if self.mesh_job.is_some() {
            return Err("A mesh operation is already running".into());
        }
        let was_dragging = self.viewport.gesture.is_some();
        self.viewport.cancel(&mut self.document);
        self.mesh_job = Some(MeshJob::start(
            self.document.geometry_snapshot(),
            operation,
        )?);
        if was_dragging {
            self.viewport.wait_for_release();
        }
        self.ui.mesh_busy = true;
        self.ui.status = format!("{}…", operation.label());
        Ok(())
    }
    fn cancel_mesh_job(&mut self) {
        if self.mesh_job.take().is_some() {
            self.ui.status = "Mesh operation cancelled; original mesh kept".into();
        }
        self.ui.mesh_busy = false;
    }
    fn poll_mesh_job(&mut self) {
        if let Some(result) = self.mesh_job.as_ref().and_then(MeshJob::poll) {
            let label = self
                .mesh_job
                .take()
                .expect("completed job")
                .operation
                .history_label();
            self.ui.mesh_busy = false;
            match result.and_then(|geometry| self.document.replace_geometry_named(geometry, label))
            {
                Ok(()) => {
                    self.ui.status = format!(
                        "Mesh updated: {} vertices, {} faces",
                        self.document.mesh().vertices.len(),
                        self.document.mesh().triangles.len()
                    )
                }
                Err(error) => {
                    self.ui.status = error;
                    if self.document.mesh_settings().auto_remesh {
                        let resolution = self.document.mesh_settings().resolution;
                        let _ = self.document.set_remesh_settings(resolution, false);
                    }
                }
            }
        }
    }
    fn save_to(&mut self, path: &str) -> bool {
        match persistence::save_project(path, &self.document.project()) {
            Ok(()) => {
                self.document.mark_saved(path);
                self.ui.status = format!("Saved {path}");
                true
            }
            Err(error) => {
                self.ui.status = error;
                false
            }
        }
    }
    fn request(&mut self, action: PendingAction, ctx: &egui::Context) {
        self.cancel_mesh_job();
        self.viewport.cancel(&mut self.document);
        if self.document.is_dirty() {
            self.ui.pending_save_path = self.document.current_path().into();
            self.ui.pending = Some(action);
        } else {
            self.execute_pending(action, ctx);
        }
    }
    fn execute_pending(&mut self, action: PendingAction, ctx: &egui::Context) {
        match action {
            PendingAction::New => {
                self.document.new_sphere();
                self.ui.status = "New sphere".into();
            }
            PendingAction::Open(path) => {
                let result = persistence::load_project(&path)
                    .and_then(|project| self.document.replace_project(project, &path));
                match result {
                    Ok(()) => {
                        self.ui.selected = 0;
                        self.ui.status = format!("Opened {path}");
                    }
                    Err(error) => self.ui.status = error,
                }
            }
            PendingAction::Quit => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    pub(crate) fn dispatch(&mut self, command: Command, ctx: &egui::Context) {
        if self.ui.primitive_warning.is_some()
            && !matches!(
                command,
                Command::ConfirmPrimitive | Command::CancelPrimitive
            )
        {
            self.ui.status = "Confirm or cancel primitive replacement first".into();
            return;
        }
        if !matches!(
            command,
            Command::UpdatePrimitive(_) | Command::EndPrimitiveEdit
        ) {
            self.document.end_primitive_edit();
        }
        if self.mesh_job.is_some()
            && matches!(
                command,
                Command::RequestPrimitive
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
                    | Command::SelectSubmodel(_)
                    | Command::DuplicateModel
                    | Command::ExtractMasked
                    | Command::ClearMask
                    | Command::InvertMask
                    | Command::DeleteMasked
                    | Command::SetTemporaryThickness(_)
                    | Command::ApplyThickness
                    | Command::SetMeshLevel(_)
                    | Command::DeleteLowerLevels
                    | Command::DeleteHigherLevels
                    | Command::SetDynamicMesh(_)
                    | Command::SetAdaptive(_)
                    | Command::SetRemeshSettings(..)
                    | Command::Subdivide
                    | Command::Remesh
            )
        {
            self.ui.status = "Wait for the mesh operation, or cancel it first".into();
            return;
        }
        let result: Result<(), String> = match command {
            Command::Request(action) => {
                self.request(action, ctx);
                Ok(())
            }
            Command::Save => {
                let path = self.ui.path.clone();
                self.save_to(&path);
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
                self.viewport.cancel(&mut self.document);
                if self.document.primitive().is_some() {
                    self.ui.primitive_editor_open = true;
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
                        self.document.initialize_primitive().map(|()| {self.ui.primitive_editor_open=true;self.ui.transform_mode=crate::viewport::TransformMode::Sculpt;self.ui.status="Parametric primitive: drag shape handles or edit parameters; convert when ready to sculpt".into();})
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
                    self.ui.primitive_editor_open = false;
                    self.ui.status =
                        "Primitive converted: geometry is now editable with sculpt brushes".into();
                })
            }
            Command::TransformMode(mode) => {
                self.viewport.finish_transform_drag(&mut self.document);
                self.ui.transform_mode = mode;
                self.ui.transform_operation = if mode == crate::viewport::TransformMode::ActionLine
                {
                    crate::viewport::TransformOperation::PlaceLine
                } else {
                    crate::viewport::TransformOperation::Move
                };
                self.ui.status = match mode { crate::viewport::TransformMode::Sculpt => "Sculpt mode", crate::viewport::TransformMode::World => "World axes: drag an axis or the center handle", crate::viewport::TransformMode::ActionLine => "Action Line: drag to place a pivot and direction, then choose Move or Scale" }.into();
                Ok(())
            }
            Command::TransformOperation(operation) => {
                self.viewport.finish_transform_drag(&mut self.document);
                self.ui.transform_operation = operation;
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
                        "No complete faces are at least 50% masked".into()
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
                .map(|index| self.ui.selected = index),
            Command::DuplicateBrush(index) => self
                .document
                .duplicate_brush(index)
                .map(|index| self.ui.selected = index),
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
                        self.ui.selected = index;
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
    pub(crate) fn show_viewport(&mut self, ui: &mut egui::Ui) {
        let (primitive_commands, editor_rect) =
            self.ui
                .primitive_popup(ui.ctx(), ui.available_rect_before_wrap(), &self.document);
        for command in primitive_commands {
            self.dispatch(command, ui.ctx());
        }
        let enabled = !self.ui.icon_capture_open()
            && self.ui.primitive_warning.is_none()
            && self.ui.pending.is_none()
            && !self.ui.settings_open
            && !self.ui.layout_open
            && self.mesh_job.is_none();
        let (commands, toolbar_rect) =
            self.ui
                .transform_toolbar(ui.ctx(), ui.available_rect_before_wrap(), enabled);
        for command in commands {
            self.dispatch(command, ui.ctx());
        }
        let (commands, resolution_rect) = self.ui.primitive_resolution_toolbar(
            ui.ctx(),
            ui.available_rect_before_wrap(),
            &self.document,
            enabled,
        );
        for command in commands {
            self.dispatch(command, ui.ctx());
        }
        let over_resolution = resolution_rect.is_some_and(|rect| {
            ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))
        });
        // A toolbar click must never also grab a handle or place an Action Line.
        let over_editor = editor_rect.is_some_and(|rect| {
            ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))
        });
        let over_toolbar = toolbar_rect.is_some_and(|rect| {
            ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))
        });
        self.viewport.show(
            ui,
            &mut self.document,
            &mut self.camera,
            ViewportOptions {
                transform_mode: self.ui.transform_mode,
                transform_operation: self.ui.transform_operation,
                respect_mask: self.ui.respect_transform_mask,
                selected: self.ui.selected,
                pressure: self.pressure,
                wire: self.ui.wire,
                wire_mode: self.ui.wire_mode,
                wire_triangles: self.ui.wire_triangles,
                backface_opacity: if self.ui.show_backfaces {
                    self.ui.backface_opacity
                } else {
                    0.
                },
                show_guides: self.ui.show_symmetry_guides,
                guide_opacity: self.ui.guide_opacity,
                enabled: enabled && !over_toolbar && !over_editor && !over_resolution,
                smooth: self.ui.preferences.smooth,
                reverse: self.ui.preferences.reverse,
                zoom_hold: self.ui.preferences.zoom_hold,
            },
        );
        if self.viewport.completed_stroke
            && self.document.mesh_settings().dynamic
            && self.document.mesh_settings().auto_remesh
            && self.mesh_job.is_none()
            && let Err(error) = self.start_mesh_job(Operation::Remesh)
        {
            self.ui.status = error;
        }
    }
}
impl eframe::App for App {
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.save_preferences();
        if let Some(gl) = gl {
            self.viewport.destroy_renderer(gl);
        }
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll_mesh_job();
        let native_pressure = ctx.input(|i| {
            i.events.iter().rev().find_map(|event| match event {
                egui::Event::Touch { force: Some(p), .. } => Some(*p),
                _ => None,
            })
        });
        self.pressure = self.tablet.pressure().or(native_pressure).unwrap_or(1.);
        if ctx.input(|i| i.viewport().close_requested())
            && self.document.is_dirty()
            && !self.closing
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(PendingAction::Quit, ctx);
        }
        for command in self.ui.toolbar(ctx, &self.document, self.camera.zoom) {
            self.dispatch(command, ctx);
        }
        for command in
            self.ui
                .brush_panel(ctx, &self.document, &self.tablet.status(), self.pressure)
        {
            self.dispatch(command, ctx);
        }
        self.ui.settings(ctx);
        self.shortcuts(ctx);
        self.ui.status_bar(ctx);
        egui::CentralPanel::default().show(ctx, |ui| self.show_viewport(ui));
        if let Some(command) = self.ui.icon_capture_popup(ctx, &self.document) {
            self.dispatch(command, ctx);
        }
        if let Some(command) = self.ui.confirmation(ctx) {
            self.dispatch(command, ctx);
        }
        if self.ui.preferences != self.saved_preferences {
            let since = self
                .preferences_changed
                .get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= std::time::Duration::from_millis(750) {
                self.save_preferences();
            }
        } else {
            self.preferences_changed = None;
        }
        if !ctx.input(|i| i.pointer.any_down()) {
            self.document.end_thickness_edit();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

#[cfg(test)]
mod tests;

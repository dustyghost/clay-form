//! Application controller: coordinates UI commands, document changes, and IO adapters.
use crate::{
    camera::Camera,
    command::{Command, PendingAction},
    document::Document,
    mesh_job::{MeshJob, Operation},
    persistence,
    preferences::{self, Action, Preferences, Shortcut},
    tablet::Tablet,
    ui::UiState,
    viewport::{Viewport, ViewportOptions},
};
use eframe::egui;

mod actions;
mod commands;
mod context_menu;
mod diagnostics;
mod files;
mod jobs;
mod masking;
pub(crate) mod paths;
mod projection;
mod recovery;
mod references;

pub struct App {
    pub(crate) document: Document,
    pub(crate) camera: Camera,
    pub(crate) viewport: Viewport,
    ui: UiState,
    tablet: Tablet,
    pressure: f32,
    diagnostic_environment: crate::diagnostics::Environment,
    job_settings: Option<crate::diagnostics::EditSettings>,
    projection: projection::ProjectionState,
    paths: paths::PathState,
    closing: bool,
    mesh_job: Option<MeshJob>,
    recovery: Option<crate::recovery::Recovery>,
    recovery_entries: Vec<crate::recovery::Entry>,
    recovery_error: Option<String>,
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
            diagnostic_environment: Default::default(),
            job_settings: None,
            projection: Default::default(),
            paths: Default::default(),
            closing: false,
            mesh_job: None,
            recovery: None,
            recovery_entries: Vec::new(),
            recovery_error: None,
            preferences_path: None,
            saved_preferences: Preferences::default(),
            preferences_changed: None,
        }
    }
}
impl App {
    pub fn with_wgpu(state: &eframe::egui_wgpu::RenderState) -> Result<Self, String> {
        let mut app = Self::new();
        app.viewport.initialize_renderer(state)?;
        let info = state.adapter.get_info();
        app.diagnostic_environment.renderer = Some("wgpu".into());
        app.diagnostic_environment.backend = Some(format!("{:?}", info.backend));
        app.diagnostic_environment.gpu = Some(info.name);
        app.diagnostic_environment.driver = Some(format!("{} {}", info.driver, info.driver_info));
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
        let _ = app
            .document
            .set_history_retention(app.ui.preferences.history_retention);
        app.initialize_recovery(path.with_file_name("recovery"));
        app.preferences_path = Some(path);
        app
    }
    fn finish_parameter_input(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| !i.pointer.any_down() || !i.focused) {
            self.document.end_edit();
        }
    }
    fn save_preferences(&mut self) {
        self.ui.preferences.sync_active_workspace();
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
            || self.ui.file_picker.is_some()
            || self.ui.recovery_open
            || self.ui.diagnostics_open
            || self.ui.settings_open
            || self.ui.layout_open
            || self.ui.alphas.draft.is_some()
            || self.ui.alphas.overwrite.is_some()
            || self.ui.icon_capture_open()
        {
            return;
        }
        if self.ui.viewport_menu.context.is_some() {
            return;
        }
        let visibility = self.temporary_visibility_active(ctx);
        let masking = self.temporary_mask_requested(ctx);
        let menu = self
            .ui
            .preferences
            .shortcut(&Action::ViewportContextMenu)
            .cloned();
        let open_menu = menu.is_some_and(|binding| {
            ctx.input_mut(|input| {
                let mut found = false;
                input.events.retain(|event| {
                    let matches = Shortcut::from_event(event).is_some_and(|shortcut| {
                        shortcut == binding
                            || (visibility
                                && self
                                    .ui
                                    .preferences
                                    .visibility_hold
                                    .matches_shortcut(&shortcut, &binding))
                            || (masking
                                && self
                                    .ui
                                    .preferences
                                    .mask_hold
                                    .matches_shortcut(&shortcut, &binding))
                    });
                    found |= matches && !matches!(event, egui::Event::Key { repeat: true, .. });
                    !matches
                });
                found
            })
        });
        if open_menu {
            self.open_viewport_menu(ctx);
            return;
        }
        if visibility && let Some(key) = &self.ui.preferences.visibility_hold.key {
            ctx.input_mut(|input| {
                input.events.retain(|event| {
                    Shortcut::from_event(event).is_none_or(|shortcut| &shortcut.key != key)
                })
            });
        }
        if masking {
            return;
        }
        for action in self.ui.preferences.actions(ctx) {
            self.execute_action(action, ctx);
            if self.ui.pending.is_some() || self.ui.file_picker.is_some() {
                break;
            }
        }
    }

    pub(crate) fn show_viewport(&mut self, ui: &mut egui::Ui) {
        let mask_consumes_escape = self.update_temporary_mask(ui);
        self.ui.viewport_menu.viewport_rect = Some(ui.available_rect_before_wrap());
        if !ui.input(|i| self.ui.preferences.visibility_hold.active(i)) {
            self.ui.viewport_menu.suppress_visibility = false;
        }
        let menu_was_open = self.ui.viewport_menu.context.is_some();
        self.show_viewport_menu(ui.ctx());
        let mut temporary_visibility = self.temporary_visibility_active(ui.ctx());
        if !menu_was_open && temporary_visibility && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.ui.viewport_menu.suppress_visibility = true;
            temporary_visibility = false;
            self.viewport.cancel(&mut self.document);
            self.viewport.wait_for_release();
            ui.input_mut(|i| {
                i.events.retain(|event| {
                    !matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Escape,
                            ..
                        }
                    )
                });
            });
        }
        if self.ui.viewport_menu.visibility_was_active
            && !temporary_visibility
            && self.viewport.gesture == Some(crate::viewport::Gesture::Select)
        {
            self.viewport.cancel(&mut self.document);
            self.viewport.wait_for_release();
        }
        self.ui.viewport_menu.visibility_was_active = temporary_visibility;
        self.paths.viewport_size = Some(ui.available_size());
        self.sync_path_brush_radius();
        self.show_paths(ui.ctx());
        if self.ui.projection.open && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_projection();
        }
        if let Some(id) = self.viewport.guided.used_path.take() {
            self.paths.selected_path = Some(id);
        }
        self.ui.edges.selected = self.viewport.selected_edges(&self.document).to_vec();
        if self.ui.file_picker.is_none()
            && self.ui.edges.selecting
            && !mask_consumes_escape
            && !ui.ctx().wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.ui.edges.selecting = false;
            self.viewport.cancel(&mut self.document);
            self.viewport.wait_for_release();
        }
        self.viewport.modeling.sync(&self.document);
        self.ui.modeling.count = self.viewport.modeling.selection().len();
        if self.ui.file_picker.is_none()
            && self.ui.modeling.active
            && !mask_consumes_escape
            && !ui.ctx().wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            if !self.viewport.modeling.dragging() {
                self.ui.modeling.active = false;
            }
            self.viewport.cancel(&mut self.document);
            self.viewport.wait_for_release();
        }
        let (primitive_commands, editor_rect) = self.ui.primitive_confirmation(ui.ctx());
        for command in primitive_commands {
            self.dispatch(command, ui.ctx());
        }
        let enabled = !menu_was_open
            && self.ui.viewport_menu.context.is_none()
            && self.ui.alphas.draft.is_none()
            && self.ui.alphas.overwrite.is_none()
            && !self.ui.icon_capture_open()
            && self.ui.primitive_warning.is_none()
            && self.ui.pending.is_none()
            && self.ui.file_picker.is_none()
            && !self.ui.recovery_open
            && !self.ui.diagnostics_open
            && !self.ui.settings_open
            && !self.ui.layout_open
            && self.mesh_job.is_none();
        if enabled
            && self.ui.arrange_references
            && !ui.ctx().wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.arrange_references(false);
        }
        if enabled {
            // Native file drops target the window. Desktop drag protocols do not
            // necessarily update the normal pointer position (or supply one at all).
            // Consume each drop once, including when egui requests another layout pass.
            for file in ui.input_mut(|i| std::mem::take(&mut i.raw.dropped_files)) {
                self.dispatch(Command::AddReference(file), ui.ctx());
            }
        }
        if enabled
            && self.ui.slice_active
            && !ui.ctx().wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.dispatch(Command::SliceTool(false), ui.ctx());
            // Releasing a cancelled cut must not start a sculpt stroke.
            self.viewport.wait_for_release();
        }
        let (commands, toolbar_rect) =
            self.ui
                .transform_toolbar(ui.ctx(), ui.available_rect_before_wrap(), enabled);
        for command in commands {
            self.dispatch(command, ui.ctx());
        }
        // A toolbar click must never also grab a handle or place an Action Line.
        let over_editor = editor_rect.is_some_and(|rect| {
            ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))
        });
        let over_toolbar = toolbar_rect.is_some_and(|rect| {
            ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))
        });
        let selected = self.ui.brush_index_for_context(&self.document, ui.ctx());
        self.capture_diagnostic_context(ui.ctx());
        self.viewport.projection_preview = self
            .paths
            .preview_mesh
            .clone()
            .or_else(|| self.projection.draft.as_ref().map(|d| d.mesh.clone()));
        self.viewport.projection_mode = self.ui.projection.open;
        self.viewport.guided.enabled = self.ui.guide_paths && !self.ui.mask_active;
        self.viewport.guided.path_starts_only = self.ui.path_starts_only;
        self.viewport.visibility_hold = Some(
            if self.ui.viewport_menu.suppress_visibility || self.ui.mask_active {
                Default::default()
            } else {
                self.ui.preferences.visibility_hold.clone()
            },
        );
        self.viewport.show(
            ui,
            &mut self.document,
            &mut self.camera,
            ViewportOptions {
                edge_select: self.ui.edges.selecting
                    && !temporary_visibility
                    && !self.ui.mask_active,
                modeling: self
                    .ui
                    .modeling
                    .active
                    .then_some(self.ui.modeling.options())
                    .filter(|_| !temporary_visibility && !self.ui.mask_active),
                arrange_references: self.ui.arrange_references,
                references_visible: self.ui.references_visible,
                slice: self
                    .ui
                    .slice_active
                    .then_some(self.ui.slice_options)
                    .filter(|_| !temporary_visibility),
                select: if temporary_visibility {
                    Some(self.ui.temporary_visibility_tool)
                } else if self.ui.mask_active {
                    self.ui.preferences.mask_preset.selection_tool()
                } else {
                    self.ui.selection_tool
                },
                slice_path_mode: self.ui.slice_path_mode,
                transform_mode: if temporary_visibility {
                    crate::viewport::TransformMode::Sculpt
                } else {
                    self.ui.transform_mode
                },
                transform_operation: self.ui.transform_operation,
                respect_mask: self.ui.respect_transform_mask,
                selected,
                temporary_mask: self.ui.mask_active,
                mask_erase: self.ui.preferences.mask_preset.erase,
                pressure: self.pressure,
                wire: self.ui.wire,
                wire_mode: self.ui.wire_mode,
                wire_triangles: self.ui.wire_triangles,
                show_polygroups: self.ui.show_polygroups,
                backface_opacity: if self.ui.show_backfaces {
                    self.ui.backface_opacity
                } else {
                    0.
                },
                show_guides: self.ui.show_symmetry_guides,
                guide_opacity: self.ui.guide_opacity,
                enabled: enabled
                    && (self.viewport.guided.dragging()
                        || (!over_toolbar
                            && !over_editor
                            && !self.ui.pointer_over_pinned_menu(ui.ctx()))),
                reverse: if self.ui.mask_active {
                    preferences::Hold::Disabled
                } else {
                    self.ui.preferences.reverse
                },
                zoom_hold: if self.ui.mask_active {
                    preferences::Hold::Disabled
                } else {
                    self.ui.preferences.zoom_hold
                },
                pan_hold: if self.ui.mask_active {
                    preferences::Hold::Disabled
                } else {
                    self.ui.preferences.pan_hold
                },
            },
        );
        self.ui.edges.selected = self.viewport.selected_edges(&self.document).to_vec();
        if self.ui.mask_active {
            self.viewport.modeling.accept_mask_revision(&self.document);
            self.paint_temporary_mask_status(ui);
        }
        self.ui.modeling.count = self.viewport.modeling.selection().len();
        if temporary_visibility {
            let rect = self.ui.viewport_menu.viewport_rect.unwrap_or(ui.max_rect());
            ui.painter().text(
                rect.left_top() + egui::vec2(12., 12.),
                egui::Align2::LEFT_TOP,
                format!(
                    "Visibility · {} · hold {} · tool menu {}",
                    self.ui.temporary_visibility_tool.label(),
                    self.ui.preferences.visibility_hold.label(),
                    self.ui
                        .preferences
                        .shortcut(&Action::ViewportContextMenu)
                        .map_or("Unbound".into(), |s| s.label())
                ),
                egui::FontId::proportional(14.),
                egui::Color32::WHITE,
            );
        }
        self.ui.modeling.can_repeat = self.viewport.modeling.last_edit.is_some();
        self.ui.modeling.repeat_label =
            self.viewport.modeling.last_edit.map_or(String::new(), |o| {
                format!("{} · {:.3}", o.tool.label(), o.amount)
            });
        if let Some(amount) = self.viewport.modeling.requested_amount.take() {
            self.ui.modeling.amount = amount;
        }
        if let Some(direction) = self.viewport.modeling.requested_direction.take() {
            self.ui.modeling.transform_direction = direction;
        }
        if let Some(count) = self.viewport.modeling.requested_loop_count.take() {
            self.ui.modeling.loop_count = count;
        }
        if let Some(index) = self.viewport.reference_selection() {
            self.ui.selected_reference = index;
        }
        if let Some(operation) = self.viewport.completed_slice.take()
            && let Err(error) = self.start_mesh_job(operation)
        {
            self.ui.status = error;
        }
        if let Some(notice) = self.viewport.modeling.notice.take() {
            self.ui.status = notice.into();
        }
        if let Some(status) = self.viewport.status.take() {
            self.ui.status = status;
        }
        if self.viewport.completed_stroke
            && !self.document.stencil().is_some_and(|s| s.enabled)
            && self.document.mesh_settings().dynamic
            && self.document.mesh_settings().auto_remesh
            && !self.document.mesh().has_hidden_faces()
            && self.mesh_job.is_none()
            && let Err(error) = self.start_mesh_job(Operation::Remesh)
        {
            self.ui.status = error;
        }
    }
}
impl eframe::App for App {
    fn on_exit(&mut self) {
        self.save_preferences();
        if self.closing || !self.document.is_dirty() {
            self.retire_recovery();
        }
        self.viewport.destroy_renderer();
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll_mesh_job();
        self.poll_projection();
        self.poll_paths();
        let native_pressure = ctx.input(|i| {
            i.events.iter().rev().find_map(|event| match event {
                egui::Event::Touch { force: Some(p), .. } => Some(*p),
                _ => None,
            })
        });
        self.pressure = self.tablet.pressure().or(native_pressure).unwrap_or(1.);
        if ctx.input(|i| i.viewport().close_requested()) && self.ui.projection.open {
            self.cancel_projection();
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.viewport.paths.active {
            let _ = self.dispatch_path(paths::PathCommand::Close);
        }
        if ctx.input(|i| i.viewport().close_requested())
            && self.document.is_dirty()
            && !self.closing
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(PendingAction::Quit, ctx);
        }
        for command in self.ui.toolbar(ctx, &self.document, &self.camera) {
            self.dispatch(command, ctx);
        }
        for command in self.ui.custom_surfaces(ctx, &self.document) {
            self.dispatch(command, ctx);
        }
        for command in self.ui.brush_panel(
            ctx,
            &self.document,
            &self.tablet.status(),
            self.pressure,
            &self.camera,
        ) {
            self.dispatch(command, ctx);
        }
        for command in self.ui.projection_window(ctx, &self.document) {
            self.dispatch(command, ctx);
        }
        self.ui.settings(ctx, &self.document);
        self.shortcuts(ctx);
        if self.recovery_error.is_some() {
            egui::TopBottomPanel::bottom("recovery_error").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(
                        egui::Color32::LIGHT_RED,
                        format!(
                            "Recovery failed: {}",
                            self.recovery_error.as_deref().unwrap()
                        ),
                    );
                    if ui.button("Recovery settings…").clicked() {
                        self.open_recovery();
                    }
                });
            });
        }
        self.ui.status_bar(ctx);
        egui::CentralPanel::default().show(ctx, |ui| self.show_viewport(ui));
        if let Some(command) = self.ui.alpha_popup(ctx) {
            self.dispatch(command, ctx);
        }
        if let Some(command) = self.ui.icon_capture_popup(ctx, &self.document) {
            self.dispatch(command, ctx);
        }
        if let Some(command) = self.ui.project_file_dialog(ctx) {
            self.dispatch(command, ctx);
        }
        if let Some(command) = self.ui.confirmation(ctx) {
            self.dispatch(command, ctx);
        }
        self.show_recovery(ctx);
        self.show_diagnostics(ctx);
        self.ui.preferences.sync_active_workspace();
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
        self.finish_parameter_input(ctx);
        self.tick_recovery(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod parameter_tests;

#[cfg(test)]
mod recovery_tests;

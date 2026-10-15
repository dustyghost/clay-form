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

mod actions;
mod commands;
mod files;
mod jobs;
mod recovery;
mod references;

pub struct App {
    pub(crate) document: Document,
    pub(crate) camera: Camera,
    pub(crate) viewport: Viewport,
    ui: UiState,
    tablet: Tablet,
    pressure: f32,
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
            || self.ui.recovery_open
            || self.ui.settings_open
            || self.ui.layout_open
            || self.ui.icon_capture_open()
        {
            return;
        }
        for action in self.ui.preferences.actions(ctx) {
            self.execute_action(action, ctx);
            if self.ui.pending.is_some() {
                break;
            }
        }
    }

    pub(crate) fn show_viewport(&mut self, ui: &mut egui::Ui) {
        self.ui.edges.selected = self.viewport.selected_edges(&self.document).to_vec();
        if self.ui.edges.selecting
            && !ui.ctx().wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.ui.edges.selecting = false;
            self.viewport.cancel(&mut self.document);
            self.viewport.wait_for_release();
        }
        self.viewport.modeling.sync(&self.document);
        self.ui.modeling.count = self.viewport.modeling.selection().len();
        if self.ui.modeling.active
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
        let enabled = !self.ui.icon_capture_open()
            && self.ui.primitive_warning.is_none()
            && self.ui.pending.is_none()
            && !self.ui.recovery_open
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
        if enabled && ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
            for file in ui.input(|i| i.raw.dropped_files.clone()) {
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
        self.viewport.show(
            ui,
            &mut self.document,
            &mut self.camera,
            ViewportOptions {
                edge_select: self.ui.edges.selecting,
                modeling: self
                    .ui
                    .modeling
                    .active
                    .then_some(self.ui.modeling.options()),
                arrange_references: self.ui.arrange_references,
                references_visible: self.ui.references_visible,
                slice: self.ui.slice_active.then_some(self.ui.slice_options),
                select: self.ui.selection_tool,
                slice_path_mode: self.ui.slice_path_mode,
                transform_mode: self.ui.transform_mode,
                transform_operation: self.ui.transform_operation,
                respect_mask: self.ui.respect_transform_mask,
                selected,
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
                    && !over_toolbar
                    && !over_editor
                    && !self.ui.pointer_over_pinned_menu(ui.ctx()),
                reverse: self.ui.preferences.reverse,
                zoom_hold: self.ui.preferences.zoom_hold,
                pan_hold: self.ui.preferences.pan_hold,
            },
        );
        self.ui.edges.selected = self.viewport.selected_edges(&self.document).to_vec();
        self.ui.modeling.count = self.viewport.modeling.selection().len();
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
        if let Some(status) = self.viewport.status.take() {
            self.ui.status = status;
        }
        if self.viewport.completed_stroke
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
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.save_preferences();
        if self.closing || !self.document.is_dirty() {
            self.retire_recovery();
        }
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
        if let Some(command) = self.ui.icon_capture_popup(ctx, &self.document) {
            self.dispatch(command, ctx);
        }
        if let Some(command) = self.ui.confirmation(ctx) {
            self.dispatch(command, ctx);
        }
        self.show_recovery(ctx);
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

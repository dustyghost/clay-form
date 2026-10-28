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
mod exports;
mod files;
mod jobs;
mod masking;
pub(crate) mod paths;
pub(crate) mod policy;
mod projection;
mod recovery;
mod references;
pub(crate) mod textures;

pub struct App {
    pub(crate) document: Document,
    pub(crate) camera: Camera,
    pub(crate) viewport: Viewport,
    ui: UiState,
    tablet: Tablet,
    pressure: f32,
    touch: crate::touch::TouchInput,
    diagnostic_environment: crate::diagnostics::Environment,
    job_settings: Option<crate::diagnostics::EditSettings>,
    projection: projection::ProjectionState,
    paths: paths::PathState,
    closing: bool,
    mesh_job: Option<MeshJob>,
    texture_exchange: Option<textures::ExchangeJob>,
    export_job: Option<
        crate::background_job::BackgroundJob<(
            std::path::PathBuf,
            crate::persistence::material_export::ExportBundle,
        )>,
    >,
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
            touch: Default::default(),
            diagnostic_environment: Default::default(),
            job_settings: None,
            projection: Default::default(),
            paths: Default::default(),
            closing: false,
            mesh_job: None,
            texture_exchange: None,
            export_job: None,
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
        app.ui.initialize_startup_notice();
        app
    }
    fn show_startup_notice(&mut self, ctx: &egui::Context) -> bool {
        if !self.ui.startup_notice_open {
            return false;
        }
        self.ui.startup_notice(ctx);
        if !self.ui.startup_notice_open {
            self.save_preferences();
            ctx.request_repaint();
        }
        // The Continue click must not reach the workspace in this frame.
        true
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
            || self.ui.about_open
            || self.ui.diagnostics_open
            || self.ui.viewport_controls.editor_open
            || self.ui.viewport_controls.arranging
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
        let mask_binding = self.temporary_mask_binding(ctx).clone();
        for action in [
            Action::ViewportContextMenu,
            Action::MaskVisibilityMenu,
            Action::TransformMenu,
        ] {
            let binding = self.ui.preferences.shortcut(&action).cloned();
            let open_menu = binding.is_some_and(|binding| {
                ctx.input_mut(|input| {
                    let mut found = false;
                    input.events.retain(|event| {
                        let matches = Shortcut::from_event(event).is_some_and(|mut shortcut| {
                            // Alt reverses a held selection; it must not block its chooser.
                            if (visibility || masking) && !binding.alt {
                                shortcut.alt = false;
                            }
                            shortcut == binding
                                || (visibility
                                    && self
                                        .ui
                                        .preferences
                                        .visibility_hold
                                        .matches_shortcut(&shortcut, &binding))
                                || (masking && mask_binding.matches_shortcut(&shortcut, &binding))
                        });
                        found |= matches && !matches!(event, egui::Event::Key { repeat: true, .. });
                        !matches
                    });
                    found
                })
            });
            if open_menu {
                self.open_quick_menu(ctx, action);
                return;
            }
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
        self.ui.preferences.lighting.install(ui.ctx());
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
        if self.ui.viewport_menu.visibility_was_active && !temporary_visibility {
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
        if let Some(id) = self.viewport.session.guided.used_path.take() {
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
        self.ui.modeling.preview_amount = self.viewport.modeling.preview_amount();
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
        let enabled = !self.ui.customization.popup_blocks_viewport
            && self.ui.customization.popup_menu.is_none()
            && !menu_was_open
            && self.ui.viewport_menu.context.is_none()
            && self.ui.alphas.draft.is_none()
            && self.ui.alphas.overwrite.is_none()
            && !self.ui.icon_capture_open()
            && self.ui.primitive_warning.is_none()
            && self.ui.pending.is_none()
            && self.ui.file_picker.is_none()
            && !self.ui.recovery_open
            && !self.ui.about_open
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
        let controls = self.ui.viewport_controls(
            ui.ctx(),
            ui.available_rect_before_wrap(),
            &self.document,
            enabled
                && self.viewport.session.gesture.is_none()
                && !self.viewport.modeling.dragging(),
        );
        for command in controls.commands {
            self.dispatch(command, ui.ctx());
        }
        for (kind, delta) in controls.navigation {
            use crate::preferences::ViewportControlKind;
            match kind {
                ViewportControlKind::Pan => self.camera.pan += delta,
                ViewportControlKind::Zoom => self.camera.zoom_drag(delta),
                ViewportControlKind::Orbit => self.camera.orbit(delta),
                ViewportControlKind::Roll => self.camera.roll_drag(delta),
                ViewportControlKind::Action(_) => unreachable!(),
            }
        }
        let light_input = self.ui.light_controls.show(
            ui.ctx(),
            ui.available_rect_before_wrap(),
            &self.camera,
            &mut self.ui.preferences.lighting,
            enabled && !controls.blocks_viewport && self.viewport.session.gesture.is_none(),
        );
        self.ui.preferences.lighting.install(ui.ctx());
        let selected = self.ui.brush_index_for_context(&self.document, ui.ctx());
        let rect = ui.available_rect_before_wrap();
        let touch = self.touch.update(
            ui,
            &self.ui.preferences.touch,
            &mut self.camera,
            enabled
                && !self.ui.viewport_controls.arranging
                && !self.ui.viewport_controls.editor_open
                && self.viewport.session.gesture.is_none()
                && !self.viewport.modeling.dragging(),
            |pos| {
                rect.contains(pos)
                    && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
                    && !toolbar_rect.is_some_and(|r| r.contains(pos))
                    && !editor_rect.is_some_and(|r| r.contains(pos))
            },
        );
        for action in touch.actions {
            self.dispatch(Command::Action(action), ui.ctx());
        }
        self.capture_diagnostic_context(ui.ctx());
        self.viewport.session.projection_preview = self
            .paths
            .preview_mesh
            .clone()
            .or_else(|| self.projection.draft.as_ref().map(|d| d.mesh.clone()));
        self.viewport.session.projection_mode = self.ui.projection.open;
        self.viewport.session.guided.enabled =
            self.ui.guide_paths && !self.ui.mask_active && !temporary_visibility;
        self.viewport.session.guided.path_starts_only = self.ui.path_starts_only;
        self.viewport.session.visibility_hold = Some(if !temporary_visibility {
            Default::default()
        } else {
            self.ui.preferences.visibility_hold.clone()
        });
        self.ui.materials.sampling &= self
            .document
            .brushes()
            .get(self.ui.selected)
            .is_some_and(|b| b.kind == crate::sculpt::Kind::Paint);
        if self.ui.materials.sampling && !self.viewport.sample_colour {
            self.viewport.cancel(&mut self.document);
        }
        self.viewport.sample_colour = self.ui.materials.sampling;
        self.viewport.size_preview = self
            .ui
            .brush_size_preview
            .filter(|(frame, _)| *frame == ui.ctx().cumulative_frame_nr())
            .and_then(|(_, index)| self.document.brushes().get(index).cloned());
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
                    Some(self.ui.preferences.visibility_selection.tool())
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
                mask_erase: self.ui.preferences.mask_preset.erase ^ ui.input(|i| i.modifiers.alt),
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
                    && !touch.blocks_pointer
                    && !controls.blocks_viewport
                    && !light_input
                    && (self.viewport.session.guided.dragging()
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
        self.ui.materials.sampling = self.viewport.sample_colour;
        let sampled = self.viewport.sampled_material.take().or_else(|| {
            self.viewport.sampled_colour.take().map(|colour| {
                let mut material = self.document.brushes()[self.ui.selected].material.clone();
                material.base_color = colour;
                material
            })
        });
        self.viewport.sampled_colour = None;
        if let Some(sampled) = sampled
            && let Err(error) = self.sample_surface_material(sampled)
        {
            self.ui.status = error;
        }
        self.ui.edges.selected = self.viewport.selected_edges(&self.document).to_vec();
        if self.ui.mask_active || temporary_visibility {
            self.viewport
                .modeling
                .accept_attribute_revision(&self.document);
        }
        if self.ui.mask_active {
            self.paint_temporary_mask_status(ui);
        }
        self.ui.modeling.count = self.viewport.modeling.selection().len();
        if temporary_visibility {
            let rect = self.ui.viewport_menu.viewport_rect.unwrap_or(ui.max_rect());
            let galley = ui.painter().layout(
                format!(
                    "Selection · Visibility · {} · {}{} · hold {} · tool menu {}",
                    self.ui.preferences.visibility_selection.shape.label(),
                    if self.ui.preferences.visibility_selection.hide ^ ui.input(|i| i.modifiers.alt)
                    {
                        "Hide"
                    } else {
                        "Isolate"
                    },
                    if self.ui.preferences.visibility_selection.flip {
                        " (flipped)"
                    } else {
                        ""
                    },
                    self.ui.preferences.visibility_hold.label(),
                    self.ui
                        .preferences
                        .shortcut(&Action::MaskVisibilityMenu)
                        .map_or("Unbound".into(), |s| s.label())
                ),
                egui::FontId::proportional(14.),
                egui::Color32::WHITE,
                (rect.width() - 172.).max(120.),
            );
            ui.painter().galley(
                rect.left_top() + egui::vec2(12., 42.),
                galley,
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
        if self.show_startup_notice(ctx) {
            return;
        }
        self.poll_mesh_job();
        self.poll_export(ctx);
        self.poll_texture_exchange(ctx);
        if self.texture_exchange.is_some() && !self.ui.uv.open {
            let mut cancel = false;
            egui::TopBottomPanel::top("texture_operation").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spinner();
                    ui.label(&self.ui.uv.operation);
                    if let Some(progress) = self.ui.uv.operation_progress {
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .desired_width(160.)
                                .show_percentage(),
                        );
                    }
                    cancel = ui.button("Cancel texture operation").clicked();
                });
            });
            if cancel {
                let _ = self.texture_command(textures::TextureCommand::Cancel);
            }
        }
        self.poll_projection();
        self.poll_paths();
        let native_pressure = ctx.input(|i| self.tablet.update_native(&i.events));
        self.pressure = native_pressure
            .or_else(|| self.tablet.pressure())
            .unwrap_or(1.);
        if ctx.input(|i| i.viewport().close_requested()) && self.ui.projection.open {
            self.cancel_projection();
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.viewport.paths.active {
            let _ = self.dispatch_path(crate::command::PathCommand::Close);
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
        for command in self.ui.uv_window(
            ctx,
            &self.document,
            &self.viewport.modeling.selected_faces(),
        ) {
            self.dispatch(command, ctx);
        }
        self.viewport.set_texture_exposure(if self.ui.uv.open {
            self.ui.uv.exposure
        } else {
            0.
        });
        self.viewport
            .set_uv_checker(self.ui.uv.open && self.ui.uv.checker);
        if matches!(
            self.ui.preferences.paint_inspection,
            crate::viewport::InspectionMode::Roughness | crate::viewport::InspectionMode::Metallic
        ) {
            self.ui.preferences.paint_inspection = crate::viewport::InspectionMode::Lit;
        }
        self.viewport
            .set_inspection_mode(self.ui.preferences.paint_inspection);
        self.ui.settings(ctx, &self.document);
        self.ui.viewport_controls_editor(ctx, &self.document);
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
        self.ui.about(ctx);
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

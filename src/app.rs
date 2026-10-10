//! Application controller: coordinates UI commands, document changes, and IO adapters.
use crate::{
    camera::Camera,
    command::{Command, PendingAction},
    document::Document,
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
            preferences_path: None,
            saved_preferences: Preferences::default(),
            preferences_changed: None,
        }
    }
}
impl App {
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
            || self.ui.pending.is_some()
            || self.ui.settings_open
            || self.ui.layout_open
        {
            return;
        }
        for action in self.ui.preferences.actions(ctx) {
            let command = match action {
                Action::Save => Some(Command::Save),
                Action::Open => Some(Command::Request(PendingAction::Open(self.ui.path.clone()))),
                Action::New => Some(Command::Request(PendingAction::New)),
                Action::Undo => Some(Command::Undo),
                Action::Redo => Some(Command::Redo),
                Action::ResetView => Some(Command::ResetView),
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
                self.viewport.cancel(&mut self.document);
                self.document.undo();
                Ok(())
            }
            Command::Redo => {
                self.viewport.cancel(&mut self.document);
                self.document.redo();
                Ok(())
            }
            Command::ClearMask => {
                self.viewport.cancel(&mut self.document);
                self.document.clear_mask();
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
                .and_then(|brush| self.document.append_brush(brush))
                .map(|index| {
                    self.ui.selected = index;
                    self.ui.status = "Brush loaded".into();
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
        self.viewport.show(
            ui,
            &mut self.document,
            &mut self.camera,
            ViewportOptions {
                selected: self.ui.selected,
                pressure: self.pressure,
                wire: self.ui.wire,
                show_guides: self.ui.show_symmetry_guides,
                guide_opacity: self.ui.guide_opacity,
                enabled: self.ui.pending.is_none()
                    && !self.ui.settings_open
                    && !self.ui.layout_open,
                smooth: self.ui.preferences.smooth,
                reverse: self.ui.preferences.reverse,
            },
        );
    }
}
impl eframe::App for App {
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.save_preferences();
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
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
        for command in self.ui.toolbar(ctx, &self.document) {
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
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

#[cfg(test)]
mod tests;

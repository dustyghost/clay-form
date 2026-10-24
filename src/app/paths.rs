//! Cancellable brush previews and commands for the persistent path editor.
use super::*;
use crate::background_job::BackgroundJob;
use crate::{
    document::{PathStrokeOptions, PathStrokePreview},
    sculpt::{
        Mesh,
        paths::{PathMode, SurfacePath},
    },
};
use std::sync::Arc;

use crate::command::PathCommand;
struct Worker {
    job: BackgroundJob<PathStrokePreview>,
    auto_apply: bool,
}
pub(crate) struct PathState {
    worker: Option<Worker>,
    preview: Option<PathStrokePreview>,
    pub preview_mesh: Option<Arc<Mesh>>,
    pub options: PathStrokeOptions,
    pub use_brush_size: bool,
    pub viewport_size: Option<egui::Vec2>,
    pub brush_radius_world: Option<f32>,
    pub reproject_distance: f32,
    pub(crate) selected_path: Option<u64>,
}
impl Default for PathState {
    fn default() -> Self {
        Self {
            worker: None,
            preview: None,
            preview_mesh: None,
            options: Default::default(),
            use_brush_size: true,
            viewport_size: None,
            brush_radius_world: None,
            reproject_distance: 0.2,
            selected_path: None,
        }
    }
}
impl PathState {
    pub(crate) fn busy(&self) -> bool {
        self.worker.is_some()
    }
    pub(crate) fn has_preview(&self) -> bool {
        self.preview.is_some()
    }
    pub(crate) fn applying_entire_path(&self) -> bool {
        self.worker.as_ref().is_some_and(|worker| worker.auto_apply)
    }
    fn cancel(&mut self) {
        self.worker = None;
        self.preview = None;
        self.preview_mesh = None;
    }
}
impl App {
    fn path_brush_radius(&self, path: &SurfacePath) -> Option<f32> {
        let size = self.paths.viewport_size?;
        if !size.is_finite() || size.x.min(size.y) <= 0. {
            return None;
        }
        // Freeze one model-space footprint for the whole operation. In perspective,
        // match the nearest side of the guide instead of a possibly hidden seam.
        let view = self.camera.basis().2;
        let point = path
            .dense_curve()
            .into_iter()
            .map(|point| point.position)
            .filter(|point| {
                self.camera.eye_distance() == 0.
                    || self.camera.eye_distance() - point.dot(view)
                        >= self.camera.eye_distance() * 0.001
            })
            .max_by(|a, b| a.dot(view).total_cmp(&b.dot(view)))?;
        let scale = self.camera.scale_at(size, point);
        (scale.is_finite() && scale > 0.).then(|| {
            (self.document.brushes()[self.ui.selected].radius / scale).clamp(0.00001, 1000.)
        })
    }
    fn automatic_path_options(&self, id: u64) -> Result<PathStrokeOptions, String> {
        let mut options = self.paths.options;
        if self.paths.use_brush_size {
            let path = self.document.path(id).ok_or("Select a saved path")?;
            options.radius = self
                .path_brush_radius(path)
                .ok_or("Show the path in the viewport before using the brush size")?;
        }
        Ok(options)
    }
    pub(crate) fn sync_path_brush_radius(&mut self) {
        if self.paths.busy() || self.paths.has_preview() {
            return;
        }
        self.paths.brush_radius_world = self
            .viewport
            .paths
            .draft
            .as_ref()
            .and_then(|path| self.path_brush_radius(path));
    }
    pub(super) fn sync_path_history_state(&mut self) {
        self.ui.paths_active = self.viewport.paths.active;
        self.ui.paths_preview_active = self.paths.busy() || self.paths.has_preview();
        self.ui.paths_history_reason = if !self.viewport.paths.active {
            None
        } else if self.viewport.paths.dirty(&self.document) {
            Some("Finish editing paths: save or discard the draft before Undo or Redo")
        } else if self.paths.busy() {
            Some("Cancel the running path stroke or wait for it to finish before Undo or Redo")
        } else {
            None
        };
        self.ui.paths_history_ready = self.ui.paths_history_reason.is_none();
    }
    pub(super) fn prepare_path_history(&mut self) -> Result<(), String> {
        if !self.viewport.paths.active {
            return Ok(());
        }
        self.sync_path_history_state();
        super::policy::EditorPolicy::new(&self.ui, &self.document).history()?;
        // A private preview is not history. Drop it before restoring committed state.
        self.paths.cancel();
        self.viewport.paths.frozen = false;
        Ok(())
    }
    pub(super) fn refresh_path_after_history(&mut self) {
        if !self.viewport.paths.active {
            return;
        }
        let id = self
            .viewport
            .paths
            .draft
            .as_ref()
            .map(|path| path.id)
            .or(self.paths.selected_path);
        let selected = self.viewport.paths.selected;
        let path = id
            .and_then(|id| self.document.path(id))
            .filter(|path| path.target == self.document.active_model_id())
            .or_else(|| {
                self.document
                    .paths()
                    .iter()
                    .find(|path| path.target == self.document.active_model_id())
            })
            .cloned();
        if let Some(path) = path {
            let same_path = id == Some(path.id);
            self.paths.selected_path = Some(path.id);
            self.viewport.paths.select(path);
            self.viewport.paths.selected = selected.filter(|&index| {
                same_path && index < self.viewport.paths.draft.as_ref().unwrap().points.len()
            });
        } else {
            // Retain the identity so Redo can restore the just-undone path.
            self.paths.selected_path = id;
            let tool = self.viewport.paths.tool;
            let ring_mode = self.viewport.paths.ring_mode;
            let ring_axis = self.viewport.paths.ring_axis;
            self.viewport.paths = Default::default();
            self.viewport.paths.active = true;
            self.viewport.paths.tool = tool;
            self.viewport.paths.ring_mode = ring_mode;
            self.viewport.paths.ring_axis = ring_axis;
        }
        self.viewport.paths.notice = None;
        self.sync_path_history_state();
    }
    pub(super) fn show_paths(&mut self, ctx: &egui::Context) {
        self.sync_path_history_state();
        if self.viewport.paths.active
            && (self.paths.busy() || self.paths.has_preview())
            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && !ctx.wants_keyboard_input()
        {
            self.paths.cancel();
        }
        self.viewport.paths.frozen = self.paths.busy() || self.paths.has_preview();
        let commands = crate::ui::paths::show(
            ctx,
            &mut self.viewport.paths,
            &mut self.paths,
            &self.document,
            &mut self.ui.selected,
        );
        for command in commands {
            self.dispatch(command, ctx);
        }
        if let Some(notice) = self.viewport.paths.notice.take() {
            self.ui.status = notice;
        }
        self.sync_path_history_state();
    }
    pub(super) fn dispatch_path(&mut self, command: PathCommand) -> Result<(), String> {
        if matches!(command, PathCommand::Close) {
            self.paths.cancel();
            self.viewport.paths = Default::default();
            self.ui.paths_active = false;
            self.viewport.wait_for_release();
            return Ok(());
        }
        self.sync_path_history_state();
        super::policy::EditorPolicy::new(&self.ui, &self.document).path()?;
        if !matches!(
            command,
            PathCommand::Open | PathCommand::New(_) | PathCommand::NewRing
        ) && !self.viewport.paths.active
        {
            if matches!(command, PathCommand::StrokeEntirePath) {
                let valid_selection = self
                    .paths
                    .selected_path
                    .and_then(|id| self.document.path(id))
                    .is_some_and(|path| path.target == self.document.active_model_id());
                if !valid_selection {
                    self.paths.selected_path = self
                        .document
                        .paths()
                        .iter()
                        .find(|path| {
                            path.target == self.document.active_model_id()
                                && path.mode == PathMode::Surface
                        })
                        .map(|path| path.id);
                }
                self.dispatch_path(PathCommand::Open)?;
            } else {
                return Err("Open Editable paths first".into());
            }
        }
        if (self.paths.busy() || self.paths.has_preview())
            && !matches!(
                command,
                PathCommand::Apply | PathCommand::CancelPreview | PathCommand::Close
            )
        {
            return Err("Apply or cancel the brush preview first".into());
        }
        match command {
            PathCommand::Open => {
                if self.viewport.paths.active {
                    return Ok(());
                }
                self.viewport.cancel(&mut self.document);
                self.ui.modeling.active = false;
                self.ui.edges.selecting = false;
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                self.ui.arrange_references = false;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                self.viewport.paths.active = true;
                self.ui.paths_active = true;
                if let Some(path) = self
                    .paths
                    .selected_path
                    .and_then(|id| self.document.path(id))
                    .filter(|p| p.target == self.document.active_model_id())
                    .or_else(|| {
                        self.document
                            .paths()
                            .iter()
                            .find(|p| p.target == self.document.active_model_id())
                    })
                {
                    self.paths.selected_path = Some(path.id);
                    self.viewport.paths.select(path.clone());
                }
                self.ui.status =
                    "Choose a saved path, or start a surface or free-space path".into();
            }
            PathCommand::NewRing => {
                self.dispatch_path(PathCommand::New(PathMode::Surface))?;
                self.viewport.paths.tool = crate::viewport::paths::PathTool::Ring;
            }
            PathCommand::New(mode) => {
                if self.viewport.paths.dirty(&self.document) {
                    return Err("Save or discard the current path before starting another".into());
                }
                if !self.viewport.paths.active {
                    self.dispatch_path(PathCommand::Open)?;
                }
                self.viewport.paths.select(SurfacePath {
                    id: 0,
                    target: self.document.active_model_id(),
                    name: format!("Path {}", self.document.paths().len() + 1),
                    mode,
                    points: Vec::new(),
                    smoothing: 0.,
                    closed: false,
                    seam: 0,
                    surface_signature: 0,
                    attached_route: None,
                    corner_rounding: Vec::new(),
                });
                self.viewport.paths.tool = crate::viewport::paths::PathTool::Add;
            }
            PathCommand::Select(id) => {
                if self.viewport.paths.dirty(&self.document) {
                    return Err(
                        "Save or discard the current edits before choosing another path".into(),
                    );
                }
                let path = self
                    .document
                    .path(id)
                    .ok_or("Path no longer exists")?
                    .clone();
                if path.target != self.document.active_model_id() {
                    return Err("Select the path's target submodel first".into());
                }
                self.viewport.paths.select(path);
                self.paths.selected_path = Some(id);
            }
            PathCommand::SaveAndSculpt => {
                self.dispatch_path(PathCommand::Save)?;
                self.ui.guide_paths = true;
                self.dispatch_path(PathCommand::Close)?;
                self.ui.status = "Path saved. Start a brush stroke near the guide and draw along the section you want to sculpt.".into();
            }
            PathCommand::Save => {
                let path = self
                    .viewport
                    .paths
                    .draft
                    .clone()
                    .ok_or("Start a path first")?;
                let id = if path.id == 0 {
                    // Opening a new loop at its chosen seam preserves that seam
                    // with create_path's canonical zero-index starting point.
                    let mut points = path.points;
                    let mut corner_rounding = path.corner_rounding;
                    if !corner_rounding.is_empty() && corner_rounding.len() != points.len() {
                        return Err("Corner rounding must match the path points".into());
                    }
                    if path.closed && !points.is_empty() {
                        let n = points.len();
                        let seam = path.seam.min(n - 1);
                        points.rotate_left(seam);
                        if !corner_rounding.is_empty() {
                            corner_rounding.rotate_left(seam);
                        }
                    }
                    self.document.create_path_with_rounding(
                        path.name,
                        path.mode,
                        points,
                        path.closed,
                        path.smoothing,
                        corner_rounding,
                    )?
                } else {
                    let id = path.id;
                    self.document.update_path(path)?;
                    id
                };
                self.viewport
                    .paths
                    .select(self.document.path(id).unwrap().clone());
                self.paths.selected_path = Some(id);
                self.ui.status = "Path saved; Undo restores the previous path".into();
            }
            PathCommand::Discard => {
                let id = self.viewport.paths.draft.as_ref().map(|p| p.id);
                self.viewport.paths.draft = id.and_then(|id| self.document.path(id).cloned());
                self.viewport.paths.selected = None;
            }
            PathCommand::Delete => {
                let id = self
                    .viewport
                    .paths
                    .draft
                    .as_ref()
                    .ok_or("Select a saved path")?
                    .id;
                self.document.delete_path(id)?;
                self.viewport.paths.draft = None;
                self.viewport.paths.selected = None;
            }
            PathCommand::Reproject => {
                if self.viewport.paths.dirty(&self.document) {
                    return Err("Save or discard edits before reprojecting the saved path".into());
                }
                let id = self
                    .viewport
                    .paths
                    .draft
                    .as_ref()
                    .ok_or("Select a saved path")?
                    .id;
                self.document
                    .reproject_path(id, self.paths.reproject_distance)?;
                self.viewport
                    .paths
                    .select(self.document.path(id).unwrap().clone());
                self.ui.status = "Path reattached to the current surface".into();
            }
            PathCommand::Preview | PathCommand::StrokeEntirePath => {
                let auto_apply = matches!(command, PathCommand::StrokeEntirePath);
                if self.viewport.paths.dirty(&self.document) {
                    return Err("Save the path before applying or previewing a brush".into());
                }
                let id = self
                    .viewport
                    .paths
                    .draft
                    .as_ref()
                    .ok_or("Select a saved path")?
                    .id;
                let options = self.automatic_path_options(id)?;
                if self.paths.use_brush_size {
                    self.paths.brush_radius_world = Some(options.radius);
                }
                let brush = &self.document.brushes()[self.ui.selected];
                let request = self.document.prepare_path_stroke(id, brush, options)?;
                let job =
                    BackgroundJob::start("clay-path-stroke", move |cancel| request.run(&cancel))?;
                self.paths.worker = Some(Worker { job, auto_apply });
                self.ui.status = if auto_apply {
                    "Stroking the entire path… Cancel stops the operation before it commits."
                } else {
                    "Computing path stroke preview…"
                }
                .into();
            }
            PathCommand::Apply => {
                let preview = self.paths.preview.take().ok_or("Preview a stroke first")?;
                let attachment_warning = preview.attachment_warning().map(str::to_owned);
                let path_id = self.viewport.paths.draft.as_ref().map(|path| path.id);
                let result = self.document.apply_path_stroke(preview);
                self.paths.cancel();
                let changed = result?;
                if let Some(path) = path_id.and_then(|id| self.document.path(id)).cloned() {
                    self.viewport.paths.select(path);
                }
                let stale = self
                    .viewport
                    .paths
                    .draft
                    .as_ref()
                    .is_some_and(|p| p.is_stale(self.document.mesh()));
                self.ui.status = if !changed {
                    "The stroke made no change.".into()
                } else if let Some(warning) = attachment_warning {
                    format!("Stroke applied. Guide attachment needs attention: {warning}")
                } else if stale {
                    "Stroke applied. Reproject the saved path before reusing it on the changed surface.".into()
                } else {
                    "Stroke applied. The saved path is ready to use again.".into()
                };
            }
            PathCommand::CancelPreview => {
                let automatic = self.paths.applying_entire_path();
                self.paths.cancel();
                self.ui.status = if automatic {
                    "Entire-path stroke cancelled; geometry unchanged"
                } else {
                    "Brush preview cancelled; geometry unchanged"
                }
                .into();
            }
            PathCommand::Close => {
                self.paths.cancel();
                self.viewport.paths = Default::default();
                self.ui.paths_active = false;
                self.viewport.wait_for_release();
            }
        }
        Ok(())
    }
    pub(super) fn poll_paths(&mut self) {
        let result = self
            .paths
            .worker
            .as_mut()
            .and_then(|worker| worker.job.poll());
        if let Some(result) = result {
            let auto_apply = self.paths.applying_entire_path();
            self.paths.worker = None;
            match result {
                Ok(preview) => {
                    if auto_apply {
                        self.paths.preview = Some(preview);
                        if let Err(error) = self.dispatch_path(PathCommand::Apply) {
                            self.paths.cancel();
                            self.ui.status = error;
                        }
                        self.sync_path_history_state();
                        return;
                    }
                    self.ui.status = format!(
                        "{} brush samples ready. Apply commits one undo step.",
                        preview.sample_count()
                    );
                    if let Some(warning) = preview.attachment_warning() {
                        self.ui
                            .status
                            .push_str(&format!(" Guide attachment needs attention: {warning}"));
                    }
                    self.paths.preview_mesh = Some(Arc::new(preview.mesh().clone()));
                    self.paths.preview = Some(preview);
                }
                Err(error) => self.ui.status = error,
            }
        }
        self.sync_path_history_state();
    }
}

#[cfg(test)]
mod radius_tests {
    use super::*;
    use crate::sculpt::{Vec3, paths::PathPoint};

    fn setup() -> (App, u64) {
        let mut app = App::default();
        let id = app
            .document
            .create_path(
                "Scale reference".into(),
                PathMode::Free,
                [Vec3(-0.4, 0., 0.2), Vec3(0.4, 0., 0.7)]
                    .into_iter()
                    .map(|position| PathPoint {
                        position,
                        normal: Vec3(0., 0., 1.),
                    })
                    .collect(),
                false,
                0.,
            )
            .unwrap();
        app.paths.viewport_size = Some(egui::vec2(800., 600.));
        (app, id)
    }
    #[test]
    fn automatic_size_uses_active_brush_zoom_viewport_and_front_path_depth() {
        let (mut app, id) = setup();
        assert!(app.paths.use_brush_size);
        let initial = app.automatic_path_options(id).unwrap().radius;
        assert!((initial - 60. / 210.).abs() < 1e-6);
        let mut brush = app.document.brushes()[app.ui.selected].clone();
        brush.radius = 90.;
        app.document.update_brush(app.ui.selected, brush).unwrap();
        app.camera.zoom = 2.;
        assert!((app.automatic_path_options(id).unwrap().radius - initial * 0.75).abs() < 1e-6);
        app.paths.viewport_size = Some(egui::vec2(1600., 1200.));
        let orthographic = app.automatic_path_options(id).unwrap().radius;
        app.camera.perspective = true;
        let eye = app.camera.eye_distance();
        let expected = orthographic * (eye - 0.7) / eye;
        assert!((app.automatic_path_options(id).unwrap().radius - expected).abs() < 1e-6);
        // Reversing traversal must not change which side sets the footprint.
        let mut path = app.document.path(id).unwrap().clone();
        path.reverse();
        app.document.update_path(path).unwrap();
        assert!((app.automatic_path_options(id).unwrap().radius - expected).abs() < 1e-6);
    }
    #[test]
    fn custom_automatic_radius_is_explicit_and_pressure_stays_separate() {
        let (mut app, id) = setup();
        app.paths.viewport_size = None;
        assert!(app.automatic_path_options(id).is_err());
        app.paths.use_brush_size = false;
        app.paths.options.radius = 0.37;
        app.paths.options.pressure = 0.42;
        app.paths.options.reverse = true;
        app.paths.options.invert = true;
        assert_eq!(app.automatic_path_options(id).unwrap(), app.paths.options);
        app.camera.zoom = 8.;
        assert_eq!(app.automatic_path_options(id).unwrap(), app.paths.options);
    }
}

//! Keep one cancellable evaluation for the Boolean view. Only Bake edits the scene.
use super::*;
use crate::{
    background_job::BackgroundJob,
    sculpt::{BooleanMethod, Mesh, boolean::evaluate, fusion},
};

#[derive(Default)]
pub(super) struct State {
    revision: Option<u64>,
    group: Option<String>,
    method: BooleanMethod,
    job: Option<BackgroundJob<Mesh>>,
    mesh: Option<Mesh>,
}
#[cfg(test)]
impl State {
    pub(super) fn result(&self) -> Option<&Mesh> {
        self.mesh.as_ref()
    }
}

impl App {
    pub(super) fn toggle_boolean_preview(&mut self) {
        self.viewport.cancel(&mut self.document);
        self.viewport.wait_for_release();
        if !self.ui.boolean_enabled && self.ui.fusion.open {
            self.cancel_fusion();
        }
        self.ui.boolean_enabled = !self.ui.boolean_enabled;
        if !self.ui.boolean_enabled {
            self.clear_boolean_preview();
        }
    }
    fn clear_boolean_preview(&mut self) {
        if let Some(job) = &self.boolean.job {
            job.cancel();
        }
        self.boolean.mesh = None;
        self.boolean.revision = None;
        self.ui.boolean_ready = false;
        self.viewport.boolean_current = false;
        self.viewport.boolean_display = None;
        self.boolean.group = None;
    }
    pub(super) fn boolean_preview(&mut self, ctx: &egui::Context) {
        let rev = self.document.display_revision();
        if self.ui.boolean_enabled && self.boolean.revision != Some(rev) {
            let group = &self
                .document
                .model(self.document.active_model_index())
                .unwrap()
                .metadata
                .group;
            let method = self.document.boolean_method();
            let switched =
                self.boolean.group.as_ref() != Some(group) || self.boolean.method != method;
            if switched || self.viewport.boolean_display.is_none() {
                self.boolean.group = Some(group.clone());
                self.boolean.method = method;
                self.viewport
                    .set_boolean_result(&self.document, &Mesh::default());
            }
            if (switched || !self.document.stroke_active())
                && let Some(job) = &self.boolean.job
            {
                job.cancel();
            }
            self.boolean.mesh = None;
            self.ui.boolean_ready = false;
            self.viewport.boolean_current = false;
            self.ui.boolean_notice = "Updating Boolean result…".into();
        }
        if let Some(result) = self.boolean.job.as_mut().and_then(BackgroundJob::poll) {
            let cancelled = self
                .boolean
                .job
                .take()
                .is_some_and(|job| job.is_cancelled());
            let current = self.boolean.revision == Some(rev);
            if self.ui.boolean_enabled
                && !cancelled
                && (current || (self.document.stroke_active() && result.is_ok()))
            {
                match result {
                    Ok(mesh) => {
                        self.viewport.set_boolean_result(&self.document, &mesh);
                        self.viewport.boolean_current = current;
                        self.ui.boolean_ready = current && !mesh.triangles.is_empty();
                        self.ui.boolean_notice = if !current {
                            "Updating Boolean result…".into()
                        } else if self.ui.boolean_ready {
                            format!("Boolean result · {} triangles", mesh.triangles.len())
                        } else {
                            "Boolean result · Empty".into()
                        };
                        self.boolean.mesh = current.then_some(mesh);
                    }
                    Err(e) => {
                        self.viewport
                            .set_boolean_result(&self.document, &Mesh::default());
                        self.ui.boolean_notice = e;
                    }
                }
            }
        }
        if self.ui.boolean_enabled
            && self.boolean.revision != Some(rev)
            && self.boolean.job.is_none()
        {
            self.boolean.revision = Some(rev);
            let resolution = self.document.boolean_resolution();
            let method = self.document.boolean_method();
            match self.document.boolean_operands().and_then(|operands| {
                BackgroundJob::start("clay-boolean-stack", move |cancel| match method {
                    BooleanMethod::Voxel => evaluate(&operands, resolution, cancel),
                    BooleanMethod::Surface => fusion::boolean::evaluate(&operands, cancel),
                })
            }) {
                Ok(job) => self.boolean.job = Some(job),
                Err(e) => {
                    self.viewport
                        .set_boolean_result(&self.document, &Mesh::default());
                    self.ui.boolean_notice = e;
                }
            }
        }
        if self.boolean.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
    }
    pub(super) fn bake_boolean_preview(&mut self) -> Result<(), String> {
        if !self.ui.boolean_enabled
            || !self.ui.boolean_ready
            || self.boolean.revision != Some(self.document.display_revision())
        {
            return Err("Wait for a non-empty, current Boolean result".into());
        }
        let mesh = self.boolean.mesh.take().ok_or("Preview the stack first")?;
        self.viewport.cancel(&mut self.document);
        if let Err(reason) = self
            .document
            .bake_boolean(mesh, self.document.display_revision())
        {
            self.boolean.revision = None;
            self.ui.boolean_ready = false;
            return Err(reason);
        }
        self.ui.boolean_enabled = false;
        self.clear_boolean_preview();
        self.ui.status = "Baked Boolean result. Undo restores the editable stack.".into();
        Ok(())
    }
}

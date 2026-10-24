//! Private, cancellable projection drafts; only Apply changes the document.
use super::*;
use crate::background_job::BackgroundJob;
use crate::sculpt::{Mesh, ProjectionOptions, ProjectionReport};
use std::sync::Arc;

pub(super) struct ProjectionDraft {
    pub mesh: Arc<Mesh>,
    pub revision: u64,
    pub settings: crate::diagnostics::EditSettings,
}
struct Worker {
    job: BackgroundJob<(Mesh, ProjectionReport)>,
    revision: u64,
    settings: crate::diagnostics::EditSettings,
}
#[derive(Default)]
pub(super) struct ProjectionState {
    worker: Option<Worker>,
    pub draft: Option<ProjectionDraft>,
}
impl App {
    pub(super) fn open_projection(&mut self) -> Result<(), String> {
        self.document.require_projection_target()?;
        if self.mesh_job.is_some() {
            return Err("Wait for the mesh operation first".into());
        }
        self.viewport.cancel(&mut self.document);
        self.ui.projection.open = true;
        if self
            .ui
            .projection
            .source
            .is_none_or(|source| self.document.projection_source(source).is_err())
        {
            self.ui.projection.source = self
                .document
                .merge_down_target()
                .map(crate::document::ProjectionSource::Submodel);
        }
        self.ui.status = "Choose the reference, then preview the active target".into();
        Ok(())
    }
    pub(super) fn clear_projection_preview(&mut self) {
        self.projection = Default::default();
        self.viewport.session.projection_preview = None;
        self.ui.projection.report = None;
        self.ui.projection.busy = false;
    }
    pub(super) fn cancel_projection(&mut self) {
        self.clear_projection_preview();
        self.ui.projection.open = false;
        self.viewport.wait_for_release();
    }
    pub(super) fn start_projection(&mut self) -> Result<(), String> {
        if !self.ui.projection.open || self.mesh_job.is_some() {
            return Err("Open Project details when no mesh job is running".into());
        }
        let source = self
            .ui
            .projection
            .source
            .ok_or("Choose a reference submodel or subdivision level")?;
        let reference = source;
        let source = self.document.projection_source(source)?;
        let options: ProjectionOptions = self.ui.projection.options;
        options.validate()?;
        self.clear_projection_preview();
        self.viewport.cancel(&mut self.document);
        let target = self.document.mesh().clone();
        let job = BackgroundJob::start("clay-detail-projection", move |cancel| {
            target.project_details(&source, options, &cancel)
        })?;
        let mut settings = self.document.capture_edit_settings();
        let view = settings
            .view_input
            .get_or_insert_with(|| serde_json::json!({}));
        view["detail_projection"] = serde_json::json!({
            "reference": match reference {
                crate::document::ProjectionSource::Submodel(i) => serde_json::json!({"submodel_index": i}),
                crate::document::ProjectionSource::Level(i) => serde_json::json!({"subdivision_level": i}),
            },
            "target_submodel_index": self.document.active_model_index(),
            "strength": options.strength,
            "max_distance": options.max_distance,
        });
        self.projection.worker = Some(Worker {
            job,
            revision: self.document.display_revision(),
            settings,
        });
        self.ui.projection.busy = true;
        self.ui.status = "Computing detail projection…".into();
        Ok(())
    }
    pub(super) fn poll_projection(&mut self) {
        let result = self.projection.worker.as_mut().and_then(|w| w.job.poll());
        if let Some(result) = result {
            let worker = self.projection.worker.take().unwrap();
            self.ui.projection.busy = false;
            if worker.revision != self.document.display_revision() {
                self.ui.status = "Scene changed; preview the projection again".into();
                return;
            }
            match result {
                Ok((mesh, report)) => {
                    self.ui.status = report.summary();
                    self.ui.projection.report = Some(report);
                    self.projection.draft = Some(ProjectionDraft {
                        mesh: Arc::new(mesh),
                        revision: worker.revision,
                        settings: worker.settings.clone(),
                    });
                }
                Err(error) => self.ui.status = error,
            }
        }
    }
    pub(super) fn apply_projection(&mut self) -> Result<(), String> {
        let draft = self
            .projection
            .draft
            .take()
            .ok_or("Preview the projection before applying it")?;
        let result = self.document.apply_projection(
            (*draft.mesh).clone(),
            draft.revision,
            Some(draft.settings),
        );
        self.cancel_projection();
        result
    }
}

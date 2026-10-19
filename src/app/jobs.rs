//! Coordinate background geometry jobs and transactional completion.
use super::*;

impl App {
    pub(super) fn start_mesh_job(&mut self, operation: Operation) -> Result<(), String> {
        if matches!(operation, Operation::ImportObj(_)) {
            if self.document.model_count() >= 64 {
                return Err("Submodels are limited to 64".into());
            }
        } else {
            self.document.require_sculpt_mesh()?;
        }
        if self.mesh_job.is_some() {
            return Err("A mesh operation is already running".into());
        }
        let was_dragging = self.viewport.gesture.is_some();
        if matches!(operation, Operation::SlicePath(..)) {
            self.viewport.pause_slice_path(&mut self.document);
        } else {
            self.viewport.cancel(&mut self.document);
        }
        self.job_settings = Some(self.document.capture_edit_settings());
        self.mesh_job = Some(MeshJob::start(
            self.document.geometry_snapshot(),
            operation.clone(),
        )?);
        if was_dragging {
            self.viewport.wait_for_release();
        }
        self.ui.mesh_busy = true;
        self.ui.status = format!("{}…", operation.label());
        Ok(())
    }
    pub(super) fn cancel_mesh_job(&mut self) {
        if self.mesh_job.take().is_some() {
            self.ui.status = "Mesh operation cancelled; original mesh kept".into();
        }
        self.ui.mesh_busy = false;
        self.job_settings = None;
    }
    pub(super) fn poll_mesh_job(&mut self) {
        if let Some(result) = self.mesh_job.as_ref().and_then(MeshJob::poll) {
            let operation = self
                .mesh_job
                .take()
                .expect("completed job")
                .operation
                .clone();
            let label = operation.history_label();
            self.ui.mesh_busy = false;
            let settings = self.job_settings.take();
            match result.and_then(|geometry| {
                if matches!(operation, Operation::Edges(_))
                    && geometry == self.document.geometry_snapshot()
                {
                    Ok(())
                } else {
                    self.document
                        .replace_geometry_recorded(geometry, label, settings)
                }
            }) {
                Ok(()) => {
                    if matches!(
                        operation,
                        Operation::ImportObj(_) | Operation::Slice(..) | Operation::SlicePath(..)
                    ) {
                        self.ui.show_polygroups = true;
                    }
                    if matches!(operation, Operation::ImportObj(_)) {
                        self.ui.selected_vertex_group = None;
                        self.viewport
                            .reset_transform_pivot(&self.document, self.ui.transform_mode);
                    }
                    self.ui.status = format!(
                        "{} complete: {} vertices, {} triangles",
                        operation.history_label(),
                        self.document.mesh().vertices.len(),
                        self.document.mesh().triangles.len()
                    )
                }
                Err(error) => {
                    self.diagnostic_warning(&format!(
                        "{} failed; detailed error text withheld",
                        operation.history_label()
                    ));
                    self.ui.status = error;
                    if matches!(operation, Operation::Remesh)
                        && self.document.mesh_settings().auto_remesh
                        && !self.document.mesh().has_hidden_faces()
                    {
                        let resolution = self.document.mesh_settings().resolution;
                        let _ = self.document.set_remesh_settings(resolution, false);
                    }
                }
            }
        }
    }
}

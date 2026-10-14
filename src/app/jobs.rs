//! Coordinate background geometry jobs and transactional completion.
use super::*;

impl App {
    pub(super) fn start_mesh_job(&mut self, operation: Operation) -> Result<(), String> {
        self.document.require_sculpt_mesh()?;
        if self.mesh_job.is_some() {
            return Err("A mesh operation is already running".into());
        }
        let was_dragging = self.viewport.gesture.is_some();
        if matches!(operation, Operation::SlicePath(..)) {
            self.viewport.pause_slice_path(&mut self.document);
        } else {
            self.viewport.cancel(&mut self.document);
        }
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
            match result.and_then(|geometry| self.document.replace_geometry_named(geometry, label))
            {
                Ok(()) => {
                    if matches!(operation, Operation::Slice(..) | Operation::SlicePath(..)) {
                        self.ui.show_polygroups = true;
                    }
                    self.ui.status = format!(
                        "Mesh updated: {} vertices, {} faces",
                        self.document.mesh().vertices.len(),
                        self.document.mesh().triangles.len()
                    )
                }
                Err(error) => {
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

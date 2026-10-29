//! Mesh command execution after shared availability and edit-boundary checks.
use super::*;

impl App {
    pub(super) fn dispatch_mesh(&mut self, command: Command) -> Result<(), String> {
        match command {
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
            Command::SetRemeshResolution(resolution) => {
                self.document.set_remesh_resolution(resolution)
            }
            Command::SetRemeshTopology(topology) => self.document.set_remesh_topology(topology),
            Command::SetQuadRemeshAlgorithm(algorithm) => {
                self.document.set_quad_remesh_algorithm(algorithm)
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
                self.reset_camera_view();
                Ok(())
            }
            Command::SetPerspective(enabled) => {
                self.set_camera_projection(enabled, self.camera.field_of_view);
                Ok(())
            }
            Command::SetFieldOfView(degrees) => {
                if degrees.is_finite() {
                    self.set_camera_projection(self.camera.perspective, degrees);
                }
                Ok(())
            }
            Command::SetSymmetry(symmetry) => {
                let modeling_drag = self.viewport.modeling.dragging();
                self.viewport.cancel(&mut self.document);
                if modeling_drag {
                    self.viewport.wait_for_release();
                }
                self.document.set_symmetry(symmetry)
            }
            _ => unreachable!("command routed to the wrong feature handler"),
        }
    }
}

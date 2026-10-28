use super::*;
use crate::diagnostics::{
    BundleWriter, EditSettings, Environment, Manifest, Retention, Stage, Transition,
};
use std::path::Path;

impl Document {
    pub(crate) fn set_diagnostic_context(&mut self, context: serde_json::Value) {
        self.diagnostic_context = Some(context);
    }
    pub(crate) fn capture_edit_settings(&self) -> EditSettings {
        let selected = self
            .diagnostic_context
            .as_ref()
            .and_then(|v| v["selected_brush"].as_u64());
        EditSettings {
            brushes: self.brushes.clone(),
            symmetry: self.symmetry.clone(),
            topology: topology_settings(&self.geometry),
            active_brush: selected.and_then(|i| self.brushes.get(i as usize)).cloned(),
            view_input: self.diagnostic_context.clone(),
            first_dab: None,
        }
    }
    pub(super) fn record_edit(&mut self, label: &'static str) {
        self.record_edit_owned(self.geometry.clone(), label);
    }
    pub(super) fn record_edit_owned(&mut self, geometry: Geometry, label: &'static str) {
        let mut settings = self.capture_edit_settings();
        settings.topology = topology_settings(&geometry);
        self.history.record_owned(geometry, label, Some(settings));
    }
    pub fn set_history_retention(&mut self, retention: Retention) -> Result<(), String> {
        retention.validate()?;
        self.history.retention = retention;
        self.history.enforce_limits();
        Ok(())
    }
    pub fn history_retention(&self) -> Retention {
        self.history.retention
    }

    /// Export borrows the document. It never ends an edit, marks saved or traverses undo/redo.
    /// The destination must be new; failure removes only the directory this call created.
    pub fn export_diagnostics(
        &self,
        directory: &Path,
        environment: Environment,
    ) -> Result<(), String> {
        self.export_diagnostics_bounded(
            directory,
            environment,
            crate::diagnostics::MAX_BUNDLE_BYTES,
        )
    }
    pub(crate) fn export_diagnostics_bounded(
        &self,
        directory: &Path,
        environment: Environment,
        limit: usize,
    ) -> Result<(), String> {
        if self.edit_in_progress() {
            return Err("Finish the current edit before exporting diagnostics".into());
        }
        let manifest = self.diagnostic_manifest(environment);
        manifest.validate()?;
        std::fs::create_dir(directory)
            .map_err(|e| format!("Choose a new diagnostic folder: {e}"))?;
        let result = (|| {
            let mut writer = BundleWriter::new(limit);
            writer.text(&directory.join("README.txt"), &manifest.summary())?;
            writer.json(&directory.join("manifest.json"), &manifest)?;
            for (i, (geometry, settings)) in self.diagnostic_stages().into_iter().enumerate() {
                let mut project = if i == manifest.current_stage {
                    self.project()
                } else {
                    Project {
                        paths: geometry.paths.clone(),
                        version: Project::VERSION,
                        alphas: Vec::new(),
                        stencil: None,
                        references: Vec::new(),
                        scene: geometry.scene.clone(),
                        model_name: geometry.name.clone(),
                        submodels: geometry.submodels.clone(),
                        mesh: geometry.mesh.clone(),
                        mesh_settings: geometry.settings.clone(),
                        brushes: settings.map_or_else(
                            || vec![Brush::new(Kind::Standard)],
                            |s| s.brushes.clone(),
                        ),
                        symmetry: settings.map_or_else(Symmetry::default, |s| s.symmetry.clone()),
                    }
                };
                for mesh in std::iter::once(&mut project.mesh)
                    .chain(project.mesh_settings.lower.iter_mut())
                    .chain(project.mesh_settings.upper.iter_mut())
                    .chain(project.submodels.iter_mut().flat_map(|m| {
                        std::iter::once(&mut m.mesh)
                            .chain(m.mesh_settings.lower.iter_mut())
                            .chain(m.mesh_settings.upper.iter_mut())
                    }))
                {
                    for texture in &mut mesh.uv.textures {
                        texture.exchange = None;
                    }
                }
                project.validate()?;
                writer.json(&directory.join(&manifest.stages[i].file), &project)?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(directory);
        }
        result
    }
    fn diagnostic_stages(&self) -> Vec<(&Geometry, Option<&EditSettings>)> {
        self.history
            .undo
            .iter()
            .map(|e| (&e.geometry, e.settings.as_ref()))
            .chain(std::iter::once((&self.geometry, None)))
            .chain(
                self.history
                    .redo
                    .iter()
                    .rev()
                    .map(|e| (&e.geometry, e.settings.as_ref())),
            )
            .collect()
    }
    pub fn diagnostic_manifest(&self, environment: Environment) -> Manifest {
        let current_stage = self.history.undo.len();
        Manifest {
            schema_version: crate::diagnostics::SCHEMA_VERSION, project_version: Project::VERSION,
            environment, current_stage, current_settings: self.capture_edit_settings(),
            retention: self.history.retention, retained_estimated_bytes: self.history.retained_bytes(),
            evicted_edits: self.history.evicted, discarded_redo_edits: self.history.discarded_redo,
            stages: self.diagnostic_stages().iter().enumerate().map(|(i, (g, s))| Stage {
                index: i, file: format!("stage-{i:03}.clay"),
                position: if i < current_stage { "undo" } else if i == current_stage { "current" } else { "redo" }.into(),
                vertices: g.mesh.vertices.len() + g.submodels.iter().map(|m| m.mesh.vertices.len()).sum::<usize>(),
                triangles: g.mesh.triangles.len() + g.submodels.iter().map(|m| m.mesh.triangles.len()).sum::<usize>(),
                settings_basis: if i == current_stage { "current project settings" } else if s.is_none() { "historical brush/symmetry unavailable; defaults for opening" } else if i < current_stage { "brush/symmetry at start of outgoing edit" } else { "brush/symmetry at start of incoming edit; later changes unavailable" }.into(),
            }).collect(),
            transitions: self.history.undo.iter().chain(self.history.redo.iter().rev()).enumerate().map(|(i, e)| Transition {
                id: e.id, from: i, to: i+1, label: e.label.into(), settings_at_edit_start: e.settings.clone(),
            }).collect(),
            missing_evidence: vec![
                "Retained branch only, ordered by edits, not an event log of undo/redo clicks. New/Open resets history; evicted and discarded branches cannot be reconstructed.".into(),
                "Brush/symmetry/view changes are outside geometry history. Settings are captured at edit boundaries going forward, not at every settings change. Historical geometry/topology settings are in stage files.".into(),
                "Full pointer/pressure samples, settings changes during a continuous edit, timestamps and replay are unavailable. First-dab pressure/invert and effective brush are recorded for sculpt strokes.".into(),
                "Historical reference images are not recorded; only the current project includes references. Camera/input can be unavailable for edits made without the UI.".into(),
                "No full warning/error log, credentials, environment variables, project path or device paths are collected. Only available safe warning categories are included; exact OS version is unavailable.".into(),
                "Retention estimates include geometry and edit settings with shared image buffers counted once, not allocator overhead; the live project is separate. Export is capped at 512 MiB total and 128 MiB per file.".into(),
            ],
        }
    }
}

fn topology_settings(geometry: &Geometry) -> MeshSettings {
    MeshSettings {
        lower: Vec::new(),
        upper: Vec::new(),
        primitive: geometry.settings.primitive.clone(),
        dynamic: geometry.settings.dynamic,
        resolution: geometry.settings.resolution,
        remesh_topology: geometry.settings.remesh_topology,
        quad_remesh_algorithm: geometry.settings.quad_remesh_algorithm,
        voxel_skin_project: geometry.settings.voxel_skin_project,
        adaptive: geometry.settings.adaptive.clone(),
    }
}

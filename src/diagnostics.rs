//! Local, versioned diagnostic bundles. Stages are independently loadable projects.
pub use crate::history::Retention;
use crate::sculpt::{Brush, Symmetry};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_BUNDLE_BYTES: usize = 512 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct EditSettings {
    pub brushes: Vec<Brush>,
    pub symmetry: Symmetry,
    /// Mesh policy at edit start, excluding geometry in subdivision levels.
    pub topology: crate::sculpt::MeshSettings,
    pub active_brush: Option<Brush>,
    /// An allowlisted UI snapshot at the edit boundary; None for headless callers.
    pub view_input: Option<serde_json::Value>,
    /// First effective dab only. This is not a recording of the stroke.
    pub first_dab: Option<serde_json::Value>,
}
impl EditSettings {
    fn summary(&self) -> String {
        let mut text = String::new();
        if let Some(b) = &self.active_brush {
            let alpha = match &b.alpha {
                crate::sculpt::Alpha::Soft => "Soft".into(),
                crate::sculpt::Alpha::Hard => "Hard".into(),
                crate::sculpt::Alpha::Square => "Square".into(),
                crate::sculpt::Alpha::Image { width, height, .. } => {
                    format!("Image {width} × {height}")
                }
            };
            text.push_str(&format!("       Brush {:?}, preset {:?}, alpha {}, radius {}, strength {}, stroke {:?}, pressure radius/strength {}/{}, pressure curve {}\n", b.kind, b.name, alpha, b.radius, b.strength, b.effective_stroke_mode(), b.pressure_radius, b.pressure, b.pressure_curve));
        } else {
            text.push_str("       Active brush unavailable; recorded library is in manifest\n");
        }
        text.push_str(&format!(
            "       Topology: dynamic {}, remesh resolution {}, auto remesh {}, adaptive {}\n",
            self.topology.dynamic,
            self.topology.resolution,
            self.topology.auto_remesh,
            self.topology.adaptive.enabled
        ));
        text.push_str(&format!(
            "       Symmetry enabled: {}; view/input: {}\n",
            self.symmetry.enabled,
            if self.view_input.is_some() {
                "recorded"
            } else {
                "unavailable"
            }
        ));
        text
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        // Count without allocating an extra serialized copy of image alphas.
        let mut writer = Counter(0);
        let _ = serde_json::to_writer(&mut writer, self);
        writer.0
    }
}
struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    pub app_version: String,
    pub build: String,
    pub os: String,
    pub architecture: String,
    pub renderer: Option<String>,
    pub backend: Option<String>,
    pub gpu: Option<String>,
    pub driver: Option<String>,
    pub input: Option<String>,
    /// Deliberately limited to safe diagnostic categories, never raw error paths.
    pub warnings: Vec<String>,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION").into(),
            build: env!("CLAY_BUILD_ID").into(),
            os: std::env::consts::OS.into(),
            architecture: std::env::consts::ARCH.into(),
            renderer: None,
            backend: None,
            gpu: None,
            driver: None,
            input: None,
            warnings: Vec::new(),
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Stage {
    pub index: usize,
    pub file: String,
    pub position: String,
    pub vertices: usize,
    pub triangles: usize,
    pub settings_basis: String,
}
#[derive(Serialize, Deserialize)]
pub struct Transition {
    pub id: u64,
    pub from: usize,
    pub to: usize,
    pub label: String,
    pub settings_at_edit_start: Option<EditSettings>,
}
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub project_version: u32,
    pub environment: Environment,
    pub current_stage: usize,
    pub current_settings: EditSettings,
    pub retention: Retention,
    pub retained_estimated_bytes: usize,
    pub evicted_edits: usize,
    pub discarded_redo_edits: usize,
    pub stages: Vec<Stage>,
    pub transitions: Vec<Transition>,
    pub missing_evidence: Vec<String>,
}
impl Manifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION
            || self.project_version != crate::sculpt::Project::VERSION
        {
            return Err("Unsupported diagnostic or project schema version".into());
        }
        self.retention.validate()?;
        if self.stages.is_empty()
            || self.stages.len() > 257
            || self.current_stage >= self.stages.len()
            || self.transitions.len() + 1 != self.stages.len()
            || self.transitions.len() > self.retention.edits
            || self.retained_estimated_bytes > self.retention.memory_mib * 1024 * 1024
        {
            return Err("Invalid diagnostic history ordering".into());
        }
        for (i, stage) in self.stages.iter().enumerate() {
            let position = if i < self.current_stage {
                "undo"
            } else if i == self.current_stage {
                "current"
            } else {
                "redo"
            };
            if stage.index != i
                || stage.file != format!("stage-{i:03}.clay")
                || stage.position != position
            {
                return Err("Invalid diagnostic stage path or index".into());
            }
        }
        let mut previous_id = 0;
        for (i, transition) in self.transitions.iter().enumerate() {
            if transition.from != i || transition.to != i + 1 || transition.id <= previous_id {
                return Err("Invalid diagnostic transition ordering".into());
            }
            previous_id = transition.id;
        }
        Ok(())
    }
    pub fn summary(&self) -> String {
        let e = &self.environment;
        let mut text = format!(
            "Clay Form diagnostic bundle — schema {}\n\nApp {} · build {}\nOS {} · {}\nRenderer: {} · backend: {}\nGPU: {} · driver: {}\nInput: {}\n\nCurrent stage: {} of {} (zero-based)\nRetention: {} edits, {} MiB; estimated retained bytes: {}\nEvicted: {}; redo edits discarded by new edits: {}\n",
            self.schema_version,
            e.app_version,
            e.build,
            e.os,
            e.architecture,
            e.renderer.as_deref().unwrap_or("unavailable"),
            e.backend.as_deref().unwrap_or("unavailable"),
            e.gpu.as_deref().unwrap_or("unavailable"),
            e.driver.as_deref().unwrap_or("unavailable"),
            e.input.as_deref().unwrap_or("unavailable"),
            self.current_stage,
            self.stages.len(),
            self.retention.edits,
            self.retention.memory_mib,
            self.retained_estimated_bytes,
            self.evicted_edits,
            self.discarded_redo_edits
        );
        text.push_str("\nCurrent settings (separate from history):\n");
        text.push_str(&self.current_settings.summary());
        text.push_str("\nStages in chronological order:\n");
        for stage in &self.stages {
            text.push_str(&format!(
                "  {} [{}]: {} vertices, {} triangles; {}\n",
                stage.file, stage.position, stage.vertices, stage.triangles, stage.settings_basis
            ));
            if let Some(t) = self.transitions.get(stage.index) {
                text.push_str(&format!("    -> edit {}: {}\n", t.id, t.label));
                if let Some(settings) = &t.settings_at_edit_start {
                    text.push_str(&settings.summary());
                } else {
                    text.push_str("       Historical edit settings unavailable\n");
                }
            }
        }
        text.push_str("\nMissing evidence and interpretation:\n");
        for missing in &self.missing_evidence {
            text.push_str(&format!("- {missing}\n"));
        }
        for warning in &e.warnings {
            text.push_str(&format!("- Warning: {warning}\n"));
        }
        text.push_str("\nInspect or restore: use the Open button on any stage-NNN.clay. Save under a new name to keep the export unchanged. This opens one stage and resets live undo history; it does not import a replayable session. manifest.json associates each edit's settings with its from/to stages. Topology, remesh, adaptive settings and all stored levels are embedded in each stage.\n\nThe current stage contains the complete current project, including embedded reference images and user-authored names. Earlier stages contain geometry with the indicated edit settings; references are unavailable there. Review content before sharing. Export is local and sends nothing.\n");
        text
    }
}

/// Read and validate a manifest without trusting stage filenames from external bundles.
pub fn load_manifest(directory: &Path) -> Result<Manifest, String> {
    let manifest: Manifest =
        crate::persistence::read_json(&directory.join("manifest.json").to_string_lossy())?;
    manifest.validate()?;
    Ok(manifest)
}

/// Streaming per-file and aggregate limits also bound output on large image/mesh payloads.
pub(crate) struct BundleWriter {
    remaining: usize,
}
impl BundleWriter {
    pub fn new(limit: usize) -> Self {
        Self {
            remaining: limit.min(MAX_BUNDLE_BYTES),
        }
    }
    pub fn json(&mut self, path: &Path, value: &impl Serialize) -> Result<(), String> {
        self.write(path, |writer| {
            serde_json::to_writer(writer, value).map_err(|e| e.to_string())
        })
    }
    pub fn text(&mut self, path: &Path, text: &str) -> Result<(), String> {
        self.write(path, |writer| {
            writer.write_all(text.as_bytes()).map_err(|e| e.to_string())
        })
    }
    fn write(
        &mut self,
        path: &Path,
        serialize: impl FnOnce(&mut LimitedWriter) -> Result<(), String>,
    ) -> Result<(), String> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        let mut writer = LimitedWriter {
            file: std::io::BufWriter::new(file),
            remaining: self.remaining.min(MAX_FILE_BYTES),
            written: 0,
        };
        serialize(&mut writer)?;
        writer.flush().map_err(|e| e.to_string())?;
        writer
            .file
            .get_ref()
            .sync_all()
            .map_err(|e| e.to_string())?;
        self.remaining -= writer.written;
        Ok(())
    }
}
struct LimitedWriter {
    file: std::io::BufWriter<std::fs::File>,
    remaining: usize,
    written: usize,
}
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(std::io::Error::other(
                "Diagnostic export exceeds 128 MiB per file or 512 MiB total; reduce history retention",
            ));
        }
        self.file.write_all(bytes)?;
        self.remaining -= bytes.len();
        self.written += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests;

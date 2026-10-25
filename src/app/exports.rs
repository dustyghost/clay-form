use super::*;
use crate::persistence::material_export::{ExportFormat, prepare_export};
impl App {
    pub(super) fn browse_export(&mut self, format: ExportFormat) {
        if self.export_job.is_some() {
            self.ui.status = "An export is already running".into();
            return;
        }
        let path = std::path::Path::new(&self.ui.path).with_extension(format.extension());
        self.browse_files(path.to_string_lossy().into(), true, false, None);
        if let Some(picker) = &mut self.ui.file_picker {
            picker.export = Some(format);
        }
    }
    pub(super) fn start_export(
        &mut self,
        path: std::path::PathBuf,
        format: ExportFormat,
        include_lights: bool,
    ) -> Result<(), String> {
        if self.export_job.is_some() {
            return Err("An export is already running".into());
        }
        let project = self.document.project();
        let lighting = (include_lights && format == ExportFormat::Blender).then(|| {
            crate::persistence::material_export::ExportLighting::capture(
                &self.ui.preferences.lighting,
                self.camera,
            )
        });
        self.export_job = Some(crate::background_job::BackgroundJob::start(
            "material-export",
            move |cancel| {
                let bytes = prepare_export(&project, format, lighting.as_ref(), cancel)?;
                Ok((path, bytes))
            },
        )?);
        self.ui.export_busy = true;
        self.ui.status = format!("{} in progress…", format.title());
        Ok(())
    }
    pub(super) fn poll_export(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.export_job else {
            return;
        };
        if let Some(result) = job.poll() {
            self.export_job = None;
            self.ui.export_busy = false;
            self.ui.status = match result.and_then(|(path, bytes)| {
                bytes.write(&path)?;
                Ok(path)
            }) {
                Ok(path) => format!("Exported {}", path.display()),
                Err(e) => e,
            };
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

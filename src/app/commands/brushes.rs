//! Brushes command execution after shared availability and edit-boundary checks.
use super::*;

impl App {
    pub(super) fn dispatch_brushes(
        &mut self,
        command: Command,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        match command {
            Command::AddBrush(kind) => self
                .document
                .append_brush(crate::sculpt::Brush::new(kind))
                .map(|index| self.ui.select_brush(index, &self.document)),
            Command::AddAlphaDragBrush => self
                .document
                .append_brush(crate::sculpt::Brush::alpha_drag())
                .map(|index| self.ui.select_brush(index, &self.document)),
            Command::DuplicateBrush(index) => self
                .document
                .duplicate_brush(index)
                .map(|index| self.ui.select_brush(index, &self.document)),
            Command::UpdateBrush(index, brush) => {
                let entering_paint = brush.kind == crate::sculpt::Kind::Paint
                    && self
                        .document
                        .brushes()
                        .get(index)
                        .is_some_and(|b| b.kind != crate::sculpt::Kind::Paint);
                self.document.update_brush(index, brush).map(|()| {
                    if entering_paint && index == self.ui.selected {
                        self.ui.show_polygroups = false;
                    }
                })
            }
            Command::SaveAlpha(index, asset) => self.document.save_alpha(index, asset).map(|i| {
                self.ui.alphas.selected = Some(crate::ui::alphas::Selection::Project(i));
                self.ui.alphas.draft = None;
                self.ui.status =
                    "Alpha saved in project library; choose Use on brush or Place stencil".into();
            }),
            Command::RemoveAlpha(index) => self
                .document
                .remove_alpha(index)
                .map(|()| self.ui.alphas.selected = None),
            Command::UseAlpha(index, brush) => self
                .document
                .alphas()
                .get(index)
                .cloned()
                .ok_or("Alpha no longer exists".to_string())
                .and_then(|asset| self.document.set_alpha(brush, asset.image)),
            Command::ExportAlpha(index, replace) => {
                let path = self.ui.alphas.export_path.clone();
                if std::path::Path::new(&path).exists() && !replace {
                    self.ui.alphas.overwrite = Some((index, path));
                    Ok(())
                } else {
                    self.document
                        .alphas()
                        .get(index)
                        .ok_or("Alpha no longer exists".to_string())
                        .and_then(|asset| persistence::save_alpha(&path, &asset.image))
                        .map(|()| {
                            self.ui.alphas.overwrite = None;
                            self.ui.status = format!("Exported 16-bit alpha: {path}");
                        })
                }
            }
            Command::SetStencil(stencil) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_stencil(stencil)
            }
            Command::AttachStencil => {
                self.viewport.cancel(&mut self.document);
                self.viewport.session.attach_stencil = true;
                Ok(())
            }
            Command::SavePreset(index) => {
                persistence::save_brush(&self.ui.preset_path, &self.document.brushes()[index])
                    .map(|()| self.ui.status = "Brush saved".into())
            }
            Command::CaptureBrushIcon(index) => {
                self.viewport.cancel(&mut self.document);
                crate::brush_icon::capture(self.document.mesh(), &self.camera).and_then(|icon| {
                    let brush = self
                        .document
                        .brushes()
                        .get(index)
                        .ok_or("Brush no longer exists")?;
                    self.ui.set_icon_shot(ctx, index, brush, icon);
                    Ok(())
                })
            }
            Command::ExportIconImage(path, icon) => image::save_buffer_with_format(
                &path,
                &icon.rgba,
                icon.size as u32,
                icon.size as u32,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .map_err(|e| e.to_string())
            .map(|()| self.ui.status = format!("Saved square capture: {path}")),
            Command::ExportBrushLibrary => {
                persistence::save_brush_library(&self.ui.library_path, self.document.brushes())
                    .map(|()| self.ui.status = "Brush library exported".into())
            }
            _ => unreachable!("command routed to the wrong feature handler"),
        }
    }
}

use super::*;
impl App {
    pub(super) fn arrange_references(&mut self, active: bool) {
        self.viewport.finish_transform_drag(&mut self.document);
        self.viewport.wait_for_release();
        self.ui.arrange_references = active;
        self.ui.status = if active {
            "Arrange references · drag images or their lower-right corners · Done / Esc to sculpt"
        } else {
            "Sculpt mode · reference images are passive"
        }
        .into();
        if active {
            self.ui.references_visible = true;
            self.ui.slice_active = false;
            self.ui.selection_tool = None;
            self.ui.modeling.active = false;
            self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
        }
    }
    pub(super) fn add_reference(&mut self, file: egui::DroppedFile) -> Result<(), String> {
        if self.document.references().len() >= crate::reference::MAX_REFERENCES {
            return Err("Keep up to 8 reference images per project".into());
        }
        let image = if let Some(path) = file.path {
            crate::reference::ReferenceImage::load(&path)?
        } else if let Some(bytes) = file.bytes {
            crate::reference::ReferenceImage::from_bytes(&file.name, &bytes)?
        } else {
            return Err("Drop a PNG or JPEG file, or enter its path in References".into());
        };
        self.document.add_reference(image)?;
        self.arrange_references(true);
        self.ui.selected_reference = self.document.references().len() - 1;
        self.ui.status = "Reference added · drag to position, then choose Done to sculpt".into();
        Ok(())
    }
}

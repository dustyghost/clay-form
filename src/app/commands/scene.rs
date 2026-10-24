//! Scene command execution after shared availability and edit-boundary checks.
use super::*;

impl App {
    pub(super) fn dispatch_scene(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::EditSubmodel(index, name, metadata) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_model_metadata(index, name, metadata)
            }
            Command::ReorderSubmodel(from, to) => {
                self.viewport.cancel(&mut self.document);
                self.document.reorder_model(from, to)
            }
            Command::AddGroup(name) => {
                self.viewport.cancel(&mut self.document);
                self.document.add_group(name)
            }
            Command::RemoveGroup(name) => {
                self.viewport.cancel(&mut self.document);
                self.document.remove_group(&name)
            }
            Command::ReorderGroup(from, to) => {
                self.viewport.cancel(&mut self.document);
                self.document.reorder_group(from, to)
            }
            Command::GroupVisible(name, visible) => {
                self.viewport.cancel(&mut self.document);
                self.document.set_group_visible(&name, visible)
            }
            Command::SelectSubmodel(index) => {
                self.viewport.cancel(&mut self.document);
                self.document.select_submodel(index)
            }
            Command::DeleteSubmodel => {
                self.viewport.cancel(&mut self.document);
                let name = self
                    .document
                    .model(self.document.active_model_index())
                    .unwrap()
                    .name
                    .to_owned();
                if self.document.delete_submodel() {
                    self.ui.selected_vertex_group = None;
                    self.ui.projection.source = None;
                    self.viewport.set_edge_selection(Vec::new(), &self.document);
                    self.viewport.modeling.sync(&self.document);
                    self.viewport
                        .reset_transform_pivot(&self.document, self.ui.transform_mode);
                    self.viewport.wait_for_release();
                    self.ui.status = format!("Deleted {name}. Undo restores the submodel.");
                } else {
                    self.ui.status = "The last model slot is already empty".into();
                }
                Ok(())
            }
            Command::MergeDown => {
                self.viewport.cancel(&mut self.document);
                self.document.merge_down().map(|()| {
                    self.ui.selected_vertex_group = None;
                    self.ui.status =
                        "Submodels merged; current surfaces kept, subdivision levels reset".into();
                })
            }
            Command::DuplicateModel | Command::ExtractMasked => {
                self.viewport.cancel(&mut self.document);
                let extract = matches!(command, Command::ExtractMasked);
                self.document.duplicate_model(extract).map(|()| {
                    self.ui.status = if extract {
                        "Masked surface extracted; source kept".into()
                    } else {
                        "Model duplicated".into()
                    }
                })
            }
            _ => unreachable!("command routed to the wrong feature handler"),
        }
    }
}

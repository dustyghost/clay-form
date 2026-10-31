use super::*;
use crate::{command::InstanceCommand as C, sculpt::instances::*};

impl App {
    fn start_instance_bake(&mut self, id: u64, fuse: bool) -> Result<(), String> {
        self.document.cancel_instance_preview();
        self.ui.instances.editor = None;
        self.viewport.object_brush = None;
        self.start_mesh_job(Operation::BakeInstances(id, fuse))
    }
    pub(super) fn dispatch_instances(&mut self, command: C) -> Result<(), String> {
        match command {
            C::NewArray | C::NewPath => {
                if matches!(command, C::NewPath) && self.document.paths().is_empty() {
                    return Err("Create a path before making a path assembly".into());
                }
                self.viewport.cancel(&mut self.document);
                self.document.cancel_instance_preview();
                self.viewport.object_brush = None;
                self.ui.modeling.active = false;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                let piece = self
                    .document
                    .instance_piece(self.document.active_model_index())?;
                let layout = if matches!(command, C::NewPath) {
                    let path = self
                        .document
                        .paths()
                        .first()
                        .ok_or("Create a path before making a path assembly")?
                        .id;
                    Layout::Path(PathAssembly {
                        path,
                        start: None,
                        middle: piece,
                        end: None,
                        count: 8,
                        spacing: 1.,
                        fit_count: true,
                        twist: 0.,
                        scale: 1.,
                    })
                } else {
                    Layout::Array(Array::new(piece))
                };
                let set = Set {
                    id: self.document.next_instance_set_id()?,
                    name: if matches!(command, C::NewPath) {
                        "Path assembly"
                    } else {
                        "Array"
                    }
                    .into(),
                    visible: true,
                    layout,
                    adjustments: Vec::new(),
                };
                self.ui.instances.editor = Some(set.clone());
                self.ui.instances.invalid = false;
                self.document.preview_instance_set(set)?;
                self.ui.status =
                    "Instance preview · adjust the controls, then Apply or Cancel".into();
            }
            C::Edit(id) => {
                self.viewport.cancel(&mut self.document);
                self.viewport.object_brush = None;
                self.document.cancel_instance_preview();
                let set = self
                    .document
                    .instances()
                    .sets
                    .iter()
                    .find(|s| s.id == id)
                    .ok_or("Instance set no longer exists")?
                    .clone();
                self.document.begin_instance_preview()?;
                self.ui.instances.editor = Some(set);
                self.ui.instances.invalid = false;
            }
            C::Preview(set) => {
                self.document.preview_instance_set(*set)?;
                self.ui.instances.invalid = false;
            }
            C::Apply => {
                if self.ui.instances.invalid {
                    return Err("Correct the invalid preview settings before applying".into());
                }
                self.document.apply_instance_preview()?;
                self.ui.instances.editor = None;
                self.ui.status = "Instances applied · source edits update linked copies".into();
            }
            C::Cancel => {
                self.document.cancel_instance_preview();
                self.ui.instances.editor = None;
                self.ui.instances.invalid = false;
            }
            C::Remove(id) => {
                self.document.remove_instance_set(id)?;
                self.ui.instances.editor = None;
            }
            C::Select(id, slot) => {
                self.document.selected_instance = Some((id, slot));
            }
            C::Detach(id, slot) => {
                self.document.detach_instance(id, slot)?;
                self.ui.instances.editor = None;
                self.ui.status =
                    "Independent submodel created · linked copies still use their source".into();
            }
            C::Capture(name, pivot, forward, up) => {
                let id = self.document.capture_object(name, pivot, forward, up)?;
                self.ui.instances.asset = Some(id);
                self.ui.status = "Object brush captured and embedded in the project".into();
            }
            C::StartBrush(id) => {
                self.viewport.cancel(&mut self.document);
                self.document.cancel_instance_preview();
                self.ui.instances.brush.validate()?;
                self.document
                    .set_object_options(id, self.ui.instances.brush)?;
                let asset = self
                    .document
                    .instances()
                    .assets
                    .iter()
                    .find(|a| a.id == id)
                    .ok_or("Object brush no longer exists")?;
                let piece = Piece {
                    source: Source::Asset(id),
                    pivot: asset.pivot,
                    forward: asset.forward,
                    up: asset.up,
                };
                self.viewport.object_brush =
                    Some(crate::viewport::objects::ObjectBrush::new(piece));
                self.ui.instances.editor = None;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                self.ui.modeling.active = false;
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                self.ui.status =
                    "Object brush · click to place, drag to repeat · Escape cancels the stroke"
                        .into();
            }
            C::StopBrush => {
                self.viewport.cancel(&mut self.document);
                self.document.cancel_instance_preview();
                self.viewport.object_brush = None;
            }
            C::Bake(id, fuse) => self.start_instance_bake(id, fuse)?,
            C::ExportAsset(id, path) => {
                let asset = self
                    .document
                    .instances()
                    .assets
                    .iter()
                    .find(|a| a.id == id)
                    .ok_or("Object brush no longer exists")?;
                crate::persistence::save_object_asset(&path, asset)?;
                self.ui.status = "Object brush exported".into();
            }
            C::ImportAsset(path) => {
                let asset = crate::persistence::load_object_asset(&path)?;
                let options = asset.options;
                self.ui.instances.asset = Some(self.document.import_object_asset(asset)?);
                self.ui.instances.brush = options;
                self.ui.status = "Object brush imported".into();
            }
            C::RemoveAsset(id) => {
                self.document.remove_object_asset(id)?;
                if self.ui.instances.asset == Some(id) {
                    self.ui.instances.asset = None;
                }
                if self
                    .viewport
                    .object_brush
                    .as_ref()
                    .is_some_and(|b| b.piece.source == Source::Asset(id))
                {
                    self.viewport.object_brush = None;
                }
            }
        }
        Ok(())
    }
}

use super::*;
use crate::persistence::object_brushes as shelf;
use crate::{command::InstanceCommand as C, sculpt::instances::*};

impl App {
    fn personal_brush_directory(&self) -> Result<std::path::PathBuf, String> {
        self.preferences_path
            .as_ref()
            .map(|p| p.with_file_name("object-brushes"))
            .ok_or_else(|| "Personal library location is unavailable".into())
    }
    fn remember_object_options(&mut self) -> Result<(), String> {
        if let Some(id) = self.ui.instances.asset
            && self.document.instances().assets.iter().any(|a| a.id == id)
        {
            self.document
                .set_object_options(id, self.ui.instances.brush)?;
        }
        Ok(())
    }
    fn pick_object(&mut self, id: Option<u64>) -> Result<(), String> {
        if let Some(id) = id {
            let asset = self
                .document
                .instances()
                .assets
                .iter()
                .find(|a| a.id == id)
                .ok_or("Captured object no longer exists")?;
            self.ui.instances.brush = asset.options;
            if let Some(active) = &mut self.viewport.object_brush {
                active.piece = Piece {
                    source: Source::Asset(id),
                    pivot: asset.pivot,
                    forward: asset.forward,
                    up: asset.up,
                };
                active.options = asset.options;
            }
        } else {
            self.viewport.object_brush = None;
        }
        self.ui.instances.asset = id;
        Ok(())
    }
    fn pick_group(&mut self, group: Option<u64>) -> Result<(), String> {
        let objects = self.document.instances();
        let next = if let Some(id) = group {
            let brush = objects
                .brushes
                .iter()
                .find(|b| b.id == id)
                .ok_or("Object brush no longer exists")?;
            self.ui.instances.group_name = brush.name.clone();
            self.ui
                .instances
                .asset
                .filter(|id| brush.objects.contains(id))
                .or_else(|| brush.objects.first().copied())
        } else {
            self.ui
                .instances
                .asset
                .filter(|id| objects.assets.iter().any(|a| a.id == *id))
                .or_else(|| objects.assets.first().map(|a| a.id))
        };
        self.ui.instances.group = group;
        self.pick_object(next)
    }
    fn start_instance_bake(&mut self, id: u64, fuse: bool) -> Result<(), String> {
        self.document.cancel_instance_preview();
        self.document.selected_instance = None;
        self.ui.instances.editor = None;
        self.viewport.object_brush = None;
        self.start_mesh_job(Operation::BakeInstances(id, fuse))
    }
    pub(super) fn refresh_object_after_history(&mut self) {
        let library = self.document.instances();
        let group = self
            .ui
            .instances
            .group
            .and_then(|id| library.brushes.iter().find(|b| b.id == id));
        self.ui.instances.group = group.map(|b| b.id);
        if let Some(b) = group {
            self.ui.instances.group_name = b.name.clone();
        }
        let asset = self.ui.instances.asset.filter(|id| {
            library.assets.iter().any(|a| a.id == *id)
                && group.is_none_or(|b| b.objects.contains(id))
        });
        let _ = self.pick_object(asset);
    }
    pub(super) fn dispatch_instances(&mut self, command: C) -> Result<(), String> {
        if self
            .viewport
            .object_brush
            .as_ref()
            .is_some_and(|b| b.stroking())
            && !matches!(&command, C::StopBrush)
        {
            return Err("Finish the current object stroke before changing brushes".into());
        }
        match command {
            C::NewArray | C::NewPath => {
                if matches!(command, C::NewPath) && self.document.paths().is_empty() {
                    return Err("Create a path before making a path assembly".into());
                }
                self.viewport.cancel(&mut self.document);
                self.document.cancel_instance_preview();
                self.document.selected_instance = None;
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
                        bend: false,
                        fit_sections: true,
                        join: JoinMode::Separate,
                        join_distance: default_join_distance(),
                        thickness: Default::default(),
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
                let name = if name.trim().is_empty() {
                    self.document
                        .model(self.document.active_model_index())
                        .ok_or("Select a source model")?
                        .name
                        .to_owned()
                } else {
                    name.trim().to_owned()
                };
                let id = self.document.capture_brush_object(
                    self.ui.instances.group,
                    name.clone(),
                    pivot,
                    forward,
                    up,
                )?;
                self.pick_object(Some(id))?;
                self.ui.status = format!("Captured {name} in the project");
            }
            C::CreateBrush(name) => {
                self.remember_object_options()?;
                let id = self.document.create_object_brush(name)?;
                self.pick_group(Some(id))?;
            }
            C::RenameBrush(id, name) => {
                self.document.rename_object_brush(id, name)?;
                self.pick_group(Some(id))?;
            }
            C::RemoveBrush(id) => {
                self.document.remove_object_brush(id)?;
                self.pick_group(None)?;
            }
            C::BrushMember(group, object, include) => {
                self.remember_object_options()?;
                self.document.set_brush_member(group, object, include)?;
                self.pick_group(Some(group))?;
            }
            C::ChooseBrush(group) => {
                self.remember_object_options()?;
                self.pick_group(group)?;
            }
            C::ChooseObject(id) => {
                if self.ui.instances.group.is_some_and(|g| {
                    !self
                        .document
                        .instances()
                        .brushes
                        .iter()
                        .any(|b| b.id == g && b.objects.contains(&id))
                }) {
                    return Err("Choose an object from the current brush".into());
                }
                self.remember_object_options()?;
                self.pick_object(Some(id))?;
            }
            C::ListPersonalBrushes => {
                self.ui.instances.personal_brushes =
                    shelf::list_personal(&self.personal_brush_directory()?)?;
            }
            C::SavePersonalBrush(id) => {
                self.remember_object_options()?;
                let directory = self.personal_brush_directory()?;
                let bundle = self.document.instances().brush_bundle(id)?;
                shelf::save_personal(&directory, &bundle)?;
                self.ui.instances.personal_brushes = shelf::list_personal(&directory)?;
                self.ui.status = format!("Saved {} in your personal brush library", bundle.name);
            }
            C::LoadPersonalBrush(name) => {
                let path = shelf::personal_path(&self.personal_brush_directory()?, &name)?;
                let id = self
                    .document
                    .import_object_brush(shelf::load(&path.to_string_lossy())?)?;
                self.pick_group(Some(id))?;
            }
            C::RemovePersonalBrush(name) => {
                let directory = self.personal_brush_directory()?;
                std::fs::remove_file(shelf::personal_path(&directory, &name)?)
                    .map_err(|e| e.to_string())?;
                self.ui.instances.personal_brushes = shelf::list_personal(&directory)?;
                self.ui.instances.remove_saved = None;
                self.ui.status = format!(
                    "Removed {name} from your personal library; project objects are unchanged"
                );
            }
            C::ExportBrush(id, path) => {
                self.remember_object_options()?;
                shelf::save(&path, &self.document.instances().brush_bundle(id)?)?;
                self.ui.status = format!("Exported object brush: {path}");
            }
            C::ImportBrush(path) => {
                let id = self.document.import_object_brush(shelf::load(&path)?)?;
                self.pick_group(Some(id))?;
                self.ui.status = "Multi-object brush imported".into();
            }
            C::StartBrush(id) => {
                self.ui.instances.stop_placing = false;
                self.ui.instances.asset = Some(id);
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
                let id = self.document.import_object_asset(asset)?;
                self.ui.instances.group = None;
                self.pick_object(Some(id))?;
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

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
    pub(super) fn remember_object_options(&mut self) -> Result<(), String> {
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
                active.piece = Some(Piece {
                    source: Source::Asset(id),
                    pivot: asset.pivot,
                    forward: asset.forward,
                    up: asset.up,
                });
                active.options = asset.options;
            }
        } else if let Some(active) = &mut self.viewport.object_brush {
            active.piece = None;
        }
        self.ui.instances.asset = id;
        Ok(())
    }
    fn pick_group(&mut self, id: u64) -> Result<(), String> {
        let group = self
            .document
            .instances()
            .brushes
            .iter()
            .find(|b| b.id == id)
            .ok_or("Object brush no longer exists")?;
        let next = self
            .ui
            .instances
            .asset
            .filter(|part| self.ui.instances.group == Some(id) && group.objects.contains(part))
            .or_else(|| group.objects.first().copied());
        self.ui.instances.group_name = group.name.clone();
        self.ui.instances.group = Some(id);
        self.pick_object(next)
    }

    fn activate_object_brush(&mut self, id: u64) -> Result<(), String> {
        if !self.document.instances().brushes.iter().any(|b| b.id == id) {
            return Err("Object brush no longer exists".into());
        }
        self.remember_object_options()?;
        self.viewport.cancel(&mut self.document);
        self.document.cancel_instance_preview();
        self.ui.instances.editor = None;
        self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
        self.ui.modeling.active = false;
        self.ui.selection_tool = None;
        self.ui.slice_active = false;
        self.ui.edges.selecting = false;
        self.ui.materials.sampling = false;
        self.ui.arrange_references = false;
        self.viewport.object_brush = Some(crate::viewport::objects::ObjectBrush::new(None));
        self.ui.instances.placing = true;
        self.ui.instances.stop_placing = false;
        self.pick_group(id)?;
        if let Ok(directory) = self.personal_brush_directory() {
            self.ui.instances.personal_brushes = shelf::list_personal(&directory)?;
        }
        Ok(())
    }
    fn start_instance_bake(&mut self, request: BakeRequest) -> Result<(), String> {
        if self.document.instance_preview_active() {
            return Err("Apply or cancel the instance preview before baking".into());
        }
        self.start_mesh_job(Operation::BakeInstances(request))?;
        self.document.selected_instance = None;
        Ok(())
    }
    pub(super) fn refresh_object_after_history(&mut self) {
        if let Some(id) = self.ui.instances.group {
            if self.document.instances().brushes.iter().any(|b| b.id == id) {
                let _ = self.pick_group(id);
            } else {
                self.viewport.object_brush = None;
                self.ui.instances.placing = false;
                self.ui.instances.group = None;
                self.ui.instances.asset = None;
            }
        }
    }
    pub(super) fn dispatch_instances(&mut self, command: C) -> Result<(), String> {
        if self
            .viewport
            .object_brush
            .as_ref()
            .is_some_and(|b| b.stroking())
        {
            return Err("Finish the current object stroke before changing brushes".into());
        }
        match command {
            C::NewArray => {
                self.viewport.cancel(&mut self.document);
                self.viewport.object_brush = None;
                self.ui.instances.placing = false;
                self.ui.modeling.active = false;
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                self.ui.edges.selecting = false;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                let id = self.document.create_submodel_array()?;
                self.document.set_array_source_mode(false)?;
                self.ui.instances.editor = self
                    .document
                    .instances()
                    .sets
                    .iter()
                    .find(|s| s.id == id)
                    .cloned();
                self.ui.instances.array_layer = 0;
                self.ui.instances.array_edit = true;
                self.ui.instances.focus_array = true;
                self.viewport.frame_array = true;
                self.ui.instances.invalid = false;
                self.ui.status =
                    "Array attached · count includes the source · drag handles or edit values"
                        .into();
            }
            C::ArrayMode(source) => {
                self.ui.instances.focus_array = true;
                self.viewport.cancel(&mut self.document);
                self.viewport.object_brush = None;
                self.ui.instances.placing = false;
                self.document.set_array_source_mode(source)?;
                self.ui.instances.array_edit = !source;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                self.ui.instances.editor = self
                    .document
                    .attached_array(self.document.active_model_index())
                    .cloned();
            }
            C::ArrayDone => {
                self.ui.instances.focus_array = true;
                self.document.set_array_source_mode(false)?;
                self.ui.instances.array_edit = false;
                self.document.selected_instance = None;
            }
            C::FrameArray => self.viewport.frame_array = true,
            C::MakeArrayEditable(id) => {
                self.ui.instances.focus_array = true;
                if self.document.instance_preview_active() {
                    self.document.apply_instance_preview()?;
                }
                self.ui.instances.bake = Some(BakeRequest {
                    scope: BakeScope::Set(id),
                    destination: crate::sculpt::instances::BakeDestination::Current,
                    fuse: false,
                });
            }
            C::NewLegacyArray
            | C::NewLinkedCopy
            | C::RepeatArray(_)
            | C::NewPath
            | C::NewPathParts => {
                if matches!(command, C::NewPath | C::NewPathParts)
                    && self.document.paths().is_empty()
                {
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
                let linked = if let C::RepeatArray(id) = command {
                    let source = self
                        .document
                        .instances()
                        .sets
                        .iter()
                        .find(|s| s.id == id)
                        .ok_or("Source array no longer exists")?;
                    let Layout::Array(a) = &source.layout else {
                        return Err("Choose an array to repeat".into());
                    };
                    Some((id, a.piece, a.output_pivot()))
                } else {
                    None
                };
                let mut piece = if let Some((_, piece, _)) = linked {
                    piece
                } else {
                    self.document
                        .instance_piece(self.document.active_model_index())?
                };
                self.ui.instances.array_layer = 0;
                let layout = if matches!(command, C::NewPath | C::NewPathParts) {
                    let path = self
                        .document
                        .paths()
                        .first()
                        .ok_or("Create a path before making a path assembly")?
                        .id;
                    Layout::Path(PathAssembly {
                        regions: matches!(command, C::NewPath).then(|| {
                            let mesh = self.document.instance_source(piece.source).unwrap();
                            let groups = PathRegions::named(mesh);
                            groups.orient_piece(mesh, &mut piece);
                            groups
                        }),
                        path,
                        start: None,
                        middle: piece,
                        end: None,
                        count: 8,
                        spacing: 1.,
                        fit_count: true,
                        twist: 0.,
                        scale: 1.,
                        bend: matches!(command, C::NewPath),
                        fit_sections: true,
                        join: if matches!(command, C::NewPath) {
                            JoinMode::Weld
                        } else {
                            JoinMode::Separate
                        },
                        join_distance: default_join_distance(),
                        thickness: Default::default(),
                    })
                } else {
                    let mut a = Array::new(piece);
                    if let Some((id, _, pivot)) = linked {
                        a.source_array = Some(id);
                        a.count = 1;
                        a.center = pivot + axis(1) * 2.;
                        a.spacing = axis(1) * 2.;
                    } else if matches!(command, C::NewLinkedCopy) {
                        a.count = 1;
                        a.center = piece.pivot + axis(0) * 2.;
                    }
                    Layout::Array(a)
                };
                let set = Set {
                    id: self.document.next_instance_set_id()?,
                    name: if matches!(command, C::NewPath) {
                        "Path object"
                    } else if matches!(command, C::NewPathParts) {
                        "Path assembly"
                    } else if linked.is_some() {
                        "Linked array"
                    } else if matches!(command, C::NewLinkedCopy) {
                        "Linked copy"
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
                // Reveal the authoring controls even when the first draft still
                // needs group assignments. Opening UI does not edit the scene.
                if matches!(command, C::NewPath | C::NewPathParts)
                    && let Some(panel) = self
                        .ui
                        .preferences
                        .panels
                        .iter_mut()
                        .find(|p| p.tool == crate::preferences::Tool::Submodels)
                {
                    panel.open = true;
                    if panel.dock == crate::preferences::Dock::Hidden {
                        panel.dock = crate::preferences::Dock::Floating;
                    }
                }
                self.document.preview_instance_set(set)?;
                self.ui.status =
                    "Instance preview · adjust the controls, then Apply or Cancel".into();
            }
            C::Edit(id) | C::EditCopy(id, _) => {
                self.ui.instances.focus_array = true;
                if let Some(owner) = self.document.array_owner(id)
                    && owner != self.document.active_model_index()
                {
                    self.document.select_submodel(owner)?;
                }
                self.ui.modeling.active = false;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                self.ui.edges.selecting = false;
                self.ui.instances.array_layer = 0;
                self.ui.instances.array_edit = true;
                self.document.set_array_source_mode(false)?;
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
                self.document.selected_instance = match command {
                    C::EditCopy(_, slot) => Some((id, slot)),
                    _ => None,
                };
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
                self.remember_object_options()?;
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
                    [self.camera.yaw, self.camera.pitch, self.camera.roll],
                )?;
                self.pick_object(Some(id))?;
                self.ui.status = format!("Captured {name} in the project");
            }
            C::CreateBrush(name) => {
                self.remember_object_options()?;
                let id = self.document.create_object_brush(name)?;
                self.activate_object_brush(id)?;
            }
            C::RenameBrush(id, name) => {
                self.document.rename_object_brush(id, name.clone())?;
                if self.ui.instances.group == Some(id) {
                    self.ui.instances.group_name = name.trim().to_owned();
                }
            }
            C::RemoveBrush(id) => {
                self.document.remove_object_brush(id)?;
                self.viewport.object_brush = None;
                self.ui.instances.group = None;
                self.ui.instances.asset = None;
                self.ui.instances.placing = false;
            }
            C::BrushMember(group, object, include) => {
                self.remember_object_options()?;
                self.document.set_brush_member(group, object, include)?;
                self.pick_group(group)?;
            }
            C::ChooseBrush(group) => self.activate_object_brush(group)?,
            C::ChooseShipGreebles => {
                let existing = self
                    .document
                    .instances()
                    .brushes
                    .iter()
                    .find(|b| b.name == SHIP_GREEBLES_NAME)
                    .map(|b| b.id);
                let id = match existing {
                    Some(id) => id,
                    None => self.document.import_object_brush(ship_greebles().clone())?,
                };
                self.activate_object_brush(id)?;
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
                self.activate_object_brush(id)?;
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
                self.activate_object_brush(id)?;
                self.ui.status = "Multi-object brush imported".into();
            }
            C::Bake(request) => self.start_instance_bake(request)?,
            C::ExportAsset(id, path) => {
                self.remember_object_options()?;
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
                self.remember_object_options()?;
                let asset = crate::persistence::load_object_asset(&path)?;
                let id = self
                    .document
                    .import_object_asset(asset, self.ui.instances.group)?;
                self.pick_object(Some(id))?;
                self.ui.status = "Object brush imported".into();
            }
            C::RemoveAsset(id) => {
                self.document.remove_object_asset(id)?;
                if let Some(group) = self.ui.instances.group {
                    self.pick_group(group)?;
                }
            }
        }
        Ok(())
    }
}

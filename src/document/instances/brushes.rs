use super::*;

fn next_id(ids: impl Iterator<Item = u64>) -> Result<u64, String> {
    ids.max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "Object brush identity limit reached".into())
}
fn available_name(brushes: &[BrushGroup], except: Option<u64>, name: &str) -> Result<(), String> {
    BrushGroup::validate_name(name)?;
    if brushes
        .iter()
        .any(|b| Some(b.id) != except && b.name == name)
    {
        return Err("An object brush with that name already exists in this project".into());
    }
    Ok(())
}
impl Document {
    pub(crate) fn finish_object_stamp(
        &mut self,
        mesh: Mesh,
        revision: u64,
        source: Source,
        options: ObjectOptions,
    ) -> Result<bool, String> {
        if revision != self.display_revision() {
            return Err("The scene changed. Stamp the surface again.".into());
        }
        self.require_sculpt_mesh()?;
        options.validate()?;
        let Source::Asset(id) = source else {
            return Err("Choose a captured model to stamp".into());
        };
        if mesh == *self.mesh() {
            return Ok(false);
        }
        let mut next = self.geometry_snapshot();
        next.mesh = mesh;
        next.scene
            .instances
            .assets
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("Captured model is missing")?
            .options = options;
        self.replace_geometry_named(next, "Object stamp")?;
        Ok(true)
    }
    /// Store the part's last-used settings with its stroke, in the same undo step.
    pub(crate) fn finish_object_stroke(
        &mut self,
        set: u64,
        source: Source,
        options: ObjectOptions,
    ) -> Result<(), String> {
        options.validate()?;
        let Source::Asset(id) = source else {
            return Err("Choose a captured object before painting".into());
        };
        self.instance_preview
            .as_mut()
            .ok_or("Object stroke preview is missing")?
            .library
            .assets
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("Captured object no longer exists")?
            .options = options;
        if options.independent {
            self.apply_object_stroke_independent(set)
        } else {
            self.apply_instance_preview()
        }
    }
    fn edit_groups<T>(
        &mut self,
        label: &'static str,
        change: impl FnOnce(&mut Library) -> Result<T, String>,
    ) -> Result<T, String> {
        if self.instance_preview_active() {
            return Err("Finish the current placement or preview first".into());
        }
        let mut library = self.instances().clone();
        let result = change(&mut library)?;
        library.validate_brushes()?;
        if self.instances().brushes != library.brushes {
            self.end_edit();
            self.end_stroke();
            self.record_edit(label);
            self.geometry.scene.instances.brushes = library.brushes;
            self.dirty = true;
        }
        Ok(result)
    }
    pub fn create_object_brush(&mut self, name: String) -> Result<u64, String> {
        self.edit_groups("Create object brush", |library| {
            let name = name.trim().to_owned();
            available_name(&library.brushes, None, &name)?;
            let id = next_id(library.brushes.iter().map(|b| b.id))?;
            library.brushes.push(BrushGroup {
                id,
                name,
                objects: Vec::new(),
            });
            Ok(id)
        })
    }
    pub fn rename_object_brush(&mut self, id: u64, name: String) -> Result<(), String> {
        self.edit_groups("Rename object brush", |library| {
            let name = name.trim().to_owned();
            available_name(&library.brushes, Some(id), &name)?;
            library
                .brushes
                .iter_mut()
                .find(|b| b.id == id)
                .ok_or("Object brush no longer exists")?
                .name = name;
            Ok(())
        })
    }
    pub fn remove_object_brush(&mut self, id: u64) -> Result<(), String> {
        self.edit_groups("Remove object brush", |library| {
            let i = library
                .brushes
                .iter()
                .position(|b| b.id == id)
                .ok_or("Object brush no longer exists")?;
            library.brushes.remove(i);
            Ok(())
        })
    }
    pub fn set_brush_member(
        &mut self,
        brush: u64,
        object: u64,
        include: bool,
    ) -> Result<(), String> {
        self.edit_groups("Edit object brush contents", |library| {
            if !library.assets.iter().any(|a| a.id == object) {
                return Err("Captured object no longer exists".into());
            }
            let objects = &mut library
                .brushes
                .iter_mut()
                .find(|b| b.id == brush)
                .ok_or("Object brush no longer exists")?
                .objects;
            if include && !objects.contains(&object) {
                objects.push(object);
            }
            if !include {
                objects.retain(|id| *id != object);
            }
            Ok(())
        })
    }
    pub fn import_object_brush(&mut self, bundle: BrushBundle) -> Result<u64, String> {
        if self.instance_preview_active() {
            return Err("Finish the current placement or preview first".into());
        }
        bundle.validate()?;
        let mut library = self.instances().clone();
        available_name(&library.brushes, None, &bundle.name)?;
        if library.assets.len() + bundle.objects.len() > MAX_ASSETS {
            return Err("Import would exceed 64 captured objects in this project".into());
        }
        let id = next_id(library.brushes.iter().map(|b| b.id))?;
        let mut brush = BrushGroup {
            id,
            name: bundle.name,
            objects: Vec::new(),
        };
        let mut last = library.assets.iter().map(|a| a.id).max().unwrap_or(0);
        for mut asset in bundle.objects {
            last = last.checked_add(1).ok_or("Object identity limit reached")?;
            asset.id = last;
            if !asset.mesh.uv.textures.is_empty() {
                for texture in &mut std::sync::Arc::make_mut(&mut asset.mesh).uv.textures {
                    texture.id = crate::sculpt::texture::Texture::new_id();
                    texture.exchange = None;
                }
            }
            brush.objects.push(last);
            library.assets.push(asset);
        }
        library.brushes.push(brush);
        let models: Vec<_> = (0..self.model_count())
            .map(|i| (self.instance_model_id(i), self.model(i).unwrap().mesh))
            .collect();
        library.validate(&models, self.paths())?;
        crate::sculpt::texture::validate_library(
            models
                .iter()
                .map(|(_, m)| *m)
                .chain(library.assets.iter().map(|a| a.mesh.as_ref())),
            &self.geometry.scene.texture_library,
        )?;
        self.end_edit();
        self.end_stroke();
        self.record_edit("Import multi-object brush");
        self.geometry.scene.instances = library;
        self.dirty = true;
        self.display_changed();
        Ok(id)
    }
}

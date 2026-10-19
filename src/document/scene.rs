//! Submodels, scene organization, and model metadata.
use super::*;

impl Document {
    pub fn active_model_index(&self) -> usize {
        self.geometry.scene.active_index
    }
    pub fn model_count(&self) -> usize {
        self.geometry.submodels.len() + 1
    }
    pub fn model(&self, index: usize) -> Option<ModelRef<'_>> {
        if index >= self.model_count() {
            return None;
        }
        if index == self.active_model_index() {
            Some(ModelRef {
                name: &self.geometry.name,
                metadata: &self.geometry.scene.active_metadata,
                mesh: self.mesh(),
                settings: self.mesh_settings(),
            })
        } else {
            let model = &self.geometry.submodels[if index > self.active_model_index() {
                index - 1
            } else {
                index
            }];
            Some(ModelRef {
                name: &model.name,
                metadata: &model.metadata,
                mesh: &model.mesh,
                settings: &model.mesh_settings,
            })
        }
    }
    pub fn submodel_names(&self) -> impl Iterator<Item = &str> {
        (0..self.model_count()).map(|i| self.model(i).unwrap().name)
    }
    pub fn groups(&self) -> &[String] {
        &self.geometry.scene.groups
    }
    pub fn active_visible(&self) -> bool {
        self.geometry.scene.active_metadata.visible
    }
    pub(super) fn ordered_models(&self) -> Vec<crate::sculpt::Submodel> {
        (0..self.model_count())
            .map(|i| {
                let m = self.model(i).unwrap();
                crate::sculpt::Submodel {
                    name: m.name.into(),
                    metadata: m.metadata.clone(),
                    mesh: m.mesh.clone(),
                    mesh_settings: m.settings.clone(),
                }
            })
            .collect()
    }
    pub(super) fn install_models(
        &mut self,
        mut models: Vec<crate::sculpt::Submodel>,
        active: usize,
    ) {
        let selected = models.remove(active);
        self.geometry.mesh = selected.mesh;
        self.geometry.settings = selected.mesh_settings;
        self.geometry.name = selected.name;
        self.geometry.scene.active_metadata = selected.metadata;
        self.geometry.scene.active_index = active;
        self.geometry.submodels = models;
        self.dirty = true;
        self.display_changed();
    }
    pub fn select_submodel(&mut self, index: usize) -> Result<(), String> {
        self.end_edit();
        if index >= self.model_count() {
            return Err("Submodel no longer exists".into());
        }
        if index == self.active_model_index() {
            return Ok(());
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Select submodel");
        self.install_models(self.ordered_models(), index);
        Ok(())
    }
    pub fn duplicate_model(&mut self, extract: bool) -> Result<(), String> {
        if self.model_count() >= 64 {
            return Err("Submodels are limited to 64".into());
        }
        if extract {
            self.require_sculpt_mesh()?;
        }
        self.end_edit();
        let mesh = if extract {
            self.mesh()
                .extract_masked_threshold(self.geometry.scene.active_metadata.mask_threshold)?
        } else {
            self.mesh().clone()
        };
        let settings = if extract {
            MeshSettings::default()
        } else {
            self.geometry.settings.clone()
        };
        self.end_edit();
        self.end_stroke();
        self.record_edit(if extract {
            "Extract masked submodel"
        } else {
            "Duplicate submodel"
        });
        let base = if extract { "Extract" } else { "Copy" };
        let mut number = 1;
        let name = loop {
            let name = format!("{base} {number}");
            if !self.submodel_names().any(|existing| existing == name) {
                break name;
            }
            number += 1;
        };
        let mut models = self.ordered_models();
        let index = self.active_model_index();
        let mut metadata = self.geometry.scene.active_metadata.clone();
        metadata.visible = true;
        models.insert(
            index,
            crate::sculpt::Submodel {
                name,
                metadata,
                mesh,
                mesh_settings: settings,
            },
        );
        self.install_models(models, index);
        Ok(())
    }
    /// Match the grouped order displayed by the Submodels panel, even when
    /// folder order differs from the underlying scene indices.
    pub fn merge_down_target(&self) -> Option<usize> {
        let order: Vec<_> = std::iter::once("")
            .chain(self.groups().iter().map(String::as_str))
            .flat_map(|group| {
                (0..self.model_count())
                    .filter(move |&i| self.model(i).unwrap().metadata.group == group)
            })
            .collect();
        let position = order.iter().position(|&i| i == self.active_model_index())?;
        order.get(position + 1).copied()
    }
    pub fn merge_down(&mut self) -> Result<(), String> {
        let target = self
            .merge_down_target()
            .ok_or("There is no submodel below the active one")?;
        let active = self.active_model_index();
        let surface = |model: ModelRef<'_>| -> Result<Mesh, String> {
            if model.settings.primitive.is_some() && model.mesh.triangles.is_empty() {
                return Err("Fill the primitive before merging it into a sculpting mesh".into());
            }
            let mut mesh = if model.metadata.temporary_thickness > 0. {
                model.mesh.solidify_with_metadata(model.metadata)?
            } else {
                model.mesh.clone()
            };
            if !model.metadata.visible {
                mesh.set_face_visibility(vec![false; mesh.triangles.len()]);
            }
            Ok(mesh)
        };
        let mesh = surface(self.model(active).unwrap())?
            .merged_with(&surface(self.model(target).unwrap())?)?;
        let mut models = self.ordered_models();
        models[active].mesh = mesh;
        models[active].mesh_settings = MeshSettings::default();
        models[active].metadata.temporary_thickness = 0.;
        models[active].metadata.visible |= models[target].metadata.visible;
        models.remove(target);
        self.end_edit();
        self.end_stroke();
        self.record_edit("Merge down");
        self.install_models(models, active - usize::from(target < active));
        Ok(())
    }
    pub fn set_model_metadata(
        &mut self,
        index: usize,
        name: String,
        metadata: crate::sculpt::ModelMetadata,
    ) -> Result<(), String> {
        metadata.validate()?;
        self.end_edit();
        if name.trim().is_empty()
            || name.len() > 128
            || index >= self.model_count()
            || (!metadata.group.is_empty() && !self.groups().contains(&metadata.group))
        {
            return Err("Invalid submodel name or group".into());
        }
        let model = self.model(index).unwrap();
        let old = model.metadata;
        if metadata.temporary_thickness > 0.
            && (old.temporary_thickness != metadata.temporary_thickness
                || old.thickness_loops != metadata.thickness_loops
                || old.thickness_crease != metadata.thickness_crease
                || old.thickness_groups != metadata.thickness_groups
                || old.thickness_group_mask != metadata.thickness_group_mask
                || old.thickness_mask_rim != metadata.thickness_mask_rim
                || (old.mask_threshold != metadata.mask_threshold && metadata.thickness_group_mask))
        {
            model.mesh.solidify_with_metadata(&metadata)?;
        }
        if model.name == name && *model.metadata == metadata {
            return Ok(());
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Edit submodel");
        if index == self.active_model_index() {
            self.geometry.name = name;
            self.geometry.scene.active_metadata = metadata;
        } else {
            let slot = if index > self.active_model_index() {
                index - 1
            } else {
                index
            };
            self.geometry.submodels[slot].name = name;
            self.geometry.submodels[slot].metadata = metadata;
        }
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn reorder_model(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.model_count() || to >= self.model_count() {
            return Err("Invalid submodel order".into());
        }
        if from == to {
            return Ok(());
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Reorder submodels");
        let active = self.active_model_index();
        let new_active = if active == from {
            to
        } else if from < active && to >= active {
            active - 1
        } else if from > active && to <= active {
            active + 1
        } else {
            active
        };
        let mut models = self.ordered_models();
        let moved = models.remove(from);
        models.insert(to, moved);
        self.install_models(models, new_active);
        Ok(())
    }
    pub fn add_group(&mut self, name: String) -> Result<(), String> {
        let name = name.trim().to_string();
        if name.is_empty()
            || name.len() > 128
            || self.groups().len() >= 64
            || self.groups().contains(&name)
        {
            return Err("Choose a unique group name (up to 128 bytes)".into());
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Add submodel group");
        self.geometry.scene.groups.push(name);
        self.dirty = true;
        Ok(())
    }
    pub fn remove_group(&mut self, name: &str) -> Result<(), String> {
        if !self.groups().iter().any(|g| g == name) {
            return Err("Group no longer exists".into());
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Ungroup submodels");
        self.geometry.scene.groups.retain(|g| g != name);
        if self.geometry.scene.active_metadata.group == name {
            self.geometry.scene.active_metadata.group.clear();
        }
        for m in &mut self.geometry.submodels {
            if m.metadata.group == name {
                m.metadata.group.clear();
            }
        }
        self.dirty = true;
        Ok(())
    }
    pub fn reorder_group(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.groups().len() || to >= self.groups().len() {
            return Err("Invalid group order".into());
        }
        if from == to {
            return Ok(());
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Reorder groups");
        let group = self.geometry.scene.groups.remove(from);
        self.geometry.scene.groups.insert(to, group);
        self.dirty = true;
        Ok(())
    }
    pub fn set_group_visible(&mut self, name: &str, visible: bool) -> Result<(), String> {
        if !self.groups().iter().any(|g| g == name) {
            return Err("Group no longer exists".into());
        }
        if !(0..self.model_count()).any(|i| {
            let m = self.model(i).unwrap();
            m.metadata.group == name && m.metadata.visible != visible
        }) {
            return Ok(());
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Group visibility");
        if self.geometry.scene.active_metadata.group == name {
            self.geometry.scene.active_metadata.visible = visible;
        }
        for m in &mut self.geometry.submodels {
            if m.metadata.group == name {
                m.metadata.visible = visible;
            }
        }
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
}

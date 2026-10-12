//! Owns persistent content and mesh edits. No UI, window, or tablet dependencies.
use crate::{
    history::History,
    sculpt::{
        AdaptiveBudget, AdaptiveSettings, Alpha, Brush, Dab, Geometry, Kind, Mesh, MeshSettings,
        Project, Symmetry, validate_brush,
    },
};
pub struct ModelRef<'a> {
    pub name: &'a str,
    pub metadata: &'a crate::sculpt::ModelMetadata,
    pub mesh: &'a Mesh,
    pub settings: &'a MeshSettings,
}
impl ModelRef<'_> {
    pub fn display_mesh(&self, active: bool) -> &Mesh {
        if active {
            self.mesh
        } else {
            self.settings.lower.first().unwrap_or(self.mesh)
        }
    }
}
pub struct Document {
    geometry: Geometry,
    display_revision: u64,
    brushes: Vec<Brush>,
    history: History,
    symmetry: Symmetry,
    dirty: bool,
    primitive_edit_active: bool,
    thickness_edit_active: bool,
    mask_adjustment_label: Option<&'static str>,
    mask_intensity_baseline: Option<(f32, Vec<Vec<f32>>)>,
    stroke_active: bool,
    stroke_deformed: bool,
    stroke_before: Option<Geometry>,
    current_path: String,
    adaptive_budget: AdaptiveBudget,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            geometry: Geometry::new(Mesh::starting_sphere()),
            display_revision: 0,
            brushes: Kind::ALL.into_iter().map(Brush::new).collect(),
            history: History::default(),
            symmetry: Symmetry::default(),
            dirty: false,
            primitive_edit_active: false,
            thickness_edit_active: false,
            mask_adjustment_label: None,
            mask_intensity_baseline: None,
            stroke_active: false,
            stroke_deformed: false,
            stroke_before: None,
            current_path: "sculpt.clay".into(),
            adaptive_budget: AdaptiveBudget::default(),
        }
    }
}
impl Document {
    /// Session-only invalidation token for display geometry, materials, and ownership.
    pub(crate) fn display_revision(&self) -> u64 {
        self.display_revision
    }
    fn display_changed(&mut self) {
        self.display_revision = self.display_revision.wrapping_add(1);
    }
    pub fn primitive(&self) -> Option<&crate::sculpt::Primitive> {
        self.mesh_settings().primitive.as_ref()
    }
    pub fn require_sculpt_mesh(&self) -> Result<(), String> {
        if self.primitive().is_some() {
            Err("Convert the primitive to a sculpting mesh first".into())
        } else {
            Ok(())
        }
    }
    pub fn initialize_primitive(&mut self) -> Result<(), String> {
        let mut primitive = crate::sculpt::Primitive::default();
        let mut min = crate::sculpt::Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = crate::sculpt::Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for p in &self.mesh().vertices {
            min = crate::sculpt::Vec3(min.0.min(p.0), min.1.min(p.1), min.2.min(p.2));
            max = crate::sculpt::Vec3(max.0.max(p.0), max.1.max(p.1), max.2.max(p.2));
        }
        primitive.center = if self.mesh().vertices.is_empty() {
            crate::sculpt::Vec3::default()
        } else {
            (min + max) * 0.5
        };
        let mut geometry = self.geometry.clone();
        geometry.mesh = primitive.mesh()?;
        geometry.settings = MeshSettings {
            primitive: Some(primitive),
            ..Default::default()
        };
        self.end_primitive_edit();
        self.replace_geometry_named(geometry, "Initialize primitive")
    }
    /// Continuous slider updates are one edit until the pointer is released.
    pub fn update_primitive(&mut self, primitive: crate::sculpt::Primitive) -> Result<(), String> {
        self.end_thickness_edit();
        let Some(current) = self.primitive() else {
            return Err("The active submodel is no longer a primitive".into());
        };
        if *current == primitive {
            return Ok(());
        }
        let mesh = primitive.mesh()?;
        self.end_stroke();
        if !self.primitive_edit_active {
            self.history
                .record_named(&self.geometry, "Edit primitive parameters");
            self.primitive_edit_active = true;
        }
        self.geometry.mesh = mesh;
        self.geometry.settings = MeshSettings {
            primitive: Some(primitive),
            ..Default::default()
        };
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn end_primitive_edit(&mut self) {
        self.primitive_edit_active = false;
        self.end_thickness_edit();
        self.end_mask_adjustment();
    }
    pub fn convert_primitive(&mut self) -> Result<(), String> {
        if self.primitive().is_none() {
            return Err("The active submodel is already a sculpting mesh".into());
        }
        if self.mesh().triangles.is_empty() {
            return Err("Fill the circle before converting it to a sculpting mesh".into());
        }
        self.end_primitive_edit();
        let mut geometry = self.geometry.clone();
        geometry.settings.primitive = None;
        self.replace_geometry_named(geometry, "Convert primitive to sculpting mesh")
    }
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
    fn ordered_models(&self) -> Vec<crate::sculpt::Submodel> {
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
    fn install_models(&mut self, mut models: Vec<crate::sculpt::Submodel>, active: usize) {
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
        self.end_primitive_edit();
        if index >= self.model_count() {
            return Err("Submodel no longer exists".into());
        }
        if index == self.active_model_index() {
            return Ok(());
        }
        self.end_stroke();
        self.history.record_named(&self.geometry, "Select submodel");
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
        self.end_primitive_edit();
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
        self.end_stroke();
        self.history.record_named(
            &self.geometry,
            if extract {
                "Extract masked submodel"
            } else {
                "Duplicate submodel"
            },
        );
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
    pub fn set_model_metadata(
        &mut self,
        index: usize,
        name: String,
        metadata: crate::sculpt::ModelMetadata,
    ) -> Result<(), String> {
        metadata.validate()?;
        self.end_mask_adjustment();
        self.end_thickness_edit();
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
        self.end_stroke();
        self.history.record_named(&self.geometry, "Edit submodel");
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
        self.end_stroke();
        self.history
            .record_named(&self.geometry, "Reorder submodels");
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
        self.end_stroke();
        self.history
            .record_named(&self.geometry, "Add submodel group");
        self.geometry.scene.groups.push(name);
        self.dirty = true;
        Ok(())
    }
    pub fn remove_group(&mut self, name: &str) -> Result<(), String> {
        if !self.groups().iter().any(|g| g == name) {
            return Err("Group no longer exists".into());
        }
        self.end_stroke();
        self.history
            .record_named(&self.geometry, "Ungroup submodels");
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
        self.end_stroke();
        self.history.record_named(&self.geometry, "Reorder groups");
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
        self.end_stroke();
        self.history
            .record_named(&self.geometry, "Group visibility");
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
    /// Apply each drag increment to every stored level; a drag is one undo entry.
    pub fn apply_transform(
        &mut self,
        transform: crate::sculpt::Transform,
        respect_mask: bool,
    ) -> Result<bool, String> {
        transform.validate()?;
        if transform.is_identity() {
            return Ok(false);
        }
        if !self.active_visible() {
            return Err("Show the active submodel before transforming it".into());
        }
        let mut geometry = self.geometry.clone();
        self.end_primitive_edit();
        if let Some(primitive) = self.primitive() {
            let primitive = primitive.transformed(transform)?;
            geometry.mesh = primitive.mesh()?;
            geometry.settings.primitive = Some(primitive);
        } else {
            for mesh in std::iter::once(&mut geometry.mesh)
                .chain(geometry.settings.lower.iter_mut())
                .chain(geometry.settings.upper.iter_mut())
            {
                for (vertex, mask) in mesh.vertices.iter_mut().zip(&mesh.mask) {
                    let mobility = if respect_mask { 1. - mask } else { 1. };
                    *vertex = *vertex + (transform.point(*vertex) - *vertex) * mobility;
                }
                mesh.validate()?;
            }
        }
        if geometry == self.geometry {
            return Ok(false);
        }
        self.begin_stroke();
        if let Some(before) = self.stroke_before.take() {
            self.history.record_owned(before, "Transform submodel");
        }
        self.geometry = geometry;
        self.dirty = true;
        self.display_changed();
        Ok(true)
    }
    pub fn mesh(&self) -> &Mesh {
        &self.geometry.mesh
    }
    pub fn mesh_settings(&self) -> &MeshSettings {
        &self.geometry.settings
    }
    pub(crate) fn geometry_snapshot(&self) -> Geometry {
        self.geometry.clone()
    }
    #[cfg(test)]
    pub(crate) fn replace_geometry(&mut self, geometry: Geometry) -> Result<(), String> {
        self.replace_geometry_named(geometry, "Mesh change")
    }
    pub(crate) fn replace_geometry_named(
        &mut self,
        geometry: Geometry,
        label: &'static str,
    ) -> Result<(), String> {
        geometry.settings.validate(&geometry.mesh)?;
        self.end_thickness_edit();
        self.end_mask_adjustment();
        self.end_stroke();
        self.history.record_named(&self.geometry, label);
        self.geometry = geometry;
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn set_mesh_level(&mut self, level: usize) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if level == self.geometry.settings.level() {
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        geometry.set_level(level)?;
        self.replace_geometry_named(geometry, "Change subdivision level")
    }
    /// Keep the active surface and the opposite side of the subdivision stack.
    pub fn delete_mesh_levels(&mut self, lower: bool) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        let levels = if lower {
            &self.geometry.settings.lower
        } else {
            &self.geometry.settings.upper
        };
        if levels.is_empty() {
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        if lower {
            geometry.settings.lower.clear();
        } else {
            geometry.settings.upper.clear();
        }
        self.replace_geometry_named(
            geometry,
            if lower {
                "Delete lower levels"
            } else {
                "Delete higher levels"
            },
        )
    }
    pub fn set_dynamic_mesh(&mut self, enabled: bool) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if self.geometry.settings.dynamic == enabled && !self.geometry.settings.adaptive.enabled {
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        geometry.set_dynamic(enabled);
        self.replace_geometry_named(geometry, "Switch mesh mode")
    }
    pub fn set_adaptive(&mut self, settings: AdaptiveSettings) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        settings.validate()?;
        if self.geometry.settings.adaptive == settings {
            return Ok(());
        }
        if self.geometry.settings.adaptive.enabled == settings.enabled {
            self.end_stroke();
            self.geometry.settings.adaptive = settings;
            self.dirty = true;
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        if settings.enabled && !geometry.settings.adaptive.enabled {
            geometry.settings.lower.clear();
            geometry.settings.upper.clear();
            geometry.settings.dynamic = false;
        }
        geometry.settings.adaptive = settings;
        self.replace_geometry_named(geometry, "Adaptive sculpt settings")
    }
    pub(crate) fn begin_sculpt_frame(&mut self) {
        self.adaptive_budget = AdaptiveBudget::default();
    }
    pub fn set_remesh_settings(&mut self, resolution: u32, auto: bool) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if !(16..=96).contains(&resolution) {
            return Err("Remesh resolution must be between 16 and 96".into());
        }
        if self.geometry.settings.resolution != resolution
            || self.geometry.settings.auto_remesh != auto
        {
            self.geometry.settings.resolution = resolution;
            self.geometry.settings.auto_remesh = auto;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn brushes(&self) -> &[Brush] {
        &self.brushes
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn current_path(&self) -> &str {
        &self.current_path
    }
    pub fn undo_label(&self) -> Option<&'static str> {
        self.history.undo_label()
    }
    pub fn redo_label(&self) -> Option<&'static str> {
        self.history.redo_label()
    }
    pub fn history_counts(&self) -> (usize, usize) {
        self.history.counts()
    }
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    pub fn project(&self) -> Project {
        Project {
            version: Project::VERSION,
            scene: self.geometry.scene.clone(),
            model_name: self.geometry.name.clone(),
            submodels: self.geometry.submodels.clone(),
            mesh: self.geometry.mesh.clone(),
            brushes: self.brushes.clone(),
            symmetry: self.symmetry.clone(),
            mesh_settings: self.geometry.settings.clone(),
        }
    }
    pub fn mark_saved(&mut self, path: &str) {
        self.current_path = path.into();
        self.dirty = false;
    }
    pub fn new_sphere(&mut self) {
        self.end_primitive_edit();
        self.geometry = Geometry::new(Mesh::starting_sphere());
        self.display_changed();
        self.history = History::default();
        self.dirty = false;
        self.end_stroke();
        self.current_path = "untitled.clay".into();
    }
    /// Validate before changing anything, so an invalid open leaves the current document intact.
    pub fn replace_project(&mut self, project: Project, path: &str) -> Result<(), String> {
        project.validate()?;
        self.end_primitive_edit();
        self.geometry = Geometry {
            scene: project.scene,
            name: project.model_name,
            submodels: project.submodels,
            mesh: project.mesh,
            settings: project.mesh_settings,
        };
        self.display_changed();
        self.brushes = project.brushes;
        self.symmetry = project.symmetry;
        self.history = History::default();
        self.end_stroke();
        self.mark_saved(path);
        Ok(())
    }
    pub fn begin_stroke(&mut self) {
        self.end_thickness_edit();
        self.end_mask_adjustment();
        if !self.stroke_active {
            self.stroke_before = Some(self.geometry.clone());
            self.stroke_active = true;
            self.stroke_deformed = false;
            self.adaptive_budget = AdaptiveBudget::default();
        }
    }
    pub fn end_stroke(&mut self) -> bool {
        let deformed = self.stroke_active && self.stroke_deformed;
        self.stroke_active = false;
        self.stroke_deformed = false;
        self.stroke_before = None;
        deformed
    }
    pub fn symmetry(&self) -> &Symmetry {
        &self.symmetry
    }
    pub fn set_symmetry(&mut self, symmetry: Symmetry) -> Result<(), String> {
        symmetry.validate()?;
        if self.symmetry != symmetry {
            self.symmetry = symmetry;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn apply_dab(&mut self, brush: &Brush, dab: Dab) -> bool {
        if !self.active_visible() || self.primitive().is_some() {
            return false;
        }
        self.begin_stroke();
        let dabs = self.symmetry.dabs(dab);
        let settings = &self.geometry.settings.adaptive;
        let before_topology =
            self.geometry
                .mesh
                .adapt(brush, &dabs, settings, &mut self.adaptive_budget, true);
        let old_mask = (brush.kind == Kind::Mask).then(|| self.geometry.mesh.mask.clone());
        let deformed = self.geometry.mesh.stamp_many(brush, &dabs);
        let mut after_dabs = dabs.clone();
        if brush.kind == Kind::Snake {
            for dab in &mut after_dabs {
                dab.center = dab.center + brush.snake_motion(*dab);
            }
        }
        let after_topology = self.geometry.mesh.adapt(
            brush,
            &after_dabs,
            settings,
            &mut self.adaptive_budget,
            false,
        );
        self.stroke_deformed |= deformed || before_topology || after_topology;
        if deformed
            || before_topology
            || after_topology
            || old_mask.is_some_and(|mask| mask != self.geometry.mesh.mask)
        {
            if let Some(before) = self.stroke_before.take() {
                self.history.record_owned(
                    before,
                    if settings.enabled {
                        "Adaptive brush stroke"
                    } else if brush.kind == Kind::Snake {
                        "Snake stroke"
                    } else {
                        "Brush stroke"
                    },
                );
            }
            self.dirty = true;
            self.display_changed();
        }
        deformed
    }
    pub fn undo(&mut self) {
        self.end_primitive_edit();
        self.end_stroke();
        if self.history.undo(&mut self.geometry) {
            self.dirty = true;
            self.display_changed();
        }
    }
    pub fn redo(&mut self) {
        self.end_primitive_edit();
        self.end_stroke();
        if self.history.redo(&mut self.geometry) {
            self.dirty = true;
            self.display_changed();
        }
    }
    fn edit_group_meshes(
        &mut self,
        label: &'static str,
        mut edit: impl FnMut(&mut Mesh) -> Result<(), String>,
    ) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        self.end_primitive_edit();
        self.end_stroke();
        let mut geometry = self.geometry.clone();
        for mesh in std::iter::once(&mut geometry.mesh)
            .chain(geometry.settings.lower.iter_mut())
            .chain(geometry.settings.upper.iter_mut())
        {
            edit(mesh)?;
        }
        if geometry == self.geometry {
            return Ok(());
        }
        self.replace_geometry_named(geometry, label)
    }
    pub fn create_vertex_group(&mut self, name: String, all: bool) -> Result<usize, String> {
        let threshold = self.geometry.scene.active_metadata.mask_threshold;
        if self.mesh().vertices.is_empty()
            || (!all
                && !self
                    .mesh()
                    .mask
                    .iter()
                    .any(|&m| crate::sculpt::mask_selected(m, threshold)))
        {
            return Err("Mask vertices above the threshold first, or use Group all".into());
        }
        let index = self.mesh().vertex_groups.len();
        self.edit_group_meshes("Create vertex group", |mesh| {
            mesh.create_vertex_group(name.clone(), threshold, all)
                .map(|_| ())
        })?;
        Ok(index)
    }
    pub fn assign_mask_to_group(&mut self, index: usize, remove: bool) -> Result<(), String> {
        let threshold = self.geometry.scene.active_metadata.mask_threshold;
        self.edit_group_meshes(
            if remove {
                "Remove masked vertices from group"
            } else {
                "Assign masked vertices to group"
            },
            |mesh| mesh.assign_mask_to_group(index, threshold, remove),
        )
    }
    pub fn mask_from_group(&mut self, index: usize) -> Result<(), String> {
        self.edit_group_meshes("Mask vertex group", |mesh| mesh.mask_from_group(index))
    }
    pub fn remove_vertex_group(&mut self, index: usize) -> Result<(), String> {
        self.edit_group_meshes("Delete vertex group", |mesh| {
            mesh.remove_vertex_group(index)
        })
    }
    pub fn set_vertex_group_visible(&mut self, index: usize, visible: bool) -> Result<(), String> {
        self.edit_group_meshes("Polygroup visibility", |mesh| {
            mesh.vertex_groups
                .get_mut(index)
                .ok_or("Vertex group no longer exists")?
                .visible = visible;
            Ok(())
        })
    }
    pub fn rename_vertex_group(&mut self, index: usize, name: String) -> Result<(), String> {
        self.edit_group_meshes("Rename vertex group", |mesh| {
            mesh.rename_vertex_group(index, name.clone())
        })
    }
    pub fn grow_mask(&mut self, grow: bool) -> Result<(), String> {
        self.edit_group_meshes(if grow { "Grow mask" } else { "Shrink mask" }, |mesh| {
            mesh.grow_mask(grow);
            Ok(())
        })
    }
    pub fn mask_peak(&self) -> f32 {
        std::iter::once(&self.geometry.mesh)
            .chain(self.geometry.settings.lower.iter())
            .chain(self.geometry.settings.upper.iter())
            .flat_map(|m| m.mask.iter())
            .copied()
            .fold(0., f32::max)
    }
    pub fn set_mask_intensity(&mut self, amount: f32) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if !amount.is_finite() || !(0. ..=1.).contains(&amount) {
            return Err("Mask intensity must be between 0% and 100%".into());
        }
        self.end_stroke();
        self.end_thickness_edit();
        self.primitive_edit_active = false;
        if self.mask_adjustment_label != Some("Mask intensity") {
            let peak = self.mask_peak();
            if peak == 0. || peak == amount {
                return Ok(());
            }
            self.history.record_named(&self.geometry, "Mask intensity");
            self.mask_intensity_baseline = Some((
                peak,
                std::iter::once(&self.geometry.mesh)
                    .chain(self.geometry.settings.lower.iter())
                    .chain(self.geometry.settings.upper.iter())
                    .map(|mesh| mesh.mask.clone())
                    .collect(),
            ));
            self.mask_adjustment_label = Some("Mask intensity");
        }
        let (peak, masks) = self.mask_intensity_baseline.as_ref().unwrap();
        for (mesh, old) in std::iter::once(&mut self.geometry.mesh)
            .chain(self.geometry.settings.lower.iter_mut())
            .chain(self.geometry.settings.upper.iter_mut())
            .zip(masks)
        {
            for (value, &original) in mesh.mask.iter_mut().zip(old) {
                *value = (original / peak * amount).clamp(0., 1.);
            }
        }
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn set_mask_threshold(&mut self, amount: f32) -> Result<(), String> {
        if !amount.is_finite() || !(0. ..=1.).contains(&amount) {
            return Err("Mask threshold must be between 0% and 100%".into());
        }
        let mut metadata = self.geometry.scene.active_metadata.clone();
        metadata.mask_threshold = amount;
        if metadata == self.geometry.scene.active_metadata {
            return Ok(());
        }
        if metadata.temporary_thickness > 0. && metadata.thickness_group_mask {
            self.mesh().solidify_with_metadata(&metadata)?;
        }
        self.end_stroke();
        self.end_thickness_edit();
        self.primitive_edit_active = false;
        if self.mask_adjustment_label != Some("Mask threshold") {
            self.history.record_named(&self.geometry, "Mask threshold");
            self.mask_adjustment_label = Some("Mask threshold");
        }
        self.geometry.scene.active_metadata = metadata;
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub(crate) fn end_mask_adjustment(&mut self) {
        self.mask_adjustment_label = None;
        self.mask_intensity_baseline = None;
    }
    pub fn set_temporary_thickness(&mut self, value: f32) -> Result<(), String> {
        let mut metadata = self.geometry.scene.active_metadata.clone();
        metadata.temporary_thickness = value;
        metadata.validate()?;
        if metadata == self.geometry.scene.active_metadata {
            return Ok(());
        }
        if value > 0. {
            self.mesh().solidify_with_metadata(&metadata)?;
        }
        self.end_stroke();
        self.end_mask_adjustment();
        self.primitive_edit_active = false;
        if !self.thickness_edit_active {
            self.history
                .record_named(&self.geometry, "Edit temporary thickness");
            self.thickness_edit_active = true;
        }
        self.geometry.scene.active_metadata = metadata;
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub(crate) fn end_thickness_edit(&mut self) {
        self.thickness_edit_active = false;
    }
    pub fn apply_thickness(&mut self) -> Result<(), String> {
        if !self.active_visible() {
            return Err("Show the active model before applying thickness".into());
        }
        let shell = self
            .mesh()
            .solidify_with_metadata(&self.geometry.scene.active_metadata)?;
        self.end_primitive_edit();
        let mut geometry = self.geometry.clone();
        geometry.mesh = shell;
        geometry.settings = MeshSettings {
            resolution: geometry.settings.resolution,
            ..Default::default()
        };
        geometry.scene.active_metadata.temporary_thickness = 0.;
        self.replace_geometry_named(geometry, "Apply thickness")
    }
    pub fn invert_mask(&mut self) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if self.mesh().mask.is_empty() {
            return Ok(());
        }
        self.end_stroke();
        self.history.record_named(&self.geometry, "Invert mask");
        for mesh in std::iter::once(&mut self.geometry.mesh)
            .chain(self.geometry.settings.lower.iter_mut())
            .chain(self.geometry.settings.upper.iter_mut())
        {
            for mask in &mut mesh.mask {
                *mask = 1. - *mask;
            }
        }
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn delete_masked(&mut self) -> Result<bool, String> {
        self.require_sculpt_mesh()?;
        if !self.active_visible() {
            return Err("Show the active model before deleting masked surface".into());
        }
        let Some(mesh) = self
            .mesh()
            .delete_masked_threshold(self.geometry.scene.active_metadata.mask_threshold)?
        else {
            return Ok(false);
        };
        let mut geometry = self.geometry.clone();
        geometry.mesh = mesh;
        // Stored levels cannot retain correspondence after cutting a hole.
        // Adaptive and voxel modes require a closed surface.
        geometry.settings = MeshSettings {
            resolution: geometry.settings.resolution,
            ..Default::default()
        };
        self.replace_geometry_named(geometry, "Delete masked surface")?;
        Ok(true)
    }
    pub fn clear_mask(&mut self) {
        if self.primitive().is_some() {
            return;
        }
        self.end_stroke();
        self.history.record_named(&self.geometry, "Clear mask");
        self.geometry.mesh.mask.fill(0.);
        for level in self
            .geometry
            .settings
            .lower
            .iter_mut()
            .chain(self.geometry.settings.upper.iter_mut())
        {
            level.mask.fill(0.);
        }
        self.dirty = true;
        self.display_changed();
    }
    pub fn update_brush(&mut self, index: usize, mut brush: Brush) -> Result<(), String> {
        validate_brush(&brush)?;
        let target = self
            .brushes
            .get_mut(index)
            .ok_or("Brush no longer exists")?;
        brush.id = Some(target.identity());
        if *target != brush {
            *target = brush;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn set_alpha(&mut self, index: usize, alpha: Alpha) -> Result<(), String> {
        let mut brush = self
            .brushes
            .get(index)
            .ok_or("Brush no longer exists")?
            .clone();
        brush.alpha = alpha;
        self.update_brush(index, brush)
    }
    pub fn append_brush(&mut self, mut brush: Brush) -> Result<usize, String> {
        validate_brush(&brush)?;
        if self.brushes.len() >= 256 {
            return Err("Brush library is limited to 256 presets".into());
        }
        brush.ensure_identity();
        if self
            .brushes
            .iter()
            .any(|b| b.identity() == brush.identity())
        {
            brush.renew_identity();
        }
        self.brushes.push(brush);
        self.dirty = true;
        Ok(self.brushes.len() - 1)
    }
    /// Validate and merge the entire batch before changing the library.
    pub fn import_brushes(
        &mut self,
        incoming: Vec<Brush>,
    ) -> Result<(usize, usize, Option<usize>), String> {
        let mut brushes = self.brushes.clone();
        let mut added = 0;
        let mut updated = 0;
        let mut last = None;
        let mut seen = std::collections::HashSet::new();
        for mut brush in incoming {
            validate_brush(&brush)?;
            brush.ensure_identity();
            let id = brush.identity();
            if let Some(index) = brushes.iter().position(|b| b.identity() == id) {
                if brushes[index] != brush {
                    brushes[index] = brush;
                    if seen.insert(id) {
                        updated += 1;
                    }
                }
                last = Some(index);
            } else {
                if brushes.len() >= 256 {
                    return Err("Brush library is limited to 256 presets".into());
                }
                last = Some(brushes.len());
                brushes.push(brush);
                seen.insert(id);
                added += 1;
            }
        }
        if brushes != self.brushes {
            self.brushes = brushes;
            self.dirty = true;
        }
        Ok((added, updated, last))
    }
    pub fn duplicate_brush(&mut self, index: usize) -> Result<usize, String> {
        let mut brush = self
            .brushes
            .get(index)
            .ok_or("Brush no longer exists")?
            .clone();
        brush.name.push_str(" copy");
        brush.renew_identity();
        self.append_brush(brush)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::Vec3;
    #[test]
    fn starting_sphere_has_a_group_and_visibility_is_saved_and_undoable() {
        let mut d = Document::default();
        assert_eq!(d.mesh().vertex_groups[0].name, "Base");
        assert!(d.mesh().vertex_groups[0].weights.iter().all(|&w| w == 1.));
        assert!(d.mesh().polygroups.iter().all(|&id| id == 1));
        assert!(!d.is_dirty());
        d.geometry.subdivide().unwrap();
        let before = d.geometry_snapshot();
        d.set_vertex_group_visible(0, false).unwrap();
        assert!(!d.mesh().vertex_groups[0].visible);
        assert!(!d.mesh_settings().lower[0].vertex_groups[0].visible);
        assert_eq!(d.mesh().vertices, before.mesh.vertices);
        let json = serde_json::to_string(&d.project()).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        loaded.validate().unwrap();
        assert!(!loaded.mesh.vertex_groups[0].visible);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert!(!d.mesh().vertex_groups[0].visible);
        d.new_sphere();
        assert!(d.mesh().vertex_groups[0].visible);
        assert_eq!(d.mesh().vertex_groups.len(), 1);
        assert!(!d.can_undo());
    }
    #[test]
    fn mask_threshold_controls_grouping_and_deletion_per_submodel() {
        let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        mesh.mask.fill(0.6);
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(mesh)).unwrap();
        d.set_mask_threshold(0.8).unwrap();
        assert!(!d.delete_masked().unwrap());
        assert!(d.create_vertex_group("Surface".into(), false).is_err());
        d.set_mask_threshold(0.6).unwrap();
        d.end_mask_adjustment();
        d.create_vertex_group("Surface".into(), false).unwrap();
        assert_eq!(d.mesh().polygroups, vec![1; 2]);
        let before = d.geometry_snapshot();
        assert!(d.delete_masked().unwrap());
        assert!(d.mesh().vertices.is_empty());
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.duplicate_model(false).unwrap();
        d.set_mask_threshold(0.9).unwrap();
        assert_eq!(d.model(0).unwrap().metadata.mask_threshold, 0.9);
        assert_eq!(d.model(1).unwrap().metadata.mask_threshold, 0.6);
        d.project().validate().unwrap();
    }
    #[test]
    fn intensity_preserves_soft_ratios_and_undo_restores_the_mask() {
        let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        mesh.mask = vec![0., 0.2, 0.4, 0.8];
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(mesh)).unwrap();
        let before = d.geometry_snapshot();
        d.set_mask_intensity(0.4).unwrap();
        assert!(
            d.mesh()
                .mask
                .iter()
                .zip([0., 0.1, 0.2, 0.4])
                .all(|(&a, b)| (a - b).abs() < 1e-6)
        );
        assert_eq!(d.mask_peak(), 0.4);
        d.set_mask_intensity(1.).unwrap();
        assert!(
            d.mesh()
                .mask
                .iter()
                .zip([0., 0.25, 0.5, 1.])
                .all(|(&a, b)| (a - b).abs() < 1e-6)
        );
        assert_eq!(d.mask_peak(), 1.);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn groups_and_mask_intensity_are_independent_undoable_and_survive_levels() {
        let mut geometry = Geometry::new(
            crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube)
                .mesh()
                .unwrap(),
        );
        geometry.subdivide().unwrap();
        geometry.mesh.mask[geometry.settings.lower[0].vertices.len()] = 0.8;
        let mut d = Document::default();
        d.replace_geometry(geometry).unwrap();
        d.duplicate_model(false).unwrap();
        let before = d.geometry_snapshot();
        d.create_vertex_group("Fine detail".into(), false).unwrap();
        let grouped = d.geometry_snapshot();
        assert_eq!(grouped.submodels, before.submodels);
        assert!(
            grouped.settings.lower[0].vertex_groups[0]
                .weights
                .iter()
                .all(|&w| w == 0.)
        );
        assert_eq!(
            grouped.mesh.vertex_groups[0]
                .weights
                .iter()
                .filter(|&&w| w > 0.)
                .count(),
            1
        );
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), grouped);
        let count = d.history_counts().0;
        d.set_mask_intensity(0.).unwrap();
        d.set_mask_intensity(0.4).unwrap();
        assert!((d.mask_peak() - 0.4).abs() < 1e-6);
        assert_eq!(d.history_counts().0, count + 1);
        assert_eq!(d.mesh().vertex_groups, grouped.mesh.vertex_groups);
        d.end_mask_adjustment();
        d.mask_from_group(0).unwrap();
        assert_eq!(d.mesh().mask, grouped.mesh.mask);
        d.set_mesh_level(1).unwrap();
        d.set_mesh_level(2).unwrap();
        assert_eq!(d.mesh().vertex_groups, grouped.mesh.vertex_groups);
        d.rename_vertex_group(0, "Detail".into()).unwrap();
        let project = d.project();
        project.validate().unwrap();
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(
            serde_json::to_value(loaded).unwrap(),
            serde_json::to_value(project).unwrap()
        );
    }
    #[test]
    fn thickness_group_preview_matches_apply_and_undo() {
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(
            crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
                .mesh()
                .unwrap(),
        ))
        .unwrap();
        d.set_temporary_thickness(0.2).unwrap();
        let model = d.model(0).unwrap();
        let mut options = model.metadata.clone();
        options.thickness_groups = true;
        options.thickness_mask_rim = true;
        d.set_model_metadata(0, model.name.into(), options.clone())
            .unwrap();
        let before = d.geometry_snapshot();
        let expected = d.mesh().solidify_with_metadata(&options).unwrap();
        d.apply_thickness().unwrap();
        assert_eq!(d.mesh(), &expected);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.mesh(), &expected);
    }
    #[test]
    fn whole_stroke_is_one_undo_step_and_new_edits_clear_redo() {
        let mut document = Document::default();
        let original = document.geometry.mesh.vertices[0].1;
        let brush = document.brushes[0].clone();
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        document.apply_dab(&brush, dab);
        document.apply_dab(&brush, dab);
        document.end_stroke();
        let sculpted = document.geometry.mesh.vertices[0].1;
        assert!(sculpted > original);
        document.undo();
        assert_eq!(document.geometry.mesh.vertices[0].1, original);
        assert!(!document.can_undo());
        document.redo();
        assert_eq!(document.geometry.mesh.vertices[0].1, sculpted);
        document.undo();
        document.apply_dab(&brush, dab);
        assert!(!document.can_redo());
    }
    #[test]
    fn invalid_project_does_not_replace_content_or_history() {
        let mut document = Document::default();
        document.clear_mask();
        let original = serde_json::to_value(document.project()).unwrap();
        let mut invalid = document.project();
        invalid.mesh.mask.clear();
        assert!(document.replace_project(invalid, "invalid.clay").is_err());
        assert_eq!(serde_json::to_value(document.project()).unwrap(), original);
        assert!(document.is_dirty());
        assert!(document.can_undo());
        assert_eq!(document.current_path(), "sculpt.clay");
    }
}

#[cfg(test)]
mod mesh_tests {
    use super::*;
    #[test]
    fn topology_changes_restore_the_entire_stack_with_undo_and_project_roundtrip() {
        let mut document = Document::default();
        let original = document.mesh().clone();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document.replace_geometry(geometry).unwrap();
        assert_eq!(document.mesh_settings().level(), 2);
        document.set_mesh_level(1).unwrap();
        let project = document.project();
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
        loaded.validate().unwrap();
        let mut restored = Document::default();
        restored.replace_project(loaded, "roundtrip.clay").unwrap();
        restored.set_mesh_level(2).unwrap();
        assert_eq!(
            restored.mesh().triangles.len(),
            original.triangles.len() * 4
        );
        document.set_dynamic_mesh(true).unwrap();
        assert_eq!(document.mesh_settings().levels(), 1);
        document.undo();
        assert!(!document.mesh_settings().dynamic);
        assert_eq!(document.mesh_settings().levels(), 2);
        assert_eq!(document.mesh_settings().level(), 1);
        document.undo();
        assert_eq!(document.mesh_settings().level(), 2);
        document.undo();
        assert_eq!(document.mesh(), &original);
        assert_eq!(document.mesh_settings().levels(), 1);
        document.redo();
        assert_eq!(document.mesh_settings().levels(), 2);
    }
    #[test]
    fn invalid_stack_is_transactional_and_mask_strokes_do_not_request_remeshing() {
        let mut document = Document::default();
        let original = document.geometry_snapshot();
        let mut invalid = original.clone();
        invalid.settings.lower.push(original.mesh.clone());
        assert!(document.replace_geometry(invalid).is_err());
        assert_eq!(document.geometry_snapshot(), original);
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
        let brush = Brush::new(Kind::Mask);
        document.apply_dab(
            &brush,
            Dab {
                center: crate::sculpt::Vec3(0., 1., 0.),
                normal: crate::sculpt::Vec3(0., 1., 0.),
                right: crate::sculpt::Vec3(1., 0., 0.),
                up: crate::sculpt::Vec3(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: crate::sculpt::Vec3::default(),
                invert: false,
            },
        );
        assert!(!document.end_stroke());
        assert!(document.mesh().mask[0] > 0.);
    }
}

#[cfg(test)]
mod protected_stroke_tests {
    use super::*;
    #[test]
    fn black_alpha_and_fully_masked_strokes_do_not_trigger_auto_remesh() {
        let mut document = Document::default();
        let dab = Dab {
            center: crate::sculpt::Vec3(0., 1., 0.),
            normal: crate::sculpt::Vec3(0., 1., 0.),
            right: crate::sculpt::Vec3(1., 0., 0.),
            up: crate::sculpt::Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: crate::sculpt::Vec3::default(),
            invert: false,
        };
        let mut brush = Brush::new(Kind::Standard);
        brush.alpha = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.].into(),
        };
        document.apply_dab(&brush, dab);
        assert!(!document.end_stroke());
        brush.alpha = Alpha::Soft;
        document.geometry.mesh.mask.fill(1.);
        document.apply_dab(&brush, dab);
        assert!(!document.end_stroke());
    }
}

#[cfg(test)]
mod delete_level_tests {
    use super::*;
    #[test]
    fn deleting_either_side_preserves_active_surface_and_restores_stack_on_undo() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.mesh.vertices.last_mut().unwrap().0 += 0.1;
        *geometry.mesh.mask.last_mut().unwrap() = 0.6;
        geometry.set_level(2).unwrap();
        document.replace_geometry(geometry).unwrap();
        let original = document.geometry_snapshot();
        document.delete_mesh_levels(true).unwrap();
        assert_eq!(document.mesh(), &original.mesh);
        assert_eq!(document.mesh_settings().level(), 1);
        assert_eq!(document.mesh_settings().upper, original.settings.upper);
        document.set_mesh_level(2).unwrap();
        document.mesh_settings().validate(document.mesh()).unwrap();
        document.undo();
        document.undo();
        assert_eq!(document.geometry_snapshot(), original);
        document.delete_mesh_levels(false).unwrap();
        assert_eq!(document.mesh(), &original.mesh);
        assert_eq!(document.mesh_settings().level(), 2);
        assert_eq!(document.mesh_settings().levels(), 2);
        assert_eq!(document.mesh_settings().lower, original.settings.lower);
        let mut fresh = document.geometry_snapshot();
        fresh.subdivide().unwrap();
        fresh.settings.validate(&fresh.mesh).unwrap();
        document.undo();
        assert_eq!(document.geometry_snapshot(), original);
        document.redo();
        assert!(document.mesh_settings().upper.is_empty());
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(&document.project()).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.mesh_settings, document.mesh_settings().clone());
    }
    #[test]
    fn deleting_absent_levels_is_a_clean_no_op() {
        let mut document = Document::default();
        document.delete_mesh_levels(true).unwrap();
        document.delete_mesh_levels(false).unwrap();
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
    }
}

#[cfg(test)]
mod history_feedback_tests {
    use super::*;
    #[test]
    fn no_effect_strokes_keep_redo_and_each_mesh_edit_has_a_history_label() {
        let mut document = Document::default();
        document.set_dynamic_mesh(true).unwrap();
        document.undo();
        assert_eq!(document.redo_label(), Some("Switch mesh mode"));
        let mut brush = Brush::new(Kind::Standard);
        brush.alpha = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.].into(),
        };
        document.apply_dab(
            &brush,
            Dab {
                center: crate::sculpt::Vec3(0., 1., 0.),
                normal: crate::sculpt::Vec3(0., 1., 0.),
                right: crate::sculpt::Vec3(1., 0., 0.),
                up: crate::sculpt::Vec3(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: crate::sculpt::Vec3::default(),
                invert: false,
            },
        );
        document.end_stroke();
        assert_eq!(document.history_counts(), (0, 1));
        document.redo();
        assert!(document.mesh_settings().dynamic);
        document.undo();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document
            .replace_geometry_named(geometry, "Subdivide mesh")
            .unwrap();
        assert_eq!(document.undo_label(), Some("Subdivide mesh"));
        assert!(!document.can_redo());
        document.delete_mesh_levels(true).unwrap();
        assert_eq!(document.undo_label(), Some("Delete lower levels"));
        document.undo();
        assert_eq!(document.redo_label(), Some("Delete lower levels"));
        document.set_mesh_level(1).unwrap();
        assert_eq!(document.undo_label(), Some("Change subdivision level"));
        document.delete_mesh_levels(false).unwrap();
        assert_eq!(document.undo_label(), Some("Delete higher levels"));
    }
}

#[cfg(test)]
mod adaptive_mode_tests {
    use super::*;
    #[test]
    fn adaptive_mode_discards_levels_undo_restores_them_and_other_modes_exclude_it() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document.replace_geometry(geometry).unwrap();
        let before = document.geometry_snapshot();
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(document.mesh_settings().levels(), 1);
        assert!(!document.mesh_settings().dynamic);
        assert_eq!(document.mesh(), &before.mesh);
        document.undo();
        assert_eq!(document.geometry_snapshot(), before);
        document.redo();
        assert!(document.mesh_settings().adaptive.enabled);
        document.set_dynamic_mesh(true).unwrap();
        assert!(!document.mesh_settings().adaptive.enabled);
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        document.set_dynamic_mesh(false).unwrap();
        assert!(!document.mesh_settings().adaptive.enabled);
    }
}

#[cfg(test)]
mod adaptive_validation_tests {
    use super::*;
    #[test]
    fn invalid_adaptive_project_is_rejected_and_live_detail_edits_do_not_flood_history() {
        let mut document = Document::default();
        let original = serde_json::to_value(document.project()).unwrap();
        let mut invalid = document.project();
        invalid.mesh.triangles.pop();
        invalid.mesh_settings.adaptive.enabled = true;
        assert!(
            document
                .replace_project(invalid, "invalid-adaptive.clay")
                .is_err()
        );
        assert_eq!(serde_json::to_value(document.project()).unwrap(), original);
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        let count = document.history_counts();
        for detail in [4., 5., 6., 7.] {
            let mut settings = document.mesh_settings().adaptive.clone();
            settings.detail = detail;
            document.set_adaptive(settings).unwrap();
        }
        assert_eq!(document.history_counts(), count);
        document.undo();
        assert!(!document.mesh_settings().adaptive.enabled);
    }
}

#[cfg(test)]
mod adaptive_smooth_tests {
    use super::*;
    use crate::sculpt::Vec3;

    #[test]
    fn smooth_cleanup_reduces_density_preserves_masks_and_undoes_as_one_stroke() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.mesh = geometry.mesh.subdivide().unwrap();
        geometry.mesh.vertices[0].1 += 0.06;
        for (v, mask) in geometry.mesh.vertices.iter().zip(&mut geometry.mesh.mask) {
            if v.1 < 0.5 {
                *mask = 1.;
            }
        }
        document
            .replace_geometry_named(geometry, "Dense test surface")
            .unwrap();
        document
            .set_adaptive(crate::sculpt::AdaptiveSettings {
                enabled: true,
                detail: 16.,
                ..Default::default()
            })
            .unwrap();
        let before = document.mesh().clone();
        let history = document.history_counts().0;
        let mut brush = Brush::new(Kind::Smooth);
        brush.strength = 0.6;
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.4,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        for _ in 0..6 {
            document.begin_sculpt_frame();
            let count = document.mesh().vertices.len();
            document.apply_dab(&brush, dab);
            assert!(document.mesh().vertices.len() <= count);
        }
        document.end_stroke();
        let after = document.mesh().clone();
        assert!(after.vertices.len() < before.vertices.len() - 50);
        assert!(
            after
                .vertices
                .iter()
                .map(|v| v.1)
                .fold(f32::NEG_INFINITY, f32::max)
                < 1.03
        );
        after.validate_closed().unwrap();
        let protected: std::collections::HashSet<_> = after
            .vertices
            .iter()
            .zip(&after.mask)
            .filter(|(_, m)| **m == 1.)
            .map(|(v, _)| (v.0.to_bits(), v.1.to_bits(), v.2.to_bits()))
            .collect();
        for (v, mask) in before.vertices.iter().zip(&before.mask) {
            if *mask == 1. {
                assert!(
                    protected.contains(&(v.0.to_bits(), v.1.to_bits(), v.2.to_bits())),
                    "protected vertex or mask changed"
                );
            }
        }
        assert_eq!(document.history_counts().0, history + 1);
        document.undo();
        assert_eq!(document.mesh(), &before);
        document.redo();
        assert_eq!(document.mesh(), &after);
    }
}

#[cfg(test)]
mod submodel_tests {
    use super::*;

    #[test]
    fn duplication_preserves_stack_and_switching_and_creation_undo_as_a_scene() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.mesh.mask[0] = 0.8;
        document.replace_geometry(geometry).unwrap();
        let source = document.geometry_snapshot();
        document.duplicate_model(false).unwrap();
        assert_eq!(document.mesh(), &source.mesh);
        assert_eq!(document.mesh_settings(), &source.settings);
        assert_eq!(
            document.submodel_names().collect::<Vec<_>>(),
            vec!["Copy 1", "Model"]
        );
        document.clear_mask();
        document.select_submodel(1).unwrap();
        assert_eq!(document.mesh().mask[0], 0.8);
        document.undo();
        assert_eq!(document.geometry.name, "Copy 1");
        assert_eq!(document.mesh().mask[0], 0.);
        document.undo();
        assert_eq!(document.mesh().mask[0], 0.8);
        document.undo();
        assert_eq!(document.geometry_snapshot(), source);
        document.redo();
        let project: Project =
            serde_json::from_slice(&serde_json::to_vec(&document.project()).unwrap()).unwrap();
        let mut restored = Document::default();
        restored.replace_project(project, "submodels.clay").unwrap();
        assert_eq!(restored.geometry_snapshot(), document.geometry_snapshot());
        let before = restored.geometry_snapshot();
        let mut invalid = restored.project();
        invalid.submodels[0].mesh.mask.clear();
        assert!(restored.replace_project(invalid, "bad.clay").is_err());
        assert_eq!(restored.geometry_snapshot(), before);
    }

    #[test]
    fn extraction_compacts_selected_faces_keeps_source_and_fails_transactionally() {
        let mut document = Document::default();
        let before = document.geometry_snapshot();
        assert!(document.duplicate_model(true).is_err());
        assert_eq!(document.geometry_snapshot(), before);
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
        for (vertex, mask) in document
            .geometry
            .mesh
            .vertices
            .iter()
            .zip(&mut document.geometry.mesh.mask)
        {
            if vertex.1 > 0.7 {
                *mask = 0.5;
            }
        }
        let source = document.geometry_snapshot();
        let faces = source
            .mesh
            .triangles
            .iter()
            .filter(|face| face.iter().all(|&i| source.mesh.mask[i] >= 0.5))
            .count();
        document.duplicate_model(true).unwrap();
        assert_eq!(document.mesh().triangles.len(), faces);
        assert!(faces > 0 && faces < source.mesh.triangles.len());
        assert!(document.mesh().vertices.len() < source.mesh.vertices.len());
        assert!(document.mesh().mask.iter().all(|&mask| mask == 0.));
        assert_eq!(document.geometry.submodels[0].mesh, source.mesh);
        document.mesh().validate().unwrap();
        document.undo();
        assert_eq!(document.geometry_snapshot(), source);
        document.redo();
        assert_eq!(document.geometry.name, "Extract 1");
    }
}

#[cfg(test)]
mod scene_transform_tests {
    use super::*;
    use crate::sculpt::{ModelMetadata, Transform, Vec3};
    #[test]
    fn transforms_follow_all_levels_preserve_inactive_models_and_undo_once_per_drag() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.mesh.mask[0] = 1.;
        geometry.settings.lower[0].mask[0] = 1.;
        document.replace_geometry(geometry).unwrap();
        document.duplicate_model(false).unwrap();
        let before = document.geometry_snapshot();
        let count = document.history_counts();
        let transform = Transform {
            pivot: Vec3(0.2, 0.1, 0.),
            translation: Vec3(0.3, 0., 0.),
            scale: Vec3(0.8, 0.8, 0.8),
            ..Default::default()
        };
        for _ in 0..3 {
            document.apply_transform(transform, true).unwrap();
        }
        assert!(
            !document.end_stroke(),
            "transforms must not trigger automatic remeshing"
        );
        assert_eq!(document.history_counts().0, count.0 + 1);
        assert_eq!(document.geometry.submodels, before.submodels);
        assert_eq!(document.mesh().vertices[0], before.mesh.vertices[0]);
        assert_eq!(
            document.mesh_settings().lower[0].vertices[0],
            before.settings.lower[0].vertices[0]
        );
        let expected = transform.point(transform.point(transform.point(before.mesh.vertices[1])));
        assert!((document.mesh().vertices[1] - expected).len() < 1e-5);
        document.mesh_settings().validate(document.mesh()).unwrap();
        let after = document.geometry_snapshot();
        document.undo();
        assert_eq!(document.geometry_snapshot(), before);
        document.redo();
        assert_eq!(document.geometry_snapshot(), after);
        document.undo();
        document.apply_transform(transform, false).unwrap();
        document.end_stroke();
        assert!(
            (document.mesh().vertices[0] - transform.point(before.mesh.vertices[0])).len() < 1e-5
        );
        assert!(!document.can_redo());
    }
    #[test]
    fn invalid_identity_and_hidden_transforms_are_transactional() {
        let mut document = Document::default();
        let before = document.geometry_snapshot();
        assert!(
            !document
                .apply_transform(
                    Transform {
                        pivot: Vec3(0.17, 0.24, 0.33),
                        ..Default::default()
                    },
                    true
                )
                .unwrap()
        );
        assert!(
            document
                .apply_transform(
                    Transform {
                        scale: Vec3(0., 1., 1.),
                        ..Default::default()
                    },
                    false
                )
                .is_err()
        );
        assert!(
            document
                .apply_transform(
                    Transform {
                        translation: Vec3(f32::NAN, 0., 0.),
                        ..Default::default()
                    },
                    false
                )
                .is_err()
        );
        assert_eq!(document.geometry_snapshot(), before);
        assert!(!document.can_undo());
        document
            .set_model_metadata(
                0,
                "Model".into(),
                ModelMetadata {
                    visible: false,
                    group: String::new(),
                    ..Default::default()
                },
            )
            .unwrap();
        let hidden = document.geometry_snapshot();
        assert!(
            document
                .apply_transform(
                    Transform {
                        translation: Vec3(1., 0., 0.),
                        ..Default::default()
                    },
                    false
                )
                .is_err()
        );
        assert_eq!(document.geometry_snapshot(), hidden);
    }
    #[test]
    fn groups_order_and_visibility_survive_selection_roundtrip_and_undo() {
        let mut document = Document::default();
        document.duplicate_model(false).unwrap();
        document.duplicate_model(false).unwrap();
        document.add_group("Body".into()).unwrap();
        document.add_group("Details".into()).unwrap();
        document
            .set_model_metadata(
                0,
                "Eyes".into(),
                ModelMetadata {
                    visible: true,
                    group: "Details".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        document
            .set_model_metadata(
                2,
                "Base".into(),
                ModelMetadata {
                    visible: true,
                    group: "Body".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let names: Vec<_> = document.submodel_names().map(str::to_string).collect();
        document.select_submodel(2).unwrap();
        assert_eq!(
            document.submodel_names().collect::<Vec<_>>(),
            names.iter().map(String::as_str).collect::<Vec<_>>()
        );
        assert_eq!(document.active_model_index(), 2);
        document.reorder_model(2, 0).unwrap();
        assert_eq!(document.active_model_index(), 0);
        assert_eq!(document.submodel_names().next(), Some("Base"));
        document.set_group_visible("Body", false).unwrap();
        assert!(!document.active_visible());
        let hidden = document.geometry_snapshot();
        document.undo();
        assert!(document.active_visible());
        document.redo();
        assert_eq!(document.geometry_snapshot(), hidden);
        document.reorder_group(1, 0).unwrap();
        assert_eq!(document.groups(), &["Details", "Body"]);
        let json = serde_json::to_vec(&document.project()).unwrap();
        let project: Project = serde_json::from_slice(&json).unwrap();
        let mut restored = Document::default();
        restored.replace_project(project, "scene.clay").unwrap();
        assert_eq!(restored.geometry_snapshot(), document.geometry_snapshot());
        restored.remove_group("Body").unwrap();
        assert_eq!(restored.model(0).unwrap().metadata.group, "");
        assert_eq!(restored.model_count(), 3);
        restored.undo();
        assert_eq!(restored.geometry_snapshot(), document.geometry_snapshot());
        let mut invalid = restored.project();
        invalid.scene.active_index = 10;
        assert!(restored.replace_project(invalid, "bad.clay").is_err());
        let mut invalid = restored.project();
        invalid.submodels[0].metadata.group = "Missing".into();
        assert!(restored.replace_project(invalid, "bad.clay").is_err());
        assert_eq!(restored.geometry_snapshot(), document.geometry_snapshot());
    }
}

#[cfg(test)]
mod primitive_tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, Transform, Vec3};
    #[test]
    fn primitive_replacement_conversion_and_parameter_drags_restore_with_undo() {
        let mut d = Document::default();
        d.duplicate_model(false).unwrap();
        let mut geometry = d.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.mesh.mask[0] = 0.7;
        d.replace_geometry(geometry).unwrap();
        let before = d.geometry_snapshot();
        d.initialize_primitive().unwrap();
        assert!(d.primitive().is_some());
        assert_eq!(d.mesh_settings().levels(), 1);
        assert!(d.mesh().mask.iter().all(|m| *m == 0.));
        assert_eq!(d.geometry.submodels, before.submodels);
        let initialized = d.geometry_snapshot();
        let count = d.history_counts();
        for size in [2.2, 2.4, 2.6] {
            let mut p = d.primitive().unwrap().clone();
            p.size.0 = size;
            d.update_primitive(p).unwrap();
        }
        d.end_primitive_edit();
        assert_eq!(d.history_counts().0, count.0 + 1);
        let preview = d.geometry_snapshot();
        d.convert_primitive().unwrap();
        assert!(d.primitive().is_none());
        assert_eq!(d.mesh(), &preview.mesh);
        d.undo();
        assert_eq!(d.geometry_snapshot(), preview);
        d.undo();
        assert_eq!(d.geometry_snapshot(), initialized);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), initialized);
    }
    #[test]
    fn live_primitive_blocks_brushes_topology_and_invalid_recipe_updates() {
        let mut d = Document::default();
        d.initialize_primitive().unwrap();
        let original = d.geometry_snapshot();
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.5,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        assert!(!d.apply_dab(&Brush::new(Kind::Standard), dab));
        assert!(!d.apply_dab(&Brush::new(Kind::Mask), dab));
        assert!(d.set_dynamic_mesh(true).is_err());
        assert!(
            d.set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .is_err()
        );
        let mut invalid = d.primitive().unwrap().clone();
        invalid.divisions[0] = 0;
        assert!(d.update_primitive(invalid).is_err());
        assert_eq!(d.geometry_snapshot(), original);
        let mut ring = Primitive::for_kind(PrimitiveKind::Circle);
        ring.filled = false;
        d.update_primitive(ring).unwrap();
        d.end_primitive_edit();
        assert!(d.convert_primitive().is_err());
    }
    #[test]
    fn primitive_recipes_follow_transforms_duplicate_switch_and_project_roundtrip() {
        let mut d = Document::default();
        d.initialize_primitive().unwrap();
        let transform = Transform {
            pivot: Vec3(0.1, 0.2, 0.3),
            translation: Vec3(0.4, 0., 0.),
            scale: Vec3(0.8, 1.2, 1.1),
            stretch: Some((Vec3(1., 1., 0.).norm(), 0.7)),
        };
        d.apply_transform(transform, false).unwrap();
        d.end_stroke();
        d.mesh_settings().validate(d.mesh()).unwrap();
        let mut p = d.primitive().unwrap().clone();
        let center = p.center;
        let basis = p.basis;
        p.divisions = [3, 4, 5];
        d.update_primitive(p).unwrap();
        d.end_primitive_edit();
        assert_eq!(d.primitive().unwrap().center, center);
        assert_eq!(d.primitive().unwrap().basis, basis);
        d.duplicate_model(false).unwrap();
        d.select_submodel(1).unwrap();
        let json = serde_json::to_vec(&d.project()).unwrap();
        let project: Project = serde_json::from_slice(&json).unwrap();
        let mut loaded = Document::default();
        loaded.replace_project(project, "primitive.clay").unwrap();
        assert_eq!(loaded.geometry_snapshot(), d.geometry_snapshot());
        let mut invalid = loaded.project();
        invalid.mesh.vertices[0].0 += 0.1;
        assert!(loaded.replace_project(invalid, "bad.clay").is_err());
        assert_eq!(loaded.geometry_snapshot(), d.geometry_snapshot());
    }
}

#[cfg(test)]
mod brush_identity_tests {
    use super::*;
    #[test]
    fn brush_import_updates_identity_without_duplicates() {
        let mut doc = Document::default();
        let library = doc.brushes().to_vec();
        let count = library.len();
        doc.mark_saved("test.clay");
        assert_eq!(doc.import_brushes(library.clone()).unwrap().0, 0);
        assert!(!doc.is_dirty());
        let copy = doc.duplicate_brush(0).unwrap();
        assert_ne!(doc.brushes()[copy].identity(), doc.brushes()[0].identity());
        let mut brush = doc.brushes()[copy].clone();
        let id = brush.identity();
        brush.name = "Edited copy".into();
        brush.icon = Some(crate::brush_icon::default_icon(Kind::Crease));
        brush.id = Some(Brush::new(Kind::Mask).identity());
        doc.update_brush(copy, brush).unwrap();
        assert_eq!(doc.brushes()[copy].identity(), id);
        let mut imported = doc.brushes()[copy].clone();
        imported.strength = 0.37;
        assert_eq!(
            doc.import_brushes(vec![imported.clone(), imported])
                .unwrap(),
            (0, 1, Some(copy))
        );
        let exported = doc.brushes().to_vec();
        doc.mark_saved("test.clay");
        assert_eq!(doc.import_brushes(exported).unwrap().0, 0);
        assert!(!doc.is_dirty());
        assert_eq!(doc.brushes().len(), count + 1);
    }
    #[test]
    fn brush_legacy_import_and_failed_batch_are_safe() {
        let mut doc = Document::default();
        let mut old = Brush::new(Kind::Clay);
        old.id = None;
        assert_eq!(old.identity(), Brush::new(Kind::Clay).identity());
        old.name = "Legacy custom".into();
        let index = doc.import_brushes(vec![old.clone()]).unwrap().2.unwrap();
        assert_eq!(doc.import_brushes(vec![old]).unwrap(), (0, 0, Some(index)));
        let before = serde_json::to_value(doc.project()).unwrap();
        doc.mark_saved("test.clay");
        let mut new = Brush::new(Kind::Grab);
        new.renew_identity();
        let mut bad = new.clone();
        bad.strength = f32::NAN;
        assert!(doc.import_brushes(vec![new, bad]).is_err());
        assert_eq!(serde_json::to_value(doc.project()).unwrap(), before);
        assert!(!doc.is_dirty());
    }
}

#[cfg(test)]
mod mask_edit_tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    fn cube_geometry() -> Geometry {
        let mut p = Primitive::for_kind(PrimitiveKind::Cube);
        p.divisions = [1; 3];
        Geometry::new(p.mesh().unwrap())
    }
    #[test]
    fn invert_mask_preserves_soft_values_and_all_levels_with_undo() {
        let mut geometry = cube_geometry();
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.set_level(2).unwrap();
        for mesh in std::iter::once(&mut geometry.mesh)
            .chain(geometry.settings.lower.iter_mut())
            .chain(geometry.settings.upper.iter_mut())
        {
            for (i, m) in mesh.mask.iter_mut().enumerate() {
                *m = (i % 5) as f32 * 0.25;
            }
        }
        let mut d = Document::default();
        d.replace_geometry(geometry).unwrap();
        d.duplicate_model(false).unwrap();
        let before = d.geometry_snapshot();
        let revision = d.display_revision();
        let count = d.history_counts().0;
        d.invert_mask().unwrap();
        let after = d.geometry_snapshot();
        assert_eq!(before.submodels, after.submodels);
        for (a, b) in std::iter::once(&before.mesh)
            .chain(before.settings.lower.iter())
            .chain(before.settings.upper.iter())
            .zip(
                std::iter::once(&after.mesh)
                    .chain(after.settings.lower.iter())
                    .chain(after.settings.upper.iter()),
            )
        {
            assert_eq!(a.vertices, b.vertices);
            assert!(
                a.mask
                    .iter()
                    .zip(&b.mask)
                    .all(|(a, b)| (*b - (1. - a)).abs() < 1e-6)
            );
        }
        assert_eq!(d.history_counts().0, count + 1);
        assert!(d.display_revision() > revision);
        assert_eq!(d.undo_label(), Some("Invert mask"));
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), after);
        d.invert_mask().unwrap();
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn delete_masked_cuts_and_compacts_preserving_remaining_quads_and_masks() {
        let mesh = Mesh {
            vertex_groups: Vec::new(),
            polygroups: Vec::new(),
            vertices: vec![
                crate::sculpt::Vec3(0., 0., 0.),
                crate::sculpt::Vec3(1., 0., 0.),
                crate::sculpt::Vec3(1., 1., 0.),
                crate::sculpt::Vec3(0., 1., 0.),
                crate::sculpt::Vec3(2., 0., 0.),
                crate::sculpt::Vec3(3., 0., 0.),
                crate::sculpt::Vec3(3., 1., 0.),
                crate::sculpt::Vec3(2., 1., 0.),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3], [4, 5, 7], [5, 6, 7]],
            quads: vec![[0, 1, 2, 3], [4, 5, 6, 7]],
            mask: vec![0.5, 0.6, 0.9, 1., 0.2, 0.49, 0., 0.8],
        };
        let cut = mesh.delete_masked().unwrap().unwrap();
        assert_eq!(cut.vertices, &mesh.vertices[4..]);
        assert_eq!(cut.mask, &mesh.mask[4..]);
        assert_eq!(cut.quads, vec![[0, 1, 2, 3]]);
        assert_eq!(cut.triangles, vec![[0, 1, 3], [1, 2, 3]]);
        cut.validate().unwrap();
        let mut mesh = cube_geometry().mesh;
        let face = mesh.quads[0];
        for i in [face[0], face[1], face[2]] {
            mesh.mask[i] = 1.;
        }
        let cut = mesh.delete_masked().unwrap().unwrap();
        assert_eq!(cut.triangles.len(), mesh.triangles.len() - 1);
        assert_eq!(cut.quads.len(), mesh.quads.len() - 1);
        cut.validate().unwrap();
        assert!(cut.validate_closed().is_err());
    }
    #[test]
    fn delete_masked_resets_levels_and_closed_modes_and_undo_restores_everything() {
        let mut geometry = cube_geometry();
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.set_level(2).unwrap();
        let q = geometry.mesh.quads[0];
        for i in q {
            geometry.mesh.mask[i] = 1.;
        }
        geometry.settings.auto_remesh = true;
        let mut d = Document::default();
        d.replace_geometry(geometry).unwrap();
        d.duplicate_model(false).unwrap();
        let before = d.geometry_snapshot();
        let count = d.history_counts().0;
        assert!(d.delete_masked().unwrap());
        let after = d.geometry_snapshot();
        assert_eq!(after.submodels, before.submodels);
        assert_eq!(d.mesh_settings().levels(), 1);
        assert!(
            !d.mesh_settings().auto_remesh
                && !d.mesh_settings().adaptive.enabled
                && !d.mesh_settings().dynamic
        );
        assert!(d.mesh().triangles.len() < before.mesh.triangles.len());
        assert!(d.mesh().validate_closed().is_err());
        assert_eq!(d.history_counts().0, count + 1);
        assert_eq!(d.undo_label(), Some("Delete masked surface"));
        d.project().validate().unwrap();
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), after);
    }
    #[test]
    fn deleting_all_masked_surface_leaves_an_empty_model_that_saves_and_undoes() {
        let mut d = Document::default();
        d.replace_geometry(cube_geometry()).unwrap();
        let original = d.geometry_snapshot();
        let count = d.history_counts();
        assert!(!d.delete_masked().unwrap());
        assert_eq!(d.history_counts(), count);
        d.invert_mask().unwrap();
        let masked = d.geometry_snapshot();
        assert!(d.delete_masked().unwrap());
        assert!(
            d.mesh().vertices.is_empty()
                && d.mesh().triangles.is_empty()
                && d.mesh().quads.is_empty()
        );
        let saved = serde_json::to_string(&d.project()).unwrap();
        let project: Project = serde_json::from_str(&saved).unwrap();
        project.validate().unwrap();
        let mut restored = Document::default();
        restored.replace_project(project, "empty.clay").unwrap();
        assert!(restored.mesh().vertices.is_empty());
        restored.initialize_primitive().unwrap();
        assert!(!restored.mesh().vertices.is_empty());
        d.undo();
        assert_eq!(d.geometry_snapshot(), masked);
        d.undo();
        assert_eq!(d.geometry_snapshot(), original);
    }
}

#[cfg(test)]
mod thickness_edit_tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    #[test]
    fn temporary_thickness_is_per_model_persistent_and_bakes_undoably() {
        let plane = Primitive::for_kind(PrimitiveKind::Plane);
        let mut geometry = Geometry::new(plane.mesh().unwrap());
        geometry.settings.primitive = Some(plane);
        let mut d = Document::default();
        d.replace_geometry(geometry).unwrap();
        let original = d.geometry_snapshot();
        let count = d.history_counts().0;
        d.set_temporary_thickness(0.1).unwrap();
        d.set_temporary_thickness(0.2).unwrap();
        assert_eq!(
            d.history_counts().0,
            count + 1,
            "one slider drag must be one undo step"
        );
        d.end_thickness_edit();
        assert_eq!(
            d.mesh(),
            &original.mesh,
            "preview must not change the editing surface"
        );
        let saved = serde_json::to_string(&d.project()).unwrap();
        let loaded: Project = serde_json::from_str(&saved).unwrap();
        loaded.validate().unwrap();
        let mut restored = Document::default();
        restored.replace_project(loaded, "thickness.clay").unwrap();
        assert_eq!(restored.model(0).unwrap().metadata.temporary_thickness, 0.2);
        d.duplicate_model(false).unwrap();
        d.set_temporary_thickness(0.3).unwrap();
        d.end_thickness_edit();
        assert_eq!(d.model(1).unwrap().metadata.temporary_thickness, 0.2);
        let before = d.geometry_snapshot();
        d.apply_thickness().unwrap();
        let after = d.geometry_snapshot();
        d.mesh().validate_closed().unwrap();
        assert!(d.primitive().is_none());
        assert_eq!(d.mesh_settings().levels(), 1);
        assert_eq!(d.model(0).unwrap().metadata.temporary_thickness, 0.);
        assert_eq!(before.submodels, after.submodels);
        assert_eq!(d.undo_label(), Some("Apply thickness"));
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), after);
        let before = d.geometry_snapshot();
        assert!(d.set_temporary_thickness(0.2).is_err());
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn thickness_off_preserves_legacy_project_shape_and_invalid_values_fail_safely() {
        let mut d = Document::default();
        let before = serde_json::to_value(d.project()).unwrap();
        assert!(!before.to_string().contains("temporary_thickness"));
        let count = d.history_counts();
        assert!(d.set_temporary_thickness(f32::NAN).is_err());
        assert!(d.set_temporary_thickness(0.1).is_err());
        assert_eq!(d.history_counts(), count);
        assert_eq!(serde_json::to_value(d.project()).unwrap(), before);
        let mut invalid = d.project();
        invalid.scene.active_metadata.temporary_thickness = -1.;
        assert!(invalid.validate().is_err());
    }
}

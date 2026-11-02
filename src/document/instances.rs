//! Instance edits are transactional; previews never enter saves or history.
use super::*;
use crate::sculpt::{Vec3, instances::*};
mod brushes;

pub type ResolvedInstances = std::sync::Arc<ResolvedCopies>;

pub(super) struct Preview {
    pub library: Library,
    pub model_ids: Vec<(usize, u64)>,
    pub displayed: bool,
}

impl Document {
    pub fn instances(&self) -> &Library {
        self.instance_preview
            .as_ref()
            .map_or(&self.geometry.scene.instances, |p| &p.library)
    }
    pub fn instance_preview_active(&self) -> bool {
        self.instance_preview.is_some()
    }
    pub fn instance_model_id(&self, index: usize) -> u64 {
        self.instance_preview
            .as_ref()
            .and_then(|p| {
                p.model_ids
                    .iter()
                    .find(|(i, _)| *i == index)
                    .map(|(_, id)| *id)
            })
            .unwrap_or_else(|| self.model(index).map_or(0, |m| m.metadata.model_id))
    }
    pub fn instance_source(&self, source: Source) -> Option<&Mesh> {
        match source {
            Source::Model(id) => (0..self.model_count())
                .find(|&i| self.instance_model_id(i) == id)
                .and_then(|i| self.model(i).map(|m| m.mesh)),
            Source::Asset(id) => self
                .instances()
                .assets
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.mesh.as_ref()),
        }
    }
    /// Reuse recipe sampling across unchanged frames, selection and camera queries.
    /// Geometry/recipe edits invalidate this alongside the display snapshot.
    pub fn instance_placements(&self) -> Result<ResolvedInstances, String> {
        self.instance_cache
            .get_or_init(|| self.resolve_instance_placements())
            .clone()
    }
    fn resolve_instance_placements(&self) -> Result<ResolvedInstances, String> {
        let mut result = ResolvedCopies::default();
        let (mut vertices, mut triangles) = (0usize, 0usize);
        let mut resolver =
            ArrayResolver::new(self.instances()).with_sources(|s| self.instance_source(s));
        for set in &self.instances().sets {
            if !set.visible {
                continue;
            }
            if let Layout::Array(a) = &set.layout
                && let Some(owner) = a.owner
                && !(0..self.model_count()).any(|i| {
                    self.instance_model_id(i) == owner && self.model(i).unwrap().metadata.visible
                })
            {
                continue;
            }
            for (slot, p) in resolver.placements(set, self.paths())? {
                let source = self
                    .instance_source(p.piece.source)
                    .ok_or("Instance source is missing")?;
                vertices = vertices.saturating_add(source.vertices.len());
                triangles = triangles.saturating_add(source.triangles.len());
                if result.len() >= MAX_INSTANCES
                    || vertices > MAX_VERTICES
                    || triangles > MAX_TRIANGLES
                {
                    return Err("Instance display budget exceeded: reduce counts or source detail (256 copies, 1M vertices / 2M triangles)".into());
                }
                result.placements.push((set.id, slot, p));
            }
            if let Some(generated) =
                set.generated(self.paths(), |s| self.instance_source(s), &|| false)?
            {
                result.placements.retain(|(id, _, _)| *id != set.id);
                for copy in generated {
                    result.placements.push((set.id, copy.slot, copy.placement));
                    result
                        .meshes
                        .insert((set.id, copy.slot), std::sync::Arc::new(copy.mesh));
                }
            }
        }
        Ok(std::sync::Arc::new(result))
    }
    pub fn instance_piece(&mut self, index: usize) -> Result<Piece, String> {
        if self.model(index).is_none() {
            return Err("Source model no longer exists".into());
        }
        let mesh = self.model(index).unwrap().mesh;
        if mesh.triangles.is_empty() {
            return Err("Choose a source with surface geometry".into());
        }
        let pivot = mesh
            .vertices
            .iter()
            .copied()
            .fold(Vec3::default(), |a, b| a + b)
            * (1. / mesh.vertices.len().max(1) as f32);
        self.begin_instance_preview()?;
        Ok(Piece {
            source: Source::Model(self.instance_model_id(index)),
            pivot,
            forward: 0,
            up: 2,
        })
    }
    pub fn begin_instance_preview(&mut self) -> Result<(), String> {
        if self.instance_preview.is_some() {
            return Ok(());
        }
        self.end_edit();
        self.end_stroke();
        let mut last = (0..self.model_count())
            .map(|i| self.model(i).unwrap().metadata.model_id)
            .chain(self.paths().iter().map(|p| p.target))
            .max()
            .unwrap_or(0);
        let mut model_ids = Vec::new();
        for i in 0..self.model_count() {
            if self.model(i).unwrap().metadata.model_id == 0 {
                last = last.checked_add(1).ok_or("Model identity limit reached")?;
                model_ids.push((i, last));
            }
        }
        self.instance_preview = Some(Preview {
            library: self.geometry.scene.instances.clone(),
            model_ids,
            displayed: false,
        });
        Ok(())
    }
    pub fn next_instance_set_id(&self) -> Result<u64, String> {
        self.instances()
            .sets
            .iter()
            .map(|s| s.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Instance identity limit reached".into())
    }
    pub fn preview_instance_set(&mut self, set: Set) -> Result<(), String> {
        self.begin_instance_preview()?;
        if self.instances().sets.iter().any(|old| old == &set) {
            return Ok(());
        }
        let before = self.instances().clone();
        let preview = self.instance_preview.as_mut().unwrap();
        if let Some(old) = preview.library.sets.iter_mut().find(|s| s.id == set.id) {
            *old = set;
        } else {
            preview.library.sets.push(set);
        }
        self.instance_cache.take();
        if let Err(error) = self
            .validate_instances()
            .and_then(|()| self.instance_placements().map(|_| ()))
        {
            self.instance_preview.as_mut().unwrap().library = before;
            self.instance_cache.take();
            return Err(error);
        }
        let resolved = self.instance_cache.take();
        self.instances_display_changed();
        self.instance_preview.as_mut().unwrap().displayed = true;
        if let Some(resolved) = resolved {
            let _ = self.instance_cache.set(resolved);
        }
        Ok(())
    }
    fn validate_instances(&self) -> Result<(), String> {
        let models: Vec<_> = (0..self.model_count())
            .map(|i| (self.instance_model_id(i), self.model(i).unwrap().mesh))
            .collect();
        // Assets are immutable during recipe editing; full mesh/texture validation
        // belongs to capture/import and project loading, not every pointer sample.
        self.instances().validate_sets(&models, self.paths())
    }
    pub fn cancel_instance_preview(&mut self) {
        if self.instance_preview.take().is_some() {
            self.instances_display_changed();
        }
    }
    pub fn apply_instance_preview(&mut self) -> Result<(), String> {
        self.validate_instances()?;
        self.instance_placements()?;
        let Some(preview) = self.instance_preview.take() else {
            return Ok(());
        };
        if preview.library == self.geometry.scene.instances {
            self.display_changed_preserving_geometry();
            return Ok(());
        }
        self.record_edit("Edit instances");
        for (i, id) in preview.model_ids {
            if i == self.active_model_index() {
                self.geometry.scene.active_metadata.model_id = id;
            } else {
                let offset = usize::from(i > self.active_model_index());
                self.geometry.submodels[i - offset].metadata.model_id = id;
            }
        }
        self.geometry.scene.instances = preview.library;
        self.dirty = true;
        // The preview was already visible, including its effective model IDs.
        // Committing it changes save/history state, not its display geometry.
        if preview.displayed {
            self.display_changed_preserving_geometry();
        } else {
            self.instances_display_changed();
        }
        Ok(())
    }
    pub fn remove_instance_set(&mut self, id: u64) -> Result<(), String> {
        self.instances().ensure_array_unreferenced(id)?;
        self.cancel_instance_preview();
        if !self.instances().sets.iter().any(|s| s.id == id) {
            return Err("Instance set no longer exists".into());
        }
        self.begin_instance_preview()?;
        self.instance_preview
            .as_mut()
            .unwrap()
            .library
            .sets
            .retain(|s| s.id != id);
        self.instance_cache.take();
        self.apply_instance_preview()?;
        if self.selected_instance.is_some_and(|(set, _)| set == id) {
            self.selected_instance = None;
        }
        Ok(())
    }
    pub fn capture_object(
        &mut self,
        name: String,
        pivot: Vec3,
        forward: usize,
        up: usize,
    ) -> Result<u64, String> {
        self.capture_brush_object(None, name, pivot, forward, up, [0.; 3])
    }
    pub fn capture_brush_object(
        &mut self,
        brush: Option<u64>,
        name: String,
        pivot: Vec3,
        forward: usize,
        up: usize,
        capture_angles: [f32; 3],
    ) -> Result<u64, String> {
        self.cancel_instance_preview();
        self.end_edit();
        self.end_stroke();
        if self.mesh().triangles.is_empty() {
            return Err("Capture a model with surface geometry".into());
        }
        if self.mesh().vertices.len() > 100_000 || self.mesh().triangles.len() > 200_000 {
            return Err(
                "Object brush sources are limited to 100,000 vertices / 200,000 triangles".into(),
            );
        }
        let id = self
            .instances()
            .assets
            .iter()
            .map(|a| a.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Object identity limit reached")?;
        let mut library = self.instances().clone();
        let mut mesh = self.mesh().clone();
        for texture in &mut mesh.uv.textures {
            texture.id = crate::sculpt::texture::Texture::new_id();
            texture.exchange = None;
        }
        library.assets.push(Asset {
            id,
            name,
            mesh: std::sync::Arc::new(mesh),
            capture_angles,
            pivot,
            forward,
            up,
            options: Default::default(),
        });
        if let Some(brush) = brush {
            library
                .brushes
                .iter_mut()
                .find(|b| b.id == brush)
                .ok_or("Object brush no longer exists")?
                .objects
                .push(id);
        }
        let models: Vec<_> = (0..self.model_count())
            .map(|i| (self.instance_model_id(i), self.model(i).unwrap().mesh))
            .collect();
        library.validate(&models, self.paths())?;
        crate::sculpt::texture::validate_library(
            models
                .iter()
                .map(|(_, mesh)| *mesh)
                .chain(library.assets.iter().map(|a| a.mesh.as_ref())),
            &self.geometry.scene.texture_library,
        )?;
        self.record_edit("Capture object brush");
        self.geometry.scene.instances = library;
        self.dirty = true;
        self.display_changed();
        Ok(id)
    }
    pub fn detach_instance(&mut self, set_id: u64, slot: usize) -> Result<(), String> {
        self.cancel_instance_preview();
        if self.model_count() >= 64 {
            return Err(
                "Submodels are limited to 64; remove one before making a copy independent".into(),
            );
        }
        let mut set = self
            .instances()
            .sets
            .iter()
            .find(|s| s.id == set_id)
            .ok_or("Instance set no longer exists")?
            .clone();
        if set.joined() {
            return Err("Bake the joined assembly to create one editable model, or choose Separate before detaching a section".into());
        }
        let p = ArrayResolver::new(self.instances())
            .with_sources(|s| self.instance_source(s))
            .placements(&set, self.paths())?
            .into_iter()
            .find(|(i, _)| *i == slot)
            .ok_or("Instance is hidden or no longer exists")?
            .1;
        let resolved = self.instance_placements()?;
        let mesh = if let Some(mesh) = resolved.meshes.get(&(set_id, slot)) {
            mesh.as_ref().clone()
        } else if let Some(generated) =
            set.generated(self.paths(), |source| self.instance_source(source), &|| {
                false
            })?
        {
            // Hidden sets have no display cache; still detach the deformed surface.
            generated
                .into_iter()
                .find(|copy| copy.slot == slot)
                .ok_or("Instance is hidden or no longer exists")?
                .mesh
        } else {
            p.mesh(
                self.instance_source(p.piece.source)
                    .ok_or("Source is missing")?,
            )?
        };
        let name = format!(
            "{} copy {}",
            set.name.chars().take(90).collect::<String>(),
            slot + 1
        );
        if let Some(a) = set.adjustments.iter_mut().find(|a| a.slot == slot) {
            a.detached = true;
        } else {
            let mut a = Adjustment::new(slot);
            a.detached = true;
            set.adjustments.push(a);
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Make instance independent");
        *self
            .geometry
            .scene
            .instances
            .sets
            .iter_mut()
            .find(|s| s.id == set_id)
            .unwrap() = set;
        self.geometry.submodels.push(crate::sculpt::Submodel {
            name,
            metadata: Default::default(),
            mesh,
            mesh_settings: Default::default(),
        });
        self.selected_instance = None;
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    /// A source cannot disappear while linked copies refer to it. This also
    /// protects Merge down and replacement workflows, not just the Delete button.
    pub(crate) fn instance_source_in_use(&self, index: usize) -> bool {
        let id = self.model(index).map_or(0, |m| m.metadata.model_id);
        id != 0
            && self.geometry.scene.instances.sets.iter().any(|set| {
                if matches!(&set.layout, Layout::Array(a) if a.owner==Some(id)) {
                    return self.geometry.scene.instances.array_is_repeated(set.id);
                }
                let mut probe = set.clone();
                let mut found = false;
                probe.sources_mut(|source| found |= *source == Source::Model(id));
                found
            })
    }
    pub fn select_instance_owner(&mut self, owner: usize) -> bool {
        if owner < self.model_count() {
            self.selected_instance = None;
            return false;
        }
        if let Ok(placements) = self.instance_placements()
            && let Some(&(set, slot, _)) = placements.get(owner - self.model_count())
        {
            self.selected_instance = Some((set, slot));
            return true;
        }
        false
    }
    pub fn apply_object_stroke_independent(&mut self, id: u64) -> Result<(), String> {
        let mut next = self.geometry_snapshot();
        next.scene.instances = self.instances().clone();
        next.bake_instances(BakeRequest::new(BakeScope::Set(id)), &|| false)?;
        self.cancel_instance_preview();
        self.replace_geometry_named(next, "Place independent objects")
    }
    pub fn import_object_asset(
        &mut self,
        mut asset: Asset,
        brush: Option<u64>,
    ) -> Result<u64, String> {
        self.cancel_instance_preview();
        asset.id = self
            .instances()
            .assets
            .iter()
            .map(|a| a.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Object identity limit reached")?;
        let id = asset.id;
        for texture in &mut std::sync::Arc::make_mut(&mut asset.mesh).uv.textures {
            texture.id = crate::sculpt::texture::Texture::new_id();
            texture.exchange = None;
        }
        let mut library = self.instances().clone();
        library.assets.push(asset);
        if let Some(brush) = brush {
            library
                .brushes
                .iter_mut()
                .find(|b| b.id == brush)
                .ok_or("Object brush no longer exists")?
                .objects
                .push(id);
        }
        let models: Vec<_> = (0..self.model_count())
            .map(|i| (self.instance_model_id(i), self.model(i).unwrap().mesh))
            .collect();
        library.validate(&models, self.paths())?;
        crate::sculpt::texture::validate_library(
            models
                .iter()
                .map(|(_, mesh)| *mesh)
                .chain(library.assets.iter().map(|a| a.mesh.as_ref())),
            &self.geometry.scene.texture_library,
        )?;
        self.end_edit();
        self.end_stroke();
        self.record_edit("Import object brush");
        self.geometry.scene.instances = library;
        self.dirty = true;
        self.display_changed();
        Ok(id)
    }
    pub fn set_object_options(&mut self, id: u64, options: ObjectOptions) -> Result<(), String> {
        options.validate()?;
        let index = self
            .instances()
            .assets
            .iter()
            .position(|a| a.id == id)
            .ok_or("Object brush no longer exists")?;
        if self.instances().assets[index].options == options {
            return Ok(());
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit("Object brush settings");
        self.geometry.scene.instances.assets[index].options = options;
        self.dirty = true;
        Ok(())
    }
    pub fn remove_object_asset(&mut self, id: u64) -> Result<(), String> {
        if self.instances().sets.iter().any(|s| {
            let mut s = s.clone();
            let mut used = false;
            s.sources_mut(|source| used |= *source == Source::Asset(id));
            used
        }) {
            return Err(
                "Bake or remove instances using this object brush before removing it".into(),
            );
        }
        if !self.instances().assets.iter().any(|a| a.id == id) {
            return Err("Object brush no longer exists".into());
        }
        self.cancel_instance_preview();
        self.end_edit();
        self.end_stroke();
        self.record_edit("Remove object brush");
        self.geometry.scene.instances.assets.retain(|a| a.id != id);
        for brush in &mut self.geometry.scene.instances.brushes {
            brush.objects.retain(|object| *object != id);
        }
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn visit_instance_points(&self, mut visit: impl FnMut(Vec3)) {
        if let Ok(placements) = self.instance_placements() {
            for &(id, slot, p) in placements.iter() {
                if let Some(mesh) = placements.meshes.get(&(id, slot)) {
                    for &v in &mesh.vertices {
                        visit(v);
                    }
                } else if let Some(mesh) = self.instance_source(p.piece.source) {
                    for &v in &mesh.vertices {
                        visit(p.point(v));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod array_layers;
    fn document() -> Document {
        let mut p = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
        p.divisions = [1; 3];
        Document {
            geometry: Geometry::new(p.mesh().unwrap()),
            ..Default::default()
        }
    }
    fn array(d: &mut Document, count: usize) -> Set {
        let piece = d.instance_piece(0).unwrap();
        let mut a = Array::new(piece);
        a.count = count;
        Set {
            id: 1,
            name: "Linked cubes".into(),
            visible: true,
            layout: Layout::Array(a),
            adjustments: Vec::new(),
        }
    }
    #[test]
    fn reusable_instance_cache_reuses_and_invalidates_transactionally() {
        use std::sync::Arc;
        let mut d = document();
        let empty = d.instance_placements().unwrap();
        let mut set = array(&mut d, 3);
        d.preview_instance_set(set.clone()).unwrap();
        let preview = d.instance_placements().unwrap();
        assert!(!Arc::ptr_eq(&empty, &preview));
        assert!(Arc::ptr_eq(&preview, &d.instance_placements().unwrap()));
        d.display_changed_preserving_geometry();
        assert!(Arc::ptr_eq(&preview, &d.instance_placements().unwrap()));
        if let Layout::Array(a) = &mut set.layout {
            a.count = 257;
        }
        assert!(d.preview_instance_set(set).is_err());
        assert_eq!(preview, d.instance_placements().unwrap());
        d.cancel_instance_preview();
        assert!(d.instance_placements().unwrap().is_empty());
        let set = array(&mut d, 2);
        d.preview_instance_set(set).unwrap();
        d.apply_instance_preview().unwrap();
        let applied = d.instance_placements().unwrap();
        d.display_changed();
        assert!(!Arc::ptr_eq(&applied, &d.instance_placements().unwrap()));
        assert_eq!(applied, d.instance_placements().unwrap());
        d.selected_instance = Some((1, 0));
        d.remove_instance_set(1).unwrap();
        assert!(d.selected_instance.is_none());
        assert!(d.instance_placements().unwrap().is_empty());
        d.undo();
        assert_eq!(d.instance_placements().unwrap().len(), 2);
    }
    #[test]
    fn reusable_instances_preview_cancel_apply_undo_and_reopen() {
        let mut d = document();
        let before = d.project();
        let set = array(&mut d, 3);
        d.preview_instance_set(set.clone()).unwrap();
        assert!(d.project() == before);
        assert!(!d.is_dirty());
        assert_eq!(d.history_counts(), (0, 0));
        assert_eq!(d.instance_placements().unwrap().len(), 3);
        d.cancel_instance_preview();
        assert!(d.project() == before);
        let set = array(&mut d, 3);
        d.preview_instance_set(set).unwrap();
        d.apply_instance_preview().unwrap();
        let saved = d.project();
        saved.validate().unwrap();
        assert_eq!(d.history_counts(), (1, 0));
        let decoded: Project =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut reopened = document();
        reopened.replace_project(decoded, "test.clay").unwrap();
        assert_eq!(
            reopened.instance_placements().unwrap(),
            d.instance_placements().unwrap()
        );
        let source = d
            .instance_source(d.instance_placements().unwrap()[0].2.piece.source)
            .unwrap();
        assert!(std::ptr::eq(source, d.mesh()));
        d.undo();
        assert!(d.project() == before);
        d.redo();
        assert!(d.project() == saved);
        d.begin_instance_preview().unwrap();
        d.apply_instance_preview().unwrap();
        assert_eq!(d.history_counts(), (1, 0));
    }
    #[test]
    fn reusable_instances_follow_source_edit_and_detach_with_attributes() {
        let mut d = document();
        let set = array(&mut d, 3);
        d.preview_instance_set(set).unwrap();
        d.apply_instance_preview().unwrap();
        let p = d.instance_placements().unwrap()[1].2;
        let old = p.point(d.mesh().vertices[0]);
        let mut g = d.geometry_snapshot();
        g.mesh.vertices[0].2 += 0.1;
        d.replace_geometry_named(g, "Edit source").unwrap();
        assert_ne!(
            p.point(d.instance_source(p.piece.source).unwrap().vertices[0]),
            old
        );
        d.detach_instance(1, 1).unwrap();
        assert_eq!(d.model_count(), 2);
        assert_eq!(d.instance_placements().unwrap().len(), 2);
        let detached = d.model(1).unwrap().mesh.clone();
        let mut g = d.geometry_snapshot();
        g.mesh.vertices[0].2 += 0.1;
        d.replace_geometry_named(g, "Edit source").unwrap();
        assert_eq!(d.model(1).unwrap().mesh, &detached);
        assert!(!d.delete_submodel());
        d.project().validate().unwrap();
    }
    #[test]
    fn reusable_instances_invalid_preview_preserves_last_valid_and_history() {
        let mut d = document();
        let set = array(&mut d, 2);
        d.preview_instance_set(set.clone()).unwrap();
        let mut bad = set.clone();
        if let Layout::Array(a) = &mut bad.layout {
            a.count = 257;
        }
        assert!(d.preview_instance_set(bad).is_err());
        assert_eq!(d.instances().sets, vec![set]);
        assert_eq!(d.history_counts(), (0, 0));
        assert!(!d.is_dirty());
        let mut bad = d.project();
        bad.scene.instances.sets = d.instances().sets.clone();
        assert!(
            bad.validate().is_err(),
            "uncommitted model IDs must not leak into saves"
        );
    }
    #[test]
    fn object_kit_members_share_assets_and_import_atomically_without_scene_ids() {
        let mut d = document();
        let ship = d.create_object_brush("Ship parts".into()).unwrap();
        let floor = d.create_object_brush("Floor props".into()).unwrap();
        let a = d
            .capture_brush_object(Some(ship), "Vent".into(), Vec3(0., 0., -1.), 0, 2, [0.; 3])
            .unwrap();
        let b = d
            .capture_brush_object(Some(ship), "Hatch".into(), Vec3(0., -1., 0.), 2, 1, [0.; 3])
            .unwrap();
        let options = ObjectOptions {
            scale: 0.4,
            rotation: 35.,
            ..Default::default()
        };
        d.set_object_options(b, options).unwrap();
        let source = d.instances().assets[0].mesh.clone();
        d.set_brush_member(floor, a, true).unwrap();
        assert_eq!(d.instances().assets.len(), 2);
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &d.instances().assets[0].mesh
        ));
        let history = d.history_counts();
        d.set_brush_member(floor, a, true).unwrap();
        assert!(d.set_brush_member(floor, 99, true).is_err());
        assert!(d.rename_object_brush(ship, "Floor props".into()).is_err());
        assert_eq!(d.history_counts(), history);
        let original = d.project();
        let mut reopened = document();
        reopened
            .replace_project(
                serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap(),
                "kits.clay",
            )
            .unwrap();
        let bundle = reopened.instances().brush_bundle(ship).unwrap();
        assert_eq!(bundle.objects[0].pivot, Vec3(0., 0., -1.));
        assert_eq!(bundle.objects[1].options, options);
        assert_eq!(bundle.objects[1].up, 1);
        d.remove_object_asset(a).unwrap();
        assert_eq!(d.instances().brushes[0].objects, vec![b]);
        assert!(d.instances().brushes[1].objects.is_empty());
        d.undo();
        assert!(d.project() == original);
        d.remove_object_brush(ship).unwrap();
        assert_eq!(d.instances().assets.len(), 2);
        d.undo();
        assert!(d.project() == original);
        let mut target = document();
        target.create_object_brush("Body parts".into()).unwrap();
        target
            .capture_object("Existing".into(), Vec3::default(), 0, 2)
            .unwrap();
        let before = target.project();
        let history = target.history_counts();
        let mut invalid = bundle.clone();
        invalid.objects[1].id = invalid.objects[0].id;
        assert!(target.import_object_brush(invalid).is_err());
        assert!(target.project() == before);
        assert_eq!(target.history_counts(), history);
        let new = target.import_object_brush(bundle).unwrap();
        assert_ne!(new, ship);
        let saved = target.project();
        saved.validate().unwrap();
        assert_eq!(target.instances().brushes[1].objects, vec![2, 3]);
        assert_eq!(target.instances().assets[2].options, options);
        assert_eq!(target.instances().assets[0].name, "Existing");
        assert_eq!(target.history_counts().0, history.0 + 1);
        target.undo();
        assert!(target.project() == before);
        target.redo();
        assert!(target.project() == saved);
    }
    #[test]
    fn reusable_object_capture_survives_source_deletion_and_file_roundtrip() {
        let mut d = document();
        let id = d
            .capture_object("Stud".into(), Vec3(0., 0., -1.), 0, 2)
            .unwrap();
        let captured = d.instances().assets[0].clone();
        assert!(d.delete_submodel());
        assert_eq!(d.instances().assets[0], captured);
        let p = Placement {
            piece: Piece {
                source: Source::Asset(id),
                pivot: captured.pivot,
                forward: 0,
                up: 2,
            },
            transform: Transform::default(),
        };
        let set = Set {
            id: 1,
            name: "Studs".into(),
            visible: true,
            layout: Layout::Manual(vec![p, p]),
            adjustments: Vec::new(),
        };
        let history = d.history_counts().0;
        d.preview_instance_set(set).unwrap();
        d.apply_instance_preview().unwrap();
        assert_eq!(d.history_counts().0, history + 1);
        assert_eq!(d.instance_placements().unwrap().len(), 2);
        let file =
            std::env::temp_dir().join(format!("clay-reusable-object-{}.json", std::process::id()));
        crate::persistence::save_object_asset(file.to_str().unwrap(), &captured).unwrap();
        let loaded = crate::persistence::load_object_asset(file.to_str().unwrap()).unwrap();
        std::fs::remove_file(file).unwrap();
        assert_eq!(loaded, captured);
        d.project().validate().unwrap();
    }
    #[test]
    fn reusable_instances_bake_is_private_cancellable_and_retains_source() {
        let mut d = document();
        let set = array(&mut d, 2);
        d.preview_instance_set(set).unwrap();
        d.apply_instance_preview().unwrap();
        let before = d.geometry_snapshot();
        let mut baked = before.clone();
        assert!(
            baked
                .bake_instances(BakeRequest::new(BakeScope::Set(1)), &|| true)
                .is_err()
        );
        assert_eq!(baked, before);
        baked
            .bake_instances(BakeRequest::new(BakeScope::Set(1)), &|| false)
            .unwrap();
        assert_eq!(baked.submodels.len(), 2);
        assert!(baked.scene.instances.sets.is_empty());
        assert_eq!(d.geometry_snapshot(), before);
        d.replace_geometry_named(baked, "Bake instances").unwrap();
        d.project().validate().unwrap();
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn reusable_fusion_is_closed_and_rejects_open_pieces() {
        let mut d = document();
        d.geometry.settings.resolution = 16;
        let mut set = array(&mut d, 2);
        if let Layout::Array(a) = &mut set.layout {
            a.spacing = Vec3(0.5, 0., 0.);
        }
        d.preview_instance_set(set).unwrap();
        d.apply_instance_preview().unwrap();
        let before = d.geometry_snapshot();
        let mut fused = before.clone();
        fused
            .bake_instances(
                BakeRequest {
                    scope: BakeScope::Set(1),
                    destination: BakeDestination::New,
                    fuse: true,
                },
                &|| false,
            )
            .unwrap();
        let mut edges = std::collections::HashMap::new();
        for &[a, b, c] in &fused.mesh.triangles {
            for (a, b) in [(a, b), (b, c), (c, a)] {
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        assert!(edges.values().all(|&n| n == 2));
        assert!(!fused.mesh.triangles.is_empty());
        assert!(fused.scene.instances.sets.is_empty());
        assert_eq!(fused.submodels[0].mesh, before.mesh);
        let mut open = before.clone();
        open.mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        let original = open.clone();
        assert!(
            open.bake_instances(
                BakeRequest {
                    scope: BakeScope::Set(1),
                    destination: BakeDestination::New,
                    fuse: true
                },
                &|| false
            )
            .is_err()
        );
        assert_eq!(open, original);
    }
    #[test]
    fn reusable_path_bend_preview_cache_persistence_and_bake_are_transactional() {
        use crate::sculpt::paths::{PathMode, PathPoint};
        let mut d = document();
        let path = d
            .create_path(
                "Rail".into(),
                PathMode::Free,
                vec![
                    PathPoint {
                        position: Vec3::default(),
                        normal: axis(2),
                    },
                    PathPoint {
                        position: Vec3(6., 0., 0.),
                        normal: axis(2),
                    },
                ],
                false,
                0.,
            )
            .unwrap();
        let piece = d.instance_piece(0).unwrap();
        let set = Set {
            id: 1,
            name: "Joined rail".into(),
            visible: true,
            adjustments: Vec::new(),
            layout: Layout::Path(PathAssembly {
                path,
                start: None,
                middle: piece,
                end: None,
                count: 3,
                spacing: 2.,
                fit_count: true,
                twist: 0.,
                scale: 1.,
                bend: true,
                fit_sections: true,
                join: JoinMode::Weld,
                join_distance: 0.05,
                thickness: Thickness {
                    mode: ThicknessMode::Profile,
                    points: vec![
                        ThicknessPoint {
                            position: 0.,
                            factor: 0.5,
                        },
                        ThicknessPoint {
                            position: 0.4,
                            factor: 1.5,
                        },
                        ThicknessPoint {
                            position: 1.,
                            factor: 0.8,
                        },
                    ],
                    ..Default::default()
                },
            }),
        };
        let before = d.project();
        let history = d.history_counts();
        d.preview_instance_set(set.clone()).unwrap();
        let preview = d.instance_placements().unwrap();
        assert_eq!(preview.len(), 1);
        assert_eq!(preview.meshes.len(), 1);
        assert!(std::sync::Arc::ptr_eq(
            &preview,
            &d.instance_placements().unwrap()
        ));
        assert!(d.project() == before);
        assert_eq!(d.history_counts(), history);
        d.cancel_instance_preview();
        assert!(d.instance_placements().unwrap().is_empty());
        d.preview_instance_set(set.clone()).unwrap();
        d.apply_instance_preview().unwrap();
        assert_eq!(d.history_counts().0, history.0 + 1);
        let saved = d.project();
        assert_eq!(saved.mesh, before.mesh);
        let loaded: Project = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut reopened = document();
        reopened.replace_project(loaded, "joined.clay").unwrap();
        assert_eq!(
            d.instance_placements().unwrap(),
            reopened.instance_placements().unwrap()
        );
        assert!(d.detach_instance(1, 0).is_err());
        let cached = d.instance_placements().unwrap();
        let mut bad = set.clone();
        if let Layout::Path(a) = &mut bad.layout {
            a.fit_sections = false;
            a.count = 8;
        }
        assert!(d.preview_instance_set(bad).is_err());
        assert_eq!(cached, d.instance_placements().unwrap());
        d.cancel_instance_preview();
        let mut geometry = d.geometry_snapshot();
        assert!(
            geometry
                .clone()
                .bake_instances(BakeRequest::new(BakeScope::Set(1)), &|| true)
                .is_err()
        );
        geometry
            .bake_instances(BakeRequest::new(BakeScope::Set(1)), &|| false)
            .unwrap();
        assert_eq!(geometry.mesh, cached.meshes[&(1, 0)].as_ref().clone());
        assert!(geometry.scene.instances.sets.is_empty());
        d.replace_geometry_named(geometry, "Bake instances")
            .unwrap();
        d.undo();
        assert!(d.project() == saved);
        d.redo();
        assert_eq!(d.model_count(), 2);
        let mut changed = reopened.path(path).unwrap().clone();
        changed.points[1].position.0 = 9.;
        reopened.update_path(changed).unwrap();
        let updated = reopened.instance_placements().unwrap();
        let end = updated.meshes[&(1, 0)]
            .vertices
            .iter()
            .map(|p| p.0)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((end - 9.).abs() < 1e-4);
        let mut separate = set;
        if let Layout::Path(a) = &mut separate.layout {
            a.join = JoinMode::Separate;
        }
        reopened.preview_instance_set(separate.clone()).unwrap();
        reopened.apply_instance_preview().unwrap();
        let expected = reopened.instance_placements().unwrap().meshes[&(1, 1)].clone();
        separate.visible = false;
        reopened.preview_instance_set(separate).unwrap();
        reopened.apply_instance_preview().unwrap();
        assert!(reopened.instance_placements().unwrap().meshes.is_empty());
        reopened.detach_instance(1, 1).unwrap();
        assert_eq!(reopened.model(1).unwrap().mesh, expected.as_ref());
        reopened.project().validate().unwrap();
    }
    #[test]
    fn whole_path_transform_updates_linked_geometry_and_undoes_atomically() {
        use crate::sculpt::paths::{PathMode, PathPoint};
        for fit_count in [false, true] {
            let mut d = document();
            d.geometry.scene.active_metadata.visible = false;
            let id = d
                .create_path(
                    "Route".into(),
                    PathMode::Free,
                    [Vec3::default(), Vec3(6., 0., 0.)]
                        .map(|position| PathPoint {
                            position,
                            normal: axis(2),
                        })
                        .to_vec(),
                    false,
                    0.,
                )
                .unwrap();
            let set = Set {
                id: 1,
                name: "Rail".into(),
                visible: true,
                adjustments: Vec::new(),
                layout: Layout::Path(PathAssembly {
                    path: id,
                    start: None,
                    middle: d.instance_piece(0).unwrap(),
                    end: None,
                    count: 3,
                    spacing: 2.,
                    fit_count,
                    twist: 0.,
                    scale: 1.,
                    bend: true,
                    fit_sections: true,
                    join: JoinMode::Weld,
                    join_distance: 0.05,
                    thickness: Thickness {
                        mode: ThicknessMode::Taper,
                        start: 0.5,
                        end: 1.5,
                        ..Default::default()
                    },
                }),
            };
            d.preview_instance_set(set).unwrap();
            d.apply_instance_preview().unwrap();
            let before = d.project();
            let history = d.history_counts().0;
            let old_mesh = d.instance_placements().unwrap().meshes[&(1, 0)].clone();
            let path = d
                .path(id)
                .unwrap()
                .transformed(
                    Vec3::default(),
                    Vec3(1., 2., 3.),
                    axis(2),
                    std::f32::consts::FRAC_PI_2,
                    2.,
                )
                .unwrap();
            d.update_path_with_section_scale(path, 2.).unwrap();
            assert_eq!(d.history_counts().0, history + 1);
            assert_eq!(d.mesh(), &before.mesh);
            let generated = d.instance_placements().unwrap();
            assert_eq!(
                generated.meshes[&(1, 0)].vertices.len(),
                old_mesh.vertices.len()
            );
            for (&a, &b) in old_mesh
                .vertices
                .iter()
                .zip(&generated.meshes[&(1, 0)].vertices)
            {
                let expected = Vec3(-a.1, a.0, a.2) * 2. + Vec3(1., 2., 3.);
                assert!(
                    (expected - b).len() < 1e-4,
                    "uniform whole-path transform must include the linked cross-section"
                );
            }
            let Layout::Path(recipe) = &d.instances().sets[0].layout else {
                unreachable!()
            };
            assert_eq!(recipe.scale, 2.);
            assert_eq!(recipe.spacing, if fit_count { 2. } else { 4. });
            let saved = d.project();
            d.undo();
            assert!(d.project() == before);
            d.redo();
            assert!(d.project() == saved);
            let mut reopened = document();
            reopened
                .replace_project(
                    serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap(),
                    "transformed.clay",
                )
                .unwrap();
            assert_eq!(reopened.instance_placements().unwrap(), generated);
            let history = d.history_counts();
            let same = d.path(id).unwrap().clone();
            assert!(d.update_path_with_section_scale(same, 1e20).is_err());
            assert!(d.project() == saved);
            assert_eq!(d.history_counts(), history);
        }
    }
    #[test]
    fn whole_path_scale_preserves_nearby_neighbour_joins() {
        use crate::sculpt::paths::{PathMode, PathPoint};
        let mut d = document();
        let id = d
            .create_path(
                "Nearby ends".into(),
                PathMode::Free,
                [Vec3::default(), Vec3(2.03, 0., 0.)]
                    .map(|position| PathPoint {
                        position,
                        normal: axis(2),
                    })
                    .to_vec(),
                false,
                0.,
            )
            .unwrap();
        let set = Set {
            id: 1,
            name: "Joined cubes".into(),
            visible: true,
            adjustments: Vec::new(),
            layout: Layout::Path(PathAssembly {
                path: id,
                start: None,
                middle: d.instance_piece(0).unwrap(),
                end: None,
                count: 2,
                spacing: 2.03,
                fit_count: true,
                twist: 0.,
                scale: 1.,
                bend: false,
                fit_sections: false,
                join: JoinMode::Weld,
                join_distance: 0.05,
                thickness: Default::default(),
            }),
        };
        d.preview_instance_set(set).unwrap();
        d.apply_instance_preview().unwrap();
        let before = d.instance_placements().unwrap().meshes[&(1, 0)].clone();
        let path = d
            .path(id)
            .unwrap()
            .transformed(Vec3::default(), Vec3::default(), axis(2), 0., 2.)
            .unwrap();
        d.update_path_with_section_scale(path, 2.).unwrap();
        let after = d.instance_placements().unwrap();
        let mesh = &after.meshes[&(1, 0)];
        assert_eq!(before.triangles, mesh.triangles);
        for (a, b) in before.vertices.iter().zip(&mesh.vertices) {
            assert!((*a * 2. - *b).len() < 1e-5);
        }
    }
    #[test]
    fn reusable_path_assembly_follows_edits_and_protects_path_deletion() {
        use crate::sculpt::paths::{PathMode, PathPoint};
        let mut d = document();
        let id = d
            .create_path(
                "Line".into(),
                PathMode::Free,
                vec![
                    PathPoint {
                        position: Vec3(0., 0., 0.),
                        normal: axis(2),
                    },
                    PathPoint {
                        position: Vec3(6., 0., 0.),
                        normal: axis(2),
                    },
                ],
                false,
                0.,
            )
            .unwrap();
        let piece = d.instance_piece(0).unwrap();
        let set = Set {
            id: 1,
            name: "Chain".into(),
            visible: true,
            adjustments: Vec::new(),
            layout: Layout::Path(PathAssembly {
                path: id,
                start: Some(piece),
                middle: piece,
                end: Some(piece),
                count: 3,
                spacing: 1.,
                fit_count: true,
                twist: 0.,
                scale: 0.2,
                bend: false,
                fit_sections: false,
                join: JoinMode::Separate,
                join_distance: default_join_distance(),
                thickness: Default::default(),
            }),
        };
        d.preview_instance_set(set).unwrap();
        d.apply_instance_preview().unwrap();
        let placements = d.instance_placements().unwrap();
        assert_eq!(placements.len(), 5);
        assert!((placements.last().unwrap().2.transform.origin.0 - 6.).abs() < 1e-4);
        let mut path = d.path(id).unwrap().clone();
        path.points[1].position.0 = 10.;
        d.update_path(path).unwrap();
        assert!(
            (d.instance_placements()
                .unwrap()
                .last()
                .unwrap()
                .2
                .transform
                .origin
                .0
                - 10.)
                .abs()
                < 1e-4
        );
        assert!(d.delete_path(id).is_err());
        d.project().validate().unwrap();
    }
}

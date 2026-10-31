//! Instance edits are transactional; previews never enter saves or history.
use super::*;
use crate::sculpt::{Vec3, instances::*};

pub type ResolvedInstances = std::sync::Arc<Vec<(u64, usize, Placement)>>;

pub(super) struct Preview {
    pub library: Library,
    pub model_ids: Vec<(usize, u64)>,
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
        let mut result = Vec::new();
        let (mut vertices, mut triangles) = (0usize, 0usize);
        for set in &self.instances().sets {
            if !set.visible {
                continue;
            }
            for (slot, p) in set.placements(self.paths())? {
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
                result.push((set.id, slot, p));
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
        self.display_changed();
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
            self.display_changed();
        }
    }
    pub fn apply_instance_preview(&mut self) -> Result<(), String> {
        self.validate_instances()?;
        self.instance_placements()?;
        let Some(preview) = self.instance_preview.take() else {
            return Ok(());
        };
        if preview.library == self.geometry.scene.instances {
            self.display_changed();
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
        self.display_changed();
        Ok(())
    }
    pub fn remove_instance_set(&mut self, id: u64) -> Result<(), String> {
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
            pivot,
            forward,
            up,
            options: Default::default(),
        });
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
        let p = set
            .placements(self.paths())?
            .into_iter()
            .find(|(i, _)| *i == slot)
            .ok_or("Instance is hidden or no longer exists")?
            .1;
        let mesh = p.mesh(
            self.instance_source(p.piece.source)
                .ok_or("Source is missing")?,
        )?;
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
        next.bake_instances(id, false, &|| false)?;
        self.cancel_instance_preview();
        self.replace_geometry_named(next, "Place independent objects")
    }
    pub fn import_object_asset(&mut self, mut asset: Asset) -> Result<u64, String> {
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
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn visit_instance_points(&self, mut visit: impl FnMut(Vec3)) {
        if let Ok(placements) = self.instance_placements() {
            for &(_, _, p) in placements.iter() {
                if let Some(mesh) = self.instance_source(p.piece.source) {
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
        d.texture_display_changed();
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
        assert!(baked.bake_instances(1, false, &|| true).is_err());
        assert_eq!(baked, before);
        baked.bake_instances(1, false, &|| false).unwrap();
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
        fused.bake_instances(1, true, &|| false).unwrap();
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
        assert!(open.bake_instances(1, true, &|| false).is_err());
        assert_eq!(open, original);
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

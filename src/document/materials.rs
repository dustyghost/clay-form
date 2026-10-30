use super::*;
use crate::sculpt::material::layers::Asset;
impl Document {
    /// Assignment always addresses this model's vertex material, regardless of the paint target.
    pub(crate) fn assign_model_material(
        &mut self,
        material: crate::sculpt::Material,
    ) -> Result<(), String> {
        material.validate()?;
        self.require_sculpt_mesh()?;
        let mut brush = Brush::new(Kind::Paint);
        brush.material = material;
        self.edit_group_meshes("Assign model material", |mesh| {
            mesh.fill_all_material(&brush);
            Ok(())
        })
    }

    pub fn materials(&self) -> &[Asset] {
        &self.geometry.scene.materials
    }
    /// Count all affected stored vertices, including inactive subdivision levels.
    pub(crate) fn material_usage(&self, id: u64) -> (usize, usize) {
        let mut models = 0;
        let mut vertices = 0;
        for index in 0..self.model_count() {
            let model = self.model(index).unwrap();
            let count = std::iter::once(model.mesh)
                .chain(&model.settings.lower)
                .chain(&model.settings.upper)
                .flat_map(|mesh| &mesh.materials)
                .filter(|material| {
                    material
                        .links
                        .iter()
                        .any(|link| link.id == id && link.weights.iter().any(|weight| *weight > 0.))
                })
                .count();
            models += usize::from(count > 0);
            vertices += count;
        }
        (models, vertices)
    }
    pub fn save_material(&mut self, mut asset: Asset) -> Result<(), String> {
        crate::sculpt::material::layers::validate_assets(std::slice::from_ref(&asset))?;
        asset.original = Some(
            self.materials()
                .iter()
                .find(|a| a.id == asset.id)
                .map_or_else(|| asset.reset_material(), Asset::reset_material),
        );
        let existing = self.materials().iter().position(|a| a.id == asset.id);
        if existing.is_some_and(|i| self.materials()[i] == asset) {
            return Ok(());
        }
        if existing.is_none() && self.materials().len() >= 256 {
            return Err("The project supports up to 256 named materials".into());
        }
        self.begin_edit(EditSession::Material(asset.id));
        if let Some(i) = existing {
            let old = self.geometry.scene.materials[i].material.clone();
            let rebase = |mesh: &mut Mesh| {
                // Large fills often repeat exactly the same material. Reuse the
                // rebased value instead of rebuilding its layers at every vertex.
                let mut previous: Option<(crate::sculpt::Material, crate::sculpt::Material)> = None;
                for material in &mut mesh.materials {
                    if !material.links.iter().any(|link| link.id == asset.id) {
                        continue;
                    }
                    if let Some((source, result)) = &previous
                        && material == source
                    {
                        *material = result.clone();
                        continue;
                    }
                    let source = material.clone();
                    material.rebase_asset(asset.id, &old, &asset.material);
                    previous = Some((source, material.clone()));
                }
            };
            rebase(&mut self.geometry.mesh);
            for mesh in self
                .geometry
                .settings
                .lower
                .iter_mut()
                .chain(&mut self.geometry.settings.upper)
            {
                rebase(mesh);
            }
            for model in &mut self.geometry.submodels {
                rebase(&mut model.mesh);
                for mesh in model
                    .mesh_settings
                    .lower
                    .iter_mut()
                    .chain(&mut model.mesh_settings.upper)
                {
                    rebase(mesh);
                }
            }
            self.geometry.scene.materials[i] = asset;
        } else {
            self.geometry.scene.materials.push(asset);
        }
        self.sync_material_brushes(&[]);
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn remove_material(&mut self, id: u64) -> Result<(), String> {
        let index = self
            .materials()
            .iter()
            .position(|a| a.id == id)
            .ok_or("Material no longer exists")?;
        self.end_edit();
        self.end_stroke();
        self.record_edit("Remove material");
        let detach = |mesh: &mut Mesh| {
            for material in &mut mesh.materials {
                material.detach_asset(id);
            }
        };
        detach(&mut self.geometry.mesh);
        for mesh in self
            .geometry
            .settings
            .lower
            .iter_mut()
            .chain(&mut self.geometry.settings.upper)
        {
            detach(mesh);
        }
        for model in &mut self.geometry.submodels {
            detach(&mut model.mesh);
            for mesh in model
                .mesh_settings
                .lower
                .iter_mut()
                .chain(&mut model.mesh_settings.upper)
            {
                detach(mesh);
            }
        }
        self.geometry.scene.materials.remove(index);
        for brush in &mut self.brushes {
            brush.material.detach_asset(id);
        }
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub(super) fn sync_material_brushes(&mut self, previous_ids: &[u64]) {
        for brush in &mut self.brushes {
            if let Some(id) = brush.material.primary_asset() {
                if let Some(asset) = self.geometry.scene.materials.iter().find(|a| a.id == id) {
                    brush.material = asset.material.with_link(id);
                } else if previous_ids.contains(&id) {
                    // Detach a definition removed by history, but preserve library
                    // presets that have not been registered in this project yet.
                    brush.material = brush.material.unlinked();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Material, PaintChannels};
    #[test]
    fn material_scope_counts_only_linked_painted_users() {
        let mut document = Document::default();
        let material = Material::default().with_link(42);
        document.geometry.mesh.materials = vec![material.clone(); document.mesh().vertices.len()];
        let count = document.mesh().vertices.len();
        assert_eq!(document.material_usage(42), (1, count));
        assert_eq!(document.material_usage(43), (0, 0));
        document.duplicate_model(false).unwrap();
        assert_eq!(document.material_usage(42), (2, count * 2));
        document.geometry.mesh.materials[0] = material.unlinked();
        assert_eq!(document.material_usage(42), (2, count * 2 - 1));
    }

    #[test]
    fn shared_scope_includes_materials_used_only_in_retained_levels() {
        let mut document = Document::default();
        let mut retained = document.mesh().clone();
        retained.materials = vec![Material::default().with_link(42); retained.vertices.len()];
        let count = retained.vertices.len();
        document.geometry.settings.upper.push(retained);
        assert_eq!(document.material_usage(42), (1, count));
    }

    #[test]
    fn shared_layered_material_edits_update_blended_uses_levels_and_undo() {
        let mut doc = Document::default();
        let mut base = Material::from_srgb([0.3, 0.2, 0.1], 0.6, 0.2);
        base.layers[0].surface.emission = [0.1, 0.2, 0.3];
        base.layers[0].surface.emission_strength = 2.;
        base.layers[0].weight = 0.4;
        let asset = Asset {
            id: 41,
            name: "Coated copper".into(),
            material: base.clone(),
            original: Some(base.clone()),
        };
        doc.save_material(asset.clone()).unwrap();
        doc.end_edit();
        let mut brush = Brush::new(Kind::Paint);
        brush.material = base.with_link(41);
        brush.strength = 0.5;
        doc.fill_material(&brush).unwrap();
        doc.geometry.subdivide().unwrap();
        let before = doc.geometry_snapshot();
        let counts = doc.history_counts();
        let mut updated = asset.clone();
        updated.material.layers[0].surface.emission_strength = 5.;
        updated.material.ior = 1.8;
        doc.save_material(updated.clone()).unwrap();
        assert_eq!(doc.history_counts().0, counts.0 + 1);
        for mesh in std::iter::once(doc.mesh()).chain(&doc.geometry.settings.lower) {
            for m in &mesh.materials {
                assert!((m.layers[0].surface.emission_strength - 2.5).abs() < 1e-5);
                assert!((m.ior - 1.65).abs() < 1e-5);
                assert!((m.links[0].weights[3] - 0.5).abs() < 1e-5);
            }
        }
        doc.end_edit();
        let after = doc.geometry_snapshot();
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), after);
        let project = doc.project();
        project.validate().unwrap();
        let bytes = serde_json::to_vec(&project).unwrap();
        let restored: Project = serde_json::from_slice(&bytes).unwrap();
        assert!(restored == project);
        // Reset restores the original shared surface, retaining the renamed definition.
        let mut reset = doc.materials()[0].clone();
        reset.name = "My coating".into();
        reset.material = reset.reset_material();
        doc.save_material(reset).unwrap();
        doc.end_edit();
        assert_eq!(doc.mesh(), &before.mesh);
        assert_eq!(doc.geometry.settings.lower, before.settings.lower);
        assert_eq!(doc.materials()[0].name, "My coating");
        assert_eq!(doc.materials()[0].original, Some(base));
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), after);
        // A colour-only fill must not replace layer/roughness bindings.
        brush.paint_channels = PaintChannels::Color;
        brush.material = Material::from_srgb([0.9, 0.1, 0.2], 0.1, 0.1);
        doc.fill_all_material(&brush).unwrap();
        let old = doc.mesh().material(0);
        updated.material.layers[0].weight = 0.8;
        doc.save_material(updated).unwrap();
        assert_eq!(doc.mesh().material(0).base_color, old.base_color);
        assert!(doc.mesh().material(0).layers[0].weight > old.layers[0].weight);
    }
}

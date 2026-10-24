use super::*;
use crate::sculpt::material::layers::Asset;
impl Document {
    pub fn materials(&self) -> &[Asset] {
        &self.geometry.scene.materials
    }
    pub fn save_material(&mut self, asset: Asset) -> Result<(), String> {
        crate::sculpt::material::layers::validate_assets(std::slice::from_ref(&asset))?;
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
                for material in &mut mesh.materials {
                    material.rebase_asset(asset.id, &old, &asset.material);
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
        self.sync_material_brushes();
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub(super) fn sync_material_brushes(&mut self) {
        for brush in &mut self.brushes {
            if let Some(id) = brush.material.primary_asset() {
                if let Some(asset) = self.geometry.scene.materials.iter().find(|a| a.id == id) {
                    brush.material = asset.material.with_link(id);
                } else {
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

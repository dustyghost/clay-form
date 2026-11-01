//! Source validation and an atomic, reversible join result.
use super::*;

impl Document {
    pub(crate) fn require_fusion_sources(&self, sources: [usize; 2]) -> Result<(), String> {
        if self.symmetry().enabled {
            return Err("Disable symmetry before joining explicit submodel pairs".into());
        }
        for index in sources {
            let model = self.model(index).ok_or("Choose an existing source model")?;
            if !model.metadata.visible {
                return Err("Show both source models before joining".into());
            }
            if model.settings.primitive.is_some() {
                return Err("Convert both primitives to sculpt meshes before joining".into());
            }
            if model.metadata.temporary_thickness > 0. {
                return Err("Apply or remove Thickness on both sources before joining".into());
            }
        }
        if self.model_count() >= 64 {
            return Err("There is no room for a joined submodel (64-model limit)".into());
        }
        Ok(())
    }
    pub(crate) fn fusion_sources(&self, sources: [usize; 2]) -> Result<[Mesh; 2], String> {
        self.require_fusion_sources(sources)?;
        Ok(sources.map(|i| self.model(i).unwrap().mesh.clone()))
    }
    pub(crate) fn apply_fusion(
        &mut self,
        mesh: Mesh,
        sources: [usize; 2],
        revision: u64,
    ) -> Result<(), String> {
        if revision != self.display_revision() {
            return Err("The scene changed; preview the join again".into());
        }
        let geometry = self.fusion_geometry(mesh, sources)?;
        self.replace_geometry_named(geometry, "Join meshes")
    }
    pub(crate) fn fusion_geometry(
        &self,
        mesh: Mesh,
        sources: [usize; 2],
    ) -> Result<Geometry, String> {
        self.require_fusion_sources(sources)?;
        let mesh = crate::sculpt::fusion::compact(mesh);
        mesh.validate()?;
        let mut models = self.ordered_models();
        let mut number = 1;
        let name = loop {
            let name = format!("Joined {number}");
            if !models.iter().any(|m| m.name == name) {
                break name;
            }
            number += 1;
        };
        let group = models[sources[0]].metadata.group.clone();
        for index in sources {
            models[index].metadata.visible = false;
        }
        let mut geometry = Geometry::new(mesh);
        geometry.paths = self.geometry.paths.clone();
        geometry.scene = self.geometry.scene.clone();
        geometry.name = name;
        geometry.settings = MeshSettings::default();
        geometry.scene.active_metadata = crate::sculpt::ModelMetadata {
            group,
            ..Default::default()
        };
        geometry.scene.active_index = models.len();
        geometry.submodels = models;
        Ok(geometry)
    }
}

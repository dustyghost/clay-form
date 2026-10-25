use super::*;
impl Document {
    pub(crate) fn require_texture_history(&self) -> Result<(), String> {
        let mut history = crate::history::History::default();
        history.retention = self.history.retention;
        history.record_owned(self.geometry.clone(), "Texture history check", None);
        if history.undo.is_empty() {
            return Err("This textured scene exceeds the current Undo memory budget. Increase History retention in the diagnostic settings before painting (up to 1024 MiB)".into());
        }
        Ok(())
    }
    pub(crate) fn sync_texture_uses(&mut self) {
        let textures = self.geometry.mesh.uv.textures.clone();
        let sync = |mesh: &mut Mesh| {
            for t in &mut mesh.uv.textures {
                if let Some(source) = textures.iter().find(|s| s.id == t.id) {
                    *t = source.clone();
                }
            }
        };
        for mesh in self
            .geometry
            .settings
            .lower
            .iter_mut()
            .chain(&mut self.geometry.settings.upper)
        {
            sync(mesh);
        }
        for model in &mut self.geometry.submodels {
            sync(&mut model.mesh);
            for mesh in model
                .mesh_settings
                .lower
                .iter_mut()
                .chain(&mut model.mesh_settings.upper)
            {
                sync(mesh);
            }
        }
    }
    pub(crate) fn assign_texture(
        &mut self,
        texture: crate::sculpt::texture::Texture,
    ) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        texture.validate()?;
        if !self.mesh().uv.current(self.mesh()) {
            return Err("Generate UVs before assigning a texture".into());
        }
        let mut candidate = self.geometry.clone();
        let assign = |mesh: &mut Mesh| {
            if let Some(old) = mesh
                .uv
                .textures
                .iter_mut()
                .find(|t| t.channel == texture.channel)
            {
                *old = texture.clone();
            } else {
                mesh.uv.textures.push(texture.clone());
            }
        };
        assign(&mut candidate.mesh);
        for mesh in candidate
            .settings
            .lower
            .iter_mut()
            .chain(&mut candidate.settings.upper)
        {
            if !mesh.uv.corners.is_empty() {
                assign(mesh);
            }
        }
        for model in &mut candidate.submodels {
            for mesh in std::iter::once(&mut model.mesh)
                .chain(model.mesh_settings.lower.iter_mut())
                .chain(model.mesh_settings.upper.iter_mut())
            {
                for t in &mut mesh.uv.textures {
                    if t.id == texture.id {
                        *t = texture.clone();
                    }
                }
            }
        }
        crate::sculpt::texture::validate_budget(
            candidate
                .submodels
                .iter()
                .map(|m| &m.mesh)
                .chain(std::iter::once(&candidate.mesh)),
        )?;
        if candidate == self.geometry {
            return Ok(());
        }
        self.replace_geometry_named(candidate, "Assign texture")
    }
    pub(crate) fn remove_texture(
        &mut self,
        channel: crate::sculpt::texture::Channel,
    ) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        let mut candidate = self.geometry.clone();
        for mesh in std::iter::once(&mut candidate.mesh)
            .chain(candidate.settings.lower.iter_mut())
            .chain(candidate.settings.upper.iter_mut())
        {
            mesh.uv.textures.retain(|t| t.channel != channel);
        }
        if candidate == self.geometry {
            return Ok(());
        }
        self.replace_geometry_named(candidate, "Remove texture")
    }
    pub(crate) fn edit_uv(
        &mut self,
        label: &'static str,
        edit: impl FnOnce(&mut crate::sculpt::uv::UvMap) -> Result<(), String>,
    ) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        let mut uv = self.mesh().uv.clone();
        edit(&mut uv)?;
        uv.validate()?;
        if uv == self.mesh().uv {
            return Ok(());
        }
        self.end_stroke();
        self.end_edit();
        self.record_edit(label);
        self.geometry.mesh.uv = uv;
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub(crate) fn mark_uv_seams(
        &mut self,
        edges: Vec<[usize; 2]>,
        mark: bool,
    ) -> Result<(), String> {
        if edges.is_empty() {
            return Err("Select edges in Modeling, then mark or clear UV seams".into());
        }
        let valid: std::collections::BTreeSet<_> = self
            .mesh()
            .triangles
            .iter()
            .flat_map(|t| (0..3).map(move |c| crate::sculpt::uv::edge(t[c], t[(c + 1) % 3])))
            .collect();
        if edges.iter().any(|e| !valid.contains(e)) {
            return Err("Edge selection is out of date".into());
        }
        let stale = !self.mesh().uv.corners.is_empty() && !self.mesh().uv.current(self.mesh());
        self.edit_uv(
            if mark {
                "Mark UV seams"
            } else {
                "Clear UV seams"
            },
            |uv| {
                if stale {
                    uv.corners.clear();
                    uv.topology.clear();
                    uv.seams.clear();
                    uv.masks.clear();
                    uv.removed.clear();
                }
                let mut seams: std::collections::BTreeSet<_> = uv.seams.iter().copied().collect();
                for edge in edges {
                    if mark {
                        seams.insert(edge);
                    } else {
                        seams.remove(&edge);
                    }
                }
                uv.seams = seams.into_iter().collect();
                Ok(())
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{
        Geometry, Primitive, PrimitiveKind,
        texture::{Channel, Texture},
    };
    #[test]
    fn texture_stroke_shared_assets_undo_save_and_reload_are_atomic() {
        let mut doc = Document::default();
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        mesh.uv = mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
        doc.replace_geometry(Geometry::new(mesh)).unwrap();
        doc.assign_texture(Texture::new(Channel::Color, 32, &Default::default()).unwrap())
            .unwrap();
        doc.duplicate_model(false).unwrap();
        let before = doc.geometry_snapshot();
        let mut changed = doc.mesh().uv.textures[0].clone();
        changed.pixels.u16_mut()[0] = 12345;
        doc.assign_texture(changed.clone()).unwrap();
        assert_eq!(doc.geometry.submodels[0].mesh.uv.textures[0], changed);
        let after = doc.geometry_snapshot();
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), after);
        let bytes = serde_json::to_vec(&doc.project()).unwrap();
        let restored: Project = serde_json::from_slice(&bytes).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.mesh.uv, doc.mesh().uv);
        let mut invalid = changed;
        invalid.width = 0;
        assert!(doc.assign_texture(invalid).is_err());
        assert_eq!(doc.geometry_snapshot(), after);
        let mut topo = doc.geometry_snapshot();
        topo.mesh.triangles[0].swap(0, 1);
        doc.replace_geometry(topo).unwrap();
        assert!(!doc.mesh().uv.current(doc.mesh()));
        doc.undo();
        assert!(doc.mesh().uv.current(doc.mesh()));
    }
}

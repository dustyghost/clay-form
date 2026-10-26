//! Mesh state, subdivision, and topology-mode changes.
use super::*;

impl Document {
    pub fn mesh(&self) -> &Mesh {
        &self.geometry.mesh
    }
    pub fn mesh_settings(&self) -> &MeshSettings {
        &self.geometry.settings
    }
    pub(crate) fn geometry_snapshot(&self) -> Geometry {
        let mut geometry = self.geometry.clone();
        Self::sync_geometry_texture_uses(&mut geometry);
        geometry
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
        self.replace_geometry_recorded(geometry, label, None)
    }
    pub(crate) fn replace_geometry_recorded(
        &mut self,
        geometry: Geometry,
        label: &'static str,
        settings: Option<crate::diagnostics::EditSettings>,
    ) -> Result<(), String> {
        let geometry = crate::sculpt::ValidatedGeometry::new(geometry, &|| false)?;
        self.commit_validated_geometry(geometry, label, settings);
        Ok(())
    }
    pub(crate) fn geometry_matches(&self, geometry: &Geometry) -> bool {
        &self.geometry == geometry
    }
    pub(crate) fn commit_validated_geometry(
        &mut self,
        geometry: crate::sculpt::ValidatedGeometry,
        label: &'static str,
        settings: Option<crate::diagnostics::EditSettings>,
    ) {
        let mut geometry = geometry.into_geometry();
        if geometry.scene.active_index == self.geometry.scene.active_index
            && geometry.mesh.uv.is_empty()
            && !self.geometry.mesh.uv.is_empty()
        {
            geometry.mesh.uv = self.geometry.mesh.uv.clone();
        }
        self.end_edit();
        self.end_stroke();
        let previous = std::mem::replace(&mut self.geometry, geometry);
        if let Some(settings) = settings {
            self.history.record_owned(previous, label, Some(settings));
        } else {
            self.record_edit_owned(previous, label);
        }
        self.dirty = true;
        self.display_changed();
        self.reconcile_paint_target();
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
        if settings.enabled && !self.mesh().creases.is_empty() {
            return Err("Clear creases before enabling adaptive sculpt".into());
        }
        if settings.enabled
            && settings.topology == crate::sculpt::Topology::Quads
            && (!self.geometry.settings.adaptive.enabled
                || self.geometry.settings.adaptive.topology != settings.topology)
        {
            self.mesh().require_adaptive_quads()?;
        }
        if self.geometry.settings.adaptive == settings {
            return Ok(());
        }
        if self.geometry.settings.adaptive.enabled == settings.enabled
            && self.geometry.settings.adaptive.topology == settings.topology
            && self.geometry.settings.adaptive.unmasked_only == settings.unmasked_only
        {
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
    pub fn set_remesh_topology(&mut self, topology: crate::sculpt::Topology) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if self.geometry.settings.remesh_topology == topology {
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        geometry.settings.remesh_topology = topology;
        self.replace_geometry_named(geometry, "Remesh topology")
    }
}

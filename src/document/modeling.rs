//! Transactional commits for the pen modeling tools.
use super::*;
use std::collections::BTreeSet;
impl Document {
    pub(crate) fn require_modeling(&self) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if !self.active_visible() {
            return Err("Show the active model before modeling".into());
        }
        if self.geometry.scene.active_metadata.temporary_thickness > 0. {
            return Err("Apply or disable Thickness before modeling".into());
        }
        Ok(())
    }
    pub(crate) fn commit_modeling(
        &mut self,
        mesh: Mesh,
        label: &'static str,
        revision: u64,
    ) -> Result<(), String> {
        self.require_modeling()?;
        if revision != self.display_revision() {
            return Err("The mesh changed; select the target again".into());
        }
        mesh.validate()?;
        if mesh == self.geometry.mesh {
            return Ok(());
        }
        let mut geometry = self.geometry.clone();
        geometry.mesh = mesh;
        geometry.settings = MeshSettings {
            resolution: geometry.settings.resolution,
            ..Default::default()
        };
        self.replace_geometry_named(geometry, label)
    }
    pub(crate) fn group_model_selection(
        &mut self,
        vertices: &[usize],
        name: &str,
        existing: Option<usize>,
        remove: bool,
    ) -> Result<(), String> {
        self.require_modeling()?;
        if self.mesh_settings().levels() != 1 {
            return Err(
                "Use a single subdivision level before assigning component selections".into(),
            );
        }
        if vertices.iter().any(|&v| v >= self.mesh().vertices.len()) {
            return Err("Select components on the current mesh".into());
        }
        let symmetry = crate::sculpt::ModelingSymmetry::new(self.mesh(), self.symmetry())?;
        let selected: BTreeSet<_> = vertices
            .iter()
            .flat_map(|&v| symmetry.vertex_copies(v))
            .collect();
        if selected.is_empty() || selected.iter().any(|&v| v >= self.mesh().vertices.len()) {
            return Err("Select vertices, edges, or faces first".into());
        }
        let mut geometry = self.geometry.clone();
        let mesh = &mut geometry.mesh;
        // Reuse group assignment, including whole-quad face labels, with a
        // temporary selection mask. The document's sculpt mask is restored.
        let mask = std::mem::replace(&mut mesh.mask, vec![0.; mesh.vertices.len()]);
        for v in selected {
            mesh.mask[v] = 1.;
        }
        if let Some(index) = existing {
            mesh.assign_mask_to_group(index, 0.5, remove)?;
        } else {
            let name = mesh.unique_group_name(if name.trim().is_empty() {
                "Selection"
            } else {
                name.trim()
            });
            mesh.create_vertex_group(name, 0.5, false)?;
        }
        mesh.mask = mask;
        mesh.validate()?;
        if geometry == self.geometry {
            return Ok(());
        }
        self.replace_geometry_named(geometry, "Group component selection")
    }
}

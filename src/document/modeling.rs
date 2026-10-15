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
        faces: Option<&[usize]>,
        name: &str,
        existing: Option<usize>,
        remove: bool,
    ) -> Result<Option<usize>, String> {
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
        // Face selections are categorical. Shared corner vertices cannot tell
        // us which faces were selected (an unselected gap may share every corner).
        let face_triangles = faces
            .map(|faces| {
                let polygons = self.mesh().polygons();
                let mut triangles = BTreeSet::new();
                for &face in faces {
                    if face >= polygons.len() {
                        return Err("Select faces on the current mesh".to_string());
                    }
                    for copy in symmetry.face_copies(face) {
                        let vertices = &polygons[copy].vertices;
                        for a in 0..vertices.len() {
                            for b in a + 1..vertices.len() {
                                for c in b + 1..vertices.len() {
                                    let mut t = [vertices[a], vertices[b], vertices[c]];
                                    t.sort_unstable();
                                    triangles.insert(t);
                                }
                            }
                        }
                    }
                }
                Ok(triangles)
            })
            .transpose()?;
        let selected: BTreeSet<_> = vertices
            .iter()
            .flat_map(|&v| symmetry.vertex_copies(v))
            .collect();
        if selected.is_empty() || selected.iter().any(|&v| v >= self.mesh().vertices.len()) {
            return Err("Select vertices, edges, or faces first".into());
        }
        let mut geometry = self.geometry.clone();
        let mesh = &mut geometry.mesh;
        // Reuse weighted vertex assignment with a temporary mask. Exact face
        // labels replace the inferred labels when the input was a face selection.
        let previous = mesh.vertex_groups.clone();
        let previous_labels = mesh.polygroups.clone();
        let mask = std::mem::replace(&mut mesh.mask, vec![0.; mesh.vertices.len()]);
        for v in selected {
            mesh.mask[v] = 1.;
        }
        let index = if let Some(index) = existing {
            mesh.assign_mask_to_group(index, 0.5, remove)?;
            index
        } else {
            let name = mesh.unique_group_name(if name.trim().is_empty() {
                "Selection"
            } else {
                name.trim()
            });
            mesh.create_replacement_group(name, 0.5, false)?
        };
        if let Some(triangles) = face_triangles {
            mesh.polygroups = previous_labels.clone();
            mesh.polygroups.resize(mesh.triangles.len(), 0);
            for (i, &triangle) in mesh.triangles.iter().enumerate() {
                let mut key = triangle;
                key.sort_unstable();
                if triangles.contains(&key) {
                    if !remove {
                        mesh.polygroups[i] = index + 1;
                    } else if mesh.polygroups[i] == index + 1 {
                        mesh.polygroups[i] = 0;
                    }
                }
            }
            mesh.canonicalize_polygroups();
        }
        mesh.replace_group_memberships(index, 0.5, remove, &previous, &previous_labels);
        mesh.mask = mask;
        let mapping = geometry.compact_vertex_groups();
        geometry.mesh.validate()?;
        if geometry != self.geometry {
            self.replace_geometry_named(geometry, "Group component selection")?;
        }
        Ok(mapping[index])
    }
}

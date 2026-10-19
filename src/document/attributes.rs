//! Vertex groups and partial face visibility.
use super::*;

impl Document {
    pub(super) fn edit_group_meshes(
        &mut self,
        label: &'static str,
        edit: impl FnMut(&mut Mesh) -> Result<(), String>,
    ) -> Result<(), String> {
        self.edit_group_registry(label, edit, false).map(|_| ())
    }
    fn edit_group_registry(
        &mut self,
        label: &'static str,
        mut edit: impl FnMut(&mut Mesh) -> Result<(), String>,
        compact: bool,
    ) -> Result<Vec<Option<usize>>, String> {
        self.require_sculpt_mesh()?;
        self.end_edit();
        self.end_stroke();
        let mut geometry = self.geometry.clone();
        for mesh in std::iter::once(&mut geometry.mesh)
            .chain(geometry.settings.lower.iter_mut())
            .chain(geometry.settings.upper.iter_mut())
        {
            edit(mesh)?;
        }
        let mapping = if compact {
            geometry.compact_vertex_groups()
        } else {
            (0..geometry.mesh.vertex_groups.len()).map(Some).collect()
        };
        geometry.mesh.validate()?;
        if geometry != self.geometry {
            self.replace_geometry_named(geometry, label)?;
        }
        Ok(mapping)
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
        let name = if name.trim().is_empty() {
            (1..)
                .map(|n| format!("Group {n}"))
                .find(|name| !self.mesh().vertex_groups.iter().any(|g| &g.name == name))
                .unwrap()
        } else {
            name
        };
        let index = if all {
            0
        } else {
            self.mesh().vertex_groups.len()
        };
        let mapping = self.edit_group_registry(
            if all {
                "Group all vertices"
            } else {
                "Create vertex group"
            },
            |mesh| {
                if all {
                    // Collapse memberships on every subdivision level, keeping the effective
                    // visibility independent of the group registry that is about to go away.
                    let visible = (0..mesh.triangles.len())
                        .map(|i| mesh.face_visible(i))
                        .collect();
                    mesh.set_face_visibility(visible);
                    mesh.vertex_groups.clear();
                    mesh.polygroups.clear();
                }
                let previous = mesh.vertex_groups.clone();
                let previous_labels = mesh.polygroups.clone();
                let index = mesh.create_replacement_group(name.clone(), threshold, all)?;
                mesh.replace_group_memberships(
                    index,
                    threshold,
                    false,
                    &previous,
                    &previous_labels,
                );
                Ok(())
            },
            true,
        )?;
        Ok(mapping[index].expect("A newly assigned group has members"))
    }
    pub fn assign_mask_to_group(
        &mut self,
        index: usize,
        remove: bool,
    ) -> Result<Option<usize>, String> {
        let threshold = self.geometry.scene.active_metadata.mask_threshold;
        self.edit_group_registry(
            if remove {
                "Remove masked vertices from group"
            } else {
                "Assign masked vertices to group"
            },
            |mesh| {
                let previous = mesh.vertex_groups.clone();
                let previous_labels = mesh.polygroups.clone();
                mesh.assign_mask_to_group(index, threshold, remove)?;
                mesh.replace_group_memberships(
                    index,
                    threshold,
                    remove,
                    &previous,
                    &previous_labels,
                );
                Ok(())
            },
            true,
        )
        .map(|mapping| mapping[index])
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
        self.edit_group_meshes("Vertex group visibility", |mesh| {
            mesh.vertex_groups
                .get_mut(index)
                .ok_or("Vertex group no longer exists")?
                .visible = visible;
            if visible && !mesh.hidden_faces.is_empty() {
                for (i, hidden) in mesh.hidden_faces.iter_mut().enumerate() {
                    if mesh.polygroups.get(i).copied().unwrap_or(0) == index + 1 {
                        *hidden = false;
                    }
                }
                if mesh.hidden_faces.iter().all(|&h| !h) {
                    mesh.hidden_faces.clear();
                }
            }
            Ok(())
        })
    }
    // Normalize group visibility consistently across all levels, but preserve
    // history/redo when the operation leaves every effective face set unchanged.
    pub(super) fn edit_visibility_meshes(
        &mut self,
        label: &'static str,
        mut edit: impl FnMut(&mut Mesh) -> Result<(), String>,
    ) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        self.end_edit();
        self.end_stroke();
        let mut geometry = self.geometry.clone();
        let old = std::iter::once(&self.geometry.mesh)
            .chain(self.geometry.settings.lower.iter())
            .chain(self.geometry.settings.upper.iter());
        let new = std::iter::once(&mut geometry.mesh)
            .chain(geometry.settings.lower.iter_mut())
            .chain(geometry.settings.upper.iter_mut());
        let mut changed = false;
        for (before, after) in old.zip(new) {
            edit(after)?;
            changed |= (0..before.triangles.len())
                .any(|i| before.face_visible(i) != after.face_visible(i));
        }
        if changed {
            self.replace_geometry_named(geometry, label)?;
        }
        Ok(())
    }
    pub fn show_all_faces(&mut self) -> Result<(), String> {
        self.edit_group_meshes("Show all faces", |mesh| {
            mesh.show_all_faces();
            Ok(())
        })
    }
    pub fn invert_visibility(&mut self) -> Result<(), String> {
        self.edit_visibility_meshes("Invert visibility", |mesh| {
            mesh.invert_visibility();
            Ok(())
        })
    }
    pub fn select_polygroup(&mut self, id: usize) -> Result<(), String> {
        let other_visible_group = (0..self.mesh().triangles.len()).any(|i| {
            self.mesh().face_visible(i) && self.mesh().polygroups.get(i).copied().unwrap_or(0) != id
        });
        // Hide the clicked group. At the last visible group, swap the visible
        // and hidden face sets so that the clicked group still disappears.
        if !other_visible_group {
            return self.invert_visibility();
        }
        self.edit_visibility_meshes("Hide vertex group faces", |mesh| {
            let visible = (0..mesh.triangles.len())
                .map(|i| mesh.face_visible(i) && mesh.polygroups.get(i).copied().unwrap_or(0) != id)
                .collect();
            mesh.set_face_visibility(visible);
            Ok(())
        })
    }
    /// The same view-space region is applied to every stored subdivision level.
    /// Quads are selected as a whole, using their center, through all depths.
    pub fn select_face_region(
        &mut self,
        contains: impl Fn(crate::sculpt::Vec3) -> bool,
        hide: bool,
    ) -> Result<(), String> {
        self.edit_visibility_meshes(
            if hide {
                "Hide selection"
            } else {
                "Isolate selection"
            },
            |mesh| {
                let mut selected: Vec<_> = mesh
                    .triangles
                    .iter()
                    .map(|t| {
                        contains(
                            (mesh.vertices[t[0]] + mesh.vertices[t[1]] + mesh.vertices[t[2]])
                                * (1. / 3.),
                        )
                    })
                    .collect();
                let key = |mut t: [usize; 3]| {
                    t.sort_unstable();
                    t
                };
                let faces: std::collections::HashMap<_, _> = mesh
                    .triangles
                    .iter()
                    .enumerate()
                    .map(|(i, &t)| (key(t), i))
                    .collect();
                for &[a, b, c, d] in &mesh.quads {
                    let inside = contains(
                        (mesh.vertices[a] + mesh.vertices[b] + mesh.vertices[c] + mesh.vertices[d])
                            * 0.25,
                    );
                    for t in [[a, b, c], [a, c, d], [a, b, d], [b, c, d]] {
                        if let Some(&i) = faces.get(&key(t)) {
                            selected[i] = inside;
                        }
                    }
                }
                let visible = selected
                    .iter()
                    .enumerate()
                    .map(|(i, &selected)| {
                        mesh.face_visible(i) && if hide { !selected } else { selected }
                    })
                    .collect();
                mesh.set_face_visibility(visible);
                Ok(())
            },
        )
    }
    pub fn split_hidden(&mut self) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if !self.active_visible() {
            return Err("Show the active model before splitting hidden faces".into());
        }
        if self.model_count() >= 64 {
            return Err("Submodels are limited to 64".into());
        }
        if self.geometry.scene.active_metadata.temporary_thickness > 0. {
            return Err("Apply or remove thickness modifier before splitting hidden faces".into());
        }
        let hidden = self.mesh().hidden_face_count();
        if hidden == 0 || hidden == self.mesh().triangles.len() {
            return Err("Keep some faces visible and some hidden before splitting".into());
        }
        let visible_mesh = self.mesh().visibility_part(true)?;
        let hidden_mesh = self.mesh().visibility_part(false)?;
        let name = (1..)
            .map(|i| format!("Hidden {i}"))
            .find(|name| !self.submodel_names().any(|n| n == name))
            .unwrap();
        let mut models = self.ordered_models();
        let active = self.active_model_index();
        models[active].mesh = visible_mesh;
        models[active].mesh_settings = MeshSettings::default();
        let mut metadata = models[active].metadata.clone();
        metadata.visible = true;
        models.insert(
            active + 1,
            crate::sculpt::Submodel {
                name,
                metadata,
                mesh: hidden_mesh,
                mesh_settings: MeshSettings::default(),
            },
        );
        self.end_edit();
        self.end_stroke();
        self.record_edit("Split hidden");
        self.install_models(models, active);
        Ok(())
    }
    pub fn rename_vertex_group(&mut self, index: usize, name: String) -> Result<(), String> {
        self.edit_group_meshes("Rename vertex group", |mesh| {
            mesh.rename_vertex_group(index, name.clone())
        })
    }
}

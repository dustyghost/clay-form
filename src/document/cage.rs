use super::*;
use crate::sculpt::cage::{Adjustment, Cage};

impl Document {
    pub(crate) fn apply_cage(
        &mut self,
        cage: Cage,
        edit: Adjustment,
        respect_mask: bool,
    ) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if !self.active_visible() {
            return Err("Show the submodel to deform it".into());
        }
        let mut changed = false;
        let levels = std::iter::once(&self.geometry.mesh)
            .chain(&self.geometry.settings.lower)
            .chain(&self.geometry.settings.upper)
            .map(|mesh| {
                let vertices = cage.vertices(mesh, edit, respect_mask)?;
                changed |= vertices != mesh.vertices;
                Ok(vertices)
            })
            .collect::<Result<Vec<_>, String>>()?;
        if changed {
            self.end_stroke();
            self.end_edit();
            self.record_edit("Deform with frame");
            for (mesh, vertices) in std::iter::once(&mut self.geometry.mesh)
                .chain(&mut self.geometry.settings.lower)
                .chain(&mut self.geometry.settings.upper)
                .zip(levels)
            {
                mesh.vertices = vertices;
            }
            self.dirty = true;
            self.display_changed();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Axis, Geometry, Mesh, Vec3};
    #[test]
    fn cage_edits_preserve_other_meshes_levels_masks_and_history() {
        let mut geometry = Geometry::new(Mesh::sphere());
        geometry.mesh.mask[0] = 1.;
        geometry.subdivide().unwrap();
        geometry.set_level(1).unwrap();
        let mut document = Document::default();
        document.replace_geometry(geometry).unwrap();
        document.duplicate_model(false).unwrap();
        let before = document.geometry_snapshot();
        let cage = Cage::fit(document.mesh()).unwrap();
        for edit in [
            Adjustment::Pull(7, Vec3(0.4, 0., 0.)),
            Adjustment::Twist(Axis::Y, 1.),
            Adjustment::Bend(Axis::X, 0.6),
        ] {
            let count = document.history_counts().0;
            document.apply_cage(cage, edit, true).unwrap();
            let after = document.geometry_snapshot();
            assert_ne!(before.mesh.vertices, after.mesh.vertices);
            assert_eq!(before.mesh.vertices[0], after.mesh.vertices[0]);
            assert_ne!(
                before.settings.upper[0].vertices,
                after.settings.upper[0].vertices
            );
            assert_eq!(before.submodels, after.submodels);
            assert_eq!(count + 1, document.history_counts().0);
            let mut loaded = Document::default();
            loaded
                .replace_project(
                    serde_json::from_slice(&serde_json::to_vec(&document.project()).unwrap())
                        .unwrap(),
                    "frame.clay",
                )
                .unwrap();
            assert_eq!(loaded.geometry_snapshot(), after);
            document.undo();
            assert_eq!(document.geometry_snapshot(), before);
            document.redo();
            assert_eq!(document.geometry_snapshot(), after);
            document.undo();
        }
    }
}

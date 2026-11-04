use super::*;
use crate::sculpt::{BooleanMethod, BooleanRole, boolean::Operand, max_triangles, max_vertices};

impl Document {
    pub(crate) fn boolean_resolution(&self) -> u32 {
        self.geometry.scene.boolean_resolution.unwrap_or(32)
    }
    pub(crate) fn set_boolean_resolution(&mut self, r: u32) -> Result<(), String> {
        if !(16..=96).contains(&r) {
            return Err("Boolean resolution must be between 16 and 96".into());
        }
        if self.boolean_resolution() != r {
            self.end_edit();
            self.end_stroke();
            self.record_edit("Boolean resolution");
            self.geometry.scene.boolean_resolution = (r != 32).then_some(r);
            self.dirty = true;
            self.display_changed();
        }
        Ok(())
    }
    pub(crate) fn boolean_method(&self) -> BooleanMethod {
        self.geometry.scene.boolean_method
    }
    pub(crate) fn set_boolean_method(&mut self, method: BooleanMethod) {
        if self.boolean_method() != method {
            self.end_edit();
            self.end_stroke();
            self.record_edit("Boolean method");
            self.geometry.scene.boolean_method = method;
            self.dirty = true;
            self.display_changed();
        }
    }
    pub(crate) fn boolean_operands(&self) -> Result<Vec<Operand>, String> {
        let group = &self.geometry.scene.active_metadata.group;
        let mut operands = Vec::new();
        let (mut vertices, mut triangles) = (0, 0);
        for i in (0..self.model_count()).rev() {
            let model = self.model(i).unwrap();
            let meta = model.metadata;
            if &meta.group != group || !meta.visible || meta.boolean_role == BooleanRole::Regular {
                continue;
            }
            let mesh = model.mesh;
            if meta.temporary_thickness > 0. {
                return Err(format!(
                    "Apply thickness on {} before Boolean evaluation",
                    model.name
                ));
            }
            if self.boolean_method() == BooleanMethod::Voxel
                && (mesh.has_hidden_faces() || mesh.mask.iter().any(|&w| w > 0.))
            {
                return Err(format!(
                    "Show all faces and clear the mask on {}",
                    model.name
                ));
            }
            vertices += mesh.vertices.len();
            triangles += mesh.triangles.len();
            if vertices > max_vertices() || triangles > max_triangles() {
                return Err("Boolean operands exceed the mesh capacity".into());
            }
            operands.push(Operand {
                role: meta.boolean_role,
                name: model.name.into(),
                mesh: if self.boolean_method() == BooleanMethod::Surface {
                    mesh.clone()
                } else {
                    Mesh {
                        vertices: mesh.vertices.clone(),
                        triangles: mesh.triangles.clone(),
                        mask: vec![0.; mesh.vertices.len()],
                        ..Default::default()
                    }
                },
            });
        }
        Ok(operands)
    }
    pub(crate) fn bake_boolean(&mut self, mesh: Mesh, revision: u64) -> Result<(), String> {
        if revision != self.display_revision() {
            return Err("Wait for the updated Boolean preview".into());
        }
        if self.model_count() >= 64 {
            return Err("A Boolean result needs a free submodel slot (64 maximum)".into());
        }
        let before = self.geometry_snapshot();
        let group = self.geometry.scene.active_metadata.group.clone();
        // import_mesh validates its input before touching the scene.
        self.geometry.import_mesh(mesh, "Boolean result")?;
        self.geometry.scene.active_metadata.group = group.clone();
        for m in &mut self.geometry.submodels {
            if m.metadata.group == group && m.metadata.boolean_role != BooleanRole::Regular {
                m.metadata.visible = false;
            }
        }
        self.end_edit();
        self.end_stroke();
        self.record_edit_owned(before, "Bake Boolean stack");
        self.selected_instance = None;
        self.reconcile_paint_target();
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
}

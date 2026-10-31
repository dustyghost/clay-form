//! Image identity is independent of a model assignment. Library edits are geometry history.
use super::*;
use crate::sculpt::texture::Texture;
impl Document {
    pub(crate) fn texture_assets(&self) -> Vec<&Texture> {
        let mut seen = std::collections::HashSet::new();
        std::iter::once(&self.geometry.mesh)
            .chain(self.geometry.submodels.iter().map(|m| &m.mesh))
            .chain(&self.geometry.settings.lower)
            .chain(&self.geometry.settings.upper)
            .chain(
                self.geometry
                    .submodels
                    .iter()
                    .flat_map(|m| m.mesh_settings.lower.iter().chain(&m.mesh_settings.upper)),
            )
            .flat_map(|m| &m.uv.textures)
            .chain(&self.geometry.scene.texture_library)
            .filter(|t| seen.insert(t.id))
            .collect()
    }
    pub(crate) fn texture_asset(&self, id: u64) -> Option<&Texture> {
        self.texture_assets().into_iter().find(|t| t.id == id)
    }
    pub(crate) fn texture_users(&self, id: u64) -> Vec<String> {
        (0..self.model_count())
            .filter_map(|i| {
                let m = self.model(i)?;
                std::iter::once(m.mesh)
                    .chain(&m.settings.lower)
                    .chain(&m.settings.upper)
                    .any(|mesh| mesh.uv.textures.iter().any(|t| t.id == id))
                    .then(|| m.name.to_owned())
            })
            .collect()
    }
    pub(crate) fn save_texture_asset(&mut self, texture: Texture) -> Result<(), String> {
        texture.validate()?;
        self.end_stroke();
        let mut candidate = self.geometry_snapshot();
        let mut found = false;
        for mesh in std::iter::once(&mut candidate.mesh)
            .chain(&mut candidate.settings.lower)
            .chain(&mut candidate.settings.upper)
            .chain(candidate.submodels.iter_mut().flat_map(|m| {
                std::iter::once(&mut m.mesh)
                    .chain(&mut m.mesh_settings.lower)
                    .chain(&mut m.mesh_settings.upper)
            }))
        {
            for image in &mut mesh.uv.textures {
                if image.id == texture.id {
                    *image = texture.clone();
                    found = true;
                }
            }
        }
        if let Some(old) = candidate
            .scene
            .texture_library
            .iter_mut()
            .find(|t| t.id == texture.id)
        {
            *old = texture.clone();
        } else if !found {
            candidate.scene.texture_library.push(texture);
        }
        self.commit_texture_library(candidate, "Edit texture library")
    }
    pub(crate) fn unassign_texture_asset(&mut self, id: u64) -> Result<(), String> {
        self.remove_texture_uses(|texture| texture.id == id)
    }
    pub(super) fn remove_texture_uses(
        &mut self,
        remove: impl Fn(&Texture) -> bool,
    ) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        self.end_stroke();
        let mut candidate = self.geometry_snapshot();
        let previous: Vec<_> = std::iter::once(&candidate.mesh)
            .chain(&candidate.settings.lower)
            .chain(&candidate.settings.upper)
            .flat_map(|mesh| &mesh.uv.textures)
            .filter(|t| remove(t))
            .cloned()
            .collect();
        for image in previous {
            Self::retain_texture(&mut candidate, image);
        }
        for mesh in std::iter::once(&mut candidate.mesh)
            .chain(&mut candidate.settings.lower)
            .chain(&mut candidate.settings.upper)
        {
            mesh.uv.textures.retain(|t| !remove(t));
        }
        self.commit_texture_library(candidate, "Unassign texture")
    }
    pub(crate) fn delete_texture_asset(&mut self, id: u64) -> Result<(), String> {
        if !self.texture_users(id).is_empty() {
            return Err("Unassign this image from every model before deleting it. Other models keep their images.".into());
        }
        if self.texture_asset(id).is_none() {
            return Err("Image no longer exists".into());
        }
        let mut candidate = self.geometry_snapshot();
        candidate.scene.texture_library.retain(|t| t.id != id);
        self.commit_texture_library(candidate, "Delete unused image")
    }
    pub(super) fn retain_texture(candidate: &mut Geometry, texture: Texture) {
        if let Some(old) = candidate
            .scene
            .texture_library
            .iter_mut()
            .find(|t| t.id == texture.id)
        {
            *old = texture;
        } else {
            candidate.scene.texture_library.push(texture);
        }
    }
    pub(super) fn commit_texture_library(
        &mut self,
        candidate: Geometry,
        label: &'static str,
    ) -> Result<(), String> {
        crate::sculpt::texture::validate_library(
            std::iter::once(&candidate.mesh).chain(candidate.submodels.iter().map(|m| &m.mesh)),
            &candidate.scene.texture_library,
        )?;
        if candidate == self.geometry {
            return Ok(());
        }
        self.require_texture_history()?;
        self.replace_geometry_named(candidate, label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{
        Material, Primitive, PrimitiveKind,
        texture::{Channel, Precision},
    };
    fn plane() -> Document {
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        mesh.uv = mesh.unwrap_uv(true, 0.01, &|| false).unwrap();
        let mut doc = Document::default();
        doc.replace_geometry(Geometry::new(mesh)).unwrap();
        doc
    }
    #[test]
    fn library_assignment_shared_edits_unassignment_and_float_roundtrip() {
        let mut doc = plane();
        let mut image = Texture::new_with_precision(
            Channel::Color,
            16,
            &Material::default(),
            Precision::Float32,
        )
        .unwrap();
        image.pixels.set_pixel(0, [2.5, -0.2, 0.5, 0.7]);
        let id = image.id;
        let uv = doc.mesh().uv.clone();
        doc.save_texture_asset(image.clone()).unwrap();
        assert_eq!(doc.mesh().uv, uv, "creation never assigns or edits UVs");
        doc.assign_texture(image.clone()).unwrap();
        doc.duplicate_model(false).unwrap();
        assert_eq!(doc.texture_users(id).len(), 2);
        assert!(
            doc.delete_texture_asset(id).is_err(),
            "in-use pixels cannot be deleted"
        );
        image.name = "Shared HDR".into();
        image.pixels.set_pixel(3, [-1., 4., 0.1, 1.]);
        let before = doc.geometry_snapshot();
        doc.save_texture_asset(image.clone()).unwrap();
        for i in 0..doc.model_count() {
            assert_eq!(doc.model(i).unwrap().mesh.uv.textures[0], image);
        }
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        doc.remove_texture(Channel::Color).unwrap();
        assert_eq!(doc.texture_users(id).len(), 1);
        assert_eq!(doc.texture_asset(id), Some(&image));
        let other = 1 - doc.active_model_index();
        doc.select_submodel(other).unwrap();
        doc.remove_texture(Channel::Color).unwrap();
        assert!(doc.texture_users(id).is_empty());
        let json = crate::persistence::json_bytes(&doc.project()).unwrap();
        let project: Project = serde_json::from_slice(&json).unwrap();
        project.validate().unwrap();
        assert_eq!(project.scene.texture_library[0], image);
        doc.delete_texture_asset(id).unwrap();
        assert!(doc.texture_asset(id).is_none());
        doc.undo();
        assert_eq!(doc.texture_asset(id), Some(&image));
        doc.assign_texture(image).unwrap();
        assert_eq!(doc.mesh().uv.corners, uv.corners);
    }
    #[test]
    fn replacement_retains_previous_pixels_and_material_assignment_ignores_image_target() {
        let mut doc = plane();
        let first = Texture::new(Channel::Color, 16, &Material::default()).unwrap();
        let second = Texture::new(Channel::Color, 16, &Material::examples()[4].1).unwrap();
        doc.assign_texture(first.clone()).unwrap();
        doc.assign_texture(second.clone()).unwrap();
        assert_eq!(doc.texture_asset(first.id), Some(&first));
        doc.texture_paint = Some(Channel::Color);
        doc.assign_model_material(Material::examples()[4].1.clone())
            .unwrap();
        assert_eq!(doc.mesh().uv.textures, vec![second]);
        assert!(
            doc.mesh()
                .materials
                .iter()
                .all(|m| *m == Material::examples()[4].1)
        );
        assert_eq!(doc.texture_paint, Some(Channel::Color));
        doc.undo();
        assert!(doc.mesh().materials.is_empty());
    }
    #[test]
    fn library_pixels_count_towards_history_and_project_limits() {
        let mut doc = plane();
        let image = Texture::new(Channel::Color, 256, &Material::default()).unwrap();
        doc.save_texture_asset(image.clone()).unwrap();
        doc.delete_texture_asset(image.id).unwrap();
        assert!(doc.history.retained_bytes() >= image.pixels.byte_len());
        let mut project = doc.project();
        project.scene.texture_library = vec![image.clone(), image];
        assert!(
            project
                .validate()
                .unwrap_err()
                .contains("Duplicate texture")
        );
    }
    #[test]
    fn paint_updates_library_pixels_and_reopened_shared_uses() {
        let mut doc = plane();
        let image = Texture::new_with_precision(
            Channel::Color,
            16,
            &Material::default(),
            Precision::Float32,
        )
        .unwrap();
        let id = image.id;
        doc.save_texture_asset(image.clone()).unwrap();
        doc.assign_texture(image.clone()).unwrap();
        doc.duplicate_model(false).unwrap();
        let saved = crate::persistence::json_bytes(&doc.project()).unwrap();
        doc.replace_project(serde_json::from_slice(&saved).unwrap(), "library.clay")
            .unwrap();
        doc.texture_paint = Some(Channel::Color);
        let mut brush = Brush::new(Kind::Paint);
        brush.texture_colour = Some([2., -1., 0.5]);
        doc.fill_all_material(&brush).unwrap();
        let painted = doc.mesh().uv.textures[0].clone();
        assert_ne!(painted.pixels, image.pixels);
        let project = doc.project();
        assert_eq!(project.scene.texture_library[0], painted);
        assert_eq!(project.submodels[0].mesh.uv.textures[0], painted);
        doc.undo();
        assert_eq!(doc.texture_asset(id).unwrap().pixels, image.pixels);
        doc.redo();
        assert_eq!(doc.texture_asset(id).unwrap().pixels, painted.pixels);
    }

    #[test]
    fn unused_images_cannot_bypass_the_project_pixel_budget() {
        let doc = plane();
        let image = Texture::new(Channel::Color, 1024, &Material::default()).unwrap();
        let mut project = doc.project();
        project.scene.texture_library = (0..49)
            .map(|_| {
                let mut copy = image.clone();
                copy.id = Texture::new_id();
                copy
            })
            .collect();
        assert!(
            project
                .validate()
                .unwrap_err()
                .contains("Texture budget exceeded")
        );
    }
    #[test]
    fn unassigning_a_retained_use_preserves_the_current_image() {
        let mut doc = plane();
        let current = Texture::new(Channel::Color, 16, &Material::default()).unwrap();
        let retained = Texture::new(Channel::Color, 16, &Material::examples()[4].1).unwrap();
        doc.assign_texture(current.clone()).unwrap();
        doc.geometry.subdivide().unwrap();
        doc.geometry.mesh.uv = doc.mesh().unwrap_uv(true, 0.01, &|| false).unwrap();
        doc.assign_texture(current.clone()).unwrap();
        doc.geometry.settings.lower[0].uv.textures = vec![retained.clone()];
        assert_eq!(doc.texture_users(retained.id).len(), 1);
        doc.unassign_texture_asset(retained.id).unwrap();
        assert_eq!(doc.mesh().uv.textures, vec![current]);
        assert!(doc.geometry.settings.lower[0].uv.textures.is_empty());
        assert_eq!(doc.texture_asset(retained.id), Some(&retained));
        doc.undo();
        let next = Texture::new(Channel::Color, 16, &Material::default()).unwrap();
        doc.assign_texture(next).unwrap();
        assert_eq!(
            doc.texture_asset(retained.id),
            Some(&retained),
            "assignment retains inactive-level pixels too"
        );
    }
}

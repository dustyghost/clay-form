use super::*;

impl Document {
    pub fn fill_all_material(&mut self, brush: &Brush) -> Result<(), String> {
        if brush.kind != Kind::Paint {
            return Err("Select a Paint brush before filling materials".into());
        }
        if !self.active_visible() {
            return Err("Show the active model before painting".into());
        }
        crate::sculpt::validate_brush(brush)?;
        if self.texture_paint.is_some() {
            return self.fill_texture_target(brush, true);
        }
        self.edit_group_meshes("Fill all materials", |mesh| {
            mesh.fill_all_material(brush);
            Ok(())
        })
    }
    pub fn fill_material(&mut self, brush: &Brush) -> Result<(), String> {
        if brush.kind != Kind::Paint {
            return Err("Select a Paint brush before filling materials".into());
        }
        if !self.active_visible() {
            return Err("Show the active model before painting".into());
        }
        crate::sculpt::validate_brush(brush)?;
        if self.texture_paint.is_some() {
            return self.fill_texture_target(brush, false);
        }
        self.edit_group_meshes("Fill material", |mesh| {
            mesh.fill_material(brush);
            Ok(())
        })
    }
    fn fill_texture_target(&mut self, brush: &Brush, all: bool) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        self.require_texture_history()?;
        self.end_stroke();
        let channel = self.texture_paint.ok_or("Select an image paint target")?;
        let mut candidate = self.geometry.clone();
        if candidate.mesh.fill_texture(channel, brush, all)? {
            candidate.mesh.pad_texture(channel);
            Self::sync_geometry_texture_uses(&mut candidate);
            self.replace_geometry_named(
                candidate,
                if all {
                    "Fill all texture"
                } else {
                    "Fill texture"
                },
            )?;
        }
        Ok(())
    }
    pub(crate) fn paint_stroke_active(&self) -> bool {
        self.paint_stroke.is_some()
    }
    pub(super) fn apply_paint_dab(
        &mut self,
        brush: &Brush,
        dab: Dab,
        stencil: Option<&crate::sculpt::alpha::Projection>,
    ) -> bool {
        if self.paint_stroke.is_none() {
            self.paint_sequence = 0;
            self.paint_scratch.begin_stroke();
            self.end_stroke();
            let dirty = self.dirty;
            self.begin_stroke();
            let mut settings = self.capture_edit_settings();
            settings.active_brush = Some(brush.clone());
            self.paint_stroke = Some((dirty, settings));
        }
        let dabs = self.symmetry.dabs(dab);
        let mut sampled_brush = brush.clone();
        sampled_brush.paint_effect.seed = self.paint_sequence;
        self.paint_sequence = self.paint_sequence.wrapping_add(1);
        let brush = &sampled_brush;
        let stencils = stencil.map_or_else(Vec::new, |s| s.copies(&self.symmetry));
        let occluders: Vec<_> = self
            .geometry
            .submodels
            .iter()
            .filter(|m| m.metadata.visible)
            .map(|m| &m.mesh)
            .collect();
        let changed = if let Some(channel) = self.texture_paint {
            self.geometry.mesh.paint_texture_with_scratch(
                channel,
                brush,
                &dabs,
                &stencils,
                &occluders,
                &mut self.paint_scratch,
            )
        } else {
            #[cfg(test)]
            crate::sculpt::texture::profile::mark("document_setup");
            self.geometry.mesh.paint_material_with_scratch(
                brush,
                &dabs,
                &stencils,
                &occluders,
                &mut self.paint_scratch,
            )
        };
        if changed {
            self.dirty = true;
            if self.texture_paint.is_some() {
                self.texture_display_changed();
            } else {
                self.display_changed();
            }
        }
        changed
    }
    pub(super) fn finish_paint_stroke(&mut self) {
        if self.paint_stroke.is_some()
            && self
                .stroke_before
                .as_ref()
                .is_some_and(|before| before.mesh.uv.textures != self.geometry.mesh.uv.textures)
            && let Some(channel) = self.texture_paint
        {
            let before = self
                .stroke_before
                .as_ref()
                .and_then(|g| g.mesh.uv.textures.iter().find(|t| t.channel == channel));
            self.geometry
                .mesh
                .pad_texture_cached(channel, before, &mut self.paint_scratch.gutter);
            self.sync_texture_uses();
            self.texture_display_changed();
        }
        if let Some((_, settings)) = self.paint_stroke.take()
            && let Some(before) = self.stroke_before.take()
            && before != self.geometry
        {
            self.history.record_owned(
                before,
                if self.texture_paint.is_some() {
                    "Paint texture"
                } else {
                    "Paint material"
                },
                Some(settings),
            );
        }
    }
    pub(crate) fn cancel_paint_stroke(&mut self) -> bool {
        let Some((dirty, _)) = self.paint_stroke.take() else {
            return false;
        };
        if let Some(before) = self.stroke_before.take() {
            self.geometry = before;
            self.display_changed();
        }
        self.dirty = dirty;
        self.end_stroke();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Material, Vec3};
    #[test]
    fn material_strokes_history_cancel_save_fill_and_level_switching() {
        let mut doc = Document::default();
        doc.set_symmetry(Symmetry {
            enabled: false,
            ..Default::default()
        })
        .unwrap();
        let before = doc.geometry_snapshot();
        let history = doc.history_counts();
        let brush = Brush::new(Kind::Paint);
        let dab = Dab {
            center: Vec3(0., 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.5,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        assert!(doc.apply_dab(&brush, dab));
        assert!(doc.apply_dab(&brush, dab));
        assert_eq!(doc.history_counts(), history, "history waits until release");
        assert!(
            !doc.end_stroke(),
            "material-only edits must not request remeshing"
        );
        let painted = doc.geometry_snapshot();
        assert_eq!(painted.mesh.vertices, before.mesh.vertices);
        assert_eq!(doc.history_counts().0, history.0 + 1);
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        let undone = doc.history_counts();
        assert!(doc.apply_dab(&brush, dab));
        assert!(doc.cancel_paint_stroke());
        assert_eq!(doc.history_counts(), undone);
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), painted);
        let file = serde_json::to_string(&doc.project()).unwrap();
        let project: Project = serde_json::from_str(&file).unwrap();
        project.validate().unwrap();
        let mut reopened = Document::default();
        reopened.replace_project(project, "paint.clay").unwrap();
        assert_eq!(reopened.mesh(), doc.mesh());
        let mut geometry = doc.geometry_snapshot();
        geometry.subdivide().unwrap();
        doc.replace_geometry(geometry).unwrap();
        let mut fill = Brush::new(Kind::Paint);
        fill.material = Material::examples()[4].1.clone();
        fill.strength = 1.;
        doc.fill_material(&fill).unwrap();
        assert!(doc.mesh().materials.iter().all(|m| *m == fill.material));
        let history = doc.history_counts();
        doc.fill_material(&fill).unwrap();
        assert_eq!(history, doc.history_counts(), "same fill is a no-op");
        let mut geometry = doc.geometry_snapshot();
        geometry.set_level(1).unwrap();
        assert!(geometry.mesh.materials.iter().all(|m| *m == fill.material));
        geometry.set_level(2).unwrap();
        assert!(geometry.mesh.materials.iter().all(|m| *m == fill.material));
    }
}

#[cfg(test)]
mod fill_all_tests {
    use super::*;
    #[test]
    fn material_fill_all_ignores_protection_preserves_other_models_and_undoes_all_levels() {
        let mut doc = Document::default();
        doc.duplicate_model(false).unwrap();
        let mut geometry = doc.geometry_snapshot();
        geometry.mesh.mask.fill(1.);
        geometry
            .mesh
            .set_face_visibility(vec![false; geometry.mesh.triangles.len()]);
        geometry.subdivide().unwrap();
        doc.replace_geometry(geometry).unwrap();
        let before = doc.geometry_snapshot();
        let history = doc.history_counts();
        let mut brush = Brush::new(Kind::Paint);
        brush.material = crate::sculpt::Material::examples()[4].1.clone();
        brush.strength = 0.;
        brush.subtract = true;
        doc.fill_all_material(&brush).unwrap();
        assert_eq!(doc.history_counts().0, history.0 + 1);
        let after = doc.geometry_snapshot();
        assert_eq!(after.mesh.vertices, before.mesh.vertices);
        assert_eq!(after.mesh.mask, before.mesh.mask);
        assert_eq!(
            after.scene, before.scene,
            "other models and metadata unchanged"
        );
        for mesh in std::iter::once(&after.mesh)
            .chain(after.settings.lower.iter())
            .chain(after.settings.upper.iter())
        {
            assert_eq!(mesh.materials.len(), mesh.vertices.len());
            assert!(mesh.materials.iter().all(|m| *m == brush.material));
        }
        let history = doc.history_counts();
        doc.fill_all_material(&brush).unwrap();
        assert_eq!(doc.history_counts(), history, "identical fill is a no-op");
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), after);
        brush.paint_channels = crate::sculpt::PaintChannels::Color;
        brush.material = crate::sculpt::Material::examples()[1].1.clone();
        doc.fill_all_material(&brush).unwrap();
        assert!(
            doc.mesh()
                .materials
                .iter()
                .all(|m| m.base_color == brush.material.base_color && m.metallic == 1.)
        );
    }
}

#[cfg(test)]
mod texture_target_tests {
    use super::*;
    use crate::sculpt::texture::{Channel, Precision, Texture};
    fn document() -> Document {
        let mut doc = Document::default();
        doc.replace_geometry(Geometry::new(crate::sculpt::texture::tests::sheets()))
            .unwrap();
        doc.texture_paint = Some(Channel::Color);
        doc.set_symmetry(Symmetry {
            enabled: false,
            ..Default::default()
        })
        .unwrap();
        doc
    }
    #[test]
    fn image_target_fill_respects_masks_channel_precision_and_history() {
        let mut doc = document();
        doc.geometry.mesh.mask.fill(1.);
        doc.geometry.mesh.uv.textures.push(
            Texture::new_with_precision(
                Channel::Roughness,
                64,
                &crate::sculpt::Material::default(),
                Precision::Float32,
            )
            .unwrap(),
        );
        doc.texture_paint = Some(Channel::Roughness);
        let before = doc.geometry_snapshot();
        let mut brush = Brush::new(Kind::Paint);
        brush.material.roughness = 0.1234567;
        brush.strength = 1.;
        assert!(
            !doc.apply_dab(&brush, crate::sculpt::texture::tests::dab()),
            "fully masked float paint is an exact no-op"
        );
        doc.fill_material(&brush).unwrap();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.fill_all_material(&brush).unwrap();
        let texture = &doc.mesh().uv.textures[1];
        assert_eq!(texture.pixels.precision(), Precision::Float32);
        assert_eq!(
            texture.pixels.pixel(32 * 64 + 16)[0],
            brush.material.roughness
        );
        assert_eq!(doc.mesh().materials, before.mesh.materials);
        assert_eq!(doc.mesh().uv.textures[0], before.mesh.uv.textures[0]);
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
    }
    #[test]
    fn shared_images_detach_once_per_stroke_and_snapshots_are_canonical() {
        let mut doc = document();
        doc.duplicate_model(false).unwrap();
        doc.texture_paint = Some(Channel::Color);
        let before = doc.geometry_snapshot();
        let mut brush = Brush::new(Kind::Paint);
        brush.material = crate::sculpt::Material::from_srgb([1., 0., 0.], 0.5, 0.);
        brush.strength = 0.2;
        let dab = crate::sculpt::texture::tests::dab();
        assert!(doc.apply_dab(&brush, dab));
        let storage = doc.mesh().uv.textures[0].pixels.storage_id();
        assert!(doc.apply_dab(&brush, dab));
        assert_eq!(storage, doc.mesh().uv.textures[0].pixels.storage_id());
        {
            let snapshot = doc.geometry_snapshot();
            assert_eq!(
                snapshot.mesh.uv.textures[0],
                snapshot.submodels[0].mesh.uv.textures[0]
            );
        }
        doc.end_stroke();
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_ne!(doc.geometry_snapshot(), before);
        assert!(doc.apply_dab(&brush, dab));
        assert!(doc.cancel_paint_stroke());
    }
}

//! Mask operations and continuous intensity/threshold edits.
use super::*;

impl Document {
    pub fn grow_mask(&mut self, grow: bool) -> Result<(), String> {
        self.edit_group_meshes(if grow { "Grow mask" } else { "Shrink mask" }, |mesh| {
            mesh.grow_mask(grow);
            Ok(())
        })
    }
    /// Projected lasso masking affects all stored levels and protects hidden
    /// faces and their boundary vertices, just like the Mask brush.
    pub fn mask_region(
        &mut self,
        contains: impl Fn(crate::sculpt::Vec3) -> bool,
        erase: bool,
    ) -> Result<(), String> {
        if !self.active_visible() {
            return Err("Show the active model before masking".into());
        }
        self.edit_group_meshes(
            if erase {
                "Erase mask lasso"
            } else {
                "Mask lasso"
            },
            |mesh| {
                let editable = mesh.sculptable_vertices();
                for (i, &point) in mesh.vertices.iter().enumerate() {
                    if editable[i] && contains(point) {
                        mesh.mask[i] = if erase { 0. } else { 1. };
                    }
                }
                Ok(())
            },
        )
    }
    pub fn mask_peak(&self) -> f32 {
        std::iter::once(&self.geometry.mesh)
            .chain(self.geometry.settings.lower.iter())
            .chain(self.geometry.settings.upper.iter())
            .flat_map(|m| m.mask.iter())
            .copied()
            .fold(0., f32::max)
    }
    pub fn blur_mask(&mut self) -> Result<(), String> {
        self.edit_group_meshes("Blur mask", |mesh| {
            mesh.blur_mask();
            Ok(())
        })
    }
    pub fn sharpen_mask(&mut self) -> Result<(), String> {
        self.edit_group_meshes("Sharpen mask", |mesh| {
            mesh.sharpen_mask();
            Ok(())
        })
    }
    pub fn set_mask_intensity(&mut self, amount: f32) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if !amount.is_finite() || !(0. ..=1.).contains(&amount) {
            return Err("Mask intensity must be between 0% and 100%".into());
        }
        self.prepare_edit(Some(EditKind::MaskIntensity));
        if self.edit.is_none() {
            let peak = self.mask_peak();
            if peak == 0. || peak == amount {
                return Ok(());
            }
            let masks = std::iter::once(&self.geometry.mesh)
                .chain(self.geometry.settings.lower.iter())
                .chain(self.geometry.settings.upper.iter())
                .map(|mesh| mesh.mask.clone())
                .collect();
            self.begin_edit(EditSession::MaskIntensity { peak, masks });
        }
        let Some(EditSession::MaskIntensity { peak, masks }) = &self.edit else {
            unreachable!("mask session started above")
        };
        for (mesh, old) in std::iter::once(&mut self.geometry.mesh)
            .chain(self.geometry.settings.lower.iter_mut())
            .chain(self.geometry.settings.upper.iter_mut())
            .zip(masks)
        {
            for (value, &original) in mesh.mask.iter_mut().zip(old) {
                *value = (original / peak * amount).clamp(0., 1.);
            }
        }
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn set_mask_threshold(&mut self, amount: f32) -> Result<(), String> {
        if !amount.is_finite() || !(0. ..=1.).contains(&amount) {
            return Err("Mask threshold must be between 0% and 100%".into());
        }
        self.prepare_edit(Some(EditKind::MaskThreshold));
        let mut metadata = self.geometry.scene.active_metadata.clone();
        metadata.mask_threshold = amount;
        if metadata == self.geometry.scene.active_metadata {
            return Ok(());
        }
        if metadata.temporary_thickness > 0. && metadata.thickness_group_mask {
            self.mesh().solidify_with_metadata(&metadata)?;
        }
        self.begin_edit(EditSession::MaskThreshold);
        self.geometry.scene.active_metadata = metadata;
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn invert_mask(&mut self) -> Result<(), String> {
        self.require_sculpt_mesh()?;
        if self.mesh().mask.is_empty() {
            return Ok(());
        }
        self.end_edit();
        self.end_stroke();
        self.history.record_named(&self.geometry, "Invert mask");
        for mesh in std::iter::once(&mut self.geometry.mesh)
            .chain(self.geometry.settings.lower.iter_mut())
            .chain(self.geometry.settings.upper.iter_mut())
        {
            for mask in &mut mesh.mask {
                *mask = 1. - *mask;
            }
        }
        self.dirty = true;
        self.display_changed();
        Ok(())
    }
    pub fn delete_masked(&mut self) -> Result<bool, String> {
        self.require_sculpt_mesh()?;
        if !self.active_visible() {
            return Err("Show the active model before deleting masked surface".into());
        }
        let Some(mesh) = self
            .mesh()
            .delete_masked_threshold(self.geometry.scene.active_metadata.mask_threshold)?
        else {
            return Ok(false);
        };
        let mut geometry = self.geometry.clone();
        geometry.mesh = mesh;
        // Stored levels cannot retain correspondence after cutting a hole.
        // Adaptive and voxel modes require a closed surface.
        geometry.settings = MeshSettings {
            resolution: geometry.settings.resolution,
            ..Default::default()
        };
        self.replace_geometry_named(geometry, "Delete masked surface")?;
        Ok(true)
    }
    pub fn clear_mask(&mut self) {
        if self.primitive().is_some() {
            return;
        }
        self.end_edit();
        self.end_stroke();
        if self.mask_peak() == 0. {
            return;
        }
        self.history.record_named(&self.geometry, "Clear mask");
        self.geometry.mesh.mask.fill(0.);
        for level in self
            .geometry
            .settings
            .lower
            .iter_mut()
            .chain(self.geometry.settings.upper.iter_mut())
        {
            level.mask.fill(0.);
        }
        self.dirty = true;
        self.display_changed();
    }
}

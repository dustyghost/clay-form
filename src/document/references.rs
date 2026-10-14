use super::Document;
use crate::reference::{self, Placement, ReferenceImage};

impl Document {
    pub fn references(&self) -> &[ReferenceImage] {
        &self.references
    }
    pub(crate) fn add_reference(&mut self, mut image: ReferenceImage) -> Result<(), String> {
        let mut images = self.references.clone();
        let step = images.len() as f32 * 0.06;
        image.placement.center = [0.2 + step, 0.35 + step * 0.5];
        images.push(image);
        reference::validate(&images)?;
        self.references = images;
        self.dirty = true;
        Ok(())
    }
    pub(crate) fn place_reference(
        &mut self,
        index: usize,
        placement: Placement,
    ) -> Result<(), String> {
        placement.validate()?;
        let image = self
            .references
            .get_mut(index)
            .ok_or("Reference image no longer exists")?;
        if image.placement != placement {
            image.placement = placement;
            self.dirty = true;
        }
        Ok(())
    }
    pub(crate) fn remove_reference(&mut self, index: usize) -> Result<(), String> {
        if index >= self.references.len() {
            return Err("Reference image no longer exists".into());
        }
        self.references.remove(index);
        self.dirty = true;
        Ok(())
    }
}

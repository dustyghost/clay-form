//! Project alpha library and stencil settings (like brush presets, outside geometry history).
use super::*;
use crate::sculpt::alpha::{Asset, Stencil, validate_library};
impl Document {
    pub fn alphas(&self) -> &[Asset] {
        &self.alphas
    }
    pub fn stencil(&self) -> Option<&Stencil> {
        self.stencil.as_ref()
    }
    pub fn save_alpha(&mut self, index: Option<usize>, asset: Asset) -> Result<usize, String> {
        let mut library = self.alphas.clone();
        let index = if let Some(index) = index {
            *library.get_mut(index).ok_or("Alpha no longer exists")? = asset;
            index
        } else {
            library.push(asset);
            library.len() - 1
        };
        validate_library(&library)?;
        if library != self.alphas {
            self.alphas = library;
            self.dirty = true;
        }
        Ok(index)
    }
    pub fn remove_alpha(&mut self, index: usize) -> Result<(), String> {
        if index >= self.alphas.len() {
            return Err("Alpha no longer exists".into());
        }
        self.alphas.remove(index);
        self.dirty = true;
        Ok(())
    }
    pub fn set_stencil(&mut self, stencil: Option<Stencil>) -> Result<(), String> {
        if let Some(s) = &stencil {
            s.validate()?;
        }
        if self.stencil != stencil {
            self.stencil = stencil;
            self.dirty = true;
        }
        Ok(())
    }
}

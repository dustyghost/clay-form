//! Brush library editing and import.
use super::*;

impl Document {
    pub fn update_brush(&mut self, index: usize, mut brush: Brush) -> Result<(), String> {
        validate_brush(&brush)?;
        let target = self
            .brushes
            .get_mut(index)
            .ok_or("Brush no longer exists")?;
        brush.id = Some(target.identity());
        if *target != brush {
            *target = brush;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn set_alpha(&mut self, index: usize, alpha: Alpha) -> Result<(), String> {
        let mut brush = self
            .brushes
            .get(index)
            .ok_or("Brush no longer exists")?
            .clone();
        brush.alpha = alpha;
        self.update_brush(index, brush)
    }
    pub fn append_brush(&mut self, mut brush: Brush) -> Result<usize, String> {
        validate_brush(&brush)?;
        if self.brushes.len() >= 256 {
            return Err("Brush library is limited to 256 presets".into());
        }
        brush.ensure_identity();
        if self
            .brushes
            .iter()
            .any(|b| b.identity() == brush.identity())
        {
            brush.renew_identity();
        }
        self.brushes.push(brush);
        self.dirty = true;
        Ok(self.brushes.len() - 1)
    }
    /// Validate and merge the entire batch before changing the library.
    pub fn import_brushes(
        &mut self,
        incoming: Vec<Brush>,
    ) -> Result<(usize, usize, Option<usize>), String> {
        let mut brushes = self.brushes.clone();
        let mut added = 0;
        let mut updated = 0;
        let mut last = None;
        let mut seen = std::collections::HashSet::new();
        for mut brush in incoming {
            validate_brush(&brush)?;
            brush.ensure_identity();
            let id = brush.identity();
            if let Some(index) = brushes.iter().position(|b| b.identity() == id) {
                if brushes[index] != brush {
                    brushes[index] = brush;
                    if seen.insert(id) {
                        updated += 1;
                    }
                }
                last = Some(index);
            } else {
                if brushes.len() >= 256 {
                    return Err("Brush library is limited to 256 presets".into());
                }
                last = Some(brushes.len());
                brushes.push(brush);
                seen.insert(id);
                added += 1;
            }
        }
        if brushes != self.brushes {
            self.brushes = brushes;
            self.dirty = true;
        }
        Ok((added, updated, last))
    }
    pub fn duplicate_brush(&mut self, index: usize) -> Result<usize, String> {
        let mut brush = self
            .brushes
            .get(index)
            .ok_or("Brush no longer exists")?
            .clone();
        brush.name.push_str(" copy");
        brush.renew_identity();
        self.append_brush(brush)
    }
}

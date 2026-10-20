//! Sculpt stroke boundaries, symmetry, and dab application.
use super::*;

impl Document {
    pub fn begin_stroke(&mut self) {
        self.end_edit();
        if !self.stroke_active {
            self.stroke_before = Some(self.geometry.clone());
            self.stroke_active = true;
            self.stroke_deformed = false;
            self.adaptive_budget = AdaptiveBudget::default();
        }
    }
    pub fn end_stroke(&mut self) -> bool {
        let deformed = self.stroke_active && self.stroke_deformed;
        self.stroke_active = false;
        self.stroke_deformed = false;
        self.stroke_before = None;
        deformed
    }
    pub fn symmetry(&self) -> &Symmetry {
        &self.symmetry
    }
    pub fn set_symmetry(&mut self, symmetry: Symmetry) -> Result<(), String> {
        symmetry.validate()?;
        if self.symmetry != symmetry {
            self.symmetry = symmetry;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn apply_dab(&mut self, brush: &Brush, dab: Dab) -> bool {
        self.apply_dab_stenciled(brush, dab, None)
    }
    pub(crate) fn apply_dab_stenciled(
        &mut self,
        brush: &Brush,
        dab: Dab,
        stencil: Option<&crate::sculpt::alpha::Projection>,
    ) -> bool {
        if !self.active_visible() || self.primitive().is_some() {
            return false;
        }
        if stencil.is_some() && matches!(brush.kind, Kind::Grab | Kind::Snake | Kind::Detail) {
            return false;
        }
        self.begin_stroke();
        let dabs = self.symmetry.dabs(dab);
        let disabled = AdaptiveSettings::default();
        let settings = if stencil.is_some() {
            &disabled
        } else {
            &self.geometry.settings.adaptive
        };
        let mut before_topology =
            self.geometry
                .mesh
                .adapt(brush, &dabs, settings, &mut self.adaptive_budget, true);
        before_topology |= self
            .geometry
            .mesh
            .prepare_drag_stamp(brush, &dabs, settings);
        let old_mask = (brush.kind == Kind::Mask).then(|| self.geometry.mesh.mask.clone());
        let stencils = stencil.map_or_else(Vec::new, |s| s.copies(&self.symmetry));
        let deformed = self
            .geometry
            .mesh
            .stamp_many_stenciled(brush, &dabs, &stencils);
        let mut after_dabs = dabs.clone();
        if brush.kind == Kind::Snake {
            for dab in &mut after_dabs {
                dab.center = dab.center + brush.snake_motion(*dab);
            }
        }
        let after_topology = self.geometry.mesh.adapt(
            brush,
            &after_dabs,
            settings,
            &mut self.adaptive_budget,
            false,
        );
        self.stroke_deformed |= deformed || before_topology || after_topology;
        if deformed
            || before_topology
            || after_topology
            || old_mask.is_some_and(|mask| mask != self.geometry.mesh.mask)
        {
            if let Some(before) = self.stroke_before.take() {
                let mut captured = self.capture_edit_settings();
                captured.active_brush = Some(brush.clone());
                captured.first_dab = Some(serde_json::json!({
                    "pressure": dab.pressure, "invert": dab.invert,
                }));
                self.history.record_owned(
                    before,
                    if settings.enabled {
                        "Adaptive brush stroke"
                    } else if brush.kind == Kind::Snake {
                        "Snake stroke"
                    } else {
                        match brush.effective_stroke_mode() {
                            crate::sculpt::StrokeMode::Freehand => "Brush stroke",
                            crate::sculpt::StrokeMode::Line => "Line stroke",
                            crate::sculpt::StrokeMode::DragRect => "Drag Rect stamp",
                            crate::sculpt::StrokeMode::DragDot => "Drag Dot stamp",
                        }
                    },
                    Some(captured),
                );
            }
            self.dirty = true;
            self.display_changed();
        }
        deformed
    }
}

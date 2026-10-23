use super::*;
use crate::preferences::MaskShape;

impl UiState {
    pub(super) fn temporary_mask_controls(&mut self, ui: &mut egui::Ui, document: &Document, commands: &mut Vec<Command>) {
        ui.collapsing("Hold to mask", |ui| {
            ui.label(format!("Hold binding: {}", self.preferences.mask_hold.label()));
            if ui.button("Configure hold and menu keys…").clicked() {
                self.settings_open = true;
            }
            ui.weak("Hold over a model in Sculpt or Modeling. Release to restore your tool and component selection.");
            for (shape, action) in [(MaskShape::Brush, Action::TemporaryMaskBrush), (MaskShape::Lasso, Action::TemporaryMaskLasso), (MaskShape::Line, Action::TemporaryMaskLine)] {
                if ui.selectable_label(self.preferences.mask_preset.shape == shape, shape.label()).customize(action.clone()).clicked() {
                    commands.push(Command::Action(action));
                }
            }
            ui.horizontal(|ui| {
                for (erase, action, label) in [(false, Action::TemporaryMaskAdd, "Add"), (true, Action::TemporaryMaskErase, "Erase")] {
                    if ui.selectable_label(self.preferences.mask_preset.erase == erase, label).customize(action.clone()).clicked() {
                        commands.push(Command::Action(action));
                    }
                }
            });
            if self.preferences.mask_preset.shape == MaskShape::Line && ui.selectable_label(self.preferences.mask_preset.flip, "Flip side").customize(Action::TemporaryMaskFlip).clicked() {
                commands.push(Command::Action(Action::TemporaryMaskFlip));
            }
            let selected = self.preferences.mask_preset.brush_index(document);
            egui::ComboBox::from_label("Mask brush preset").selected_text(selected.map_or("Default Mask", |i| document.brushes()[i].name.as_str())).show_ui(ui, |ui| {
                for (i, brush) in document.brushes().iter().enumerate().filter(|(_,b)| b.kind == Kind::Mask) {
                    if ui.selectable_label(selected == Some(i), &brush.name).clicked() {
                        commands.push(Command::Action(Action::TemporaryMaskPreset(brush.identity())));
                    }
                }
            });
            ui.weak("Edit the selected Mask preset in Brush settings to change pressure, strength, alpha and stroke behavior. This preset is independent of your regular sculpt brush.");
        });
    }

    pub(super) fn mask_binding_conflicts(&self) -> Vec<String> {
        let hold = &self.preferences.mask_hold;
        if !hold.enabled() { return Vec::new(); }
        let mut conflicts = Vec::new();
        let visibility = &self.preferences.visibility_hold;
        if visibility.enabled() {
            conflicts.push(format!("If {} and {} are both held, visibility takes priority.", hold.label(), visibility.label()));
        }
        for (label, modifier) in [("Smooth", self.preferences.smooth), ("Reverse", self.preferences.reverse), ("Zoom", self.preferences.zoom_hold), ("Pan", self.preferences.pan_hold)] {
            let overlaps = match modifier { Hold::Ctrl => hold.ctrl, Hold::Shift => hold.shift, Hold::Alt => hold.alt, Hold::Disabled => false };
            if overlaps { conflicts.push(format!("{} overlaps {label}; masking takes priority while held.", hold.label())); }
        }
        if hold.ctrl || hold.command { conflicts.push("Masking takes priority over Modeling selection removal while held.".into()); }
        for (action, binding) in &self.preferences.bindings {
            if let Some(binding) = binding
                && hold.key.as_ref() == Some(&binding.key)
                && (!hold.ctrl || binding.ctrl)
                && (!hold.shift || binding.shift)
                && (!hold.alt || binding.alt)
                && (!hold.command || binding.command)
            {
                conflicts.push(format!("{} also invokes {}; that shortcut is suppressed while masking. Use different hold and menu keys.", binding.label(), action.label()));
            }
        }
        conflicts
    }
}

impl UiState {
    pub(super) fn temporary_mask_menu_entries(&self, document: &Document) -> Vec<super::context_menu::ViewportMenuEntry> {
        use super::context_menu::ViewportMenuEntry;
        let preset = &self.preferences.mask_preset;
        let enabled = document.primitive().is_none() && document.active_visible();
        let mut entries = [
            (Action::TemporaryMaskBrush, "Surface brush", preset.shape == MaskShape::Brush, "Paint the visible surface using the remembered Mask brush preset."),
            (Action::TemporaryMaskLasso, "Through-depth lasso", preset.shape == MaskShape::Lasso, "Draw a closed outline to mask vertices at every depth; hidden geometry stays protected."),
            (Action::TemporaryMaskLine, "Through-depth line", preset.shape == MaskShape::Line, "Draw a dividing line to mask one side through every depth."),
            (Action::TemporaryMaskAdd, "Add mask", !preset.erase, "Add protection with the temporary mask tool."),
            (Action::TemporaryMaskErase, "Erase mask", preset.erase, "Erase protection with the temporary mask tool."),
        ].into_iter().map(|(action,label,selected,hint)| ViewportMenuEntry { action, label: label.into(), selected, enabled, hint: hint.into() }).collect::<Vec<_>>();
        if preset.shape == MaskShape::Line {
            entries.push(ViewportMenuEntry { action: Action::TemporaryMaskFlip, label: "Flip side".into(), selected: preset.flip, enabled, hint: "Mask the opposite side of the dividing line.".into() });
        }
        let index = preset.brush_index(document);
        for (i, brush) in document.brushes().iter().enumerate().filter(|(_,b)| b.kind == Kind::Mask) {
            entries.push(ViewportMenuEntry { action: Action::TemporaryMaskPreset(brush.identity()), label: format!("Brush preset: {}", brush.name), selected: index == Some(i), enabled, hint: "Remembers this Mask preset independently of your ordinary sculpt brush.".into() });
        }
        entries
    }
}

//! One-shot symmetry repair, independent of brush symmetry.
use super::*;

impl UiState {
    pub(super) fn mirror_unavailable(&self, document: &Document) -> Option<&'static str> {
        if self.pending.is_some() || self.recovery_open || self.settings_open || self.layout_open {
            Some("Close the dialog before mirroring.")
        } else if document.primitive().is_some() {
            Some("Convert the primitive to a sculpting mesh first.")
        } else if !document.active_visible() || document.mesh().has_hidden_faces() {
            Some("Show the active model and all of its faces first.")
        } else if document.mesh().triangles.is_empty() {
            Some("The active model has no surface to mirror.")
        } else if document
            .model(document.active_model_index())
            .unwrap()
            .metadata
            .temporary_thickness
            > 0.
        {
            Some("Apply or remove thickness modifier first.")
        } else {
            None
        }
    }

    pub(super) fn mirror_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        ui.strong("Mirror one side");
        ui.horizontal(|ui| {
            ui.label("Axis");
            for axis in Axis::ALL {
                ui.selectable_value(&mut self.mirror_axis, axis, format!("{axis:?}"));
            }
        });
        let axis = self.mirror_axis;
        let offset = document.symmetry().center.dot(axis.vector());
        ui.label(format!("Mirror plane: {axis:?} = {offset:.3}"));
        ui.label("Replaces the opposite half of the active model, including masked areas. Keeps this mesh level only. Undo restores it.");
        ui.horizontal(|ui| {
            for positive in [true, false] {
                let action = Action::Mirror(axis, positive);
                let (from, to) = if positive { ('+', '-') } else { ('-', '+') };
                if ui.add_enabled(self.action_enabled(&action, document), egui::Button::new(format!("{from}{axis:?} to {to}{axis:?}")))
                    .on_hover_text(format!("Keep the {from}{axis:?} side and replace the {to}{axis:?} side. Uses the symmetry center below; brush symmetry can be off."))
                    .customize(action.clone())
                    .clicked()
                {
                    commands.push(Command::Action(action));
                }
            }
        });
        if let Some(reason) = self.mirror_unavailable(document) {
            ui.weak(reason);
        }
    }
}

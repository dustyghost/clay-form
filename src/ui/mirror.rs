//! One-shot symmetry repair, independent of brush symmetry.
use super::*;

impl UiState {
    pub(crate) fn mirror_unavailable(&self, document: &Document) -> Option<&'static str> {
        crate::app::policy::EditorPolicy::new(self, document).mirror_unavailable()
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
        ui.add_enabled(
            document.mesh().quad_mirror_available(),
            egui::Checkbox::new(
                &mut self.mirror_quad_bridge,
                "Bridge with quads (experimental)",
            ),
        )
        .on_hover_text(
            "Join the two sides with quads on a closed all-quad mesh without UVs or creases.",
        );
        if self.mirror_quad_bridge && document.mesh().quad_mirror_available() {
            ui.checkbox(&mut self.mirror_centreline, "Add centreline");
        }
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

use super::*;
use crate::sculpt::BorderTarget;
pub(crate) struct EdgeControls {
    pub target: BorderTarget,
    pub selected: Vec<[usize; 2]>,
    pub selecting: bool,
    pub loops: u32,
    pub width: f32,
    pub relax: f32,
    pub smoothness: f32,
}
impl Default for EdgeControls {
    fn default() -> Self {
        Self {
            target: BorderTarget::Holes,
            selected: Vec::new(),
            selecting: false,
            loops: 1,
            width: 0.15,
            relax: 0.5,
            smoothness: 1.,
        }
    }
}
impl UiState {
    pub(crate) fn edge_unavailable(
        &self,
        action: &Action,
        document: &Document,
    ) -> Option<&'static str> {
        crate::app::policy::EditorPolicy::new(self, document).edge_unavailable(action)
    }

    pub(super) fn edge_help(
        &self,
        response: egui::Response,
        action: &Action,
        document: &Document,
    ) -> egui::Response {
        let Some(description) = action.help() else {
            return response;
        };
        let mut text = match self.edge_unavailable(action, document) {
            Some(reason) => format!("{reason}\n\n{description}"),
            None => description.to_owned(),
        };
        if matches!(
            action,
            Action::CreaseEdges
                | Action::UncreaseEdges
                | Action::AddSupportLoops
                | Action::RelaxBorders
                | Action::SelectBorderEdges
        ) {
            text.push_str(&format!("\nAffected edges: {}.", self.edges.target.label()));
        }
        if let Some(shortcut) = self.preferences.shortcut(action) {
            text.push_str(&format!("\nShortcut: {}", shortcut.label()));
        }
        design::help(response, text)
    }

    fn edge_action(
        &self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
        action: Action,
    ) {
        let button = if action == Action::ClearCreases {
            design::button(ui, Glyph::Delete, action.label())
        } else {
            egui::Button::new(action.label()).min_size(Vec2::new(0., 28.))
        };
        let response = ui.add_enabled(self.action_enabled(&action, document), button);
        if self
            .edge_help(response, &action, document)
            .customize(action.clone())
            .clicked()
        {
            commands.push(Command::Action(action));
        }
    }

    pub(super) fn smooth_subdivision_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        // Leave room for the numeric value and label in narrow docked panels.
        ui.spacing_mut().slider_width = (ui.available_width() * 0.3).clamp(48., 100.);
        design::help(
            ui.add(egui::Slider::new(&mut self.edges.smoothness, 0. ..=1.).text("Smoothness")),
            "0 refines without rounding; 1 fully smooths uncreased areas. Used when you click Smooth subdivide; this slider does not change the mesh by itself.",
        );
        self.edge_action(ui, document, commands, Action::SmoothSubdivide);
        ui.weak("Replaces stored levels. Undo restores them.");
    }

    pub(super) fn edge_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if self.edges.selecting {
            ui.label("Edge selection active · Esc to exit");
        }
        ui.collapsing("Edges and creases", |ui| {
            ui.strong("Selection");
            let mut selecting = self.edges.selecting;
            let response = ui.add_enabled(self.action_enabled(&Action::SelectEdges, document),
                egui::Checkbox::new(&mut selecting, "Edge selection"));
            if self.edge_help(response, &Action::SelectEdges, document).customize(Action::SelectEdges).changed() {
                commands.push(Command::Action(Action::SelectEdges));
            }
            ui.label(format!("{} selected · {} creased", self.edges.selected.len(), document.mesh().creases.len()));
            ui.horizontal_wrapped(|ui| {
                self.edge_action(ui, document, commands, Action::SelectCreases);
                self.edge_action(ui, document, commands, Action::ExpandEdgeLoop);
            });
            self.edge_action(ui, document, commands, Action::ClearEdgeSelection);
            ui.weak("Cyan: selected · Orange: creased");
            ui.separator();
            ui.strong("Affected edges");
            egui::ComboBox::from_id_salt("edge_target").width(ui.available_width())
                .selected_text(self.edges.target.label()).show_ui(ui, |ui| {
                    for target in BorderTarget::ALL {
                        let help = match target {
                            BorderTarget::Holes => "Open mesh boundaries, including outer outlines and holes.",
                            BorderTarget::Masked => "Borders of complete masked polygons, using the submodel's selection threshold.",
                            BorderTarget::VisibleGroups => "Boundaries between visible polygroups, isolated visible regions, and open borders.",
                            BorderTarget::Selected => "Only the edges currently highlighted in cyan.",
                        };
                        design::help(ui.selectable_value(&mut self.edges.target, target, target.label()), help);
                    }
                });
            self.edge_action(ui, document, commands, Action::SelectBorderEdges);
            ui.weak("Edits replace stored levels. Undo restores them.");
            ui.separator();
            ui.strong("Creases");
            ui.horizontal_wrapped(|ui| {
                self.edge_action(ui, document, commands, Action::CreaseEdges);
                self.edge_action(ui, document, commands, Action::UncreaseEdges);
            });
            ui.add_space(4.);
            self.edge_action(ui, document, commands, Action::ClearCreases);
            ui.separator();
            ui.collapsing("Support loops", |ui| {
                design::help(ui.add(egui::Slider::new(&mut self.edges.loops, 1..=8).text("Loop count")),
                    "Number of support loops to insert around each affected border when you click Add support loops.");
                design::help(ui.add(egui::Slider::new(&mut self.edges.width, 0.01..=0.45).text("Inset width")),
                    "Total inset as a fraction of nearby border-edge length. Reduce it for narrow or irregular regions.");
                self.edge_action(ui, document, commands, Action::AddSupportLoops);
            });
            ui.collapsing("Border cleanup", |ui| {
                design::help(ui.add(egui::Slider::new(&mut self.edges.relax, 0. ..=1.).text("Relax amount")),
                    "How far border vertices move when you click Relax borders. 0 leaves them unchanged; 1 applies full relaxation.");
                self.edge_action(ui, document, commands, Action::RelaxBorders);
            });
        });
    }
}

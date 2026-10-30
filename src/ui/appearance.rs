//! Independent appearance controls share the paint preset's inputs, never a second material draft.
use super::*;

impl UiState {
    pub(crate) fn paint_input_index(&self, document: &Document) -> Option<usize> {
        let brushes = document.brushes();
        if brushes
            .get(self.selected)
            .is_some_and(|b| b.kind == Kind::Paint)
        {
            return Some(self.selected);
        }
        self.appearance_brush
            .as_ref()
            .and_then(|id| {
                brushes
                    .iter()
                    .position(|b| b.kind == Kind::Paint && b.identity() == *id)
            })
            .or_else(|| brushes.iter().position(|b| b.kind == Kind::Paint))
    }

    pub(super) fn appearance_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
        colours: bool,
    ) {
        if self.mesh_busy {
            ui.disable();
        }
        let Some(mut index) = self.paint_input_index(document) else {
            ui.label("Create a Paint preset to store appearance inputs.");
            if ui.button("Create Paint preset").clicked() {
                commands.push(Command::AddBrush(Kind::Paint));
            }
            return;
        };
        egui::ComboBox::from_label("Paint input preset")
            .selected_text(&document.brushes()[index].name)
            .show_ui(ui, |ui| {
                for (i, brush) in document
                    .brushes()
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.kind == Kind::Paint)
                {
                    if ui.selectable_value(&mut index, i, &brush.name).changed()
                        && document
                            .brushes()
                            .get(self.selected)
                            .is_some_and(|b| b.kind == Kind::Paint)
                    {
                        self.select_brush(i, document);
                    }
                }
            });
        self.appearance_brush = Some(document.brushes()[index].identity());
        ui.weak("Inputs are saved in this preset. Switching Paint presets uses that preset’s own material and colour.");
        let mut brush = document.brushes()[index].clone();
        if colours {
            ui.strong("Colour for future strokes");
            ui.weak("Choosing a colour does not recolour the model or edit a shared material.");
            self.materials
                .colour_input(ui, &mut brush, &mut self.preferences.materials);
        } else {
            self.materials.show(
                ui,
                &mut brush,
                &mut self.preferences.materials,
                document,
                commands,
            );
        }
        if brush != document.brushes()[index] {
            commands.push(Command::UpdateBrush(index, brush));
        }
        if ui.button("Paint with these inputs").clicked() {
            commands.push(Command::Texture(
                crate::app::textures::TextureCommand::Paint(document.texture_paint),
            ));
        }
    }
}

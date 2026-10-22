use super::*;
impl UiState {
    pub(super) fn reference_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        ui.label("Drop PNG/JPEG images onto the model view, or enter a path:");
        ui.add(
            egui::TextEdit::singleline(&mut self.reference_path)
                .hint_text("/path/to/reference.png"),
        );
        if ui
            .add_enabled(
                self.action_enabled(&Action::AddReference, document),
                design::button(ui, Glyph::Plus, "Add image"),
            )
            .customize(Action::AddReference)
            .clicked()
        {
            commands.push(Command::Action(Action::AddReference));
            ui.close_kind(egui::UiKind::Menu);
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !document.references().is_empty(),
                    egui::Button::new(if self.arrange_references {
                        "Done arranging"
                    } else {
                        "Arrange images"
                    })
                    .selected(self.arrange_references),
                )
                .customize(Action::ArrangeReferences)
                .clicked()
            {
                commands.push(Command::Action(Action::ArrangeReferences));
                ui.close_kind(egui::UiKind::Menu);
            }
            if design::checkbox(ui, &mut self.references_visible, "Show references")
                .customize(Action::ShowReferences)
                .changed()
                && !self.references_visible
            {
                commands.push(Command::ArrangeReferences(false));
            }
        });
        ui.small("Images stay fixed to the screen. Sculpt normally unless arranging.");
        if document.references().is_empty() {
            return;
        }
        self.selected_reference = self.selected_reference.min(document.references().len() - 1);
        egui::ScrollArea::vertical()
            .id_salt("reference_list")
            .max_height(140.)
            .show(ui, |ui| {
                for (index, image) in document.references().iter().enumerate() {
                    ui.horizontal(|ui| {
                        let mut placement = image.placement;
                        design::checkbox(ui, &mut placement.visible, "")
                            .on_hover_text("Show image");
                        ui.selectable_value(
                            &mut self.selected_reference,
                            index,
                            format!(
                                "{} · {}{}",
                                index + 1,
                                image.name,
                                if placement.locked { " (locked)" } else { "" }
                            ),
                        );
                        if placement != image.placement {
                            commands.push(Command::PlaceReference(index, placement));
                        }
                    });
                }
            });
        let index = self.selected_reference;
        let image = &document.references()[index];
        ui.separator();
        ui.label(&image.name);
        let mut placement = image.placement;
        design::checkbox(ui, &mut placement.locked, "Lock position and size");
        ui.add(
            egui::Slider::new(&mut placement.opacity, 0.05..=1.)
                .text("Opacity")
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.)),
        );
        ui.add_enabled_ui(!placement.locked, |ui| {
            ui.add(
                egui::Slider::new(&mut placement.width, 0.05..=0.9)
                    .text("Size")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.)),
            );
            if ui.button("Center image").clicked() {
                placement.center = [0.5, 0.5];
                placement.visible = true;
            }
        });
        if placement != image.placement {
            commands.push(Command::PlaceReference(index, placement));
        }
        if ui
            .add(design::button(ui, Glyph::Delete, "Remove image"))
            .clicked()
        {
            commands.push(Command::RemoveReference(index));
        }
        ui.small("Saved inside the project. Reference edits are separate from sculpt Undo/Redo.");
    }
}

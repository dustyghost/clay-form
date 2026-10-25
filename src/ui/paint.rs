use super::*;
use crate::sculpt::Brush;
impl UiState {
    pub(super) fn paint_controls(
        &mut self,
        ui: &mut egui::Ui,
        brush: &mut Brush,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        use crate::{app::textures::TextureCommand as T, sculpt::PaintMode};
        ui.label("Paint target");
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(document.texture_paint.is_none(), "Vertices")
                .clicked()
            {
                commands.push(Command::Texture(T::Paint(None)));
            }
            for texture in &document.mesh().uv.textures {
                if ui
                    .selectable_label(
                        document.texture_paint == Some(texture.channel),
                        format!("{} texture", texture.channel.label()),
                    )
                    .clicked()
                {
                    commands.push(Command::Texture(T::Paint(Some(texture.channel))));
                }
            }
            if ui.button("Set up texture painting…").clicked() {
                self.uv.open = true;
                self.uv.texture_tab = document.mesh().uv.current(document.mesh());
            }
        });
        ui.weak(
            "Paint directly on the 3D model. The target chooses image pixels or vertex materials.",
        );
        ui.horizontal(|ui| {
            use crate::sculpt::texture::Channel;
            let mut material = brush.material.clone();
            let changed = match document.texture_paint {
                Some(Channel::Roughness) => ui
                    .add(
                        egui::Slider::new(&mut material.roughness, 0. ..=1.)
                            .text("Paint roughness"),
                    )
                    .changed(),
                Some(Channel::Metallic) => ui
                    .add(egui::Slider::new(&mut material.metallic, 0. ..=1.).text("Paint metallic"))
                    .changed(),
                _ => {
                    ui.label("Paint colour");
                    ui.color_edit_button_rgb(&mut material.base_color)
                        .on_hover_text(
                            "Change this brush's colour without editing a shared material",
                        )
                        .changed()
                }
            };
            if changed {
                brush.material = material.unlinked();
            }
        });
        let label = |mode| match mode {
            PaintMode::Stroke => "Regular stroke",
            PaintMode::Airbrush => "Airbrush",
            PaintMode::Spray => "Spray dots",
        };
        egui::ComboBox::from_label("Paint mode")
            .selected_text(label(brush.paint_effect.mode))
            .show_ui(ui, |ui| {
                for mode in [PaintMode::Stroke, PaintMode::Airbrush, PaintMode::Spray] {
                    ui.selectable_value(&mut brush.paint_effect.mode, mode, label(mode));
                }
            });
        if brush.paint_effect.mode != PaintMode::Stroke {
            ui.add(egui::Slider::new(&mut brush.paint_effect.flow, 0.01..=1.).text("Flow"));
            if brush.paint_effect.mode == PaintMode::Spray {
                ui.add(
                    egui::Slider::new(&mut brush.paint_effect.density, 0.02..=1.)
                        .text("Dot density"),
                );
                ui.add(
                    egui::Slider::new(&mut brush.paint_effect.dot_size, 0.005..=0.2)
                        .text("Dot size / radius"),
                );
            }
            ui.weak("Freehand: hold the pen down to build up colour. Strength and pressure control deposition; radius sets the footprint.");
        }
        ui.separator();
    }
}

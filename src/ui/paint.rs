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
        use crate::{
            app::textures::TextureCommand as T,
            sculpt::{
                PaintChannels, PaintMode, StrokeMode,
                texture::{Channel, Precision},
            },
            viewport::InspectionMode,
        };
        let paint_target = document
            .texture_paint
            .filter(|channel| *channel == Channel::Color);
        if paint_target != document.texture_paint {
            commands.push(Command::Texture(T::Paint(None)));
        }
        if matches!(
            self.preferences.paint_inspection,
            InspectionMode::Roughness | InspectionMode::Metallic
        ) {
            self.preferences.paint_inspection = InspectionMode::Lit;
        }
        let mesh = document.mesh();
        let texture = paint_target.and_then(|channel| {
            mesh.uv
                .textures
                .iter()
                .find(|texture| texture.channel == channel)
        });
        ui.strong("Paint");
        if self.preferences.paint_workspace_backup.is_none()
            && ui.button("Focus Paint workspace").on_hover_text("Keep one paint sidebar and give the model more screen space. Restore your layout from the top toolbar.").clicked()
        {
            self.preferences.focus_paint_workspace();
        }

        egui::ComboBox::from_label("Model")
            .selected_text(document.model(document.active_model_index()).unwrap().name)
            .show_ui(ui, |ui| {
                for index in 0..document.model_count() {
                    if ui
                        .selectable_label(
                            index == document.active_model_index(),
                            document.model(index).unwrap().name,
                        )
                        .clicked()
                    {
                        commands.push(Command::SelectSubmodel(index));
                    }
                }
            });
        egui::ComboBox::from_label("Target")
            .selected_text(texture.map_or("Vertices".into(), |t| {
                format!("{} image", t.channel.label())
            }))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(paint_target.is_none(), "Vertices")
                    .clicked()
                {
                    commands.push(Command::Texture(T::Paint(None)));
                }
                for texture in mesh
                    .uv
                    .textures
                    .iter()
                    .filter(|texture| texture.channel == Channel::Color)
                {
                    if ui
                        .selectable_label(
                            paint_target == Some(texture.channel),
                            format!("{} image", texture.channel.label()),
                        )
                        .clicked()
                    {
                        commands.push(Command::Texture(T::Paint(Some(texture.channel))));
                    }
                }
            });
        if let Some(texture) = texture {
            let precision = match texture.pixels.precision() {
                Precision::U16 => "16-bit integer",
                Precision::Float32 => "32-bit float",
            };
            ui.weak(format!(
                "{} × {} · {precision}",
                texture.width, texture.height
            ));
            let users = (0..document.model_count())
                .filter(|index| {
                    document
                        .model(*index)
                        .unwrap()
                        .mesh
                        .uv
                        .textures
                        .iter()
                        .any(|image| image.id == texture.id)
                })
                .count();
            if users > 1 {
                ui.weak(format!("Shared image · strokes update {users} models"));
            }
        } else {
            ui.weak("Vertex materials · saved with this model");
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Set up texture painting…").clicked() {
                self.uv.open = true;
                self.uv.texture_tab = mesh.uv.current(mesh);
            }
            if ui
                .button("UV / image view")
                .on_hover_text("Open the linked UV and image workspace alongside the model")
                .clicked()
            {
                self.uv.open = true;
                self.uv.texture_tab = !mesh.uv.textures.is_empty();
            }
        });
        let inspection_label = |mode| match mode {
            InspectionMode::Lit => "Lit material",
            InspectionMode::Color => "Unlit colour",
            InspectionMode::Roughness => "Roughness",
            InspectionMode::Metallic => "Metallic",
            InspectionMode::UvChecker => "UV checker",
        };
        egui::ComboBox::from_label("Inspect")
            .selected_text(inspection_label(self.preferences.paint_inspection))
            .show_ui(ui, |ui| {
                for mode in [
                    InspectionMode::Lit,
                    InspectionMode::Color,
                    InspectionMode::UvChecker,
                ] {
                    ui.selectable_value(
                        &mut self.preferences.paint_inspection,
                        mode,
                        inspection_label(mode),
                    );
                }
            });
        if paint_target.is_none() {
            egui::ComboBox::from_label("Paint channels")
                .selected_text(brush.paint_channels.label())
                .show_ui(ui, |ui| {
                    for channel in PaintChannels::ALL {
                        ui.selectable_value(&mut brush.paint_channels, channel, channel.label());
                    }
                });
        }
        let colour = matches!(paint_target, Some(Channel::Color))
            || (paint_target.is_none()
                && matches!(
                    brush.paint_channels,
                    PaintChannels::Material | PaintChannels::Color
                ));
        let roughness = matches!(paint_target, Some(Channel::Roughness))
            || (paint_target.is_none()
                && matches!(
                    brush.paint_channels,
                    PaintChannels::Material | PaintChannels::Roughness
                ));
        let metallic = matches!(paint_target, Some(Channel::Metallic))
            || (paint_target.is_none()
                && matches!(
                    brush.paint_channels,
                    PaintChannels::Material | PaintChannels::Metallic
                ));
        let mut material = brush.material.clone();
        let mut changed = false;
        if colour {
            ui.horizontal(|ui| {
                ui.label("Brush colour");
                changed |= ui.color_edit_button_rgb(&mut material.base_color)
                    .on_hover_text("Set colour for future strokes. Existing paint and shared materials stay unchanged.").changed();
            });
        }
        if roughness {
            changed |= ui
                .add(egui::Slider::new(&mut material.roughness, 0. ..=1.).text("Brush roughness"))
                .changed();
        }
        if metallic {
            changed |= ui
                .add(egui::Slider::new(&mut material.metallic, 0. ..=1.).text("Brush metallic"))
                .changed();
        }
        if changed {
            brush.material = material.unlinked();
            if colour {
                brush.texture_colour = None;
            }
        }
        if texture.is_some_and(|t| {
            t.channel == Channel::Color && t.pixels.precision() == Precision::Float32
        }) {
            ui.collapsing("HDR colour values", |ui| {
                let mut values = brush.texture_colour.unwrap_or(brush.material.base_color);
                let mut edited = false;
                ui.label("Linear RGB (float)");
                ui.horizontal(|ui| {
                    for (label, value) in ["R: ", "G: ", "B: "].into_iter().zip(&mut values) {
                        edited |= ui
                            .add(
                                egui::DragValue::new(value)
                                    .prefix(label)
                                    .speed(0.01)
                                    .max_decimals(4),
                            )
                            .changed();
                    }
                });
                if edited {
                    brush.texture_colour = Some(values);
                }
            });
        }
        if colour {
            self.materials
                .brush_palette(ui, brush, &mut self.preferences.materials);
        } else {
            self.materials.sample_control(
                ui,
                if roughness {
                    "Sample roughness"
                } else {
                    "Sample metallic"
                },
            );
        }
        let label = |mode| match mode {
            PaintMode::Stroke => "Regular stroke",
            PaintMode::Airbrush => "Airbrush",
            PaintMode::Spray => "Spray dots",
        };
        egui::ComboBox::from_label("Application")
            .selected_text(label(brush.paint_effect.mode))
            .show_ui(ui, |ui| {
                for mode in [PaintMode::Stroke, PaintMode::Airbrush, PaintMode::Spray] {
                    ui.selectable_value(&mut brush.paint_effect.mode, mode, label(mode));
                }
            });
        if brush.paint_effect.mode != PaintMode::Stroke {
            let continuous = brush.effective_stroke_mode() == StrokeMode::Freehand;
            ui.add_enabled(
                continuous,
                egui::Slider::new(&mut brush.paint_effect.flow, 0.01..=1.).text("Flow / second"),
            )
            .on_hover_text("Continuous buildup while holding the pen down in Freehand");
            if !continuous {
                ui.weak("Choose the Freehand path for continuous flow.");
            }
            if brush.paint_effect.mode == PaintMode::Spray {
                ui.add(
                    egui::Slider::new(&mut brush.paint_effect.density, 0.02..=1.)
                        .text("Dot density"),
                );
                let mut percent = brush.paint_effect.dot_size * 100.;
                if ui
                    .add(
                        egui::Slider::new(&mut percent, 0.5..=20.)
                            .suffix("% of radius")
                            .text("Dot size"),
                    )
                    .changed()
                {
                    brush.paint_effect.dot_size = percent / 100.;
                }
            }
        }
        ui.separator();
    }
}

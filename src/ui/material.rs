use crate::sculpt::{Brush, Material, PaintChannels};
use eframe::egui;

pub(super) fn paint_controls(ui: &mut egui::Ui, brush: &mut Brush) {
    let examples = Material::examples();
    let label = examples
        .iter()
        .find(|(_, m)| *m == brush.material)
        .map_or("Custom", |(name, _)| *name);
    egui::ComboBox::from_label("Example material")
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (name, material) in examples {
                ui.selectable_value(&mut brush.material, material, name);
            }
        });
    egui::ComboBox::from_label("Paint channels")
        .selected_text(brush.paint_channels.label())
        .show_ui(ui, |ui| {
            for channel in PaintChannels::ALL {
                ui.selectable_value(&mut brush.paint_channels, channel, channel.label());
            }
        });
    ui.horizontal(|ui| {
        ui.label("Colour");
        let mut color = brush.material.base_color;
        if ui.color_edit_button_rgb(&mut color).changed() {
            brush.material.base_color = color;
        }
    });
    ui.add(egui::Slider::new(&mut brush.material.roughness, 0. ..=1.).text("Roughness"));
    ui.add(egui::Slider::new(&mut brush.material.metallic, 0. ..=1.).text("Metallic"));
    ui.weak("Paint onto vertices. Masks and hidden faces are protected. Subtract / reverse blends the selected channels back to default clay. Save as .clay to preserve materials.");
}

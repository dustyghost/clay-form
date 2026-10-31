use crate::{
    sculpt::paths::SurfacePath,
    viewport::paths::transform::{Operation, Pivot, State},
};
use eframe::egui;

pub(super) fn controls(
    ui: &mut egui::Ui,
    path: &mut SurfacePath,
    state: &mut State,
    notice: &mut Option<String>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.operation, Operation::Move, "Move path");
        ui.selectable_value(&mut state.operation, Operation::Rotate, "Rotate path");
        ui.selectable_value(&mut state.operation, Operation::Scale, "Scale path");
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Pivot");
        ui.selectable_value(&mut state.pivot, Pivot::Centre, "Centre");
        ui.selectable_value(&mut state.pivot, Pivot::Start, "Start");
        ui.selectable_value(&mut state.pivot, Pivot::Origin, "Origin");
    });
    match state.operation {
        Operation::Move => {
            ui.horizontal_wrapped(|ui| {
                for (label, value) in [
                    ("X ", &mut state.translation.0),
                    ("Y ", &mut state.translation.1),
                    ("Z ", &mut state.translation.2),
                ] {
                    ui.add(
                        egui::DragValue::new(value)
                            .range(-1e6..=1e6)
                            .speed(0.01)
                            .prefix(label),
                    );
                }
            });
            ui.weak("Drag the centre to move in the view plane, or a coloured axis handle to constrain movement.");
        }
        Operation::Rotate => {
            ui.horizontal_wrapped(|ui| {
                ui.label("Numeric axis");
                for (index, label) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.selectable_value(&mut state.axis, index, *label);
                }
                ui.add(
                    egui::DragValue::new(&mut state.angle)
                        .range(-3600. ..=3600.)
                        .speed(1.)
                        .suffix("°"),
                );
            });
            ui.weak("Drag a coloured rotation ring, or the outer ring to rotate around the view.");
        }
        Operation::Scale => {
            ui.add(
                egui::DragValue::new(&mut state.scale)
                    .range(0.001..=1000.)
                    .speed(0.01)
                    .prefix("Uniform scale ")
                    .suffix("×"),
            );
            ui.checkbox(&mut state.scale_sections, "Scale linked sections")
                .on_hover_text("Also scales section size, fixed repeat spacing, join distance and per-copy offsets when the path is saved. Reusable source meshes stay unchanged.");
            ui.weak("Drag the square handle to scale the entire path uniformly around the pivot.");
        }
    }
    if ui.button("Apply transform to draft").clicked()
        && let Err(error) = state.apply_numeric(path)
    {
        *notice = Some(error);
    }
}

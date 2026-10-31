use super::*;
use crate::sculpt::capacity::{self, MeshCapacity};

impl UiState {
    pub(super) fn mesh_capacity_settings(&mut self, ctx: &egui::Context) {
        let mut open = self.mesh_capacity_open;
        egui::Window::new("Mesh capacity")
            .open(&mut open)
            .default_width(420.)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Maximum geometry per mesh. Quads count as two triangles.");
                ui.label("Higher limits allow larger meshes and use more memory.");
                ui.separator();
                let limits = &mut self.preferences.mesh_capacity;
                egui::Grid::new("mesh_capacity_limits").show(ui, |ui| {
                    let vertices_label = ui.label("Maximum vertices");
                    ui.add(
                        egui::DragValue::new(&mut limits.vertices)
                            .range(MeshCapacity::DEFAULT.vertices..=MeshCapacity::MAX.vertices)
                            .speed(100_000),
                    )
                    .labelled_by(vertices_label.id);
                    ui.end_row();
                    let triangles_label = ui.label("Maximum triangles");
                    ui.add(
                        egui::DragValue::new(&mut limits.triangles)
                            .range(MeshCapacity::DEFAULT.triangles..=MeshCapacity::MAX.triangles)
                            .speed(100_000),
                    )
                    .labelled_by(triangles_label.id);
                    ui.end_row();
                });
                if ui
                    .add(design::button(
                        ui,
                        design::Glyph::Restore,
                        "Reset mesh limits",
                    ))
                    .clicked()
                {
                    *limits = MeshCapacity::default();
                }
                ui.separator();
                let active = capacity::current();
                ui.label(format!(
                    "Active: {} vertices · {} triangles",
                    active.vertices, active.triangles
                ));
                ui.label("Changes save automatically. Restart Clay Form to apply.");
                if *limits != active {
                    ui.strong("Restart required to apply these limits.");
                }
            });
        self.mesh_capacity_open = open;
    }
}

#[cfg(test)]
mod tests;

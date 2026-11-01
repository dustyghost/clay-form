//! Session-only controls for the guided layout preview.
use super::{projection::ProjectionControls, *};
use crate::sculpt::paths::PathMode;

impl ProjectionControls {
    fn layout_options(&mut self, ui: &mut egui::Ui, document: &Document) {
        ui.horizontal(|ui| {
            ui.label("Target polygons");
            let range = 1..=document.mesh().triangles.len().div_ceil(2);
            ui.add(egui::DragValue::new(&mut self.layout.target).range(range));
        });
        ui.weak("Boundaries, creases and face groups can limit reduction.");
        ui.checkbox(
            &mut self.layout.preserve_sharp,
            "Protect bends of 45° or more",
        );

        ui.separator();
        ui.label("Edge direction");
        let selected = self.flow_path.and_then(|id| document.path(id));
        egui::ComboBox::from_id_salt("retopology_path")
            .selected_text(selected.map_or("No guide", |p| &p.name))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.flow_path, None, "No guide");
                for path in document.paths().iter().filter(|p| {
                    p.target == document.active_model_id() && p.mode == PathMode::Surface
                }) {
                    ui.selectable_value(&mut self.flow_path, Some(path.id), &path.name);
                }
            });
        ui.weak("Save a surface path in Editable paths to steer nearby quad edges.");
        ui.add_enabled_ui(self.flow_path.is_some(), |ui| {
            ui.add(egui::Slider::new(&mut self.layout.flow, 0. ..=1.).text("Path strength"));
            ui.checkbox(
                &mut self.layout.mirror_path_x,
                "Mirror path influence across X = 0",
            );
        });

        ui.separator();
        ui.label("Polygon density");
        ui.weak("Paint with the Mask brush before opening this tool. Higher mask weights retain more vertices during reduction.");
        ui.add(egui::Slider::new(&mut self.layout.density, 0. ..=1.).text("Density strength"));
    }
}

impl UiState {
    pub(super) fn retopology_window(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        let mut open = true;
        let state = &mut self.projection;
        let window = egui::Window::new("Guided retopology")
            .open(&mut open)
            .default_width(380.)
            .default_height(640.)
            .resizable(false);
        window.show(ctx, |ui| {
            let source = document.model(document.active_model_index()).unwrap();
            ui.label(format!("Source · {}", source.name));
            ui.weak("Experimental quad-dominant layout. Apply adds an editable model and keeps the source below it for detail projection.");
            if state.busy {
                ui.spinner();
            }
            if let Some(report) = &state.report {
                ui.label(report.summary());
                if state.flow_path.is_some() && state.layout.flow > 0. {
                    ui.checkbox(&mut state.hide_flow_guide, "Hide path overlay");
                }
            }
            let mut requested = None;
            ui.horizontal_wrapped(|ui| {
                let preview = design::button(ui, Glyph::Mesh, "Preview layout");
                if ui.add_enabled(!state.busy, preview).clicked() {
                    requested = Some(Command::PreviewProjection);
                }
                let apply = design::button(ui, Glyph::Plus, "Apply as new model");
                let ready = !state.busy && state.report.as_ref().is_some_and(SurfaceReport::can_apply);
                if ui.add_enabled(ready, apply).clicked() {
                    requested = Some(Command::ApplyProjection);
                }
                if ui.add(design::button(ui, Glyph::Delete, "Cancel")).clicked() {
                    requested = Some(Command::CancelProjection);
                }
            });
            ui.separator();
            let old = (state.layout, state.flow_path);
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 235.).max(50.))
                .show(ui, |ui| {
                    ui.add_enabled_ui(!state.busy, |ui| state.layout_options(ui, document));
                });
            if old != (state.layout, state.flow_path) {
                commands.push(Command::ClearProjectionPreview);
                if matches!(requested, Some(Command::ApplyProjection)) {
                    requested = None;
                }
            }
            commands.extend(requested);
        });
        if !open {
            commands.push(Command::CancelProjection);
        }
        commands
    }
}

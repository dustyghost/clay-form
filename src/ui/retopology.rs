//! Session-only controls for the guided layout preview.
use super::{projection::ProjectionControls, *};
use crate::sculpt::paths::PathMode;

impl ProjectionControls {
    fn layout_options(&mut self, ui: &mut egui::Ui, document: &Document) {
        ui.horizontal(|ui| {
            ui.label("Target quads");
            let range = 4..=50_000;
            ui.add(egui::DragValue::new(&mut self.layout.target).range(range));
        });
        ui.weak("A fresh quad layout follows the surface. Face-group borders may move.");
        ui.checkbox(
            &mut self.layout.preserve_sharp,
            "Protect bends of 45° or more",
        );
        ui.horizontal(|ui| {
            ui.label("Input triangle limit");
            design::help(
                ui.add(
                    egui::DragValue::new(&mut self.layout.input_triangle_limit)
                        .range(1..=crate::sculpt::max_triangles())
                        .speed(1_000),
                ),
                "Defaults to 200,000. Raise for larger tests, up to the active mesh capacity. Higher limits may use more time and memory. Kept for this app session.",
            );
        });

        ui.separator();
        ui.label("Edge direction");
        let paths: Vec<_> = document
            .paths()
            .iter()
            .filter(|p| p.target == document.active_model_id() && p.mode == PathMode::Surface)
            .collect();
        if paths.is_empty() {
            ui.weak("Save surface paths on this model to use as optional guides.");
        }
        for path in paths {
            let mut selected = self.flow_paths.contains(&path.id);
            if ui.checkbox(&mut selected, &path.name).changed() {
                if selected {
                    self.flow_paths.push(path.id);
                } else {
                    self.flow_paths.retain(|&id| id != path.id);
                }
            }
        }
        ui.weak("Select any number of paths. Guides steer nearby edges; closed paths do not guarantee complete edge loops.");
        ui.add_enabled_ui(!self.flow_paths.is_empty(), |ui| {
            ui.add(egui::Slider::new(&mut self.layout.flow, 0. ..=1.).text("Path strength"));
            ui.checkbox(&mut self.layout.mirror_path_x, "Mirror paths across X = 0");
        });

        ui.separator();
        ui.label("Polygon density");
        ui.add(egui::Slider::new(&mut self.layout.adaptive, 0. ..=1.).text("Shape adaptation"));
        ui.weak("Zero uses uniform spacing; higher values put more quads around bends. Paint adjusts that automatic spacing and is saved separately from the sculpt mask.");
        use crate::sculpt::retopology::DensityPaint;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.density_paint, None, "Inspect");
            ui.selectable_value(
                &mut self.density_paint,
                Some(DensityPaint::More),
                "More detail",
            );
            ui.selectable_value(
                &mut self.density_paint,
                Some(DensityPaint::Less),
                "Less detail",
            );
            ui.selectable_value(
                &mut self.density_paint,
                Some(DensityPaint::Automatic),
                "Automatic / erase",
            );
        });
        if self.density_paint.is_some() {
            ui.weak("Paint on the model: orange = more, blue = less, grey = automatic. Right-drag to orbit. Painting supports Undo.");
            ui.add(egui::Slider::new(&mut self.density_radius, 5. ..=200.).text("Brush radius"));
            ui.add(
                egui::Slider::new(&mut self.density_brush_strength, 0.05..=1.)
                    .text("Paint strength"),
            );
        }
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
            .default_height((ctx.content_rect().height() - 48.).max(300.))
            .resizable(false);
        window.show(ctx, |ui| {
            let source = document.model(document.active_model_index()).unwrap();
            ui.label(format!("Source · {}", source.name));
            ui.label(format!("{} source triangles", document.mesh().triangles.len()));
            ui.weak("Experimental automatic quad reconstruction. Apply adds an editable model and keeps the source below it for detail projection.");
            let budget = state.layout.require_input_budget(document.mesh());
            if let Err(error) = &budget {
                ui.label(error);
            }
            if state.busy {
                ui.spinner();
                ui.label(&state.progress);
            }
            if let Some(report) = &state.report {
                ui.label(report.summary());
                if !state.flow_paths.is_empty() && state.layout.flow > 0. {
                    ui.checkbox(&mut state.hide_flow_guide, "Hide path overlay");
                }
            }
            let mut requested = None;
            ui.horizontal_wrapped(|ui| {
                let preview = design::button(ui, Glyph::Mesh, "Preview layout");
                if design::help(
                    ui.add_enabled(!state.busy && budget.is_ok(), preview),
                    budget.as_ref().err().map(String::as_str).unwrap_or("Compute a private layout preview"),
                ).clicked() {
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
            let old = (state.layout, state.flow_paths.clone(), state.density_paint);
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 300.).max(50.))
                .show(ui, |ui| {
                    ui.add_enabled_ui(!state.busy, |ui| state.layout_options(ui, document));
                });
            if old != (state.layout, state.flow_paths.clone(), state.density_paint) {
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

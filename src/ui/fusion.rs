use super::*;
use crate::sculpt::fusion::{Boundary, JoinOptions};

pub(crate) struct FusionControls {
    pub open: bool,
    pub intersections: bool,
    pub collar: usize,
    pub sources: [usize; 2],
    pub loops: [Vec<Boundary>; 2],
    pub selected: [Option<usize>; 2],
    pub picking: usize,
    pub options: JoinOptions,
    pub busy: bool,
    pub ready: bool,
    pub show_patch: bool,
    pub show_removed: bool,
    pub notice: String,
}
impl Default for FusionControls {
    fn default() -> Self {
        Self {
            open: false,
            intersections: false,
            collar: 1,
            sources: [0, 0],
            loops: Default::default(),
            selected: [None, None],
            picking: 0,
            options: Default::default(),
            busy: false,
            ready: false,
            show_patch: true,
            show_removed: false,
            notice: String::new(),
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FusionKey {
    pub sources: [usize; 2],
    pub selected: [Option<usize>; 2],
    pub options: JoinOptions,
    pub intersections: bool,
    pub collar: usize,
}
impl FusionControls {
    pub(crate) fn key(&self) -> FusionKey {
        FusionKey {
            sources: self.sources,
            selected: self.selected,
            options: self.options,
            intersections: self.intersections,
            collar: self.collar,
        }
    }

    fn source_selector(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        side: usize,
        show_loops: bool,
    ) {
        ui.separator();
        ui.horizontal(|ui| {
            let label = if side == 0 {
                "First source"
            } else {
                "Second source"
            };
            ui.radio_value(&mut self.picking, side, label);
            egui::ComboBox::from_id_salt(("join_model", side))
                .selected_text(
                    document
                        .model(self.sources[side])
                        .map_or("Missing model", |m| m.name),
                )
                .show_ui(ui, |ui| {
                    for i in 0..document.model_count() {
                        let model = document.model(i).unwrap();
                        if model.metadata.visible {
                            ui.selectable_value(
                                &mut self.sources[side],
                                i,
                                format!("{} · {}", model.name, i + 1),
                            );
                        }
                    }
                });
        });
        if !self.intersections && show_loops {
            egui::ComboBox::from_id_salt(("join_loop", side))
                .selected_text(
                    self.selected[side]
                        .map_or("Choose a hole".into(), |i| format!("Hole {}", i + 1)),
                )
                .show_ui(ui, |ui| {
                    for (i, boundary) in self.loops[side].iter().enumerate() {
                        let label = format!(
                            "Hole {} · {} vertices · perimeter {:.3}",
                            i + 1,
                            boundary.vertices.len(),
                            boundary.perimeter
                        );
                        ui.selectable_value(&mut self.selected[side], Some(i), label);
                    }
                });
        }
    }
}
impl UiState {
    pub(crate) fn fusion_window(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        if !self.fusion.open {
            return commands;
        }
        let mut open = true;
        egui::Window::new("Join meshes")
            .open(&mut open)
            .order(egui::Order::Foreground)
            .default_pos(egui::pos2(16., 80.))
            .default_width(380.)
            .resizable(false)
            .show(ctx, |ui| {
                let s = &mut self.fusion;
                let old = s.key();
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 230.).max(120.))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!s.busy, |ui| {
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut s.intersections, false, "Open ends");
                                ui.selectable_value(&mut s.intersections, true, "Intersections");
                            });
                            if s.intersections {
                                ui.label("Fuse overlapping closed solids with local patches.");
                                ui.weak("Buried and crossing polygons are removed. Other source faces stay exact.");
                            } else {
                                ui.label("Connect two selected holes across a gap.");
                                ui.weak("Original surfaces stay fixed. The connector adds local faces.");
                            }
                            for side in 0..2 {
                                s.source_selector(ui, document, side, old.sources == s.sources);
                            }
                            if s.intersections {
                                ui.add(egui::Slider::new(&mut s.collar, 0..=8).text("Extra collar rings"));
                            } else {
                                ui.weak("Click a highlighted rim in the viewport to choose the active opening.");
                            }
                            ui.add(egui::Slider::new(&mut s.options.segments, 1..=64).text("Connector segments"));
                            ui.add(egui::DragValue::new(&mut s.options.twist).range(-4096..=4096).prefix("Alignment offset "));
                            ui.add(egui::Slider::new(&mut s.options.curvature, 0. ..=1.).text("Curvature"));
                        });
                        if old.sources != s.sources {
                            commands.push(Command::DiscoverFusion);
                        } else if old != s.key() {
                            commands.push(Command::ClearFusion);
                        }
                        ui.separator();
                        ui.add(egui::Label::new(&s.notice).wrap());
                        if s.ready {
                            ui.checkbox(&mut s.show_patch, "Highlight new patch");
                            if s.intersections {
                                ui.checkbox(&mut s.show_removed, "Show removed source faces");
                            }
                        }
                        if s.busy { ui.spinner(); }
                        ui.weak("Apply creates a joined model and hides both originals, retaining their subdivision levels. New connector faces are unmapped in UVs. Escape cancels.");
                    });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    let unchanged = old == s.key();
                    let can_preview = !s.busy && unchanged && (s.intersections || s.selected.iter().all(Option::is_some));
                    if ui.add_enabled(can_preview, design::button(ui, Glyph::Mesh, "Preview join")).clicked() {
                        commands.push(Command::PreviewFusion);
                    }
                    if ui.add_enabled(!s.busy && unchanged && s.ready, design::button(ui, Glyph::Mesh, "Apply join")).clicked() {
                        commands.push(Command::ApplyFusion);
                    }
                    if ui.add(design::button(ui, Glyph::Delete, "Cancel join")).clicked() {
                        commands.push(Command::CancelFusion);
                    }
                });
            });
        if !open {
            commands.push(Command::CancelFusion);
        }
        commands
    }
}

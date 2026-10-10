//! Immediate-mode UI. Widgets emit commands; they never perform IO or sculpt edits.
use crate::{
    command::{Command, PendingAction},
    document::Document,
    sculpt::{Alpha, Kind},
};
use eframe::egui::{self, Color32, Sense, Vec2};
pub(crate) struct UiState {
    pub selected: usize,
    pub path: String,
    pub preset_path: String,
    pub alpha_path: String,
    pub status: String,
    pub pending: Option<PendingAction>,
    pub pending_save_path: String,
    pub wire: bool,
    pub symmetry: bool,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            selected: 0,
            path: "sculpt.clay".into(),
            preset_path: "brush.brush.json".into(),
            alpha_path: String::new(),
            status: "Ready • Sculpt with left drag; Alt reverses; Shift smooths".into(),
            pending: None,
            pending_save_path: "sculpt.clay".into(),
            wire: false,
            symmetry: false,
        }
    }
}
impl UiState {
    pub fn toolbar(&mut self, ctx: &egui::Context, document: &Document) -> Vec<Command> {
        let mut commands = Vec::new();
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("CLAY FORM");
                if ui.button("New sphere").clicked() {
                    commands.push(Command::Request(PendingAction::New));
                }
                ui.label("Project path");
                ui.text_edit_singleline(&mut self.path);
                if ui.button("Open").clicked() {
                    commands.push(Command::Request(PendingAction::Open(self.path.clone())));
                }
                if ui.button("Save").clicked() {
                    commands.push(Command::Save);
                }
                if ui.button("Export OBJ").clicked() {
                    commands.push(Command::Export);
                }
                if document.is_dirty() {
                    ui.label("• Modified");
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(document.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    commands.push(Command::Undo);
                }
                if ui
                    .add_enabled(document.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    commands.push(Command::Redo);
                }
                ui.checkbox(&mut self.wire, "Wireframe");
                ui.checkbox(&mut self.symmetry, "Mirror X");
                if ui.button("Clear mask").clicked() {
                    commands.push(Command::ClearMask);
                }
                if ui.button("Reset view").clicked() {
                    commands.push(Command::ResetView);
                }
            });
        });
        commands
    }
    pub fn brush_panel(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        tablet_status: &str,
        pressure: f32,
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        egui::SidePanel::left("brushes")
            .default_width(260.)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.brush_controls(ui, document, tablet_status, pressure, &mut commands)
                });
            });
        commands
    }
    pub fn status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });
    }
    pub fn confirmation(&mut self, ctx: &egui::Context) -> Option<Command> {
        self.pending.as_ref()?;
        let mut command = None;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Save your current project before continuing?");
                ui.label("Save current project to:");
                ui.text_edit_singleline(&mut self.pending_save_path);
                ui.horizontal(|ui| {
                    if ui.button("Save and continue").clicked() {
                        command = Some(Command::SaveAndContinue);
                    }
                    if ui.button("Discard changes").clicked() {
                        command = Some(Command::DiscardAndContinue);
                    }
                    if ui.button("Cancel").clicked() {
                        command = Some(Command::CancelPending);
                    }
                });
                ui.label(&self.status);
            });
        command
    }
    fn brush_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        tablet_status: &str,
        pressure: f32,
        commands: &mut Vec<Command>,
    ) {
        ui.heading("Brush library");
        for (i, b) in document.brushes().iter().enumerate() {
            ui.selectable_value(&mut self.selected, i, &b.name);
        }
        if ui.button("Duplicate brush").clicked() {
            commands.push(Command::DuplicateBrush(self.selected));
        }
        ui.separator();
        let original = &document.brushes()[self.selected];
        let mut edited = original.clone();
        let b = &mut edited;
        ui.text_edit_singleline(&mut b.name);
        egui::ComboBox::from_label("Operation")
            .selected_text(format!("{:?}", b.kind))
            .show_ui(ui, |ui| {
                for k in Kind::ALL {
                    ui.selectable_value(&mut b.kind, k, format!("{k:?}"));
                }
            });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut b.subtract, false, "+ Add");
            ui.selectable_value(&mut b.subtract, true, "− Subtract");
        });
        ui.add(egui::Slider::new(&mut b.radius, 1. ..=500.).text("Radius (px)"));
        ui.add(egui::Slider::new(&mut b.strength, 0. ..=1.).text("Strength"));
        ui.add(egui::Slider::new(&mut b.spacing, 0.01..=1.).text("Spacing"));
        ui.add(egui::Slider::new(&mut b.falloff, 0.1..=8.).text("Falloff"));
        ui.add(egui::Slider::new(&mut b.rotation, 0. ..=360.).text("Alpha rotation"));
        ui.checkbox(&mut b.pressure, "Pressure → strength");
        ui.checkbox(&mut b.pressure_radius, "Pressure → radius");
        ui.add(egui::Slider::new(&mut b.pressure_curve, 0.1..=4.).text("Pressure curve"));
        ui.label("Alpha");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut b.alpha, Alpha::Soft, "Soft");
            ui.selectable_value(&mut b.alpha, Alpha::Hard, "Hard");
            ui.selectable_value(&mut b.alpha, Alpha::Square, "Square");
        });
        if matches!(b.alpha, Alpha::Image { .. }) {
            ui.label("Custom image alpha loaded");
        }
        if b != original {
            commands.push(Command::UpdateBrush(self.selected, b.clone()));
        }
        let (r, p) = ui.allocate_painter(Vec2::splat(100.), Sense::hover());
        let b = &edited;
        for y in 0..32 {
            for x in 0..32 {
                let value = (255. * b.weight(x as f32 / 15.5 - 1., 1. - y as f32 / 15.5)) as u8;
                p.rect_filled(
                    egui::Rect::from_min_size(
                        r.rect.min + Vec2::new(x as f32, y as f32) * 3.125,
                        Vec2::splat(3.125),
                    ),
                    0.,
                    Color32::from_gray(value),
                );
            }
        }
        ui.label("Image path (PNG/JPEG)");
        ui.text_edit_singleline(&mut self.alpha_path);
        if ui.button("Load alpha").clicked() {
            commands.push(Command::LoadAlpha(self.selected));
        }
        ui.separator();
        ui.label("Preset path");
        ui.text_edit_singleline(&mut self.preset_path);
        ui.horizontal(|ui| {
            if ui.button("Save preset").clicked() {
                commands.push(Command::SavePreset(self.selected));
            }
            if ui.button("Load preset").clicked() {
                commands.push(Command::LoadPreset);
            }
        });
        ui.separator();
        ui.label(tablet_status);
        ui.label(format!("Pressure: {:.0}%", pressure * 100.));
        ui.label("Left / pen: sculpt; empty space: orbit\nCorner gizmo: drag or click axis\nAlt: reverse brush\nShift: smooth\nRight drag: orbit\nMiddle drag: pan\nWheel: zoom");
    }
}

//! One set of transform controls reused by the sidebar and floating toolbar.
use super::design;
use super::{UiState, customization::ActionResponse};
use crate::preferences::BuiltinMenu;
use crate::{
    command::Command,
    preferences::Action,
    viewport::{TransformMode as Mode, TransformOperation as Operation},
};
use eframe::egui::{self, Vec2};

impl UiState {
    fn transform_operations(&self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        ui.horizontal_wrapped(|ui| {
            for operation in Operation::ALL
                .into_iter()
                .filter(|op| op.available(self.transform_mode))
            {
                if ui
                    .selectable_label(self.transform_operation == operation, operation.label())
                    .on_hover_text(operation.hint())
                    .customize(operation.into())
                    .clicked()
                {
                    commands.push(Command::TransformOperation(operation));
                }
            }
            if ui
                .button("Reset pivot")
                .on_hover_text(
                    "Return the pivot to the active model's center; preserve Action Line direction",
                )
                .customize(Action::ResetPivot)
                .clicked()
            {
                commands.push(Command::ResetTransformPivot);
            }
        });
        let mut respect = self.respect_transform_mask;
        if design::checkbox(ui, &mut respect, "Respect mask").customize(Action::RespectMask)
            .on_hover_text("Protect masked vertices; soft masks reduce the rotation angle. Turn off to transform the whole model").changed() {
            commands.push(Command::RespectTransformMask(respect));
        }
    }
    pub(crate) fn transform_toolbar(
        &mut self,
        ctx: &egui::Context,
        viewport: egui::Rect,
        enabled: bool,
    ) -> (Vec<Command>, Option<egui::Rect>) {
        if self.transform_mode == Mode::Sculpt
            || self.preferences.menu_placed(BuiltinMenu::Transform)
        {
            return (Vec::new(), None);
        }
        let mut commands = Vec::new();
        let response = egui::Area::new(egui::Id::new("viewport_transform_toolbar"))
            .order(egui::Order::Foreground)
            .fixed_pos(viewport.left_top() + Vec2::splat(12.))
            .constrain_to(viewport)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let width = if self.transform_operation == Operation::Universal {
                        200.
                    } else {
                        300.
                    };
                    ui.set_max_width((viewport.width() - 32.).clamp(160., width));
                    ui.add_enabled_ui(enabled, |ui| {
                        let compact = self.transform_operation == Operation::Universal
                            && viewport.height() < 260.;
                        if !compact {
                            self.menu_placement_controls(ui, BuiltinMenu::Transform, false);
                        }
                        if self.transform_operation == Operation::Universal {
                            ui.horizontal_wrapped(|ui| {
                                // Keep the compact toolbar clear of the centre pivot
                                // when the persistent appearance strip leaves a short viewport.
                                if !compact || viewport.width() >= 500. {
                                    ui.strong("Universal")
                                        .on_hover_text(Operation::Universal.hint());
                                }
                                if ui
                                    .small_button("Reset pivot")
                                    .customize(Action::ResetPivot)
                                    .clicked()
                                {
                                    commands.push(Command::ResetTransformPivot);
                                }
                                if ui.small_button("Done").clicked() {
                                    commands.push(Command::TransformMode(Mode::Sculpt));
                                }
                            });
                            if compact {
                                return;
                            }
                            ui.weak("Drag centre diamond to move pivot");
                            let key = self
                                .preferences
                                .shortcut(&Action::UniversalTransform)
                                .map_or("Unbound".into(), |s| s.label());
                            let menu = self
                                .preferences
                                .shortcut(&Action::TransformMenu)
                                .map_or("Unbound".into(), |s| s.label());
                            ui.weak(format!("{key}: sculpt · {menu}: transform options"));
                            return;
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.strong("Transform");
                            for (mode, label) in [
                                (Mode::World, "World axes"),
                                (Mode::ActionLine, "Action Line"),
                            ] {
                                if ui
                                    .selectable_label(self.transform_mode == mode, label)
                                    .customize(mode.into())
                                    .clicked()
                                {
                                    commands.push(Command::TransformMode(mode));
                                }
                            }
                            if ui
                                .small_button("Done")
                                .on_hover_text("Return to sculpting")
                                .clicked()
                            {
                                commands.push(Command::TransformMode(Mode::Sculpt));
                            }
                        });
                        ui.separator();
                        self.transform_operations(ui, &mut commands);
                    });
                });
            });
        (commands, Some(response.response.rect))
    }
    pub(super) fn transform_controls(&mut self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        ui.add_enabled_ui(!self.mesh_busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [(Mode::Sculpt, "Sculpt"), (Mode::World, "World axes"), (Mode::ActionLine, "Action Line")] {
                    if ui.selectable_label(self.transform_mode == mode, label).customize(mode.into()).clicked() {
                        commands.push(Command::TransformMode(mode));
                    }
                }
            });
            if self.transform_mode != Mode::Sculpt {
                self.transform_operations(ui, commands);
                ui.label(self.transform_operation.hint());
                ui.label("Pivot and line placement only affect the controls. Each geometry drag is one undo step; all subdivision levels follow.");
            }
        });
    }
}

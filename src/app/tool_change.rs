//! Resolve private tool drafts before selecting a sculpt brush from any UI route.
use super::*;
use crate::command::{InstanceCommand, PathCommand};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Decision {
    Apply,
    Discard,
    KeepEditing,
}

impl App {
    pub(super) fn prepare_brush_change(&mut self, action: &Action) -> bool {
        let pending = self.paths.busy()
            || self.paths.has_preview()
            || (self.viewport.paths.active && self.viewport.paths.has_unfinished(&self.document))
            || self.ui.projection.busy
            || self.projection.draft.is_some()
            || self.ui.fusion.busy
            || self.ui.fusion.ready
            || self.document.instance_preview_active();
        if pending {
            self.ui.tool_change = Some(action.clone());
            self.ui.tool_change_error = None;
            self.viewport.wait_for_release();
            return true;
        }
        self.leave_preview_tool();
        false
    }

    fn leave_preview_tool(&mut self) {
        if self.viewport.paths.active {
            let _ = self.dispatch_path(PathCommand::Close);
        }
        if self.ui.projection.open {
            self.cancel_projection();
        }
        if self.ui.fusion.open {
            self.cancel_fusion();
        }
        self.viewport.cancel(&mut self.document);
        self.ui.slice_active = false;
        self.ui.selection_tool = None;
        self.ui.modeling.active = false;
        self.ui.edges.selecting = false;
        self.ui.arrange_references = false;
        self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
        self.sync_path_history_state();
        self.viewport.wait_for_release();
    }

    fn tool_change_can_apply(&self) -> bool {
        !self.paths.busy()
            && !self.ui.projection.busy
            && !self.ui.fusion.busy
            && (!self.document.instance_preview_active() || !self.ui.instances.invalid)
            && (!self.ui.projection.open
                || self
                    .ui
                    .projection
                    .report
                    .as_ref()
                    .is_some_and(crate::ui::SurfaceReport::can_apply))
            && (!self.ui.fusion.open || self.ui.fusion.ready)
            && !self.viewport.paths.has_suspended_draft()
            && (!self.viewport.paths.active
                || self.paths.has_preview()
                || (self
                    .viewport
                    .paths
                    .draft
                    .as_ref()
                    .is_some_and(|p| p.points.len() >= 2)
                    && !self.viewport.paths.gesture_active()))
    }

    pub(super) fn resolve_tool_change(&mut self, decision: Decision, ctx: &egui::Context) {
        if decision == Decision::Apply && !self.tool_change_can_apply() {
            return;
        }
        let Some(action) = self.ui.tool_change.take() else {
            return;
        };
        self.viewport.wait_for_release();
        if decision == Decision::KeepEditing {
            return;
        }
        let result = if decision == Decision::Apply {
            if self.paths.has_preview() {
                self.dispatch_path(PathCommand::Apply)
            } else if self.viewport.paths.active
                && self.viewport.paths.has_unfinished(&self.document)
            {
                self.dispatch_path(PathCommand::Save)
            } else if self.projection.draft.is_some() {
                self.apply_projection()
            } else if self.ui.fusion.ready {
                self.apply_fusion()
            } else if self.document.instance_preview_active() {
                self.dispatch_instances(InstanceCommand::Apply)
            } else {
                Ok(())
            }
        } else {
            if self.document.instance_preview_active() {
                let _ = self.dispatch_instances(InstanceCommand::Cancel);
            }
            Ok(())
        };
        if let Err(error) = result {
            self.ui.tool_change_error = Some(error.clone());
            self.ui.status = error;
            self.ui.tool_change = Some(action);
            return;
        }
        self.leave_preview_tool();
        self.execute_action(action, ctx);
    }

    pub(super) fn show_tool_change(&mut self, ctx: &egui::Context) {
        let Some(action) = &self.ui.tool_change else {
            return;
        };
        let target = self.ui.action_label(action, &self.document);
        let can_apply = self.tool_change_can_apply();
        let running = self.paths.busy() || self.ui.projection.busy || self.ui.fusion.busy;
        let modal = egui::Modal::new(egui::Id::new("change_tool_preview")).show(ctx, |ui| {
            ui.set_max_width(380.);
            ui.heading("Finish the current tool");
            ui.label(format!("Before switching to {target}, choose what to do with the pending edits."));
            if running {
                ui.label("A preview is running. Wait to apply it, or discard to cancel it and switch now.");
            }
            let mut decision = None;
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(can_apply, egui::Button::new("Apply and switch")).clicked() {
                    decision = Some(Decision::Apply);
                }
                if ui.button("Discard and switch").clicked() {
                    decision = Some(Decision::Discard);
                }
                if ui.button("Keep editing").clicked() {
                    decision = Some(Decision::KeepEditing);
                }
            });
            if !can_apply && !running {
                ui.weak("Keep editing to complete or correct the draft before applying it.");
            }
            if let Some(error) = &self.ui.tool_change_error { ui.label(error); }
            decision
        });
        let decision = modal
            .inner
            .or_else(|| modal.should_close().then_some(Decision::KeepEditing));
        // Escape belongs to this decision, never to the draft behind it.
        ctx.input_mut(|input| {
            input.events.retain(|event| {
                !matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        ..
                    }
                )
            })
        });
        if let Some(decision) = decision {
            self.resolve_tool_change(decision, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_settings_allow_brush_switch_navigation_and_compatible_edits() {
        for mode in 0..3 {
            let mut app = App::default();
            match mode {
                0 => app.ui.settings_open = true,
                1 => app.ui.mesh_capacity_open = true,
                _ => app.ui.layout_open = true,
            }
            let ctx = egui::Context::default();
            let before = app.document.geometry_snapshot();
            app.dispatch(
                Command::Action(Action::SelectBrush(app.document.brushes()[1].identity())),
                &ctx,
            );
            assert_eq!(app.ui.selected, 1);
            app.dispatch(Command::Action(Action::ZoomIn), &ctx);
            assert!(app.camera.zoom > 1.);
            assert_eq!(app.document.geometry_snapshot(), before);
            assert!(app.ui.action_enabled(&Action::InvertMask, &app.document));
        }
    }
    #[test]
    fn untouched_surface_dialog_closes_and_instance_preview_requires_decision() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.dispatch(Command::OpenRelaxation, &ctx);
        assert!(app.ui.projection.open);
        let before = app.document.geometry_snapshot();
        app.dispatch(Command::Action(Action::NextBrush), &ctx);
        assert!(!app.ui.projection.open);
        assert!(app.ui.tool_change.is_none());
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Instances(InstanceCommand::NewLegacyArray), &ctx);
        assert!(app.document.instance_preview_active());
        app.dispatch(Command::Action(Action::PreviousBrush), &ctx);
        assert!(app.ui.tool_change.is_some());
        app.resolve_tool_change(Decision::KeepEditing, &ctx);
        assert!(app.document.instance_preview_active());
        app.dispatch(Command::Action(Action::PreviousBrush), &ctx);
        app.resolve_tool_change(Decision::Discard, &ctx);
        assert!(!app.document.instance_preview_active());
        assert!(app.ui.instances.editor.is_none());
        assert_eq!(app.document.geometry_snapshot(), before);
    }
}

//! Temporary masking changes viewport routing without replacing the selected tool.
use super::*;
use crate::{preferences::MaskShape, viewport::TransformMode};

impl App {
    pub(super) fn temporary_mask_requested(&self, ctx: &egui::Context) -> bool {
        !self.ui.mask_suppressed
            && !ctx.wants_keyboard_input()
            && self.temporary_mask_available()
            && ctx.input(|i| self.ui.preferences.mask_hold.active(i))
            && !self.temporary_visibility_active(ctx)
    }
    pub(super) fn temporary_mask_available(&self) -> bool {
        self.document.active_visible()
            && self.document.primitive().is_none()
            && self.ui.transform_mode == TransformMode::Sculpt
            && !self.ui.slice_active
            && !self.ui.paths_active
            && !self.viewport.paths.active
            && !self.ui.projection.open
            && !self.viewport.attach_stencil
            && !self.ui.arrange_references
            && self.mesh_job.is_none()
            && !self.ui.settings_open
            && !self.ui.layout_open
            && self.ui.file_picker.is_none()
            && self.ui.pending.is_none()
            && self.ui.primitive_warning.is_none()
            && !self.ui.recovery_open
            && !self.ui.diagnostics_open
            && self.ui.alphas.draft.is_none()
            && self.ui.alphas.overwrite.is_none()
            && !self.ui.icon_capture_open()
    }

    /// Releasing the hold ends painted dabs, discards unfinished outlines/stamps,
    /// and blocks the still-down pen until release. The normal tool never changes.
    pub(super) fn update_temporary_mask(&mut self, ui: &egui::Ui) -> bool {
        let ctx = ui.ctx();
        let held = ctx.input(|i| self.ui.preferences.mask_hold.active(i));
        let visibility = ctx.input(|i| self.ui.preferences.visibility_hold.active(i));
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let was_active = self.ui.mask_active;
        if !held && ctx.input(|i| i.focused) {
            self.ui.mask_suppressed = false;
        }
        if was_active && !ctx.input(|i| i.focused) {
            self.ui.mask_suppressed = true;
        }
        if was_active && escape {
            self.ui.mask_suppressed = true;
        }
        let over_viewport = ctx.input(|i| {
            i.pointer
                .hover_pos()
                .is_some_and(|p| ui.available_rect_before_wrap().contains(p))
        });
        let eligible = self.temporary_mask_available()
            && ctx.input(|i| i.focused)
            && !ctx.wants_keyboard_input();
        let active = held
            && !visibility
            && !self.ui.mask_suppressed
            && eligible
            && (was_active || over_viewport);
        if active != was_active {
            // Never mix a mask gesture into an already-started sculpt/modeling edit.
            let was_dragging = self.viewport.gesture.is_some();
            self.viewport.cancel(&mut self.document);
            if was_dragging || (!active && ctx.input(|i| i.pointer.primary_down())) {
                self.viewport.wait_for_release();
            }
            self.ui.mask_active = active;
        }
        if held && !eligible && !ctx.wants_keyboard_input() {
            self.ui.status = "Hold to mask is available on an editable visible model in Sculpt or Modeling. Finish the current editor or transform first.".into();
        }
        was_active && escape
    }

    pub(super) fn change_temporary_mask(&mut self, action: Action) {
        if self.ui.mask_active {
            self.viewport.cancel(&mut self.document);
            self.viewport.wait_for_release();
        }
        let preset = &mut self.ui.preferences.mask_preset;
        match action {
            Action::TemporaryMaskBrush => preset.shape = MaskShape::Brush,
            Action::TemporaryMaskLasso => preset.shape = MaskShape::Lasso,
            Action::TemporaryMaskLine => preset.shape = MaskShape::Line,
            Action::TemporaryMaskAdd => preset.erase = false,
            Action::TemporaryMaskErase => preset.erase = true,
            Action::TemporaryMaskFlip => preset.flip = !preset.flip,
            Action::TemporaryMaskPreset(id) => {
                if self
                    .document
                    .brushes()
                    .iter()
                    .any(|b| b.kind == crate::sculpt::Kind::Mask && b.identity() == id)
                {
                    preset.brush_id = Some(id);
                }
            }
            _ => unreachable!("temporary mask action"),
        }
    }

    pub(super) fn paint_temporary_mask_status(&self, ui: &egui::Ui) {
        let preset = &self.ui.preferences.mask_preset;
        let text = format!(
            "Hold to mask · {} · {} · {} · release to restore tool",
            self.ui.preferences.mask_hold.label(),
            preset.shape.label(),
            if preset.erase { "Erase" } else { "Add" }
        );
        ui.painter().text(
            ui.max_rect().center_top() + egui::vec2(0., 26.),
            egui::Align2::CENTER_TOP,
            text,
            egui::FontId::proportional(14.),
            egui::Color32::LIGHT_GREEN,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::{HoldBinding, MaskPreset};
    use crate::sculpt::Kind;
    use egui::{Event, Key, Modifiers, Pos2, Vec2};

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    fn key(key: Key, pressed: bool) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }
    fn frame(app: &mut App, ctx: &egui::Context, modifiers: Modifiers, events: Vec<Event>) {
        frame_focused(app, ctx, modifiers, events, true);
    }
    fn frame_focused(
        app: &mut App,
        ctx: &egui::Context,
        modifiers: Modifiers,
        events: Vec<Event>,
        focused: bool,
    ) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000., 800.),
                )),
                modifiers,
                events,
                focused,
                ..Default::default()
            },
            |ctx| {
                app.shortcuts(ctx);
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
    }
    fn setup() -> (App, egui::Context, Pos2) {
        let mut app = App::default();
        app.ui.preferences.mask_hold = HoldBinding {
            key: Some("M".into()),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let p = Pos2::new(500., 400.);
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![Event::PointerMoved(p)],
        );
        (app, ctx, p)
    }

    #[test]
    fn temporary_mask_hold_only_is_noop_and_releases_original_tool() {
        let (mut app, ctx, p) = setup();
        let before = app.document.geometry_snapshot();
        let selected = app.ui.selected;
        let brush = app.document.brushes()[selected].clone();
        frame(&mut app, &ctx, Modifiers::NONE, vec![key(Key::M, true)]);
        assert!(app.ui.mask_active);
        assert_eq!(app.ui.selected, selected);
        frame(&mut app, &ctx, Modifiers::NONE, vec![key(Key::M, false)]);
        assert!(!app.ui.mask_active);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), (0, 0));
        assert!(app.document.brushes()[selected] == brush);
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![button(p, true), Event::PointerMoved(p + Vec2::new(20., 0.))],
        );
        frame(&mut app, &ctx, Modifiers::NONE, vec![button(p, false)]);
        assert_ne!(app.document.mesh().vertices, before.mesh.vertices);
    }

    #[test]
    fn temporary_mask_brush_preserves_geometry_undo_settings_and_midstroke_release() {
        let (mut app, ctx, p) = setup();
        let before = app.document.geometry_snapshot();
        let original = app.document.brushes()[app.ui.selected].clone();
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![key(Key::M, true), button(p, true)],
        );
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![Event::PointerMoved(p + Vec2::new(25., 0.))],
        );
        assert!(app.document.mesh().mask.iter().any(|&m| m > 0.));
        frame(&mut app, &ctx, Modifiers::NONE, vec![key(Key::M, false)]);
        assert!(!app.ui.mask_active);
        let masked = app.document.geometry_snapshot();
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![Event::PointerMoved(p + Vec2::new(90., 0.))],
        );
        frame(&mut app, &ctx, Modifiers::NONE, vec![button(p, false)]);
        assert_eq!(app.document.geometry_snapshot(), masked);
        assert_eq!(app.document.mesh().vertices, before.mesh.vertices);
        assert_eq!(app.document.mesh().triangles, before.mesh.triangles);
        assert!(app.document.brushes()[app.ui.selected] == original);
        assert_eq!(app.document.history_counts(), (1, 0));
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Redo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), masked);
        let path = std::env::temp_dir().join(format!("clay-mask-hold-{}.clay", std::process::id()));
        persistence::save_project(path.to_str().unwrap(), &app.document.project()).unwrap();
        let reopened = persistence::load_project(path.to_str().unwrap()).unwrap();
        assert_eq!(reopened.mesh.mask, app.document.mesh().mask);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn temporary_mask_modifier_chords_do_not_zoom_smooth_or_reverse() {
        for modifiers in [Modifiers::CTRL, Modifiers::SHIFT, Modifiers::ALT] {
            let (mut app, ctx, p) = setup();
            app.ui.preferences.mask_hold = HoldBinding {
                ctrl: modifiers.ctrl,
                shift: modifiers.shift,
                alt: modifiers.alt,
                ..Default::default()
            };
            let vertices = app.document.mesh().vertices.clone();
            let camera = (
                app.camera.yaw,
                app.camera.pitch,
                app.camera.zoom,
                app.camera.pan,
            );
            frame(&mut app, &ctx, modifiers, vec![button(p, true)]);
            frame(
                &mut app,
                &ctx,
                modifiers,
                vec![Event::PointerMoved(p + Vec2::new(0., 30.))],
            );
            frame(&mut app, &ctx, modifiers, vec![button(p, false)]);
            assert!(app.document.mesh().mask.iter().any(|&m| m > 0.));
            assert_eq!(app.document.mesh().vertices, vertices);
            assert_eq!(
                (
                    app.camera.yaw,
                    app.camera.pitch,
                    app.camera.zoom,
                    app.camera.pan
                ),
                camera
            );
        }
    }

    #[test]
    fn temporary_mask_lasso_and_line_remember_erase_and_cancel_on_hold_release() {
        for shape in [MaskShape::Lasso, MaskShape::Line] {
            let (mut app, ctx, _) = setup();
            app.ui.preferences.mask_preset.shape = shape;
            let start = Pos2::new(490., 150.);
            let points = [
                Pos2::new(780., 150.),
                Pos2::new(780., 680.),
                Pos2::new(490., 680.),
            ];
            let before = app.document.geometry_snapshot();
            for cancel in [true, false] {
                frame(
                    &mut app,
                    &ctx,
                    Modifiers::NONE,
                    vec![
                        key(Key::M, true),
                        Event::PointerMoved(start),
                        button(start, true),
                    ],
                );
                frame(
                    &mut app,
                    &ctx,
                    Modifiers::NONE,
                    points.into_iter().map(Event::PointerMoved).collect(),
                );
                if cancel {
                    frame(&mut app, &ctx, Modifiers::NONE, vec![key(Key::M, false)]);
                }
                frame(
                    &mut app,
                    &ctx,
                    Modifiers::NONE,
                    vec![button(points[2], false)],
                );
                if cancel {
                    assert_eq!(app.document.geometry_snapshot(), before);
                }
            }
            assert!(app.document.mesh().mask.iter().any(|&m| m > 0.));
            assert_eq!(app.document.mesh().vertices, before.mesh.vertices);
            assert_eq!(app.document.history_counts(), (1, 0));
            app.change_temporary_mask(Action::TemporaryMaskErase);
            frame(&mut app, &ctx, Modifiers::NONE, vec![]);
            frame(
                &mut app,
                &ctx,
                Modifiers::NONE,
                vec![Event::PointerMoved(start), button(start, true)],
            );
            frame(
                &mut app,
                &ctx,
                Modifiers::NONE,
                points.into_iter().map(Event::PointerMoved).collect(),
            );
            frame(
                &mut app,
                &ctx,
                Modifiers::NONE,
                vec![button(points[2], false)],
            );
            assert!(app.document.mesh().mask.iter().all(|&m| m == 0.));
        }
    }

    #[test]
    fn temporary_mask_keeps_modeling_selection_and_escape_restores_modeling() {
        let (mut app, ctx, p) = setup();
        app.execute_action(Action::ModelFaces, &ctx);
        frame(&mut app, &ctx, Modifiers::NONE, vec![button(p, true)]);
        frame(&mut app, &ctx, Modifiers::NONE, vec![button(p, false)]);
        let selection = app.viewport.modeling.selection().clone();
        assert!(!selection.is_empty());
        let options = app.ui.modeling.options();
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![key(Key::M, true), button(p, true)],
        );
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![Event::PointerMoved(p + Vec2::new(25., 0.))],
        );
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![button(p, false), key(Key::Escape, true)],
        );
        assert!(!app.ui.mask_active);
        assert!(app.ui.modeling.active);
        assert!(app.ui.modeling.options() == options);
        assert_eq!(app.viewport.modeling.selection(), &selection);
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![key(Key::Escape, false)],
        );
        assert!(
            !app.ui.mask_active,
            "Escape suppresses held mode until its binding is released"
        );
        assert_eq!(app.viewport.modeling.selection(), &selection);
    }

    #[test]
    fn temporary_mask_preset_preferences_round_trip_and_legacy_default() {
        let (mut app, _, _) = setup();
        let brush = app
            .document
            .brushes()
            .iter()
            .find(|b| b.kind == Kind::Mask)
            .unwrap();
        app.ui.preferences.mask_preset = MaskPreset {
            shape: MaskShape::Line,
            erase: true,
            flip: true,
            brush_id: Some(brush.identity()),
        };
        let encoded = serde_json::to_string(&app.ui.preferences).unwrap();
        let loaded: Preferences = serde_json::from_str(&encoded).unwrap();
        assert_eq!(loaded, app.ui.preferences);
        let legacy: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.mask_hold, HoldBinding::default());
        assert_eq!(legacy.mask_preset, MaskPreset::default());
    }
    #[test]
    fn temporary_mask_focus_loss_and_visibility_priority_do_not_leak_edits() {
        let (mut app, ctx, p) = setup();
        app.ui.preferences.mask_preset.shape = MaskShape::Lasso;
        let before = app.document.geometry_snapshot();
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![key(Key::M, true), button(p, true)],
        );
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![Event::PointerMoved(p + Vec2::new(100., 100.))],
        );
        frame_focused(&mut app, &ctx, Modifiers::NONE, vec![], false);
        assert!(!app.ui.mask_active);
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![button(p, false), key(Key::M, false)],
        );
        assert_eq!(app.document.geometry_snapshot(), before);
        app.ui.preferences.mask_hold = HoldBinding {
            ctrl: true,
            ..Default::default()
        };
        frame(
            &mut app,
            &ctx,
            Modifiers::CTRL | Modifiers::SHIFT,
            vec![Event::PointerMoved(p)],
        );
        assert!(!app.ui.mask_active);
        assert_eq!(
            app.viewport_menu_context(&ctx),
            crate::ui::context_menu::ViewportMenuContext::Visibility
        );
        frame(&mut app, &ctx, Modifiers::CTRL, vec![]);
        assert!(app.ui.mask_active);
        app.ui.preferences.mask_hold = HoldBinding::default();
        frame(&mut app, &ctx, Modifiers::CTRL, vec![]);
        assert!(!app.ui.mask_active);
        assert_eq!(app.document.geometry_snapshot(), before);
    }

    #[test]
    fn temporary_mask_unavailable_editors_never_override_their_tool() {
        let (mut app, ctx, p) = setup();
        app.ui.slice_active = true;
        let before = app.document.geometry_snapshot();
        frame(
            &mut app,
            &ctx,
            Modifiers::NONE,
            vec![key(Key::M, true), Event::PointerMoved(p)],
        );
        assert!(!app.ui.mask_active);
        assert!(app.ui.slice_active);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert!(app.ui.status.contains("Hold to mask is available"));
    }
}

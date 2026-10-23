use super::*;
use crate::ui::context_menu::ViewportMenuContext;

impl App {
    pub(crate) fn temporary_visibility_active(&self, ctx: &egui::Context) -> bool {
        !self.ui.viewport_menu.suppress_visibility
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| self.ui.preferences.visibility_hold.active(i))
            && self.document.primitive().is_none()
            && self.document.active_visible()
            && !self.ui.paths_active
            && !self.ui.arrange_references
    }
    pub(crate) fn viewport_menu_context(&self, ctx: &egui::Context) -> ViewportMenuContext {
        if self.temporary_visibility_active(ctx) {
            ViewportMenuContext::Visibility
        } else if matches!(
            self.ui.selection_tool,
            Some(
                crate::viewport::SelectionTool::MaskLasso
                    | crate::viewport::SelectionTool::MaskLine { .. }
            )
        ) {
            ViewportMenuContext::Mask
        } else {
            ViewportMenuContext::Modeling
        }
    }
    pub(crate) fn open_viewport_menu(&mut self, ctx: &egui::Context) {
        if self.viewport.gesture.is_some()
            || self.viewport.modeling.dragging()
            || ctx.wants_keyboard_input()
        {
            self.ui.status =
                "Finish the current gesture before opening the viewport tool menu.".into();
            return;
        }
        let Some(rect) = self.ui.viewport_menu.viewport_rect else {
            return;
        };
        let position = ctx
            .pointer_hover_pos()
            .filter(|p| rect.contains(*p))
            .unwrap_or(rect.center());
        let context = self.viewport_menu_context(ctx);
        self.ui.viewport_menu.context = Some(context);
        self.ui.viewport_menu.position = Some(position);
        self.ui.viewport_menu.selected = 0;
        self.ui.viewport_menu.just_opened = true;
        self.ui.viewport_menu.hold = (context == ViewportMenuContext::Visibility)
            .then(|| self.ui.preferences.visibility_hold.clone());
        self.viewport.wait_for_release();
    }
    pub(crate) fn show_viewport_menu(&mut self, ctx: &egui::Context) {
        let was_open = self.ui.viewport_menu.context.is_some();
        let (action, closed) = self.ui.viewport_menu_popup(ctx, &self.document);
        if let Some(action) = action {
            self.execute_action(action, ctx);
        }
        if was_open || closed {
            self.viewport.wait_for_release();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        preferences::{HoldBinding, Shortcut},
        sculpt::{Geometry, Primitive, PrimitiveKind},
        viewport::{ComponentMode, ModelTool},
    };
    fn setup() -> (App, egui::Context) {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
        primitive.divisions = [3; 3];
        app.document
            .replace_geometry(Geometry::new(primitive.mesh().unwrap()))
            .unwrap();
        app.camera.yaw = 0.;
        app.camera.pitch = 0.;
        app.dispatch(Command::ModelingMode(true), &ctx);
        frame(&mut app, &ctx, egui::Modifiers::NONE, vec![]);
        (app, ctx)
    }
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    fn frame(
        app: &mut App,
        ctx: &egui::Context,
        modifiers: egui::Modifiers,
        events: Vec<egui::Event>,
    ) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                modifiers,
                events,
                ..Default::default()
            },
            |ctx| {
                app.shortcuts(ctx);
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
    }
    #[test]
    fn viewport_menu_keeps_modeling_selection_and_history_for_each_mode() {
        for mode in [
            ComponentMode::Face,
            ComponentMode::Edge,
            ComponentMode::Vertex,
        ] {
            let (mut app, ctx) = setup();
            app.dispatch(Command::ModelingComponent(mode), &ctx);
            let rect = app.ui.viewport_menu.viewport_rect.unwrap();
            let spacing = rect.width().min(rect.height()) * 0.35 / 3.;
            let pos = rect.center()
                + match mode {
                    ComponentMode::Face => egui::Vec2::ZERO,
                    ComponentMode::Edge => egui::vec2(spacing, 0.),
                    ComponentMode::Vertex => egui::vec2(spacing, spacing),
                };
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &ctx,
                    egui::Modifiers::NONE,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert!(
                app.ui.modeling.count > 0,
                "select a component before opening the menu"
            );
            let before = app.document.geometry_snapshot();
            let history = app.document.history_counts();
            let selection = app.viewport.modeling.selection().clone();
            frame(
                &mut app,
                &ctx,
                egui::Modifiers::NONE,
                vec![key(egui::Key::Q, egui::Modifiers::NONE)],
            );
            assert_eq!(
                app.ui.viewport_menu.context,
                Some(ViewportMenuContext::Modeling)
            );
            frame(
                &mut app,
                &ctx,
                egui::Modifiers::NONE,
                vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            );
            frame(
                &mut app,
                &ctx,
                egui::Modifiers::NONE,
                vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            );
            assert_eq!(app.ui.modeling.tool, ModelTool::Move);
            assert!(app.ui.modeling.active);
            assert_eq!(app.viewport.modeling.selection(), &selection);
            assert_eq!(app.document.geometry_snapshot(), before);
            assert_eq!(app.document.history_counts(), history);
        }
    }
    #[test]
    fn viewport_menu_alternate_hold_and_menu_binding_restore_prior_tool() {
        let (mut app, ctx) = setup();
        app.ui.modeling.set_tool(ModelTool::Inset);
        app.ui.preferences.visibility_hold = HoldBinding {
            alt: true,
            ..Default::default()
        };
        app.ui
            .preferences
            .bind(
                Action::ViewportContextMenu,
                Shortcut::new(egui::Key::F2, egui::Modifiers::NONE),
            )
            .unwrap();
        let before = app.document.geometry_snapshot();
        let history = app.document.history_counts();
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![key(egui::Key::F2, egui::Modifiers::ALT)],
        );
        assert_eq!(
            app.ui.viewport_menu.context,
            Some(ViewportMenuContext::Visibility)
        );
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::ALT)],
        );
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![key(egui::Key::Enter, egui::Modifiers::ALT)],
        );
        assert_eq!(
            app.ui.temporary_visibility_tool,
            crate::viewport::SelectionTool::Lasso
        );
        frame(&mut app, &ctx, egui::Modifiers::NONE, vec![]);
        assert!(app.ui.modeling.active);
        assert_eq!(app.ui.modeling.tool, ModelTool::Inset);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), history);
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![key(egui::Key::F2, egui::Modifiers::ALT)],
        );
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![key(egui::Key::Escape, egui::Modifiers::ALT)],
        );
        assert!(app.ui.viewport_menu.context.is_none());
        assert!(app.ui.modeling.active);
        assert_eq!(app.ui.modeling.tool, ModelTool::Inset);
    }
    #[test]
    fn viewport_menu_focus_loss_release_and_text_entry_are_safe() {
        let (mut app, ctx) = setup();
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            vec![key(
                egui::Key::Q,
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            )],
        );
        assert!(app.ui.viewport_menu.context.is_some());
        frame(&mut app, &ctx, egui::Modifiers::NONE, vec![]);
        assert!(app.ui.viewport_menu.context.is_none());
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::NONE,
            vec![key(egui::Key::Q, egui::Modifiers::NONE)],
        );
        let _ = ctx.run(
            egui::RawInput {
                focused: false,
                ..Default::default()
            },
            |ctx| app.show_viewport_menu(ctx),
        );
        assert!(app.ui.viewport_menu.context.is_none());
        app.viewport.gesture = Some(crate::viewport::Gesture::Sculpt);
        app.open_viewport_menu(&ctx);
        assert!(app.ui.viewport_menu.context.is_none());
        app.viewport.gesture = None;
        app.ui.settings_open = true;
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::NONE,
            vec![key(egui::Key::Q, egui::Modifiers::NONE)],
        );
        assert!(app.ui.viewport_menu.context.is_none());
    }
    #[test]
    fn viewport_menu_dismissal_does_not_paint_and_text_entry_suppresses_invocation() {
        let (mut app, ctx) = setup();
        let before = app.document.geometry_snapshot();
        let history = app.document.history_counts();
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::NONE,
            vec![key(egui::Key::Q, egui::Modifiers::NONE)],
        );
        let pos = egui::pos2(80., 400.);
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                egui::Modifiers::NONE,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.ui.viewport_menu.context.is_none());
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), history);
        let mut text = String::new();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.text_edit_singleline(&mut text).request_focus();
            });
        });
        let _ = ctx.run(
            egui::RawInput {
                events: vec![key(egui::Key::Q, egui::Modifiers::NONE)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.text_edit_singleline(&mut text);
                });
                app.shortcuts(ctx);
            },
        );
        assert!(app.ui.viewport_menu.context.is_none());
    }
    #[test]
    fn viewport_menu_preferences_round_trip_clear_and_mac_chord() {
        let mut prefs = Preferences::default();
        prefs.visibility_hold = HoldBinding {
            key: Some("V".into()),
            alt: true,
            ..Default::default()
        };
        prefs.clear_binding(&Action::ViewportContextMenu);
        let copy: Preferences =
            serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
        assert_eq!(copy.visibility_hold, prefs.visibility_hold);
        assert!(copy.shortcut(&Action::ViewportContextMenu).is_none());
        let hold = HoldBinding::visibility_default();
        assert!(hold.matches_shortcut(
            &Shortcut::new(
                egui::Key::Q,
                egui::Modifiers::MAC_CMD | egui::Modifiers::SHIFT
            ),
            &Shortcut::new(egui::Key::Q, egui::Modifiers::NONE)
        ));
        let old: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(old.visibility_hold, HoldBinding::visibility_default());
    }
}

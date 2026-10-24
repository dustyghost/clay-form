use super::*;
use crate::ui::context_menu::ViewportMenuContext;

impl App {
    pub(crate) fn temporary_visibility_active(&self, ctx: &egui::Context) -> bool {
        self.ui.preferences.selection_mode == preferences::SelectionMode::Visibility
            && !self.ui.viewport_menu.suppress_visibility
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.focused && self.ui.preferences.visibility_hold.active(i))
            && self.document.primitive().is_none()
            && self.document.active_visible()
            && !self.ui.paths_active
            && !self.ui.arrange_references
    }
    pub(crate) fn open_viewport_menu(&mut self, ctx: &egui::Context) {
        self.open_quick_menu(ctx, Action::ViewportContextMenu);
    }
    pub(crate) fn open_quick_menu(&mut self, ctx: &egui::Context, action: Action) {
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
        let context = match action {
            Action::TransformMenu => ViewportMenuContext::Transform,
            Action::MaskVisibilityMenu => {
                if self.temporary_visibility_active(ctx) {
                    ViewportMenuContext::Visibility
                } else if self.temporary_mask_requested(ctx) {
                    ViewportMenuContext::Mask
                } else {
                    match self.ui.preferences.selection_mode {
                        preferences::SelectionMode::Mask => ViewportMenuContext::Mask,
                        preferences::SelectionMode::Visibility => ViewportMenuContext::Visibility,
                    }
                }
            }
            _ => ViewportMenuContext::Modeling,
        };
        self.ui.viewport_menu.context = Some(context);
        self.ui.viewport_menu.position = Some(position);
        self.ui.viewport_menu.selected = self
            .ui
            .viewport_menu_entries(context, &self.document)
            .iter()
            .position(|entry| matches!(entry.action, Action::ModelTool(_)))
            .unwrap_or(0);
        self.ui.viewport_menu.just_opened = true;
        self.ui.viewport_menu.hold =
            if action == Action::MaskVisibilityMenu && self.temporary_visibility_active(ctx) {
                Some(self.ui.preferences.visibility_hold.clone())
            } else if context == ViewportMenuContext::Mask && self.temporary_mask_requested(ctx) {
                Some(self.temporary_mask_binding(ctx).clone())
            } else {
                None
            };
        self.viewport.wait_for_release();
    }
    pub(crate) fn show_viewport_menu(&mut self, ctx: &egui::Context) {
        let context = self.ui.viewport_menu.context;
        let was_open = context.is_some();
        let (action, closed) = self.ui.viewport_menu_popup(ctx, &self.document);
        if let Some(action) = action {
            if context == Some(ViewportMenuContext::Visibility)
                && matches!(
                    action,
                    Action::SelectVisibilityLine | Action::SelectVisibilityPath
                )
            {
                // The popup may already be closed. Its choices configure the hold,
                // whereas the same actions elsewhere start persistent selection tools.
                self.ui.preferences.visibility_selection.shape =
                    if action == Action::SelectVisibilityLine {
                        preferences::VisibilityShape::Line
                    } else {
                        preferences::VisibilityShape::Path
                    };
            } else if action == Action::UniversalTransform {
                self.dispatch(
                    Command::TransformOperation(crate::viewport::TransformOperation::Universal),
                    ctx,
                );
            } else {
                self.execute_action(action, ctx);
            }
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
    ) -> egui::FullOutput {
        ctx.run(
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
        )
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
            app.execute_action(Action::ModelTool(ModelTool::Rotate), &ctx);
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
            let target = match mode {
                ComponentMode::Face => crate::viewport::SelectionSpan::Flat,
                ComponentMode::Edge => crate::viewport::SelectionSpan::Border,
                ComponentMode::Vertex => crate::viewport::SelectionSpan::Connected,
            };
            let entries = app
                .ui
                .viewport_menu_entries(ViewportMenuContext::Modeling, &app.document);
            assert!(entries.iter().all(|entry| match entry.action {
                Action::ModelTarget(span) => span.available(mode),
                _ => true,
            }));
            app.ui.viewport_menu.selected = entries
                .iter()
                .position(|entry| entry.action == Action::ModelTarget(target))
                .unwrap();
            frame(
                &mut app,
                &ctx,
                egui::Modifiers::NONE,
                vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            );
            assert_eq!(app.ui.modeling.span, target);
            assert_eq!(app.ui.modeling.tool, ModelTool::Rotate);
            assert_eq!(app.viewport.modeling.selection(), &selection);
            assert_eq!(
                app.ui.viewport_menu.context,
                Some(ViewportMenuContext::Modeling)
            );
            frame(
                &mut app,
                &ctx,
                egui::Modifiers::NONE,
                vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            app.ui.viewport_menu.selected = entries
                .iter()
                .position(|entry| entry.action == Action::ModelTool(ModelTool::Select))
                .unwrap();
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
                Action::MaskVisibilityMenu,
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
            app.ui.preferences.visibility_selection.shape,
            crate::preferences::VisibilityShape::Lasso
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
                egui::Key::X,
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
        let mut prefs = Preferences {
            visibility_hold: HoldBinding {
                key: Some("V".into()),
                alt: true,
                ..Default::default()
            },
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
    #[test]
    fn quick_menus_route_q_x_e_and_keep_underlying_selection() {
        let tap = |code, modifiers| {
            let mut release = key(code, modifiers);
            if let egui::Event::Key { pressed, .. } = &mut release {
                *pressed = false;
            }
            vec![key(code, modifiers), release]
        };
        let (mut app, ctx) = setup();
        let before = app.document.geometry_snapshot();
        let history = app.document.history_counts();
        let hold = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
        for (key_code, context) in [
            (egui::Key::Q, ViewportMenuContext::Modeling),
            (egui::Key::X, ViewportMenuContext::Visibility),
            (egui::Key::E, ViewportMenuContext::Transform),
        ] {
            frame(&mut app, &ctx, hold, tap(key_code, hold));
            assert_eq!(app.ui.viewport_menu.context, Some(context));
            frame(&mut app, &ctx, hold, tap(egui::Key::Escape, hold));
            assert!(app.ui.modeling.active);
        }
        frame(&mut app, &ctx, egui::Modifiers::NONE, vec![]);
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::NONE,
            tap(egui::Key::E, egui::Modifiers::NONE),
        );
        let entries = app
            .ui
            .viewport_menu_entries(ViewportMenuContext::Transform, &app.document);
        assert_eq!(entries[0].action, Action::ModelTool(ModelTool::Move));
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::NONE,
            tap(egui::Key::Enter, egui::Modifiers::NONE),
        );
        assert_eq!(app.ui.modeling.tool, ModelTool::Move);
        assert!(app.ui.viewport_menu.context.is_none());
        app.dispatch(Command::ModelingMode(false), &ctx);
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::NONE,
            tap(egui::Key::E, egui::Modifiers::NONE),
        );
        let entries = app
            .ui
            .viewport_menu_entries(ViewportMenuContext::Transform, &app.document);
        assert_eq!(entries[0].action, Action::Move);
        frame(
            &mut app,
            &ctx,
            egui::Modifiers::NONE,
            tap(egui::Key::Enter, egui::Modifiers::NONE),
        );
        assert_eq!(app.ui.transform_mode, crate::viewport::TransformMode::World);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), history);
    }

    #[test]
    fn quick_selection_menu_configures_presets_without_starting_a_tool() {
        use crate::preferences::VisibilityShape;
        for (action, shape) in [
            (Action::SelectVisibilityLine, VisibilityShape::Line),
            (Action::SelectVisibilityPath, VisibilityShape::Path),
        ] {
            let (mut app, ctx) = setup();
            let before = app.document.geometry_snapshot();
            let history = app.document.history_counts();
            app.open_quick_menu(&ctx, Action::MaskVisibilityMenu);
            app.ui.viewport_menu.selected = app
                .ui
                .viewport_menu_entries(ViewportMenuContext::Visibility, &app.document)
                .iter()
                .position(|entry| entry.action == action)
                .unwrap();
            // Exercise the real popup closing before its chosen action is dispatched.
            frame(
                &mut app,
                &ctx,
                egui::Modifiers::NONE,
                vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            );
            assert!(app.ui.viewport_menu.context.is_none());
            assert!(
                app.ui.selection_tool.is_none(),
                "Choosing {shape:?} must only set the hold preset"
            );
            assert!(app.ui.modeling.active);
            assert_eq!(app.ui.preferences.visibility_selection.shape, shape);
            assert_eq!(app.document.geometry_snapshot(), before);
            assert_eq!(app.document.history_counts(), history);
        }
    }

    #[test]
    fn shared_selection_tabs_control_hold_and_restore_brush_without_edits() {
        let (mut app, ctx) = setup();
        let before = app.document.geometry_snapshot();
        let history = app.document.history_counts();
        let selected = app.ui.selected;
        let mask_brush = app.ui.preferences.mask_preset.brush_index(&app.document);
        let hold = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
        let p = app.ui.viewport_menu.viewport_rect.unwrap().center();
        frame(&mut app, &ctx, hold, vec![egui::Event::PointerMoved(p)]);
        assert!(app.temporary_visibility_active(&ctx));
        assert_eq!(
            app.ui.brush_index_for_context(&app.document, &ctx),
            mask_brush
        );
        frame(&mut app, &ctx, hold, vec![key(egui::Key::X, hold)]);
        assert_eq!(
            app.ui.viewport_menu.context,
            Some(ViewportMenuContext::Visibility)
        );
        for (label, mode) in [
            ("Mask", preferences::SelectionMode::Mask),
            ("Visibility", preferences::SelectionMode::Visibility),
        ] {
            let output = frame(&mut app, &ctx, hold, vec![]);
            let point = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .expect("selection tab is rendered");
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &ctx,
                    hold,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: hold,
                        },
                    ],
                );
            }
            frame(&mut app, &ctx, hold, vec![egui::Event::PointerMoved(p)]);
            assert_eq!(app.ui.preferences.selection_mode, mode);
            assert_eq!(app.ui.mask_active, mode == preferences::SelectionMode::Mask);
            assert_eq!(
                app.temporary_visibility_active(&ctx),
                mode == preferences::SelectionMode::Visibility
            );
            assert_eq!(
                app.ui.brush_index_for_context(&app.document, &ctx),
                mask_brush
            );
            assert_eq!(app.document.geometry_snapshot(), before);
        }
        frame(&mut app, &ctx, egui::Modifiers::NONE, vec![]);
        assert!(app.ui.viewport_menu.context.is_none());
        assert!(!app.ui.selection_brush_active());
        assert_eq!(
            app.ui.brush_index_for_context(&app.document, &ctx),
            Some(selected)
        );
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), history);
        assert!(app.ui.modeling.active);
        let path =
            std::env::temp_dir().join(format!("clay-selection-mode-{}.json", std::process::id()));
        app.ui.preferences.selection_mode = preferences::SelectionMode::Mask;
        preferences::save(&path, &app.ui.preferences).unwrap();
        assert_eq!(
            preferences::load(&path).unwrap().selection_mode,
            preferences::SelectionMode::Mask
        );
        std::fs::remove_file(path).unwrap();
    }
}

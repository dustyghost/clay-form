//! End-to-end input checks for the complete working interface.
use super::*;
use crate::preferences::{
    ViewportControl, ViewportControlIcon, ViewportControlKind, ViewportControlStyle,
};

fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1440., 1000.),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.workspace_ui(ctx),
    )
}

fn settle(app: &mut App, ctx: &egui::Context) -> egui::FullOutput {
    for _ in 0..3 {
        frame(app, ctx, vec![]);
    }
    frame(app, ctx, vec![])
}

fn key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}

fn press(app: &mut App, ctx: &egui::Context, key_code: egui::Key, modifiers: egui::Modifiers) {
    frame(app, ctx, vec![key(key_code, true, modifiers)]);
    frame(app, ctx, vec![key(key_code, false, modifiers)]);
}

fn text_center(output: &egui::FullOutput, label: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing interface text: {label}"))
}

fn click(app: &mut App, ctx: &egui::Context, position: Pos2) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(position),
            primary_event(position, true),
        ],
    );
    frame(app, ctx, vec![primary_event(position, false)]);
}

fn search(app: &mut App, ctx: &egui::Context, query: &str) -> egui::FullOutput {
    app.execute_action(Action::Search, ctx);
    settle(app, ctx);
    frame(app, ctx, vec![egui::Event::Text(query.into())]);
    settle(app, ctx)
}

#[test]
fn search_runs_a_command_and_undoes_it_through_the_normal_dispatcher() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    settle(&mut app, &ctx);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    search(&mut app, &ctx, "invert mask");
    press(&mut app, &ctx, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(!app.ui.search_open());
    assert!(app.document.mesh().mask.iter().all(|weight| *weight == 1.));
    assert_eq!(app.document.history_counts().0, history.0 + 1);

    search(&mut app, &ctx, "undo");
    press(&mut app, &ctx, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(!app.ui.search_open());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts().0, history.0);
}

#[test]
fn search_uses_the_current_configured_shortcut() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    settle(&mut app, &ctx);
    press(&mut app, &ctx, egui::Key::K, egui::Modifiers::CTRL);
    assert!(app.ui.search_open());
    press(&mut app, &ctx, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!app.ui.search_open());
    app.ui
        .preferences
        .bind(
            Action::Search,
            Shortcut::new(egui::Key::J, egui::Modifiers::CTRL),
        )
        .unwrap();
    press(&mut app, &ctx, egui::Key::K, egui::Modifiers::CTRL);
    assert!(!app.ui.search_open());
    press(&mut app, &ctx, egui::Key::J, egui::Modifiers::CTRL);
    assert!(app.ui.search_open());
}

#[test]
fn plain_search_shortcuts_leave_text_input_intact_but_ctrl_k_stays_global() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    app.ui
        .preferences
        .bind(
            Action::Search,
            Shortcut::new(egui::Key::K, egui::Modifiers::NONE),
        )
        .unwrap();
    search(&mut app, &ctx, "mas");
    assert!(ctx.wants_keyboard_input());
    let output = frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::K, true, egui::Modifiers::NONE),
            egui::Event::Text("k".into()),
        ],
    );
    assert!(
        output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "mask")),
        "a plain Search binding must not reopen the modal and clear its query"
    );
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::K, false, egui::Modifiers::NONE)],
    );
    press(&mut app, &ctx, egui::Key::Escape, egui::Modifiers::NONE);
    app.ui
        .preferences
        .bind(
            Action::Search,
            Shortcut::new(egui::Key::K, egui::Modifiers::CTRL),
        )
        .unwrap();
    app.ui.layout_open = true;
    let output = settle(&mut app, &ctx);
    let field = text_center(&output, "New workspace name");
    click(&mut app, &ctx, field);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Text("Detail desk".into())],
    );
    assert!(ctx.wants_keyboard_input());
    press(&mut app, &ctx, egui::Key::K, egui::Modifiers::CTRL);
    assert!(
        app.ui.search_open(),
        "Ctrl+K should work from an existing text field"
    );
}

#[test]
fn running_zen_from_search_closes_quick_menus_and_leaves_the_canvas_usable() {
    for custom_menu in [false, true] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        settle(&mut app, &ctx);
        if custom_menu {
            app.ui.customization.popup_menu = Some(app.ui.preferences.menus[0].id);
            app.ui.customization.popup_blocks_viewport = true;
        } else {
            app.open_viewport_menu(&ctx);
            assert!(app.ui.viewport_menu.context.is_some());
        }
        search(&mut app, &ctx, "zen mode");
        press(&mut app, &ctx, egui::Key::Enter, egui::Modifiers::NONE);
        settle(&mut app, &ctx);
        assert!(app.ui.zen_mode);
        assert!(!app.ui.search_open());
        assert!(app.ui.customization.popup_menu.is_none());
        assert!(!app.ui.customization.popup_blocks_viewport);
        assert!(app.ui.viewport_menu.context.is_none());
        assert!(!egui::Popup::is_any_open(&ctx));
        let start = app.ui.viewport_menu.viewport_rect.unwrap().center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start + Vec2::new(30., 0.))],
        );
        frame(&mut app, &ctx, vec![primary_event(start, false)]);
        assert!(
            app.document.can_undo(),
            "old popup state must not block the canvas"
        );
    }
}

#[test]
fn switching_workspaces_after_a_search_reveal_keeps_the_saved_hidden_panel() {
    use crate::preferences::{Dock, Tool};
    let mut app = App::default();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    app.ui
        .preferences
        .panels
        .iter_mut()
        .find(|p| p.tool == Tool::Tablet)
        .unwrap()
        .dock = Dock::Hidden;
    // Let the sidebar settle its measured minimum width before saving a layout.
    settle(&mut app, &ctx);
    app.ui.preferences.save_workspace_as("Sculpt A").unwrap();
    app.ui.preferences.save_workspace_as("Sculpt B").unwrap();
    app.ui
        .preferences
        .panels
        .iter_mut()
        .find(|p| p.tool == Tool::Tablet)
        .unwrap()
        .dock = Dock::Right;
    app.ui.preferences.switch_workspace(0);
    settle(&mut app, &ctx);
    // App::update normally syncs the active layout after workspace_ui returns.
    app.ui.preferences.sync_active_workspace();
    let saved_a = app.ui.preferences.workspaces[0].clone();
    let output = search(&mut app, &ctx, "tablet pressure");
    let show = text_center(&output, "Show in UI");
    click(&mut app, &ctx, show);
    assert!(!app.ui.search_open());
    assert_ne!(
        app.ui
            .preferences
            .panels
            .iter()
            .find(|p| p.tool == Tool::Tablet)
            .unwrap()
            .dock,
        Dock::Hidden
    );
    let output = settle(&mut app, &ctx);
    let selector = text_center(&output, "Workspace: Sculpt A");
    click(&mut app, &ctx, selector);
    let output = settle(&mut app, &ctx);
    let next = text_center(&output, "Sculpt B");
    click(&mut app, &ctx, next);
    settle(&mut app, &ctx);
    assert_eq!(app.ui.preferences.active_workspace, Some(1));
    assert_eq!(app.ui.preferences.workspaces[0], saved_a);
    assert!(!app.ui.search_has_reveal());
}

#[test]
fn search_escape_and_pointer_dismissal_preserve_the_active_slice_tool() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    app.ui.slice_active = true;
    settle(&mut app, &ctx);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let selected = app.ui.selected;
    let camera = app.camera;
    search(&mut app, &ctx, "cut");
    press(&mut app, &ctx, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!app.ui.search_open());
    assert!(app.ui.slice_active, "search Escape must not cancel Slice");
    assert_eq!(app.ui.selected, selected);

    let output = search(&mut app, &ctx, "roll");
    let close = text_center(&output, "Close");
    click(&mut app, &ctx, close);
    assert!(!app.ui.search_open());
    assert!(app.ui.slice_active);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert_eq!(app.camera.basis(), camera.basis());
    assert_eq!(app.camera.pan, camera.pan);
    assert!(app.viewport.session.gesture.is_none());
}

#[test]
fn dismissing_search_does_not_discard_an_existing_surface_preview() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    settle(&mut app, &ctx);
    app.open_projection().unwrap();
    // Supply an already-computed private draft. This exercises dismissal rather
    // than spending time on an unrelated geometry solver.
    let mesh = std::sync::Arc::new(app.document.mesh().clone());
    app.projection.draft = Some(crate::app::projection::ProjectionDraft {
        mesh: mesh.clone(),
        revision: app.document.display_revision(),
        settings: app.document.capture_edit_settings(),
    });
    app.viewport.session.projection_preview = Some(mesh.clone());
    settle(&mut app, &ctx);
    search(&mut app, &ctx, "cut");
    press(&mut app, &ctx, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(app.ui.projection.open);
    assert!(std::sync::Arc::ptr_eq(
        &app.projection.draft.as_ref().unwrap().mesh,
        &mesh,
    ));
    assert!(std::sync::Arc::ptr_eq(
        app.viewport.session.projection_preview.as_ref().unwrap(),
        &mesh,
    ));
    assert!(!app.document.can_undo());
}

#[test]
fn search_typing_and_outside_pointer_drag_cannot_sculpt_or_run_shortcuts() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    settle(&mut app, &ctx);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let wire = app.ui.wire;
    let model = app.document.active_model_index();
    search(&mut app, &ctx, "wire");
    // W is a viewport command, but it belongs to the search input while open.
    press(&mut app, &ctx, egui::Key::W, egui::Modifiers::NONE);
    assert_eq!(app.ui.wire, wire);
    let start = app.ui.viewport_menu.viewport_rect.unwrap().center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start + Vec2::new(45., 40.))],
    );
    frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert_eq!(app.document.active_model_index(), model);
    assert!(app.viewport.session.gesture.is_none());
}

#[test]
fn zen_keeps_custom_navigation_positions_camera_and_workspace() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    app.ui.preferences.viewport_controls = vec![ViewportControl {
        kind: ViewportControlKind::Roll,
        label: "My roll control".into(),
        style: ViewportControlStyle::Text,
        icon: ViewportControlIcon::Auto,
        position: [0.37, 0.68],
        offset: [17., -11.],
    }];
    app.ui
        .preferences
        .save_workspace_as("Personal sculpt")
        .unwrap();
    app.camera.yaw = 0.35;
    app.camera.pitch = 0.2;
    app.camera.roll = -0.4;
    app.camera.pan = Vec2::new(20., -10.);
    let camera = app.camera;
    let before = app.document.geometry_snapshot();
    let selected = app.ui.selected;
    settle(&mut app, &ctx);
    let preferences = app.ui.preferences.clone();
    let control = || {
        ctx.data(|data| {
            data.get_temp::<egui::Rect>(egui::Id::new(("viewport_control_response", 0)))
        })
        .unwrap()
    };
    let normal = control();
    app.execute_action(Action::ToggleZen, &ctx);
    settle(&mut app, &ctx);
    assert!(app.ui.zen_mode);
    assert_eq!(control(), normal, "Zen must not move custom view controls");
    assert_eq!(app.camera.basis(), camera.basis());
    assert_eq!(app.camera.pan, camera.pan);
    assert_eq!(app.camera.zoom, camera.zoom);
    assert_eq!(app.ui.preferences, preferences);
    assert_eq!(app.ui.selected, selected);
    assert_eq!(app.document.geometry_snapshot(), before);

    search(&mut app, &ctx, "roll");
    assert!(app.ui.search_open());
    press(&mut app, &ctx, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(app.ui.zen_mode);
    let output = search(&mut app, &ctx, "mesh capacity");
    let show = text_center(&output, "Show in UI");
    click(&mut app, &ctx, show);
    assert!(!app.ui.zen_mode);
    assert!(app.ui.mesh_capacity_open);
    let output = settle(&mut app, &ctx);
    let back = text_center(&output, "Return to workspace");
    click(&mut app, &ctx, back);
    settle(&mut app, &ctx);
    assert!(app.ui.zen_mode);
    assert!(!app.ui.mesh_capacity_open);
    assert_eq!(
        control(),
        normal,
        "Return to Zen must recover the original control anchors"
    );
    app.execute_action(Action::ToggleZen, &ctx);
    settle(&mut app, &ctx);
    assert!(!app.ui.zen_mode);
    assert_eq!(control(), normal);
    assert_eq!(app.ui.preferences, preferences);
}

#[test]
fn tab_toggles_zen_only_with_viewport_focus() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    settle(&mut app, &ctx);
    let viewport = app.ui.viewport_menu.viewport_rect.unwrap();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(viewport.center())],
    );
    ctx.memory_mut(|memory| {
        if let Some(id) = memory.focused() {
            memory.surrender_focus(id);
        }
    });
    press(&mut app, &ctx, egui::Key::Tab, egui::Modifiers::NONE);
    assert!(app.ui.zen_mode);
    press(&mut app, &ctx, egui::Key::Tab, egui::Modifiers::NONE);
    assert!(!app.ui.zen_mode);

    search(&mut app, &ctx, "mesh capacity");
    press(&mut app, &ctx, egui::Key::Tab, egui::Modifiers::NONE);
    assert!(app.ui.search_open());
    assert!(!app.ui.zen_mode, "Tab in a text field must navigate focus");
    assert!(ctx.memory(|memory| memory.focused().is_some()));
}

use super::*;
use crate::reference::tests::image;

fn setup() -> (App, egui::Context, egui::Rect) {
    let mut app = App::default();
    app.document.add_reference(image()).unwrap();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    app.document.mark_saved("references.clay");
    (app, ctx, rect)
}
fn point(rect: egui::Rect, p: [f32; 2]) -> Pos2 {
    rect.min + Vec2::new(rect.width() * p[0], rect.height() * p[1])
}
#[test]
fn references_move_resize_and_cancel_without_editing_mesh_or_camera() {
    let (mut app, ctx, rect) = setup();
    let original = app.document.references()[0].placement;
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::ArrangeReferences(true), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    let start = point(rect, original.center);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    let end = start + Vec2::new(100., 60.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(
        app.document.references()[0].placement,
        original,
        "drag is a preview until release"
    );
    assert!(!app.document.is_dirty());
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    let moved = app.document.references()[0].placement;
    assert!((moved.center[0] - original.center[0] - 100. / rect.width()).abs() < 1e-5);
    assert!((moved.center[1] - original.center[1] - 60. / rect.height()).abs() < 1e-5);
    assert!(app.document.is_dirty());
    assert!(!app.document.can_undo());
    let corner = point(rect, moved.center)
        + Vec2::new(
            rect.width() * moved.width / 2.,
            rect.width() * moved.width * 160. / 120. / 2.,
        )
        - Vec2::splat(3.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(corner),
            primary_event(corner, true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(corner + Vec2::splat(60.)),
            primary_event(corner + Vec2::splat(60.), false),
        ],
    );
    assert!(app.document.references()[0].placement.width > moved.width);
    let resized = app.document.references()[0].placement;
    let start = point(rect, resized.center);
    app.document.mark_saved("references.clay");
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start + Vec2::splat(25.))],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert!(!app.ui.arrange_references);
    assert_eq!(app.document.references()[0].placement, resized);
    assert!(!app.document.is_dirty());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.camera.yaw, crate::camera::Camera::default().yaw);
}
#[test]
fn references_are_passive_while_sculpting_and_locked_images_do_not_move() {
    let (mut app, ctx, rect) = setup();
    let mut placement = app.document.references()[0].placement;
    placement.center = [0.5, 0.5];
    placement.width = 0.5;
    placement.locked = true;
    app.dispatch(Command::PlaceReference(0, placement), &ctx);
    let before = app.document.geometry_snapshot();
    let center = rect.center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::new(25., 0.))],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    assert_ne!(
        app.document.geometry_snapshot(),
        before,
        "image must not intercept sculpting"
    );
    assert_eq!(app.document.references()[0].placement, placement);
    app.dispatch(Command::ArrangeReferences(true), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    let before = app.document.geometry_snapshot();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::splat(50.))],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    assert_eq!(app.document.references()[0].placement, placement);
    assert_eq!(app.document.geometry_snapshot(), before);
    let secondary = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Secondary,
        modifiers: Default::default(),
    };
    let yaw = app.camera.yaw;
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center), secondary(center, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::new(80., 0.))],
    );
    navigation_frame(&mut app, &ctx, vec![secondary(center, false)]);
    assert_ne!(app.camera.yaw, yaw);
    assert_eq!(app.document.references()[0].placement, placement);
}
#[test]
fn reference_actions_guard_imports_and_share_menu_placement() {
    let (mut app, ctx, _) = setup();
    let before = app.document.project();
    app.dispatch(
        Command::AddReference(egui::DroppedFile {
            bytes: Some(vec![1, 2, 3].into()),
            ..Default::default()
        }),
        &ctx,
    );
    assert!(app.document.project() == before);
    assert!(!app.document.is_dirty());
    app.dispatch(Command::ArrangeReferences(true), &ctx);
    app.execute_action(Action::ShowReferences, &ctx);
    assert!(!app.ui.references_visible && !app.ui.arrange_references);
    assert!(!app.document.is_dirty());
    app.execute_action(Action::ReferenceSettings, &ctx);
    use crate::preferences::{BuiltinMenu, Dock};
    assert!(app.ui.preferences.menu_placed(BuiltinMenu::References));
    app.ui
        .preferences
        .dock_menu(BuiltinMenu::References, Dock::Right);
    let prefs: Preferences =
        serde_json::from_slice(&serde_json::to_vec(&app.ui.preferences).unwrap()).unwrap();
    prefs.validate().unwrap();
    assert!(
        prefs
            .menu_sections
            .iter()
            .any(|s| s.menu == BuiltinMenu::References && s.dock == Dock::Right)
    );
    app.ui.pending = Some(PendingAction::New);
    app.dispatch(Command::RemoveReference(0), &ctx);
    assert_eq!(app.document.references().len(), 1);
    app.ui.pending = None;
    app.dispatch(Command::RemoveReference(0), &ctx);
    assert!(app.document.references().is_empty());
    assert!(app.document.is_dirty());
}

#[test]
fn reference_drag_interrupted_by_undo_does_not_start_a_sculpt_stroke() {
    let (mut app, ctx, rect) = setup();
    app.document.duplicate_model(false).unwrap();
    let mut placement = app.document.references()[0].placement;
    placement.center = [0.5, 0.5];
    app.document.place_reference(0, placement).unwrap();
    app.dispatch(Command::ArrangeReferences(true), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    let center = rect.center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::splat(10.))],
    );
    app.dispatch(Command::Undo, &ctx);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::splat(30.))],
    );
    assert!(app.viewport.gesture.is_none());
    navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert_eq!(app.document.references()[0].placement, placement);
}

#[test]
fn reference_drop_embeds_the_file_and_focus_loss_discards_a_drag() {
    let (mut app, ctx, rect) = setup();
    let directory = crate::recovery::tests::Directory::new();
    std::fs::create_dir_all(&directory.0).unwrap();
    let path = directory.0.join("reference.png");
    let image = image();
    image::save_buffer(
        &path,
        &image.pixels.rgba,
        image.pixels.size[0] as u32,
        image.pixels.size[1] as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            events: vec![egui::Event::PointerMoved(rect.center())],
            dropped_files: vec![egui::DroppedFile {
                path: Some(path.clone()),
                ..Default::default()
            }],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
    assert_eq!(app.document.references().len(), 2);
    assert!(app.ui.arrange_references);
    let saved = directory.0.join("with-references.clay");
    assert!(app.save_to(&saved.to_string_lossy()));
    std::fs::remove_file(path).unwrap();
    assert!(persistence::load_project(&saved.to_string_lossy()).unwrap() == app.document.project());
    navigation_frame(&mut app, &ctx, vec![]);
    let placement = app.document.references()[1].placement;
    let start = point(rect, placement.center);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start + Vec2::splat(30.))],
    );
    let _ = ctx.run(
        egui::RawInput {
            focused: false,
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert_eq!(app.document.references()[1].placement, placement);
    assert!(!app.document.is_dirty());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn reference_images_offscreen_preview() {
    use crate::preferences::{BuiltinMenu, Dock};
    for size in [[1200, 850], [760, 560]] {
        let mut capture = crate::viewport::UiCapture::new(size);
        let ctx = egui::Context::default();
        let mut app = App::default();
        app.viewport.initialize_renderer(&capture.state).unwrap();
        for panel in &mut app.ui.preferences.panels {
            panel.dock = Dock::Hidden;
        }
        app.ui
            .preferences
            .dock_menu(BuiltinMenu::References, Dock::Right);
        app.document.add_reference(image()).unwrap();
        let mut second = image();
        second.name = "Side study".into();
        app.document.add_reference(second).unwrap();
        let mut placement = app.document.references()[1].placement;
        placement.center = [0.77, 0.75];
        placement.width = 0.22;
        placement.locked = true;
        placement.opacity = 0.7;
        app.document.place_reference(1, placement).unwrap();
        app.ui.arrange_references = true;
        let before = app.document.geometry_snapshot();
        for step in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(size[0] as f32, size[1] as f32),
                    )),
                    time: Some(step as f64),
                    ..Default::default()
                },
                |ctx| {
                    let _ = app.ui.toolbar(ctx, &app.document, &app.camera);
                    let _ = app
                        .ui
                        .brush_panel(ctx, &app.document, "Preview", 1., &app.camera);
                    app.ui.status_bar(ctx);
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
            let rgba = capture.render(&ctx, output);
            if step == 2 {
                assert!(
                    rgba.as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|p| p[0].abs_diff(230) < 3
                            && p[1].abs_diff(190) < 3
                            && p[2].abs_diff(120) < 3)
                        .count()
                        > 1000,
                    "reference must appear over the GPU viewport"
                );
                if let Ok(path) = std::env::var("CLAY_REFERENCE_PREVIEW") {
                    let path = if size[0] == 760 {
                        format!("{path}-small.png")
                    } else {
                        path
                    };
                    capture.save(path, &rgba);
                }
            }
        }
        assert_eq!(app.document.geometry_snapshot(), before);
        app.viewport.destroy_renderer();
    }
}

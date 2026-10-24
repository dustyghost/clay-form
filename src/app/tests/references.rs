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
    assert!(app.viewport.session.gesture.is_none());
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

fn reference_drop_frame(app: &mut App, ctx: &egui::Context, files: Vec<egui::DroppedFile>) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            dropped_files: files,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
}

#[test]
fn reference_native_drop_without_pointer_or_with_stale_position_imports_png_and_jpeg() {
    let (mut app, ctx, _) = setup();
    let directory = crate::recovery::tests::Directory::new();
    std::fs::create_dir_all(&directory.0).unwrap();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    for (name, events) in [
        ("native.png", vec![egui::Event::PointerGone]),
        (
            "native.jpg",
            vec![egui::Event::PointerMoved(Pos2::new(-10., -10.))],
        ),
    ] {
        let path = directory.0.join(name);
        image::RgbImage::from_pixel(16, 24, image::Rgb([30, 100, 180]))
            .save(&path)
            .unwrap();
        navigation_frame(&mut app, &ctx, events);
        let count = app.document.references().len();
        reference_drop_frame(
            &mut app,
            &ctx,
            vec![egui::DroppedFile {
                path: Some(path.clone()),
                ..Default::default()
            }],
        );
        assert_eq!(
            app.document.references().len(),
            count + 1,
            "{name} drop must not depend on normal pointer events"
        );
        assert!(app.ui.arrange_references && app.ui.references_visible);
        assert_eq!(app.ui.selected_reference, count);
        assert!(app.document.references()[count].placement.visible);
        assert!(path.exists());
    }
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
}

#[test]
fn reference_native_drop_respects_modal_guards_and_reports_invalid_files() {
    let (mut app, ctx, _) = setup();
    let invalid = || egui::DroppedFile {
        name: "broken.png".into(),
        bytes: Some(vec![1, 2, 3].into()),
        ..Default::default()
    };
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
    let before = app.document.project();
    app.ui.settings_open = true;
    reference_drop_frame(&mut app, &ctx, vec![invalid()]);
    assert!(app.document.project() == before);
    assert!(!app.document.is_dirty());
    app.ui.settings_open = false;
    reference_drop_frame(&mut app, &ctx, vec![invalid()]);
    assert!(app.ui.status.contains("PNG or JPEG"), "{}", app.ui.status);
    assert!(app.document.project() == before);
    assert!(!app.document.is_dirty());
}

#[test]
fn reference_removal_preserves_sources_other_images_and_releases_capacity() {
    let (mut app, ctx, _) = setup();
    let directory = crate::recovery::tests::Directory::new();
    std::fs::create_dir_all(&directory.0).unwrap();
    let source = directory.0.join("source.png");
    image::RgbImage::from_pixel(16, 24, image::Rgb([30, 100, 180]))
        .save(&source)
        .unwrap();
    app.add_reference(egui::DroppedFile {
        path: Some(source.clone()),
        ..Default::default()
    })
    .unwrap();
    let mut placement = app.document.references()[1].placement;
    placement.visible = false;
    placement.locked = true;
    app.document.place_reference(1, placement).unwrap();
    for _ in 2..crate::reference::MAX_REFERENCES {
        app.document.add_reference(image()).unwrap();
    }
    let mut expected = app.document.references().to_vec();
    expected.remove(1);
    app.document.mark_saved("before-removal.clay");
    app.dispatch(Command::RemoveReference(1), &ctx);
    assert_eq!(app.document.references(), expected);
    assert!(app.document.is_dirty());
    assert!(
        source.exists(),
        "removing a reference must leave the source file intact"
    );
    let saved = directory.0.join("removed.clay");
    assert!(app.save_to(&saved.to_string_lossy()));
    assert_eq!(
        persistence::load_project(&saved.to_string_lossy())
            .unwrap()
            .references,
        expected
    );
    app.add_reference(egui::DroppedFile {
        path: Some(source),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        app.document.references().len(),
        crate::reference::MAX_REFERENCES
    );
}

#[test]
fn reference_remove_control_is_accessible_for_hidden_locked_image() {
    use crate::preferences::{BuiltinMenu, Dock};
    let (mut app, ctx, _) = setup();
    let mut project = app.document.project();
    project.references[0].name = "A long reference image study name".into();
    app.document
        .replace_project(project, "hidden.clay")
        .unwrap();
    let mut placement = app.document.references()[0].placement;
    placement.visible = false;
    placement.locked = true;
    app.document.place_reference(0, placement).unwrap();
    app.document.mark_saved("hidden.clay");
    app.ui.references_visible = false;
    for panel in &mut app.ui.preferences.panels {
        panel.dock = Dock::Hidden;
    }
    app.ui
        .preferences
        .dock_menu(BuiltinMenu::References, Dock::Right);
    let frame = |app: &mut App, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(760., 560.))),
                events,
                ..Default::default()
            },
            |ctx| {
                for command in app.ui.toolbar(ctx, &app.document, &app.camera) {
                    app.dispatch(command, ctx);
                }
                let commands = app
                    .ui
                    .brush_panel(ctx, &app.document, "Preview", 1., &app.camera);
                for command in commands {
                    app.dispatch(command, ctx);
                }
            },
        )
    };
    frame(&mut app, vec![]);
    let output = frame(&mut app, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == "Remove image" => {
                let point = t.pos + t.galley.size() * 0.5;
                shape.clip_rect.contains(point).then_some(point)
            }
            _ => None,
        })
        .expect("Remove image should be visible without scrolling the reference controls");
    frame(
        &mut app,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    frame(&mut app, vec![primary_event(point, false)]);
    assert!(app.document.references().is_empty());
    assert!(app.document.is_dirty());
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

use super::*;
use crate::sculpt::{Kind, StrokeMode};

#[test]
fn reset_view_fits_large_offset_models_via_f_without_editing() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube)
        .mesh()
        .unwrap();
    for point in &mut mesh.vertices {
        *point = *point * 30. + crate::sculpt::Vec3(50., -40., 10.);
    }
    app.document
        .replace_geometry(crate::sculpt::Geometry::new(mesh))
        .unwrap();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let dirty = app.document.is_dirty();
    app.camera.yaw = 0.7;
    app.camera.pitch = 0.4;
    app.camera.roll = 0.5;
    app.camera.zoom = 4.;
    app.camera.pan = Vec2::new(300., -200.);
    app.camera.perspective = true;
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| app.shortcuts(ctx),
    );
    assert_eq!(
        (app.camera.yaw, app.camera.pitch, app.camera.roll),
        (0., 0., 0.)
    );
    assert!(!app.camera.perspective);
    let mut bounds = egui::Rect::NOTHING;
    for &point in &app.document.mesh().vertices {
        let projected = rect.center()
            + app.camera.pan
            + Vec2::new(point.0, -point.1) * app.camera.scale_at(rect.size(), point);
        assert!(
            rect.shrink(25.).contains(projected),
            "{projected:?} outside {rect:?}"
        );
        bounds.extend_with(projected);
    }
    assert!(bounds.center().distance(rect.center()) < 0.01);
    assert!(bounds.height() > rect.height() * 0.6);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert_eq!(app.document.is_dirty(), dirty);
    let zoom = app.camera.zoom;
    app.dispatch(Command::ZoomIn, &ctx);
    assert!((app.camera.zoom / zoom - 1.2).abs() < 1e-5);
}

#[test]
fn reset_view_fits_visible_submodels_in_wide_and_tall_viewports() {
    use crate::sculpt::{Geometry, ModelMetadata, Primitive, PrimitiveKind, Submodel, Vec3};
    for size in [Vec2::new(400., 900.), Vec2::new(1200., 400.)] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let mesh = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
        let mut geometry = Geometry::new(mesh.clone());
        for (offset, visible, hidden_faces) in [
            (Vec3(12., -6., 0.), true, false),
            (Vec3(1000., 1000., 0.), false, false),
            (Vec3(-1000., -1000., 0.), true, true),
        ] {
            let mut shifted = mesh.clone();
            for point in &mut shifted.vertices {
                *point = *point + offset;
            }
            if hidden_faces {
                shifted.hidden_faces = vec![true; shifted.triangles.len()];
            }
            geometry.submodels.push(Submodel {
                name: "Part".into(),
                metadata: ModelMetadata {
                    visible,
                    ..Default::default()
                },
                mesh: shifted,
                mesh_settings: Default::default(),
            });
        }
        app.document.replace_geometry(geometry).unwrap();
        let rect = egui::Rect::from_min_size(Pos2::new(350., 120.), size);
        app.ui.viewport_menu.viewport_rect = Some(rect);
        app.dispatch(Command::ResetView, &ctx);
        let mut bounds = egui::Rect::NOTHING;
        for index in 0..2 {
            for &point in &app.document.model(index).unwrap().mesh.vertices {
                let screen = rect.center()
                    + app.camera.pan
                    + Vec2::new(point.0, -point.1) * app.camera.scale_at(size, point);
                assert!(rect.shrink(25.).contains(screen));
                bounds.extend_with(screen);
            }
        }
        assert!(bounds.center().distance(rect.center()) < 0.01);
        assert!(
            bounds.width() > size.x * 0.6 || bounds.height() > size.y * 0.6,
            "hidden geometry must not shrink the visible models"
        );
        let fitted = (app.camera.zoom, app.camera.pan);
        app.dispatch(Command::ResetView, &ctx);
        assert_eq!((app.camera.zoom, app.camera.pan), fitted);
    }
}

#[test]
fn reset_view_with_no_visible_geometry_keeps_default_camera() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut geometry = app.document.geometry_snapshot();
    geometry.scene.active_metadata.visible = false;
    app.document.replace_geometry(geometry).unwrap();
    navigation_frame(&mut app, &ctx, vec![]);
    app.camera.zoom = 4.;
    app.camera.pan = Vec2::splat(200.);
    app.dispatch(Command::ResetView, &ctx);
    assert_eq!(app.camera.zoom, 1.);
    assert_eq!(app.camera.pan, Vec2::ZERO);
}

#[test]
fn perspective_actions_preserve_geometry_history_and_rebind_and_validate_fov() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let original = app.document.geometry_snapshot();
    assert!(
        app.ui
            .preferences
            .bindings
            .iter()
            .find(|(a, _)| *a == Action::Perspective)
            .unwrap()
            .1
            .is_none()
    );
    app.camera.yaw = 0.4;
    app.camera.pitch = 2.;
    app.camera.pan = Vec2::new(20., -30.);
    app.camera.zoom = 2.;
    app.dispatch(Command::Action(Action::Perspective), &ctx);
    assert!(app.camera.perspective);
    assert_eq!((app.camera.yaw, app.camera.pitch), (0.4, 2.));
    assert_eq!(app.camera.pan, Vec2::new(20., -30.));
    assert!(app.camera.zoom < 2. && app.camera.zoom > 1.5);
    for (input, expected) in [
        (0., 20.),
        (100., 75.),
        (f32::NAN, 75.),
        (f32::INFINITY, 75.),
        (50., 50.),
    ] {
        app.dispatch(Command::SetFieldOfView(input), &ctx);
        assert_eq!(app.camera.field_of_view, expected);
    }
    app.ui
        .preferences
        .bind(
            Action::Perspective,
            crate::preferences::Shortcut::new(egui::Key::F3, egui::Modifiers::NONE),
        )
        .unwrap();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::F3,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| app.shortcuts(ctx),
    );
    assert!(!app.camera.perspective);
    app.dispatch(Command::ResetView, &ctx);
    assert_eq!(app.camera.field_of_view, 45.);
    assert_eq!(app.document.geometry_snapshot(), original);
    assert!(!app.document.is_dirty());
    assert!(!app.document.can_undo());
}

#[test]
fn perspective_sculpt_stamps_undo_and_projection_changes_cancel_pending_input() {
    for mode in StrokeMode::ALL {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.dispatch(Command::SetPerspective(true), &ctx);
        app.dispatch(Command::SetFieldOfView(65.), &ctx);
        let mut brush = app.document.brushes()[0].clone();
        brush.kind = Kind::Standard;
        brush.stroke_mode = mode;
        app.document.update_brush(0, brush).unwrap();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let start = rect.center() + Vec2::new(60., -20.);
        let end = start + Vec2::new(30., 50.);
        let original = app.document.geometry_snapshot();
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_ne!(app.document.geometry_snapshot(), original, "{mode:?}");
        assert_eq!(app.document.history_counts().0, 1);
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), original);
        assert!(app.camera.perspective);
        if mode != StrokeMode::Freehand {
            navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), primary_event(start, true)],
            );
            navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            app.dispatch(Command::SetPerspective(false), &ctx);
            navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
            assert_eq!(
                app.document.geometry_snapshot(),
                original,
                "projection must discard a pending {mode:?} stamp"
            );
            assert!(app.document.can_redo());
            app.dispatch(Command::SetPerspective(true), &ctx);
            navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), primary_event(start, true)],
            );
            navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            app.dispatch(Command::ResetView, &ctx);
            navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
            assert_eq!(
                app.document.geometry_snapshot(),
                original,
                "Reset view must cancel the pending stamp when returning to orthographic"
            );
            assert!(!app.camera.perspective);
        }
    }
}

#[test]
fn perspective_switch_and_fov_preserve_visible_model_framing_without_drift() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let cube = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
    app.document
        .replace_geometry(crate::sculpt::Geometry::new(cube.mesh().unwrap()))
        .unwrap();
    app.camera.yaw = 0.5;
    app.camera.pitch = 0.25;
    let apparent = |app: &App| {
        app.camera.zoom
            * app
                .camera
                .projected_extent(app.document.mesh().vertices.iter().copied())
    };
    let before = apparent(&app);
    for _ in 0..5 {
        app.dispatch(Command::SetPerspective(true), &ctx);
        assert!((apparent(&app) - before).abs() < 1e-5);
        for fov in [75., 20., 45.] {
            app.dispatch(Command::SetFieldOfView(fov), &ctx);
            assert!((apparent(&app) - before).abs() < 1e-5);
        }
        app.dispatch(Command::SetPerspective(false), &ctx);
        assert!((app.camera.zoom - 1.).abs() < 1e-5);
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn perspective_view_offscreen_preview() {
    use crate::sculpt::{Geometry, Primitive, PrimitiveKind};
    use crate::viewport::UiCapture;
    use crate::{
        App,
        command::Command,
        preferences::{BuiltinMenu, Dock, MenuSection},
    };
    let mut capture = UiCapture::new([1000, 800]);
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.viewport.initialize_renderer(&capture.state).unwrap();
    app.document
        .replace_geometry(Geometry::new(
            Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap(),
        ))
        .unwrap();
    app.camera.yaw = 0.5;
    app.camera.pitch = 0.25;
    app.camera.zoom = 0.75;
    app.dispatch(Command::SetPerspective(true), &ctx);
    app.dispatch(Command::SetFieldOfView(65.), &ctx);
    for panel in &mut app.ui.preferences.panels {
        panel.dock = Dock::Hidden;
    }
    app.ui.preferences.menu_sections.push(MenuSection {
        menu: BuiltinMenu::View,
        dock: Dock::Left,
        open: true,
    });
    app.ui.show_polygroups = false;
    app.ui.wire = true;
    let before = app.document.geometry_snapshot();
    for perspective in [false, true] {
        app.dispatch(Command::SetPerspective(perspective), &ctx);
        for step in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000., 800.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    let _ = app.ui.toolbar(ctx, &app.document, &app.camera);
                    let _ = app
                        .ui
                        .brush_panel(ctx, &app.document, "Preview", 1., &app.camera);
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
            let rgba = capture.render(&ctx, output);
            if step == 2 {
                let prefix = std::env::var("CLAY_PERSPECTIVE_PREVIEW")
                    .unwrap_or_else(|_| "/tmp/clay-perspective".into());
                capture.save(
                    format!(
                        "{prefix}-{}.png",
                        if perspective { "ui" } else { "orthographic" }
                    ),
                    &rgba,
                );
            }
        }
    }
    assert_eq!(app.document.geometry_snapshot(), before);
    app.viewport.destroy_renderer();
}

#[test]
fn perspective_eye_stays_outside_large_offset_models_at_every_fov() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube)
        .mesh()
        .unwrap();
    for point in &mut mesh.vertices {
        *point = *point * 10. + crate::sculpt::Vec3(0., 0., 40.);
    }
    app.document
        .replace_geometry(crate::sculpt::Geometry::new(mesh))
        .unwrap();
    app.dispatch(Command::SetPerspective(true), &ctx);
    let radius = app
        .document
        .mesh()
        .vertices
        .iter()
        .map(|p| p.len())
        .fold(0_f32, f32::max);
    for fov in [20., 45., 75.] {
        app.dispatch(Command::SetFieldOfView(fov), &ctx);
        assert!(app.camera.eye_distance() > radius * 1.1);
    }
}

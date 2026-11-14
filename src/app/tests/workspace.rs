//! End-to-end checks for the first sculpt workspace and compact project access.
use super::*;

fn frame(
    app: &mut App,
    ctx: &egui::Context,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| app.workspace_ui(ctx),
    )
}

fn text_point(output: &egui::FullOutput, label: &str) -> Pos2 {
    fn find(shape: &egui::Shape, clip: egui::Rect, label: &str) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let point = text.pos + text.galley.size() * 0.5;
                clip.contains(point).then_some(point)
            }
            egui::Shape::Vec(shapes) => shapes
                .iter()
                .rev()
                .find_map(|shape| find(shape, clip, label)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| find(&shape.shape, shape.clip_rect, label))
        .unwrap_or_else(|| panic!("Missing visible control: {label}"))
}

fn pointer(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn escape(app: &mut App, ctx: &egui::Context, size: Vec2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            size,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    frame(app, ctx, size, vec![]);
}

#[test]
fn compact_workspace_keeps_project_search_and_undo_reachable() {
    for (size, compact, scale) in [
        (Vec2::new(1280., 900.), false, 1.),
        (Vec2::new(800., 720.), true, 1.),
        (Vec2::new(640., 480.), true, 1.),
        // Same logical area at higher pixel density, plus a smaller logical
        // viewport equivalent to scaling a fixed-size physical window.
        (Vec2::new(800., 720.), true, 1.5),
        (Vec2::new(640., 480.), true, 1.5),
    ] {
        eprintln!("Workspace case: logical {size:?}, compact {compact}, pixels per point {scale}");
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(scale);
        ctx.style_mut(|style| style.animation_time = 0.);
        let mut app = App::default();
        app.ui.preferences.compact_toolbar = compact;
        for _ in 0..4 {
            frame(&mut app, &ctx, size, vec![]);
        }
        let output = frame(&mut app, &ctx, size, vec![]);
        for label in ["Project", "Save", "Undo", "Redo", "Search…", "Zen mode"] {
            let point = text_point(&output, label);
            assert!(
                egui::Rect::from_min_size(Pos2::ZERO, size).contains(point),
                "{label} is outside the window"
            );
        }
        text_point(
            &output,
            if compact {
                "Workspace: Custom"
            } else {
                "Workspace…"
            },
        );
        let point = text_point(&output, "Project");
        for pressed in [true, false] {
            frame(&mut app, &ctx, size, pointer(point, pressed));
        }
        let output = frame(&mut app, &ctx, size, vec![]);
        for label in ["New sphere", "Open…", "Save as…", "Import", "Export"] {
            text_point(&output, label);
        }
        assert!(
            !app.document.is_dirty(),
            "Browsing controls must not modify the sculpt"
        );
        escape(&mut app, &ctx, size);
        assert!(!egui::Popup::is_any_open(&ctx));
        assert!(app.ui.file_picker.is_none());
        let output = frame(&mut app, &ctx, size, vec![]);
        let search = text_point(&output, "Search…");
        for pressed in [true, false] {
            frame(&mut app, &ctx, size, pointer(search, pressed));
        }
        assert!(
            app.ui.search_open(),
            "Search opens at {size:?}, scale {scale}"
        );
        // egui performs an initial invisible sizing pass for a newly opened modal.
        for _ in 0..3 {
            frame(&mut app, &ctx, size, vec![]);
        }
        let output = frame(&mut app, &ctx, size, vec![]);
        text_point(&output, "Close");
        escape(&mut app, &ctx, size);
        assert!(!app.ui.search_open());
    }
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "interactive workspace captures; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn discoverable_workspace_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW"),
    );
    std::fs::create_dir_all(&directory).unwrap();
    for (dimensions, compact, name) in [
        ([1280, 900], false, "workspace-standard"),
        ([800, 720], true, "workspace-compact"),
        ([640, 480], true, "workspace-small"),
    ] {
        let size = Vec2::new(dimensions[0] as f32, dimensions[1] as f32);
        let mut capture = crate::viewport::UiCapture::new(dimensions);
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let mut app = App::default();
        app.ui.preferences.compact_toolbar = compact;
        app.camera.zoom = 0.6;
        app.viewport.initialize_renderer(&capture.state).unwrap();
        let mut output = frame(&mut app, &ctx, size, vec![]);
        for _ in 0..2 {
            capture.render(&ctx, output);
            output = frame(&mut app, &ctx, size, vec![]);
        }
        let rgba = capture.render(&ctx, output.clone());
        capture.save(directory.join(format!("{name}.png")), &rgba);
        let point = text_point(&output, "Project");
        for pressed in [true, false] {
            let output = frame(&mut app, &ctx, size, pointer(point, pressed));
            capture.render(&ctx, output);
        }
        let output = frame(&mut app, &ctx, size, vec![]);
        let rgba = capture.render(&ctx, output);
        capture.save(directory.join(format!("{name}-project.png")), &rgba);
        // Escape dismisses Project even when its popup covers the viewport centre.
        escape(&mut app, &ctx, size);
        assert!(!egui::Popup::is_any_open(&ctx));
        assert!(app.ui.file_picker.is_none());
        let output = frame(&mut app, &ctx, size, vec![]);
        capture.render(&ctx, output);
        let point = app.ui.viewport_menu.viewport_rect.unwrap().center();
        if !compact {
            let before = app.document.geometry_snapshot();
            let start = point - Vec2::new(30., 0.);
            let output = frame(&mut app, &ctx, size, pointer(start, true));
            capture.render(&ctx, output);
            for step in 1..=12 {
                let output = frame(
                    &mut app,
                    &ctx,
                    size,
                    vec![egui::Event::PointerMoved(
                        start + Vec2::new(step as f32 * 5., 0.),
                    )],
                );
                capture.render(&ctx, output);
            }
            let output = frame(
                &mut app,
                &ctx,
                size,
                pointer(start + Vec2::new(60., 0.), false),
            );
            capture.render(&ctx, output);
            assert_ne!(
                app.document.geometry_snapshot(),
                before,
                "The first sculpt gesture should change the sphere"
            );
            let sculpt = app.document.geometry_snapshot();
            let output = frame(&mut app, &ctx, size, vec![]);
            let undo = text_point(&output, "Undo");
            capture.render(&ctx, output);
            for pressed in [true, false] {
                let output = frame(&mut app, &ctx, size, pointer(undo, pressed));
                capture.render(&ctx, output);
            }
            assert_eq!(app.document.geometry_snapshot(), before);
            app.dispatch(Command::Action(Action::Redo), &ctx);
            assert_eq!(app.document.geometry_snapshot(), sculpt);
            let path = directory.join("workspace-first-sculpt.clay");
            app.ui.path = path.to_string_lossy().into_owned();
            let output = frame(&mut app, &ctx, size, vec![]);
            let save = text_point(&output, "Save");
            capture.render(&ctx, output);
            for pressed in [true, false] {
                let output = frame(&mut app, &ctx, size, pointer(save, pressed));
                capture.render(&ctx, output);
            }
            assert!(app.ui.file_picker.is_some());
            app.dispatch(Command::AcceptProjectFile(true), &ctx);
            assert!(path.exists());
            app.dispatch(Command::Action(Action::New), &ctx);
            app.ui.path = path.to_string_lossy().into_owned();
            app.dispatch(Command::Action(Action::Open), &ctx);
            app.dispatch(Command::AcceptProjectFile(false), &ctx);
            assert_eq!(app.document.geometry_snapshot(), sculpt);
            let output = frame(&mut app, &ctx, size, vec![]);
            let rgba = capture.render(&ctx, output);
            capture.save(directory.join("workspace-first-sculpt-reopened.png"), &rgba);
        }
        if compact {
            let output = frame(&mut app, &ctx, size, vec![]);
            let search = text_point(&output, "Search…");
            capture.render(&ctx, output);
            for pressed in [true, false] {
                let output = frame(&mut app, &ctx, size, pointer(search, pressed));
                capture.render(&ctx, output);
            }
            assert!(app.ui.search_open(), "Search opens at {size:?}");
            for _ in 0..3 {
                let output = frame(&mut app, &ctx, size, vec![]);
                capture.render(&ctx, output);
            }
            let output = frame(&mut app, &ctx, size, vec![egui::Event::Text("cut".into())]);
            capture.render(&ctx, output);
            let output = frame(&mut app, &ctx, size, vec![]);
            let rgba = capture.render(&ctx, output.clone());
            capture.save(directory.join(format!("{name}-search-cut.png")), &rgba);
            let close = text_point(&output, "Close");
            for pressed in [true, false] {
                let output = frame(&mut app, &ctx, size, pointer(close, pressed));
                capture.render(&ctx, output);
            }
        }
        let output = frame(&mut app, &ctx, size, vec![]);
        let zen = text_point(&output, "Zen mode");
        capture.render(&ctx, output);
        for pressed in [true, false] {
            let output = frame(&mut app, &ctx, size, pointer(zen, pressed));
            capture.render(&ctx, output);
        }
        let output = frame(&mut app, &ctx, size, vec![]);
        assert!(app.ui.zen_mode);
        text_point(&output, "Exit Zen mode");
        text_point(&output, "Search…");
        let rgba = capture.render(&ctx, output);
        capture.save(directory.join(format!("{name}-zen.png")), &rgba);
        app.viewport.destroy_renderer();
    }
}

use super::*;

fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 760.),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            assert!(
                app.show_startup_notice(ctx),
                "startup must consume this frame"
            );
        },
    )
}

fn click(app: &mut App, ctx: &egui::Context, label: &str) {
    let output = frame(app, ctx, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing startup control: {label}"));
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(pos), primary_event(pos, true)],
    );
    frame(app, ctx, vec![primary_event(pos, false)]);
}

#[test]
fn startup_notice_continue_and_version_specific_opt_out_survive_restart() {
    let directory = crate::recovery::tests::Directory::new();
    let path = directory.0.join("preferences.json");
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.);
    let mut app = App::default();
    app.preferences_path = Some(path.clone());
    app.ui.startup_notice_open = true;
    app.ui.recovery_open = true;
    let before = app.document.geometry_snapshot();
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    assert!(output.shapes.iter().any(|s| matches!(&s.shape,
        egui::Shape::Text(t) if t.galley.text().contains(env!("CARGO_PKG_VERSION")))));
    click(&mut app, &ctx, "Continue to Clay Form");
    assert!(!app.ui.startup_notice_open);
    assert!(!app.show_startup_notice(&ctx));
    assert!(app.ui.preferences.dismissed_startup_version.is_empty());
    assert!(
        app.ui.recovery_open,
        "recovery prompt must remain available after startup"
    );
    assert_eq!(app.document.geometry_snapshot(), before);
    app.ui.initialize_startup_notice();
    assert_eq!(
        app.ui.startup_notice_open,
        !env!("CARGO_PKG_VERSION_PRE").is_empty()
    );

    // Exercise the actual checkbox, Continue button and immediate disk save.
    app.ui.startup_notice_open = true;
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, "Don’t show again for this version");
    click(&mut app, &ctx, "Continue to Clay Form");
    let mut restarted = App::default();
    restarted.ui.preferences = preferences::load(&path).unwrap();
    restarted.ui.initialize_startup_notice();
    assert!(!restarted.ui.startup_notice_open);
    restarted.ui.preferences.dismissed_startup_version = "previous-version".into();
    restarted.ui.initialize_startup_notice();
    assert_eq!(
        restarted.ui.startup_notice_open,
        !env!("CARGO_PKG_VERSION_PRE").is_empty()
    );

    let mut old = serde_json::to_value(Preferences::default()).unwrap();
    old.as_object_mut()
        .unwrap()
        .remove("dismissed_startup_version");
    let migrated: Preferences = serde_json::from_value(old).unwrap();
    assert!(migrated.dismissed_startup_version.is_empty());
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "manual startup notice visual review; requires Vulkan"]
fn startup_notice_offscreen_review() {
    let mut capture = crate::viewport::UiCapture::new([1000, 760]);
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.ui.startup_notice_open = true;
    for _ in 0..4 {
        let output = frame(&mut app, &ctx, vec![]);
        let rgba = capture.render(&ctx, output);
        capture.save("/tmp/clay-startup.png", &rgba);
    }
}

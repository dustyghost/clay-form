use super::*;

fn about_frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
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
            app.ui.toolbar(ctx, &app.document, &app.camera);
            app.shortcuts(ctx);
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            app.ui.about(ctx);
        },
    )
}

#[test]
fn about_button_opens_ownership_and_licences_without_editing_document() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    about_frame(&mut app, &ctx, vec![]);
    let output = about_frame(&mut app, &ctx, vec![]);
    let position = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "About" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("About button must be reachable");
    about_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(position),
            primary_event(position, true),
        ],
    );
    about_frame(&mut app, &ctx, vec![primary_event(position, false)]);
    assert!(app.ui.about_open);
    about_frame(&mut app, &ctx, vec![]);
    let output = about_frame(&mut app, &ctx, vec![]);
    assert!(output.shapes.iter().any(|s| matches!(&s.shape,
        egui::Shape::Text(t) if t.galley.text() == "Your creations belong to you")));
    // A click behind the window cannot sculpt while the licence window is open.
    let center = Pos2::new(500., 380.);
    about_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    about_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::new(50., 0.))],
    );
    about_frame(&mut app, &ctx, vec![primary_event(center, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_undo());
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "manual About window visual review; requires Vulkan"]
fn about_window_offscreen_review() {
    let mut capture = crate::viewport::UiCapture::new([1000, 760]);
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.viewport.initialize_renderer(&capture.state).unwrap();
    app.ui.about_open = true;
    for _ in 0..4 {
        let output = about_frame(&mut app, &ctx, vec![]);
        let rgba = capture.render(&ctx, output);
        capture.save("/tmp/clay-about.png", &rgba);
    }
}

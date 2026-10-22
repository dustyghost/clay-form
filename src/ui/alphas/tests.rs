use super::*;

#[test]
fn library_resizes_below_other_scrolled_sidebar_content() {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    let mut state = UiState::default();
    let document = Document::default();
    let mut brush = document.brushes()[0].clone();
    let mut frame = |events| {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(300., 640.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .vertical_scroll_offset(400.)
                        .show(ui, |ui| {
                            // Earlier panels consume the parent's suggested height even
                            // after they have been scrolled out of view.
                            ui.add_space(400.);
                            state.alpha_panel(ui, &document, 0, &mut brush, &mut Vec::new());
                            ui.add_space(500.);
                        });
                });
            },
        );
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Drag to resize library" => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("resize handle is visible in the scrolled sidebar")
    };
    frame(vec![]);
    let mut start = frame(vec![]);
    let pointer = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    for delta in [70., -140.] {
        frame(vec![egui::Event::PointerMoved(start - Vec2::new(0., 100.))]);
        frame(vec![egui::Event::PointerMoved(start), pointer(start, true)]);
        let end = start + Vec2::new(0., delta);
        // Real drags arrive over multiple frames, not as one large pointer jump.
        for step in 1..=10 {
            frame(vec![egui::Event::PointerMoved(
                start.lerp(end, step as f32 / 10.),
            )]);
        }
        frame(vec![pointer(end, false)]);
        let resized = frame(vec![]);
        assert!(
            (resized.y - start.y - delta).abs() < 2.,
            "the library must resize by {delta} pixels despite preceding sidebar content: {start:?} -> {resized:?}"
        );
        start = resized;
    }
}

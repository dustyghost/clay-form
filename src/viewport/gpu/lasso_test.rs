//! Render the lasso controls and live outline through the real GPU path.
use super::{ui_capture::UiCapture, *};
use crate::{command::Command, ui::UiState, viewport::SelectionTool};

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_lasso_preview_and_commit() {
    let size = [1000, 800];
    let mut capture = UiCapture::new(size);
    for (tool, alt, name) in [
        (SelectionTool::MaskLasso, false, "mask"),
        (SelectionTool::Lasso, true, "hide"),
    ] {
        let ctx = egui::Context::default();
        let mut app = crate::App::default();
        app.viewport.initialize_renderer(&capture.gl).unwrap();
        app.dispatch(Command::SelectTool(Some(tool)), &ctx);
        let mut controls = UiState::default();
        controls.selection_tool = Some(tool);
        let before = app.document.geometry_snapshot();
        let points = [
            egui::pos2(500., 240.),
            egui::pos2(760., 240.),
            egui::pos2(760., 430.),
            egui::pos2(630., 430.),
            egui::pos2(630., 660.),
            egui::pos2(500., 660.),
            egui::pos2(500., 240.),
        ];
        let modifiers = if alt {
            egui::Modifiers::ALT
        } else {
            egui::Modifiers::NONE
        };
        let button = |pressed| egui::Event::PointerButton {
            pos: points[0],
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        let mut preview = Vec::new();
        for frame in 0..6 {
            let events = match frame {
                1 => vec![egui::Event::PointerMoved(points[0]), button(true)],
                2 => points[1..]
                    .iter()
                    .copied()
                    .map(egui::Event::PointerMoved)
                    .collect(),
                4 => vec![button(false)],
                _ => vec![],
            };
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(size[0] as f32, size[1] as f32),
                    )),
                    modifiers,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    controls.toolbar(ctx, &app.document, app.camera.zoom);
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
            let rgba = capture.render(&ctx, output);
            if frame == 3 || frame == 5 {
                if frame == 3 {
                    assert_eq!(app.document.geometry_snapshot(), before);
                    preview = rgba.clone();
                } else {
                    assert_ne!(app.document.geometry_snapshot(), before);
                    assert_ne!(rgba, preview);
                    assert_eq!(app.document.history_counts(), (1, 0));
                }
                if let Ok(path) = std::env::var("CLAY_LASSO_PREVIEW") {
                    capture.save(format!("{path}-{name}-{frame}.png"), &rgba);
                }
            }
        }
        app.viewport.destroy_renderer(&capture.gl);
    }
}

//! Render the lasso controls and live outline through the real GPU path.
use super::{ui_capture::UiCapture, *};
use crate::{command::Command, ui::UiState, viewport::SelectionTool};

#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn gpu_lasso_preview_and_commit() {
    let size = [1000, 800];
    let mut capture = UiCapture::new(size);
    for (tool, alt, name) in [
        (SelectionTool::MaskLasso, false, "mask"),
        (SelectionTool::Lasso, true, "hide"),
    ] {
        let ctx = egui::Context::default();
        let mut app = crate::App::default();
        app.viewport.initialize_renderer(&capture.state).unwrap();
        app.dispatch(Command::SelectTool(Some(tool)), &ctx);
        let mut controls = UiState::default();
        controls.selection_tool = Some(tool);
        let before = app.document.geometry_snapshot();
        let mut points = [egui::Pos2::ZERO; 7];
        let modifiers = if alt {
            egui::Modifiers::ALT
        } else {
            egui::Modifiers::NONE
        };
        let mut preview = Vec::new();
        // Two idle frames settle egui's wrapped toolbar before positioning the gesture.
        for frame in 0..7 {
            let button = |pressed| egui::Event::PointerButton {
                pos: points[0],
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
            };
            let events = match frame {
                2 => vec![egui::Event::PointerMoved(points[0]), button(true)],
                3 => points[1..]
                    .iter()
                    .copied()
                    .map(egui::Event::PointerMoved)
                    .collect(),
                5 => vec![button(false)],
                _ => vec![],
            };
            let mut viewport_rect = egui::Rect::NOTHING;
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
                    controls.toolbar(ctx, &app.document, &app.camera);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        viewport_rect = ui.available_rect_before_wrap();
                        app.show_viewport(ui);
                    });
                },
            );
            if frame == 1 {
                points = [
                    [0.50, 0.12],
                    [0.76, 0.12],
                    [0.76, 0.48],
                    [0.63, 0.48],
                    [0.63, 0.80],
                    [0.50, 0.80],
                    [0.50, 0.12],
                ]
                .map(|[x, y]| viewport_rect.lerp_inside(egui::vec2(x, y)));
            }
            let rgba = capture.render(&ctx, output);
            if frame == 4 || frame == 6 {
                if frame == 4 {
                    assert!(
                        app.viewport.selection.is_some(),
                        "lasso must be active before release"
                    );
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
        app.viewport.destroy_renderer();
    }
}

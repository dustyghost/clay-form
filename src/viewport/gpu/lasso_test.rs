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
                        app.viewport.session.selection.is_some(),
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

#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn gpu_line_mask_preview_front_back_and_undo() {
    let size = [1000, 800];
    let mut capture = UiCapture::new(size);
    for perspective in [false, true] {
        let ctx = egui::Context::default();
        let mut app = crate::App::default();
        app.camera.perspective = perspective;
        app.viewport.initialize_renderer(&capture.state).unwrap();
        let tool = SelectionTool::MaskLine {
            erase: false,
            flip: false,
        };
        app.dispatch(Command::SelectTool(Some(tool)), &ctx);
        let before = app.document.geometry_snapshot();
        let mut controls = UiState::default();
        controls.selection_tool = Some(tool);
        let mut start = egui::Pos2::ZERO;
        let mut end = start;
        for frame in 0..10 {
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let events = match frame {
                2 => vec![egui::Event::PointerMoved(start), button(start, true)],
                3 => vec![egui::Event::PointerMoved(end)],
                5 => vec![button(end, false)],
                _ => vec![],
            };
            if frame == 7 {
                app.camera.yaw = std::f32::consts::PI;
            }
            if frame == 9 {
                app.dispatch(Command::Undo, &ctx);
            }
            let mut rect = egui::Rect::NOTHING;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(size[0] as f32, size[1] as f32),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    controls.toolbar(ctx, &app.document, &app.camera);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rect = ui.available_rect_before_wrap();
                        app.show_viewport(ui);
                    });
                },
            );
            if frame == 1 {
                start = rect.lerp_inside(egui::vec2(0.5, 0.06));
                end = rect.lerp_inside(egui::vec2(0.5, 0.94));
            }
            let rgba = capture.render(&ctx, output);
            if frame == 4 {
                assert!(app.viewport.session.selection.is_some());
                assert_eq!(app.document.geometry_snapshot(), before);
            }
            if frame == 6 || frame == 8 {
                assert_eq!(app.document.history_counts(), (1, 0));
                assert_ne!(app.document.geometry_snapshot(), before);
                // Both exposed hemispheres must contain mask shading.
                let mask = &app.document.mesh().mask;
                let view = app.camera.basis().2;
                assert!(
                    app.document
                        .mesh()
                        .vertices
                        .iter()
                        .enumerate()
                        .any(|(i, p)| p.dot(view) > 0.5 && mask[i] == 1.)
                );
            }
            if frame == 9 {
                assert_eq!(app.document.geometry_snapshot(), before);
            }
            if [4, 6, 8, 9].contains(&frame)
                && let Ok(path) = std::env::var("CLAY_LINE_MASK_PREVIEW")
            {
                capture.save(
                    format!(
                        "{path}-{}-{frame}.png",
                        if perspective { "perspective" } else { "ortho" }
                    ),
                    &rgba,
                );
            }
        }
        app.viewport.destroy_renderer();
    }
}

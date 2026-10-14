//! Render the lasso controls and live outline through the real GPU path.
use super::{tests::Egl, *};
use crate::{command::Command, ui::UiState, viewport::SelectionTool};

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_lasso_preview_and_commit() {
    unsafe {
        let egl = Egl::new();
        let gl = Arc::new(glow::Context::from_loader_function(|name| {
            (egl.get_proc)(std::ffi::CString::new(name).unwrap().as_ptr())
        }));
        let size = [1000, 800];
        let texture = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            size[0],
            size[1],
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        let framebuffer = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        assert_eq!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER),
            glow::FRAMEBUFFER_COMPLETE
        );
        let mut painter = eframe::egui_glow::Painter::new(gl.clone(), "", None, false).unwrap();
        for (tool, alt, name) in [
            (SelectionTool::MaskLasso, false, "mask"),
            (SelectionTool::Lasso, true, "hide"),
        ] {
            let ctx = egui::Context::default();
            let mut app = crate::App::default();
            app.viewport.initialize_renderer(&gl).unwrap();
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
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
                gl.clear_color(0.07, 0.07, 0.07, 1.);
                gl.clear(glow::COLOR_BUFFER_BIT);
                let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
                painter.paint_and_update_textures(
                    [size[0] as u32, size[1] as u32],
                    output.pixels_per_point,
                    &shapes,
                    &output.textures_delta,
                );
                assert_eq!(gl.get_error(), glow::NO_ERROR);
                if frame == 3 || frame == 5 {
                    let mut rgba = vec![0; (size[0] * size[1] * 4) as usize];
                    gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(framebuffer));
                    gl.read_pixels(
                        0,
                        0,
                        size[0],
                        size[1],
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelPackData::Slice(Some(&mut rgba)),
                    );
                    if frame == 3 {
                        assert_eq!(app.document.geometry_snapshot(), before);
                        preview = rgba.clone();
                    } else {
                        assert_ne!(app.document.geometry_snapshot(), before);
                        assert_ne!(rgba, preview);
                        assert_eq!(app.document.history_counts(), (1, 0));
                    }
                    if let Ok(path) = std::env::var("CLAY_LASSO_PREVIEW") {
                        let flipped: Vec<_> = rgba
                            .chunks_exact(size[0] as usize * 4)
                            .rev()
                            .flatten()
                            .copied()
                            .collect();
                        image::save_buffer(
                            format!("{path}-{name}-{frame}.png"),
                            &flipped,
                            size[0] as u32,
                            size[1] as u32,
                            image::ColorType::Rgba8,
                        )
                        .unwrap();
                    }
                }
            }
            app.viewport.destroy_renderer(&gl);
        }
        painter.destroy();
        gl.delete_framebuffer(framebuffer);
        gl.delete_texture(texture);
    }
}

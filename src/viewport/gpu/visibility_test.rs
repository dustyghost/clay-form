use super::{tests::Egl, *};
use crate::camera::Camera;

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_visibility_hide_split_and_undo() {
    unsafe {
        let egl = Egl::new();
        let gl = glow::Context::from_loader_function(|name| {
            let name = std::ffi::CString::new(name).unwrap();
            (egl.get_proc)(name.as_ptr())
        });
        let mut document = Document::default();
        let mut renderer = Renderer::new(&gl).unwrap();
        let mut frames = Vec::new();
        for phase in 0..4 {
            match phase {
                1 => document.select_face_region(|p| p.0 > 0., true).unwrap(),
                2 => document.split_hidden().unwrap(),
                3 => document.undo(),
                _ => (),
            }
            let display = renderer.geometry(&document);
            let (right, up, view) = Camera::default().basis();
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(512.));
            let params = Params::new(
                rect,
                right,
                up,
                view,
                180.,
                rect.center(),
                display.depth_extent,
            );
            let mut gpu = renderer.gpu.lock().unwrap();
            gpu.upload(&gl, &display);
            gpu.paint(
                &gl,
                &params,
                [512, 512],
                false,
                WireMode::Visible,
                false,
                &Preview::default(),
            );
            let mut pixels = vec![0; 512 * 512 * 4];
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(gpu.target.as_ref().unwrap().framebuffer),
            );
            gl.read_pixels(
                0,
                0,
                512,
                512,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            assert_eq!(gl.get_error(), glow::NO_ERROR);
            let pixel = |x: usize| &pixels[(256 * 512 + x) * 4..(256 * 512 + x) * 4 + 4];
            let background = &[23, 26, 31, 255];
            assert_ne!(pixel(180), background, "visible half must render");
            if phase == 1 || phase == 3 {
                assert_eq!(pixel(330), background, "hidden half must be absent");
            } else {
                assert_ne!(pixel(330), background, "split restores both surfaces");
            }
            frames.push(pixels);
        }
        assert_eq!(frames[1], frames[3], "undo must restore the hidden display");
        if let Ok(path) = std::env::var("CLAY_VISIBILITY_PREVIEW") {
            let mut pixels = Vec::with_capacity(512 * 512 * 4 * 4);
            for y in (0..512).rev() {
                for frame in &frames {
                    pixels.extend_from_slice(&frame[y * 512 * 4..(y + 1) * 512 * 4]);
                }
            }
            image::save_buffer(path, &pixels, 2048, 512, image::ColorType::Rgba8).unwrap();
        }
        renderer.destroy(&gl);
    }
}

//! Shared offscreen egui/OpenGL capture for interactive-control regressions.
use super::tests::Egl;
use eframe::{
    egui,
    egui_glow::Painter,
    glow::{self, HasContext},
};
use std::sync::Arc;

pub(crate) struct UiCapture {
    pub gl: Arc<glow::Context>,
    painter: Painter,
    size: [i32; 2],
    framebuffer: glow::Framebuffer,
    texture: glow::Texture,
    _egl: Egl,
}
impl UiCapture {
    pub fn new(size: [i32; 2]) -> Self {
        unsafe {
            let egl = Egl::new();
            let gl = Arc::new(glow::Context::from_loader_function(|name| {
                (egl.get_proc)(std::ffi::CString::new(name).unwrap().as_ptr())
            }));
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
            let painter = Painter::new(gl.clone(), "", None, false).unwrap();
            Self {
                gl,
                painter,
                size,
                framebuffer,
                texture,
                _egl: egl,
            }
        }
    }
    pub fn render(&mut self, ctx: &egui::Context, output: egui::FullOutput) -> Vec<u8> {
        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
            self.gl.clear_color(0.07, 0.07, 0.07, 1.);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
            self.painter.paint_and_update_textures(
                self.size.map(|n| n as u32),
                output.pixels_per_point,
                &shapes,
                &output.textures_delta,
            );
            let mut rgba = vec![0; (self.size[0] * self.size[1] * 4) as usize];
            self.gl
                .bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.framebuffer));
            self.gl.read_pixels(
                0,
                0,
                self.size[0],
                self.size[1],
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut rgba)),
            );
            assert_eq!(self.gl.get_error(), glow::NO_ERROR);
            rgba
        }
    }
    pub fn save(&self, path: impl AsRef<std::path::Path>, rgba: &[u8]) {
        let flipped: Vec<_> = rgba
            .chunks_exact(self.size[0] as usize * 4)
            .rev()
            .flatten()
            .copied()
            .collect();
        image::save_buffer(
            path,
            &flipped,
            self.size[0] as u32,
            self.size[1] as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
impl Drop for UiCapture {
    fn drop(&mut self) {
        self.painter.destroy();
        unsafe {
            self.gl.delete_framebuffer(self.framebuffer);
            self.gl.delete_texture(self.texture);
        }
    }
}

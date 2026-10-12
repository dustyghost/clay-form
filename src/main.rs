use clay_form::App;
use eframe::egui;
fn main() -> eframe::Result {
    eframe::run_native(
        "Clay Form",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Glow,
            viewport: egui::ViewportBuilder::default().with_inner_size([1280., 850.]),
            ..Default::default()
        },
        Box::new(|cc| {
            let gl = cc.gl.as_ref().ok_or("OpenGL renderer is required")?;
            Ok(Box::new(App::with_gl(gl)?))
        }),
    )
}

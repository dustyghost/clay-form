#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clay_form::App;
use eframe::egui;
fn main() -> eframe::Result {
    let title = format!("Clay Form {}", env!("CARGO_PKG_VERSION"));
    let mut options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([1280., 850.]),
        ..Default::default()
    };
    clay_form::configure_native_input(&mut options);
    eframe::run_native(
        "Clay Form",
        options,
        Box::new(|cc| {
            let state = cc
                .wgpu_render_state
                .as_ref()
                .ok_or("wgpu renderer is required")?;
            Ok(Box::new(App::with_wgpu(state)?))
        }),
    )
}

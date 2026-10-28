#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clay_form::App;
use eframe::egui;
fn main() -> eframe::Result {
    eframe::run_native(
        "Clay Form",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([1280., 850.]),
            ..Default::default()
        },
        Box::new(|cc| {
            let state = cc
                .wgpu_render_state
                .as_ref()
                .ok_or("wgpu renderer is required")?;
            Ok(Box::new(App::with_wgpu(state)?))
        }),
    )
}

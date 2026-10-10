use clay_form::App;
use eframe::egui;
fn main() -> eframe::Result {
    eframe::run_native(
        "Clay Form",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1280., 850.]),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::<App>::default())),
    )
}

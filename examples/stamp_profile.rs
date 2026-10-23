//! Manual release-mode stamp timings. Optional output captures exact geometry.
use clay_form::{
    document::Document,
    sculpt::{AdaptiveSettings, Brush, Dab, Kind, StrokeMode, Vec3, alpha},
};
use std::{error::Error, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let mut brush = Brush::new(Kind::Standard);
    brush.stroke_mode = StrokeMode::DragRect;
    brush.radius = 252.10057;
    brush.rotation = 55.801712;
    brush.alpha = alpha::builtins()
        .iter()
        .find(|a| a.name == "Reactor rings")
        .unwrap()
        .image
        .clone();
    let dab = Dab {
        center: Vec3(-0.18910993, 0.22439388, 0.9556),
        normal: Vec3(-0.18930681, 0.22462495, 0.955_88),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 0.8365707,
        pressure: 1.,
        delta: Vec3::default(),
        invert: false,
    };
    for iteration in 0..4 {
        let mut document = Document::default();
        document.set_adaptive(AdaptiveSettings {
            enabled: true,
            ..Default::default()
        })?;
        let started = Instant::now();
        assert!(document.apply_dab(&brush, dab));
        document.end_stroke();
        println!(
            "iteration={iteration}, stamp_ms={:.2}, vertices={}",
            started.elapsed().as_secs_f64() * 1000.,
            document.mesh().vertices.len()
        );
        if iteration == 0
            && let Some(path) = std::env::args().nth(1)
        {
            std::fs::write(path, serde_json::to_vec(document.mesh())?)?;
        }
    }
    Ok(())
}

//! Focused Detail path profile; --synthetic needs no private export.
//! CLAY_PROFILE_AUTOMATIC=1 also compares a complete automatic stroke to live replay.
use clay_form::{
    document::{Document, PathStrokeOptions},
    sculpt::{
        AdaptiveSettings, Brush, Kind, Primitive, PrimitiveKind, Vec3,
        paths::{PathMode, PathPoint},
    },
};
use std::{error::Error, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let filename = args
        .get(1)
        .ok_or("usage: path_profile PROJECT RADIUS DABS [OUTPUT]")?;
    let radius: f32 = args.get(2).ok_or("missing radius")?.parse()?;
    let limit: usize = args
        .get(3)
        .ok_or("missing dab limit (0 = full path)")?
        .parse()?;
    let mut document = Document::default();
    if filename == "--synthetic" {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
        let divisions = std::env::var("CLAY_PROFILE_DIVISIONS")
            .unwrap_or_else(|_| "48".into())
            .parse()?;
        primitive.divisions = [divisions, divisions, 2];
        let mut project = document.project();
        project.mesh = primitive.mesh()?;
        document.replace_project(project, "synthetic.clay")?;
        document.set_adaptive(AdaptiveSettings {
            enabled: true,
            ..Default::default()
        })?;
        document.create_path(
            "Profile curve".into(),
            PathMode::Surface,
            [Vec3(-0.6, -0.2, 1.), Vec3(0., 0.2, 1.), Vec3(0.6, -0.2, 1.)]
                .into_iter()
                .map(|position| PathPoint {
                    position,
                    normal: Vec3(0., 0., 1.),
                })
                .collect(),
            false,
            0.5,
        )?;
    } else {
        document.replace_project(clay_form::persistence::load_project(filename)?, filename)?;
    }
    if let Ok(source) = std::env::var("CLAY_PROFILE_SOURCE") {
        serde_json::to_writer(std::fs::File::create(source)?, &document.project())?;
    }
    let id = document.paths().first().ok_or("project has no path")?.id;
    let brush = Brush::new(Kind::Detail);
    let options = PathStrokeOptions {
        radius,
        ..Default::default()
    };
    let started = Instant::now();
    let samples = document.path_samples(id, radius * brush.spacing)?;
    println!(
        "samples={}, sampling_ms={:.3}",
        samples.len(),
        started.elapsed().as_secs_f64() * 1000.
    );
    let mut session = document
        .prepare_path_stroke(id, &brush, options)?
        .begin_live();
    let started = Instant::now();
    let mut chunk = started;
    for (index, sample) in samples
        .iter()
        .take(if limit == 0 { samples.len() } else { limit })
        .enumerate()
    {
        session.begin_frame();
        session.apply_sample(*sample, options.pressure, options.invert)?;
        if (index + 1) % 32 == 0 {
            println!(
                "dabs={}, vertices={}, last32_ms={:.3}, total_ms={:.3}",
                index + 1,
                session.mesh().vertices.len(),
                chunk.elapsed().as_secs_f64() * 1000.,
                started.elapsed().as_secs_f64() * 1000.
            );
            chunk = Instant::now();
        }
    }
    let dabs_ms = started.elapsed().as_secs_f64() * 1000.;
    let started = Instant::now();
    let preview = session.finish()?;
    println!(
        "dabs_ms={dabs_ms:.3}, finish_ms={:.3}, vertices={}, warning={:?}",
        started.elapsed().as_secs_f64() * 1000.,
        preview.mesh().vertices.len(),
        preview.attachment_warning()
    );
    if std::env::var_os("CLAY_PROFILE_AUTOMATIC").is_some() {
        if limit != 0 {
            return Err("automatic comparison requires DABS=0 (the complete path)".into());
        }
        let started = Instant::now();
        let automatic = document
            .prepare_path_stroke(id, &brush, options)?
            .run(&|| false)?;
        println!(
            "automatic_total_ms={:.3}, same_mesh={}, same_attachment_warning={}",
            started.elapsed().as_secs_f64() * 1000.,
            automatic.mesh() == preview.mesh(),
            automatic.attachment_warning() == preview.attachment_warning()
        );
        assert!(
            automatic.mesh() == preview.mesh(),
            "Automatic and live dabs must produce identical geometry"
        );
    }
    document.apply_path_stroke(preview)?;
    if let Some(output) = args.get(4) {
        serde_json::to_writer(std::fs::File::create(output)?, &document.project())?;
    }
    Ok(())
}

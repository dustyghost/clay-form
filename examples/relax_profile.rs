//! Focused Relax input-cost profile without rendering or private geometry.
use clay_form::{
    document::Document,
    sculpt::{Brush, Dab, Kind, Primitive, PrimitiveKind, Vec3},
};
use std::{error::Error, time::Instant};
fn main() -> Result<(), Box<dyn Error>> {
    let divisions: u32 = std::env::args()
        .nth(1)
        .as_deref()
        .unwrap_or("128")
        .parse()?;
    let mut p = Primitive::for_kind(PrimitiveKind::Cube);
    let depth: u32 = std::env::args().nth(2).as_deref().unwrap_or("2").parse()?;
    p.divisions = [divisions, divisions, depth];
    let mut mesh = p.mesh()?;
    for v in &mut mesh.vertices {
        if v.2 > 0.99 && v.0.abs() < 0.9 && v.1.abs() < 0.9 {
            v.0 += 0.2 / divisions as f32 * (v.1 * 25.).sin();
        }
    }
    let mut doc = Document::default();
    let mut project = doc.project();
    project.mesh = mesh;
    doc.replace_project(project, "profile.clay")?;
    let brush = Brush::new(Kind::Relax);
    let dab = Dab {
        center: Vec3(0., 0., 1.),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 0.25,
        pressure: 1.,
        delta: Vec3::default(),
        invert: false,
    };
    let mut times = Vec::new();
    for _ in 0..13 {
        let start = Instant::now();
        doc.apply_dab(&brush, dab);
        times.push(start.elapsed().as_secs_f64() * 1000.);
    }
    doc.end_stroke();
    println!(
        "vertices={}, first_ms={:.3}",
        doc.mesh().vertices.len(),
        times[0]
    );
    times.remove(0);
    times.sort_by(f64::total_cmp);
    println!(
        "cached_median_ms={:.3}, cached_max_ms={:.3}, undo_steps={}",
        times[times.len() / 2],
        times[times.len() - 1],
        doc.history_counts().0
    );
    Ok(())
}

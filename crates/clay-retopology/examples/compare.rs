//! Read one Clay mesh and write an OBJ for an independent solver comparison.
use clay_retopology::{
    Options, Surface, Vec3, reconstruct_regularized_with_progress, reconstruct_with_progress,
};
use std::{io::Write, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(args.len() == 4 || (args.len() == 5 && args[4] == "--regularize")) {
        return Err("usage: compare source.clay output.obj target-quads [--regularize]".into());
    }
    let source = std::fs::canonicalize(&args[1])?;
    if std::path::Path::new(&args[2]).exists() {
        return Err("Output already exists; choose a new comparison file".into());
    }
    let doc: serde_json::Value =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(source)?))?;
    let mesh = &doc["mesh"];
    let positions: Vec<[f64; 3]> = serde_json::from_value(mesh["vertices"].clone())?;
    let triangles: Vec<[usize; 3]> = serde_json::from_value(mesh["triangles"].clone())?;
    let surface = Surface {
        positions: positions.into_iter().map(Vec3::from_array).collect(),
        triangles,
    };
    drop(doc);
    let options = Options {
        target_quads: args[3].parse()?,
        preserve_sharp: false,
        max_vertices: 150_000,
        ..Options::default()
    };
    let start = Instant::now();
    let reconstruct = if args.len() == 5 {
        reconstruct_regularized_with_progress
    } else {
        reconstruct_with_progress
    };
    let result = reconstruct(&surface, &options, &|| false, &|stage| {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        let memory = status
            .lines()
            .find(|l| l.starts_with("VmRSS:"))
            .unwrap_or("");
        eprintln!("{:.3}s {stage:?} {memory}", start.elapsed().as_secs_f64());
    });
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let peak = status
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .unwrap_or("");
    eprintln!("{:.3}s finished · {peak}", start.elapsed().as_secs_f64());
    let result = result?;
    eprintln!(
        "{:.3}s: {} vertices, {} quads; {:?}",
        start.elapsed().as_secs_f64(),
        result.positions.len(),
        result.quads.len(),
        result.diagnostics
    );
    let mut out = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&args[2])?,
    );
    for p in result.positions {
        writeln!(out, "v {} {} {}", p.x, p.y, p.z)?;
    }
    for [a, b, c, d] in result.quads {
        writeln!(out, "f {} {} {} {}", a + 1, b + 1, c + 1, d + 1)?;
    }
    Ok(())
}

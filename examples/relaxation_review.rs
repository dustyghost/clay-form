//! Reproduce relaxation measurements without opening or altering the source project.
//! cargo run --example relaxation_review -- input.clay output.clay [strength iterations tolerance]
use clay_form::{
    persistence::{load_project, save_project},
    sculpt::{Mesh, RelaxationOptions},
};

fn describe(mesh: &Mesh) {
    let mut aspects: Vec<f32> = mesh
        .quads
        .iter()
        .map(|q| {
            let lengths = std::array::from_fn::<_, 4, _>(|i| {
                (mesh.vertices[q[i]] - mesh.vertices[q[(i + 1) % 4]]).len()
            });
            lengths.into_iter().fold(0., f32::max)
                / lengths.into_iter().fold(f32::INFINITY, f32::min)
        })
        .collect();
    aspects.sort_by(f32::total_cmp);
    if !aspects.is_empty() {
        println!(
            "Quad edge ratio: mean {:.6}, 95th percentile {:.6}",
            aspects.iter().sum::<f32>() / aspects.len() as f32,
            aspects[aspects.len() * 95 / 100]
        );
    }
    let mut areas = vec![0.; mesh.vertices.len()];
    let mut volume = 0.;
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| mesh.vertices[i]);
        let area = (b - a).cross(c - a).len() / 6.;
        for &i in t {
            areas[i] += area;
        }
        volume += a.dot(b.cross(c)) / 6.;
    }
    let mean = areas.iter().sum::<f32>() / areas.len().max(1) as f32;
    if mean > 0. {
        println!(
            "Vertex area coefficient of variation: {:.6}",
            (areas.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / areas.len() as f32).sqrt()
                / mean
        );
    }
    println!(
        "{} vertices, {} triangles, signed volume {:.6}",
        mesh.vertices.len(),
        mesh.triangles.len(),
        volume
    );
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 && args.len() != 6 {
        return Err(
            "Usage: relaxation_review input.clay output.clay [strength iterations tolerance]"
                .into(),
        );
    }
    let input = std::path::Path::new(&args[1])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if std::path::Path::new(&args[2])
        .canonicalize()
        .is_ok_and(|p| p == input)
    {
        return Err("Choose a separate output file to preserve the input".into());
    }
    let options = if args.len() == 6 {
        RelaxationOptions {
            strength: args[3].parse().map_err(|_| "Invalid strength")?,
            iterations: args[4].parse().map_err(|_| "Invalid iterations")?,
            max_deviation: args[5].parse().map_err(|_| "Invalid tolerance")?,
        }
    } else {
        RelaxationOptions::default()
    };
    let mut project = load_project(&args[1])?;
    println!("Before:");
    describe(&project.mesh);
    let start = std::time::Instant::now();
    let (mesh, report) = project.mesh.relax_mesh(&project.mesh, options, &|| false)?;
    println!("After ({options:?}, {:?}):", start.elapsed());
    describe(&mesh);
    println!("{}", report.summary());
    project.mesh = mesh;
    save_project(&args[2], &project)
}

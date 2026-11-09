use super::*;
use crate::sculpt::paths::{PathMode, PathPoint, mesh_signature};

/// Two closed surfaces with exactly the same source connectivity. The cylinder
/// has sharp cap rims. The asymmetric bulb has a curved silhouette and poles.
pub(crate) fn fixture(bulb: bool) -> (Mesh, SurfacePath) {
    let (n, rows) = (32, 19);
    let mut mesh = Mesh::default();
    for row in 0..rows {
        let u = (row + 1) as f32 / (rows + 1) as f32;
        let latitude = std::f32::consts::PI * (u - 0.5);
        for j in 0..n {
            let angle = std::f32::consts::TAU * j as f32 / n as f32;
            let (s, c) = angle.sin_cos();
            let p = if bulb {
                let radius = latitude.cos()
                    * (0.94 - 0.27 * latitude.sin())
                    * (1. + 0.11 * (2. * angle + latitude).cos() * latitude.cos());
                Vec3(
                    radius * s + 0.13 * (2. * latitude).sin(),
                    1.3 * latitude.sin(),
                    radius * c,
                )
            } else {
                Vec3(s, 2.4 * (u - 0.5), c)
            };
            mesh.vertices.push(p);
        }
    }
    for i in 0..rows - 1 {
        for j in 0..n {
            let a = i * n + j;
            let b = i * n + (j + 1) % n;
            mesh.triangles.extend([[a, b, b + n], [a, b + n, a + n]]);
        }
    }
    let end = mesh.vertices.len();
    let y = if bulb { 1.3 } else { 1.08 };
    mesh.vertices.extend([Vec3(0., -y, 0.), Vec3(0., y, 0.)]);
    for j in 0..n {
        let k = (j + 1) % n;
        mesh.triangles.extend([
            [end, k, j],
            [end + 1, (rows - 1) * n + j, (rows - 1) * n + k],
        ]);
    }
    mesh.mask = mesh
        .vertices
        .iter()
        .map(|v| (-5. * v.1).clamp(0., 1.))
        .collect();
    let normals = mesh.normals();
    let points = (3..=15)
        .map(|i| {
            let v = i * n + (n + i - 9) % n;
            PathPoint {
                position: mesh.vertices[v],
                normal: normals[v],
            }
        })
        .collect();
    let path = SurfacePath {
        id: 1,
        name: "Diagonal".into(),
        target: 1,
        mode: PathMode::Surface,
        points,
        closed: false,
        smoothing: 0.,
        corner_rounding: Vec::new(),
        seam: 0,
        surface_signature: mesh_signature(&mesh),
        attached_route: None,
    };
    (mesh, path)
}

#[test]
fn native_retopology_solver_coordinates_match_project_export_without_losing_f32_precision() {
    let (mesh, _) = fixture(true);
    let positions = solver_positions(&mesh.vertices).unwrap();
    let encoded = serde_json::to_vec(&mesh).unwrap();
    let exported: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    for ((source, solver), saved) in mesh
        .vertices
        .iter()
        .zip(positions)
        .zip(exported["vertices"].as_array().unwrap())
    {
        assert_eq!(
            *source,
            Vec3(solver.x as f32, solver.y as f32, solver.z as f32)
        );
        assert_eq!(
            solver.to_array(),
            [
                saved[0].as_f64().unwrap(),
                saved[1].as_f64().unwrap(),
                saved[2].as_f64().unwrap()
            ]
        );
    }
}

#[test]
fn native_retopology_reconstructs_quads_without_using_the_sculpt_mask_as_density() {
    let (source, _) = fixture(true);
    let options = Options {
        target: 300,
        ..Default::default()
    };
    let first = source.guided_retopology(options, None, &|| false).unwrap();
    first.mesh.validate_closed().unwrap();
    assert_eq!(first.mesh.triangles.len(), first.mesh.quads.len() * 2);
    assert!((180..=420).contains(&first.mesh.quads.len()));
    let mut unmasked = source.clone();
    unmasked.mask.fill(0.);
    let second = unmasked
        .guided_retopology(options, None, &|| false)
        .unwrap();
    assert_eq!(first.mesh.vertices, second.mesh.vertices);
    assert_eq!(first.mesh.quads, second.mesh.quads);
    assert_ne!(first.mesh.mask, second.mesh.mask);
    assert_ne!(source.vertices, first.mesh.vertices);
}

#[test]
fn native_retopology_multiple_guides_mirror_and_stale_attachments() {
    let (source, path) = fixture(true);
    let mut second = path.clone();
    second.id = 2;
    second.points.reverse();
    let options = Options {
        target: 300,
        mirror_path_x: true,
        ..Default::default()
    };
    let result = source
        .guided_retopology_with_progress(options, &[path.clone(), second], &|| false, &|_| {})
        .unwrap();
    assert_eq!(result.guides.len(), 4);
    assert!((result.guides[0][0].position.0 + result.guides[1][0].position.0).abs() < 1e-6);
    let mut stale = source.clone();
    stale.vertices[0].0 += 0.1;
    assert!(
        stale
            .guided_retopology(options, Some(&path), &|| false)
            .is_err()
    );
}

#[test]
fn native_retopology_limits_and_worker_cancellation_do_not_change_source() {
    let (source, path) = fixture(true);
    let before = source.clone();
    assert!(
        source
            .guided_retopology(
                Options {
                    input_triangle_limit: source.triangles.len() - 1,
                    ..Default::default()
                },
                None,
                &|| false
            )
            .err()
            .unwrap()
            .contains("Input triangle limit")
    );
    let cancel = std::cell::Cell::new(false);
    let error = source
        .guided_retopology_with_progress(Options::default(), &[path], &|| cancel.get(), &|s| {
            if matches!(s, clay_retopology::Stage::Preparation { iteration: 1, .. }) {
                cancel.set(true);
            }
        })
        .err()
        .unwrap();
    assert!(error.contains("cancelled"));
    assert_eq!(source, before);
}

#[test]
fn density_paint_is_signed_persistent_and_independent_of_selection_groups_and_mask() {
    use crate::sculpt::{Brush, Dab, Kind};
    let (mut source, _) = fixture(true);
    let before = source.clone();
    let mut brush = Brush::new(Kind::Mask);
    brush.strength = 1.;
    brush.pressure = false;
    brush.alpha = crate::sculpt::Alpha::Hard;
    let normal = source.normals()[320];
    let dab = Dab {
        center: source.vertices[320],
        normal,
        right: Vec3(0., 1., 0.),
        up: normal.cross(Vec3(0., 1., 0.)),
        radius: 0.25,
        pressure: 1.,
        delta: Vec3::default(),
        invert: false,
    };
    for (mode, value) in [
        (DensityPaint::More, 1.),
        (DensityPaint::Less, -1.),
        (DensityPaint::Automatic, 0.),
    ] {
        assert!(
            source
                .paint_retopology_density(&brush, &[dab], mode)
                .unwrap()
        );
        assert!((source.retopology_detail()[320] - value).abs() < 1e-6);
        assert_eq!(source.mask, before.mask);
        assert_eq!(source.vertices, before.vertices);
        assert_eq!(source.triangles, before.triangles);
        let saved = serde_json::to_vec(&source).unwrap();
        let reopened: Mesh = serde_json::from_slice(&saved).unwrap();
        assert_eq!(source, reopened);
        source.validate().unwrap();
    }
    let before = source.clone();
    assert!(source.assign_mask_to_group(0, 0.5, false).is_err());
    assert!(source.remove_vertex_group(0).is_err());
    assert!(source.rename_vertex_group(0, "Selection".into()).is_err());
    assert_eq!(source, before);
}

#[test]
#[ignore = "whole-sculpt adapter validation; set CLAY_RETOPO_SOURCE and optionally CLAY_RETOPO_OUTPUT"]
fn native_retopology_whole_sculpt_adapter() {
    let path = std::env::var("CLAY_RETOPO_SOURCE").unwrap();
    let value: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(path).unwrap()).unwrap();
    let source: Mesh = serde_json::from_value(value["mesh"].clone()).unwrap();
    let start = std::time::Instant::now();
    let result = source
        .guided_retopology_with_progress(
            Options {
                target: std::env::var("CLAY_RETOPO_TARGET")
                    .ok()
                    .map(|s| s.parse().unwrap())
                    .unwrap_or(10000),
                ..Default::default()
            },
            &[],
            &|| false,
            &|s| eprintln!("{:.3}s {s:?}", start.elapsed().as_secs_f64()),
        )
        .unwrap();
    result.mesh.validate_closed().unwrap();
    eprintln!(
        "{:.3}s {} quads",
        start.elapsed().as_secs_f64(),
        result.mesh.quads.len()
    );
    if let Ok(path) = std::env::var("CLAY_RETOPO_OUTPUT") {
        use std::io::Write;
        let mut output = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(path)
                .unwrap(),
        );
        for p in result.mesh.vertices {
            writeln!(output, "v {} {} {}", p.0, p.1, p.2).unwrap();
        }
        for q in result.mesh.quads {
            writeln!(
                output,
                "f {} {} {} {}",
                q[0] + 1,
                q[1] + 1,
                q[2] + 1,
                q[3] + 1
            )
            .unwrap();
        }
    }
}

use super::*;
use crate::sculpt::{
    paths::{PathMode, PathPoint, mesh_signature},
    projection::Surface as Reference,
};

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
fn surface_error(from: &Mesh, to: &Mesh) -> f32 {
    let surface = Reference::new(to, &|| false).unwrap();
    let mut budget = 20_000_000;
    let mut worst = 0_f32;
    for t in &from.triangles {
        let [a, b, c] = t.map(|i| from.vertices[i]);
        for p in [
            a,
            b,
            c,
            (a + b) * 0.5,
            (b + c) * 0.5,
            (c + a) * 0.5,
            (a + b + c) * (1. / 3.),
        ] {
            let d = surface
                .nearest(p, 5., None, &mut budget)
                .unwrap()
                .unwrap()
                .2;
            worst = worst.max(d);
        }
    }
    worst
}
fn coverage(mesh: &Mesh, samples: &[crate::sculpt::paths::PathSample]) -> f32 {
    let edges = mesh.polygon_edges();
    let aligned = samples
        .iter()
        .filter(|s| {
            edges.iter().any(|&[a, b]| {
                let p = mesh.vertices[a];
                let e = mesh.vertices[b] - p;
                let t = ((s.position - p).dot(e) / e.dot(e)).clamp(0., 1.);
                e.norm().dot(s.tangent).abs() >= 15_f32.to_radians().cos()
                    && (p + e * t - s.position).len() <= 0.065
            })
        })
        .count();
    100. * aligned as f32 / samples.len() as f32
}
#[test]
fn guided_retopology_compares_flow_and_heat_independently() {
    for bulb in [false, true] {
        let (source, path) = fixture(bulb);
        let samples = path.samples_on_mesh(&source, 0.018).unwrap();
        let mut scores = Vec::new();
        for (flow, density) in [(0., 0.), (1., 0.), (0., 1.), (1., 1.)] {
            let start = std::time::Instant::now();
            let layout = source
                .guided_retopology(
                    Options {
                        target: 460,
                        flow,
                        density,
                        ..Default::default()
                    },
                    Some(&path),
                    &|| false,
                )
                .unwrap();
            let elapsed = start.elapsed();
            let mesh = &layout.mesh;
            mesh.validate_closed().unwrap();
            Surface::new(mesh, &|| false).unwrap();
            let hot = mesh.vertices.iter().filter(|v| v.1 < -0.2).count();
            let along = coverage(mesh, &samples);
            let from = surface_error(&source, mesh);
            let to = surface_error(mesh, &source);
            println!(
                "bulb={bulb} flow={flow} heat={density} polygons={} quads={} hot={hot}/{} aligned={along:.1}% source→out={from:.5} out→source={to:.5} time={elapsed:.0?}",
                layout.polygons,
                mesh.quads.len(),
                mesh.vertices.len()
            );
            assert!(from.max(to) < 0.1);
            assert_eq!(layout.polygons, 460);
            assert!(mesh.quads.len() > layout.polygons / 2);
            scores.push((along, hot));
        }
        assert!(scores[1].0 > scores[0].0 + 5., "{scores:?}");
        assert!(scores[2].1 as f32 > scores[0].1 as f32 * 1.1, "{scores:?}");
    }
}

#[test]
fn guided_retopology_rejects_pinched_fans_and_cancels_inside_worker() {
    let (mesh, path) = fixture(false);
    let calls = std::cell::Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 25
    };
    assert!(
        mesh.guided_retopology(Options::default(), Some(&path), &cancelled)
            .err()
            .unwrap()
            .contains("cancelled")
    );
    let pinch = Mesh {
        vertices: vec![
            Vec3(0., 0., 0.),
            Vec3(1., 0., 0.),
            Vec3(0., 1., 0.),
            Vec3(-1., 0., 0.),
            Vec3(0., -1., 0.),
        ],
        triangles: vec![[0, 1, 2], [0, 3, 4]],
        mask: vec![0.; 5],
        ..Default::default()
    };
    assert!(
        pinch
            .guided_retopology(
                Options {
                    target: 1,
                    ..Default::default()
                },
                None,
                &|| false
            )
            .err()
            .unwrap()
            .contains("nonmanifold vertex")
    );
}

#[test]
#[ignore = "measures existing voxel algorithms on the retopology fixtures"]
fn guided_retopology_voxel_comparison() {
    use crate::sculpt::{QuadRemeshAlgorithm, Topology};
    for bulb in [false, true] {
        let (mesh, path) = fixture(bulb);
        let samples = path.samples_on_mesh(&mesh, 0.018).unwrap();
        for algorithm in [QuadRemeshAlgorithm::Fast, QuadRemeshAlgorithm::Optimized] {
            let start = std::time::Instant::now();
            let result =
                mesh.remesh_algorithm_cancellable(16, Topology::Quads, algorithm, &|| false);
            let elapsed = start.elapsed();
            match result {
                Ok(result) => {
                    result.validate_closed().unwrap();
                    println!(
                        "bulb={bulb} {algorithm:?} quads={} aligned={:.1}% source→out={:.5} out→source={:.5} time={elapsed:.0?}",
                        result.quads.len(),
                        coverage(&result, &samples),
                        surface_error(&mesh, &result),
                        surface_error(&result, &mesh)
                    );
                }
                Err(error) => println!("bulb={bulb} {algorithm:?} time={elapsed:.0?} {error}"),
            }
        }
    }
}

#[test]
fn guided_retopology_preserves_open_rims_creases_and_separate_parts() {
    let (mut mesh, _) = fixture(false);
    // Two separated open tubes cut from the same cylinder; no shared vertices.
    let rings = 32 * 19;
    mesh.vertices.truncate(rings);
    mesh.mask.truncate(rings);
    mesh.triangles.retain(|t| {
        t.iter().all(|&v| v < rings)
            && !(t.iter().any(|&v| v / 32 == 8) && t.iter().any(|&v| v / 32 == 9))
    });
    let boundaries: Vec<_> = [0, 8, 9, 18]
        .into_iter()
        .flat_map(|r| (0..32).map(move |j| r * 32 + j))
        .map(|i| mesh.vertices[i])
        .collect();
    let crease = [3 * 32, 4 * 32];
    mesh.creases = vec![crease];
    let result = mesh
        .guided_retopology(
            Options {
                target: 340,
                ..Default::default()
            },
            None,
            &|| false,
        )
        .unwrap()
        .mesh;
    assert!(boundaries.iter().all(|p| result.vertices.contains(p)));
    assert_eq!(result.creases.len(), 1);
    assert_eq!(
        result.creases[0].map(|i| result.vertices[i]),
        crease.map(|i| mesh.vertices[i])
    );
    let edges = Surface::new(&result, &|| false).unwrap().edges;
    assert_eq!(edges.values().filter(|(_, b)| b.is_none()).count(), 4 * 32);
    assert!(
        result
            .triangles
            .iter()
            .all(|t| !(t.iter().any(|&v| result.vertices[v].1 < -0.06)
                && t.iter().any(|&v| result.vertices[v].1 > -0.06)))
    );
}

#[test]
fn guided_retopology_worker_enforces_configured_input_limit() {
    let (mesh, _) = fixture(false);
    for input_triangle_limit in [
        0,
        mesh.triangles.len() - 1,
        super::super::max_triangles() + 1,
    ] {
        let error = mesh
            .guided_retopology(
                Options {
                    input_triangle_limit,
                    ..Default::default()
                },
                None,
                &|| false,
            )
            .err()
            .unwrap();
        assert!(
            error.to_lowercase().contains("input triangle limit"),
            "{error}"
        );
    }
    mesh.guided_retopology(
        Options {
            input_triangle_limit: mesh.triangles.len(),
            ..Default::default()
        },
        None,
        &|| false,
    )
    .unwrap();
}

#[test]
fn guided_retopology_mirrors_a_closed_surface_guide_and_rejects_stale_paths() {
    let (mesh, mut path) = fixture(false);
    path.closed = true;
    let normals = mesh.normals();
    path.points = (0..32)
        .step_by(2)
        .map(|j| {
            let v = 9 * 32 + j;
            PathPoint {
                position: mesh.vertices[v],
                normal: normals[v],
            }
        })
        .collect();
    let result = mesh
        .guided_retopology(
            Options {
                mirror_path_x: true,
                ..Default::default()
            },
            Some(&path),
            &|| false,
        )
        .unwrap();
    assert_eq!(result.guides.len(), 2);
    for samples in result.guides.iter() {
        assert_eq!(samples.first(), samples.last());
    }
    for (a, b) in result.guides[0].iter().zip(&result.guides[1]) {
        assert_eq!(a.position, Vec3(-b.position.0, b.position.1, b.position.2));
        assert_eq!(a.tangent, Vec3(-b.tangent.0, b.tangent.1, b.tangent.2));
    }
    path.surface_signature = 0;
    assert!(
        mesh.guided_retopology(Options::default(), Some(&path), &|| false)
            .err()
            .unwrap()
            .contains("reproject")
    );
}

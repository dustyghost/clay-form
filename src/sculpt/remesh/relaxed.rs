//! Quad distribution using the shared, surface-constrained relaxation engine.
use super::Mesh;
#[cfg(test)]
use super::Vec3;

pub(super) fn redistribute(
    mesh: &mut Mesh,
    step: f32,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    // Work in voxel units so tolerance and numerical thresholds behave equally
    // for tiny and large models. The source reconstruction remains immutable.
    let origin = mesh.vertices[0];
    let mut reference = mesh.clone();
    for p in &mut reference.vertices {
        *p = (*p - origin) * (1. / step);
    }
    let options = crate::sculpt::RelaxationOptions {
        strength: 0.5,
        iterations: 30,
        max_deviation: 0.15,
    };
    let (mut relaxed, _) = reference.relax_mesh(&reference, options, &cancelled).map_err(|error| {
        if error.contains("work limit") {
            "Relaxed quads reached its work limit. Lower Resolution or choose Surface nets; the original model is unchanged.".to_string()
        } else {
            error
        }
    })?;
    for p in &mut relaxed.vertices {
        *p = *p * step + origin;
    }
    relaxed.validate_closed()?;
    *mesh = relaxed;
    Ok(())
}

#[cfg(test)]
fn valid_move(before: [Vec3; 4], after: [Vec3; 4], step: f32) -> bool {
    let [a, b, c, d] = after;
    let n0 = (b - a).cross(c - a);
    let n1 = (c - a).cross(d - a);
    if n0.len().min(n1.len()) < step * step * 1e-8 || n0.dot(n1) <= 0. {
        return false;
    }
    for [i, j, k] in [[0, 1, 2], [0, 2, 3]] {
        let old = (before[j] - before[i]).cross(before[k] - before[i]);
        let new = (after[j] - after[i]).cross(after[k] - after[i]);
        if old.dot(new) <= 0.25 * old.len() * new.len() {
            return false;
        }
    }
    // Avoid concave corners and retain at least half the previous worst scaled
    // corner Jacobian (up to 0.1); this cannot turn a good face into a sliver.
    let quality = |p: [Vec3; 4]| {
        let n = ((p[1] - p[0]).cross(p[2] - p[0]) + (p[2] - p[0]).cross(p[3] - p[0])).norm();
        (0..4)
            .map(|i| {
                let u = p[(i + 1) % 4] - p[i];
                let v = p[(i + 3) % 4] - p[i];
                u.cross(v).dot(n) / (u.len() * v.len()).max(1e-20)
            })
            .fold(f32::INFINITY, f32::min)
    };
    let old = quality(before);
    quality(after) + 1e-6 >= old.min((old * 0.5).min(0.1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, QuadRemeshAlgorithm, Topology};

    fn edge_cv(mesh: &Mesh) -> f32 {
        let mut edges = std::collections::BTreeSet::new();
        for q in &mesh.quads {
            for i in 0..4 {
                let a = q[i];
                let b = q[(i + 1) % 4];
                edges.insert((a.min(b), a.max(b)));
            }
        }
        let lengths: Vec<_> = edges
            .iter()
            .map(|&(a, b)| (mesh.vertices[a] - mesh.vertices[b]).len())
            .collect();
        let mean = lengths.iter().sum::<f32>() / lengths.len() as f32;
        (lengths.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / lengths.len() as f32).sqrt()
            / mean
    }

    fn sampled_deviation(reference: &Mesh, output: &Mesh, distance: f32) -> f32 {
        let surface = crate::sculpt::projection::Surface::new(reference, &|| false).unwrap();
        let mut budget = 20_000_000;
        let mut maximum = 0f32;
        for t in &output.triangles {
            let [a, b, c] = t.map(|i| output.vertices[i]);
            for p in [
                (a + b + c) * (1. / 3.),
                (a + b) * 0.5,
                (b + c) * 0.5,
                (c + a) * 0.5,
            ] {
                let (_, q, _) = surface
                    .nearest(p, distance, None, &mut budget)
                    .unwrap()
                    .unwrap();
                maximum = maximum.max((p - q).len());
            }
        }
        maximum
    }

    #[test]
    fn relaxed_quads_improve_spacing_and_keep_surface_and_connectivity() {
        for kind in [
            PrimitiveKind::UvSphere,
            PrimitiveKind::Cube,
            PrimitiveKind::Torus,
        ] {
            let input = Primitive::for_kind(kind).mesh().unwrap();
            let original = input
                .remesh_topology_cancellable(24, Topology::Quads, &|| false)
                .unwrap();
            let output = input
                .remesh_algorithm_cancellable(
                    24,
                    Topology::Quads,
                    QuadRemeshAlgorithm::Relaxed,
                    &|| false,
                )
                .unwrap();
            output.validate_closed().unwrap();
            assert_eq!(output.quads, original.quads);
            assert_eq!(output.triangles, original.triangles);
            let before = edge_cv(&original);
            let after = edge_cv(&output);
            eprintln!("{kind:?} edge CV {before:.4} -> {after:.4}");
            assert!(after < before * 0.85, "{kind:?}: {before} -> {after}");
            let surface = crate::sculpt::projection::Surface::new(&original, &|| false).unwrap();
            let mut budget = 10_000_000;
            for &p in &output.vertices {
                let (_, nearest, _) = surface.nearest(p, 0.1, None, &mut budget).unwrap().unwrap();
                assert!((p - nearest).len() < 1e-5);
            }
            for (q, r) in output.quads.iter().zip(&original.quads) {
                assert!(valid_move(
                    r.map(|i| original.vertices[i]),
                    q.map(|i| output.vertices[i]),
                    2. / 24.
                ));
            }
        }
    }

    #[test]
    fn relaxed_quads_keep_thin_nearby_components_separate() {
        let mut first = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
        for p in &mut first.vertices {
            p.2 *= 0.02;
        }
        // Perturb in-plane spacing, with another sheet closer than the allowed
        // relaxation travel. Projection must stay in its own connected patch.
        for p in &mut first.vertices {
            p.0 += 0.04 * (p.1 * 4.).sin() * (1. - p.0.abs());
        }
        let count = first.vertices.len();
        let second = first.clone();
        first
            .vertices
            .extend(second.vertices.iter().map(|&p| p + Vec3(0., 0., 0.05)));
        first.mask.extend(second.mask);
        first
            .quads
            .extend(second.quads.iter().map(|q| q.map(|i| i + count)));
        first
            .triangles
            .extend(second.triangles.iter().map(|q| q.map(|i| i + count)));
        let before = first.clone();
        redistribute(&mut first, 0.25, &|| false).unwrap();
        assert_eq!(first.quads, before.quads);
        for (i, p) in first.vertices.iter().enumerate() {
            let z = before.vertices[i].2;
            assert!(
                (p.2 - z).abs() < 1e-5,
                "vertex {i} changed sheet: {z} -> {}",
                p.2
            );
        }
    }

    #[test]
    #[ignore = "local comparison; requires CLAY_REVIEW_PROJECT and CLAY_GUIDE_PREVIEW"]
    fn relaxed_quads_project_comparison() {
        let path = std::env::var("CLAY_REVIEW_PROJECT").unwrap();
        let directory = std::path::PathBuf::from(std::env::var("CLAY_GUIDE_PREVIEW").unwrap());
        std::fs::create_dir_all(&directory).unwrap();
        let project: crate::sculpt::Project =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let resolution = project.mesh_settings.resolution;
        let mut outputs = Vec::new();
        for (algorithm, name) in [
            (QuadRemeshAlgorithm::SurfaceNets, "surface-nets"),
            (QuadRemeshAlgorithm::Relaxed, "relaxed-quads"),
        ] {
            let start = std::time::Instant::now();
            let mesh = project
                .mesh
                .remesh_algorithm_cancellable(resolution, Topology::Quads, algorithm, &|| false)
                .unwrap();
            eprintln!(
                "{name}: {} quads, edge CV {:.6}, {:?} debug runtime",
                mesh.quads.len(),
                edge_cv(&mesh),
                start.elapsed()
            );
            let mut saved = project.clone();
            saved.mesh = mesh.clone();
            saved.mesh_settings.quad_remesh_algorithm = algorithm;
            std::fs::write(
                directory.join(format!("{name}.clay")),
                serde_json::to_vec(&saved).unwrap(),
            )
            .unwrap();
            outputs.push(mesh);
        }
        assert_eq!(outputs[0].quads, outputs[1].quads);
        assert!(edge_cv(&outputs[1]) < edge_cv(&outputs[0]) * 0.85);
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &project.mesh.vertices {
            for (i, x) in [p.0, p.1, p.2].iter().enumerate() {
                min[i] = min[i].min(*x);
                max[i] = max[i].max(*x);
            }
        }
        let step = (0..3).map(|i| max[i] - min[i]).fold(0f32, f32::max) / resolution as f32;
        let deviation = sampled_deviation(&outputs[0], &outputs[1], step);
        eprintln!(
            "maximum sampled deviation: {deviation:.6} ({:.6} voxels)",
            deviation / step
        );
        assert!(deviation < step * 0.151);
    }

    #[test]
    fn relaxed_quads_cancellation_does_not_replace_the_source() {
        let input = Mesh::sphere();
        let before = input.clone();
        let calls = std::cell::Cell::new(0);
        let error = input
            .remesh_algorithm_cancellable(
                24,
                Topology::Quads,
                QuadRemeshAlgorithm::Relaxed,
                &|| {
                    let n = calls.get() + 1;
                    calls.set(n);
                    n > 300
                },
            )
            .unwrap_err();
        assert!(error.contains("cancelled"));
        assert_eq!(input, before);
    }
}

//! Transactional integration and independent validation of the quad optimiser.
use super::{MAX_TRIANGLES, MAX_VERTICES, Mesh, Vec3, optimized};
use crate::sculpt::projection::Surface;
use std::collections::BTreeMap;

const DEVIATION: f32 = 0.4;
const SHAPE_ERROR: &str = "Optimized quads could not preserve this surface within 0.4 voxel. Increase Resolution or choose Surface nets or Relaxed quads; the original model is unchanged.";

fn mesh(vertices: Vec<Vec3>, quads: Vec<[usize; 4]>) -> Mesh {
    Mesh {
        mask: vec![0.; vertices.len()],
        vertices,
        triangles: quads
            .iter()
            .flat_map(|&[a, b, c, d]| [[a, b, c], [a, c, d]])
            .collect(),
        quads,
        ..Default::default()
    }
}

// A valid surface can still contain crowded short edges at a narrow crease.
// Prefer fewer severely uneven faces before considering their worst ratio.
fn uneven_faces(mesh: &Mesh) -> (usize, f32) {
    let mut count = 0;
    let mut worst = 1f32;
    for q in &mesh.quads {
        let lengths: [f32; 4] =
            std::array::from_fn(|i| (mesh.vertices[q[i]] - mesh.vertices[q[(i + 1) % 4]]).len());
        let ratio = lengths.into_iter().fold(0f32, f32::max)
            / lengths.into_iter().fold(f32::INFINITY, f32::min).max(1e-20);
        count += usize::from(ratio > 8.);
        worst = worst.max(ratio);
    }
    (count, worst)
}

// Run in voxel units. Sampling is a bounded approximation, not a Hausdorff proof.
fn check_surface(reference: &Mesh, output: &Mesh, cancel: &dyn Fn() -> bool) -> Result<(), String> {
    output.require_adaptive_quads()?;
    if reference.vertices.len() as isize - reference.quads.len() as isize
        != output.vertices.len() as isize - output.quads.len() as isize
    {
        return Err(
            "Optimized quads changed surface topology; the original model is unchanged.".into(),
        );
    }
    for q in &output.quads {
        let [a, b, c, d] = q.map(|v| output.vertices[v]);
        let n0 = (b - a).cross(c - a);
        let n1 = (c - a).cross(d - a);
        if n0.len().min(n1.len()) < 1e-8 || n0.dot(n1) <= 0. {
            return Err(SHAPE_ERROR.into());
        }
        let n = (n0 + n1).norm();
        let points = [a, b, c, d];
        if (0..4).any(|i| {
            let u = points[(i + 1) % 4] - points[i];
            let v = points[(i + 3) % 4] - points[i];
            u.cross(v).dot(n) <= 0.1 * u.len() * v.len()
        }) {
            return Err(SHAPE_ERROR.into());
        }
    }
    let mut budget = 100_000_000;
    for (from, to) in [(reference, output), (output, reference)] {
        let surface = Surface::new(to, &cancel)?;
        for (i, t) in from.triangles.iter().enumerate() {
            if i % 256 == 0 && cancel() {
                return Err("Quad optimisation cancelled".into());
            }
            let [a, b, c] = t.map(|v| from.vertices[v]);
            for p in [
                a,
                b,
                c,
                (a + b) * 0.5,
                (b + c) * 0.5,
                (c + a) * 0.5,
                (a + b + c) * (1. / 3.),
            ] {
                if surface.nearest(p, DEVIATION, None, &mut budget)?.is_none() {
                    return Err(SHAPE_ERROR.into());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn redistribute(
    source: &mut Mesh,
    step: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    if cancel() {
        return Err("Quad optimisation cancelled".into());
    }
    source.require_adaptive_quads()?;
    if !step.is_finite() || step <= 0. {
        return Err("Invalid remesh voxel size".into());
    }
    // Separate components before optimisation AND validation. A nearby unrelated
    // sheet must never satisfy a distance check or attract projected vertices.
    let surface = Surface::new(source, &cancel)?;
    let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..source.quads.len() {
        components
            .entry(surface.components[i * 2])
            .or_default()
            .push(i);
    }
    drop(surface);
    let mut result = Mesh::default();
    for faces in components.values() {
        if cancel() {
            return Err("Quad optimisation cancelled".into());
        }
        let used: std::collections::BTreeSet<_> =
            faces.iter().flat_map(|&f| source.quads[f]).collect();
        let origin = source.vertices[*used.first().unwrap()];
        let ids: BTreeMap<_, _> = used.iter().enumerate().map(|(i, &v)| (v, i)).collect();
        let vertices = used
            .iter()
            .map(|&v| (source.vertices[v] - origin) * (1. / step))
            .collect();
        let quads = faces
            .iter()
            .map(|&f| source.quads[f].map(|v| ids[&v]))
            .collect();
        let reference = mesh(vertices, quads);
        let (vertices, quads) =
            optimized::optimize(&reference.vertices, &reference.quads, 1., cancel)?;
        let mut output = mesh(vertices, quads);
        check_surface(&reference, &output, cancel)?;
        let quality = uneven_faces(&output);
        if quality.0 > 0 && output.quads != reference.quads {
            // Keep the validated candidate if this optional, less aggressive
            // attempt cannot improve it. Do not relax the surface-error bound.
            if let Ok(Some((vertices, quads))) =
                optimized::optimize_levels(&reference.vertices, &reference.quads, 1., cancel, 1)
            {
                let candidate = mesh(vertices, quads);
                let candidate_quality = uneven_faces(&candidate);
                if candidate_quality < quality
                    && check_surface(&reference, &candidate, cancel).is_ok()
                {
                    output = candidate;
                }
            }
            if cancel() {
                return Err("Quad optimisation cancelled".into());
            }
        }
        if result.vertices.len() + output.vertices.len() > MAX_VERTICES
            || result.triangles.len() + output.triangles.len() > MAX_TRIANGLES
        {
            return Err("Optimized quads exceeds mesh limits; choose a lower resolution".into());
        }
        let offset = result.vertices.len();
        result
            .vertices
            .extend(output.vertices.iter().map(|&p| p * step + origin));
        result
            .quads
            .extend(output.quads.iter().map(|q| q.map(|v| v + offset)));
        result
            .triangles
            .extend(output.triangles.iter().map(|t| t.map(|v| v + offset)));
    }
    result.mask = vec![0.; result.vertices.len()];
    result.require_adaptive_quads()?;
    *source = result;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, QuadRemeshAlgorithm, Topology};

    fn irregular(mesh: &Mesh) -> usize {
        let mut degree = vec![0; mesh.vertices.len()];
        for &v in mesh.quads.iter().flatten() {
            degree[v] += 1;
        }
        degree.iter().filter(|&&n| n != 4).count()
    }

    #[test]
    fn optimized_quads_rebuild_junctions_and_preserve_closed_surfaces() {
        for kind in [
            PrimitiveKind::UvSphere,
            PrimitiveKind::Cube,
            PrimitiveKind::Torus,
        ] {
            let input = Primitive::for_kind(kind).mesh().unwrap();
            let original = input
                .remesh_topology_cancellable(16, Topology::Quads, &|| false)
                .unwrap();
            let mut output = original.clone();
            // The primitive bounds are not all the same; use the actual voxel size.
            let mut lo = [f32::INFINITY; 3];
            let mut hi = [f32::NEG_INFINITY; 3];
            for p in &input.vertices {
                for (i, x) in [p.0, p.1, p.2].into_iter().enumerate() {
                    lo[i] = lo[i].min(x);
                    hi[i] = hi[i].max(x);
                }
            }
            let step = (0..3).map(|i| hi[i] - lo[i]).fold(0f32, f32::max) / 16.;
            let start = std::time::Instant::now();
            redistribute(&mut output, step, &|| false).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            eprintln!(
                "{kind:?}: {} -> {} irregular, {} -> {} quads, {:?}",
                irregular(&original),
                irregular(&output),
                original.quads.len(),
                output.quads.len(),
                start.elapsed()
            );
            if kind == PrimitiveKind::Cube {
                assert_eq!(output.quads, original.quads);
            } else {
                assert!(irregular(&output) < irregular(&original), "{kind:?}");
            }
            assert!(output.quads.len() <= original.quads.len());
            output.require_adaptive_quads().unwrap();
            if kind == PrimitiveKind::UvSphere {
                // Exercise the actual topology-changing path at small and large
                // scales; powers of two avoid introducing a different input mesh
                // through rounding before normalisation.
                for scale in [1. / 1024., 1024.] {
                    let mut scaled = original.clone();
                    for p in &mut scaled.vertices {
                        *p = *p * scale;
                    }
                    redistribute(&mut scaled, step * scale, &|| false).unwrap();
                    assert_eq!(scaled.quads, output.quads);
                    for (p, q) in scaled.vertices.iter().zip(&output.vertices) {
                        assert!((*p * (1. / scale) - *q).len() < 1e-5);
                    }
                }
            }
        }
    }

    #[test]
    fn optimized_quads_cancel_without_replacing_the_source() {
        let input = Primitive::for_kind(PrimitiveKind::UvSphere).mesh().unwrap();
        let mut source = input
            .remesh_topology_cancellable(16, Topology::Quads, &|| false)
            .unwrap();
        let before = source.clone();
        let calls = std::cell::Cell::new(0);
        let error = redistribute(&mut source, 0.1, &|| {
            calls.set(calls.get() + 1);
            calls.get() > 40
        })
        .unwrap_err();
        assert!(error.contains("cancelled"), "{error}");
        assert_eq!(source, before);
        // Also cancel inside the optimiser, after reference construction.
        let calls = std::cell::Cell::new(0);
        let error = optimized::optimize(&source.vertices, &source.quads, 0.1, &|| {
            calls.set(calls.get() + 1);
            calls.get() > 12
        })
        .unwrap_err();
        assert!(error.contains("cancelled"), "{error}");
    }

    #[test]
    fn optimized_quads_keep_nearby_components_and_are_scale_independent() {
        // Two thin, nearby boxes exercise component-local validation and the
        // unchanged-layout shortcut. Their regular quads have high edge ratios.
        let mut first = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
        for p in &mut first.vertices {
            p.2 *= 0.02;
        }
        let mut source = first.clone();
        let offset = source.vertices.len();
        source
            .vertices
            .extend(first.vertices.iter().map(|&p| p + Vec3(0., 0., 0.05)));
        source
            .quads
            .extend(first.quads.iter().map(|q| q.map(|v| v + offset)));
        source
            .triangles
            .extend(first.triangles.iter().map(|t| t.map(|v| v + offset)));
        source.mask = vec![0.; source.vertices.len()];
        let before = source.clone();
        redistribute(&mut source, 0.1, &|| false).unwrap();
        assert_eq!(source.quads, before.quads);
        for (p, q) in source.vertices.iter().zip(&before.vertices) {
            assert!((*p - *q).len() < 1e-5);
        }
        let surface = Surface::new(&source, &|| false).unwrap();
        assert_eq!(
            surface
                .components
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            2
        );
        // A displaced copy passes the topology check but must fail the shape gate.
        let mut displaced = first.clone();
        for p in &mut displaced.vertices {
            p.0 += 0.5;
        }
        assert!(check_surface(&first, &displaced, &|| false).is_err());
        for scale in [0.001, 1000.] {
            let mut scaled = before.clone();
            for p in &mut scaled.vertices {
                *p = *p * scale;
            }
            redistribute(&mut scaled, 0.1 * scale, &|| false).unwrap();
            assert_eq!(scaled.quads, source.quads);
            for (p, q) in scaled.vertices.iter().zip(&source.vertices) {
                assert!((*p * (1. / scale) - *q).len() < 1e-5);
            }
        }
    }

    #[test]
    #[ignore = "manual release comparison; requires CLAY_REVIEW_PROJECT and CLAY_GUIDE_PREVIEW"]
    fn optimized_quads_project_comparison() {
        let path = std::env::var("CLAY_REVIEW_PROJECT").unwrap();
        let directory = std::path::PathBuf::from(std::env::var("CLAY_GUIDE_PREVIEW").unwrap());
        std::fs::create_dir_all(&directory).unwrap();
        let mut project: crate::sculpt::Project =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let start = std::time::Instant::now();
        project.mesh = project
            .mesh
            .remesh_algorithm_cancellable(
                project.mesh_settings.resolution,
                Topology::Quads,
                QuadRemeshAlgorithm::Optimized,
                &|| false,
            )
            .unwrap();
        eprintln!(
            "Optimized quads: {} quads, {} irregular vertices, {:?}",
            project.mesh.quads.len(),
            irregular(&project.mesh),
            start.elapsed()
        );
        project.mesh_settings.quad_remesh_algorithm = QuadRemeshAlgorithm::Optimized;
        std::fs::write(
            directory.join("optimized-quads.clay"),
            serde_json::to_vec(&project).unwrap(),
        )
        .unwrap();
    }
}

//! Exposed square faces of occupied voxels, optionally fitted to the input mesh.
//! Occupancy samples are voxel centres; corners sit half a cell away. Shared
//! corners are joined through adjacent boundary faces, keeping edge/corner-only
//! contacts in separate polygon fans.
use super::{Grid, Mesh, Vec3, max_triangles, max_vertices};
use crate::sculpt::ProjectionOptions;
use std::collections::HashMap;

const CONTACT: &str = "Fast quads has an unresolved edge or corner contact. Increase Resolution or choose Triangles; the original model is unchanged.";

pub(super) fn extract(
    grid: &Grid,
    inside: &[bool],
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    let [nx, ny, nz] = grid.dims;
    let mut mesh = Mesh::default();
    let mut lattice_quads = Vec::new();
    let mut owners = Vec::new();
    for z in 1..nz - 1 {
        if cancel() {
            return Err("Mesh operation cancelled".into());
        }
        for y in 1..ny - 1 {
            for x in 1..nx - 1 {
                if !inside[grid.index(x, y, z)] {
                    continue;
                }
                for (neighbor, corners) in [
                    (
                        [x - 1, y, z],
                        [[x, y, z], [x, y, z + 1], [x, y + 1, z + 1], [x, y + 1, z]],
                    ),
                    (
                        [x + 1, y, z],
                        [
                            [x + 1, y, z],
                            [x + 1, y + 1, z],
                            [x + 1, y + 1, z + 1],
                            [x + 1, y, z + 1],
                        ],
                    ),
                    (
                        [x, y - 1, z],
                        [[x, y, z], [x + 1, y, z], [x + 1, y, z + 1], [x, y, z + 1]],
                    ),
                    (
                        [x, y + 1, z],
                        [
                            [x, y + 1, z],
                            [x, y + 1, z + 1],
                            [x + 1, y + 1, z + 1],
                            [x + 1, y + 1, z],
                        ],
                    ),
                    (
                        [x, y, z - 1],
                        [[x, y, z], [x, y + 1, z], [x + 1, y + 1, z], [x + 1, y, z]],
                    ),
                    (
                        [x, y, z + 1],
                        [
                            [x, y, z + 1],
                            [x + 1, y, z + 1],
                            [x + 1, y + 1, z + 1],
                            [x, y + 1, z + 1],
                        ],
                    ),
                ] {
                    if inside[grid.index(neighbor[0], neighbor[1], neighbor[2])] {
                        continue;
                    }
                    lattice_quads.push(corners.map(|[a, b, c]| a + (nx + 1) * (b + (ny + 1) * c)));
                    owners.push(grid.index(x, y, z));
                    if lattice_quads.len() * 2 > max_triangles() {
                        return Err(
                            "Fast quads exceeds mesh limits; choose a lower resolution".into()
                        );
                    }
                }
            }
        }
    }
    if lattice_quads.is_empty() {
        return Err("No quad surface found; increase Resolution".into());
    }
    // Each face starts with its own four corners. Join only through paired
    // boundary edges; a checkerboard edge has four incident faces, paired by
    // occupied voxel rather than welding two sheets together.
    let mut parent: Vec<_> = (0..lattice_quads.len() * 4).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut edges: HashMap<[usize; 2], Vec<(usize, usize)>> = HashMap::new();
    for (f, q) in lattice_quads.iter().enumerate() {
        if f % 1024 == 0 && cancel() {
            return Err("Mesh operation cancelled".into());
        }
        for i in 0..4 {
            let (a, b) = (q[i], q[(i + 1) % 4]);
            edges.entry([a.min(b), a.max(b)]).or_default().push((f, i));
        }
    }
    for (index, incident) in edges.values().enumerate() {
        if index % 1024 == 0 && cancel() {
            return Err("Mesh operation cancelled".into());
        }
        if incident.len() != 2 && incident.len() != 4 {
            return Err(CONTACT.into());
        }
        for &(f, i) in incident {
            let &(g, j) = incident
                .iter()
                .find(|&&(g, _)| g != f && (incident.len() == 2 || owners[g] == owners[f]))
                .ok_or(CONTACT)?;
            if lattice_quads[f][i] != lattice_quads[g][(j + 1) % 4]
                || lattice_quads[f][(i + 1) % 4] != lattice_quads[g][j]
            {
                return Err(CONTACT.into());
            }
            let a = root(&mut parent, f * 4 + i);
            let b = root(&mut parent, g * 4 + (j + 1) % 4);
            parent[b] = a;
            let a = root(&mut parent, f * 4 + (i + 1) % 4);
            let b = root(&mut parent, g * 4 + j);
            parent[b] = a;
        }
    }
    let mut ids = vec![usize::MAX; parent.len()];
    for (f, q) in lattice_quads.iter().enumerate() {
        if f % 1024 == 0 && cancel() {
            return Err("Mesh operation cancelled".into());
        }
        let q = std::array::from_fn(|i| {
            let r = root(&mut parent, f * 4 + i);
            if ids[r] == usize::MAX {
                ids[r] = mesh.vertices.len();
                let corner = q[i];
                let x = corner % (nx + 1);
                let y = corner / (nx + 1) % (ny + 1);
                let z = corner / ((nx + 1) * (ny + 1));
                mesh.vertices.push(
                    grid.origin + Vec3(x as f32 - 0.5, y as f32 - 0.5, z as f32 - 0.5) * grid.step,
                );
                mesh.mask.push(0.);
            }
            ids[r]
        });
        mesh.quads.push(q);
        let [a, b, c, d] = q;
        mesh.triangles.extend([[a, b, c], [a, c, d]]);
        if mesh.vertices.len() > max_vertices() {
            return Err("Fast quads exceeds mesh limits; choose a lower resolution".into());
        }
    }
    mesh.require_adaptive_quads()
        .map_err(|_| CONTACT.to_string())?;
    Ok(mesh)
}

pub(super) fn project(
    mut mesh: Mesh,
    source: &Mesh,
    step: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    // Normalize so projection bounds and degeneracy thresholds scale with voxels.
    // Project onto the original sculpt, not the stepped skin or a surface-net proxy.
    let origin = mesh.vertices[0];
    for p in &mut mesh.vertices {
        *p = (*p - origin) * (1. / step);
    }
    // A stair riser can collapse to a line when projected straight onto a flat
    // source patch. Round its footprint slightly first, keeping every quad and
    // limiting redistribution to half a voxel from the literal skin.
    let skin = mesh.vertices.clone();
    let mut neighbors = vec![Vec::new(); skin.len()];
    for q in &mesh.quads {
        for i in 0..4 {
            neighbors[q[i]].extend([q[(i + 1) % 4], q[(i + 3) % 4]]);
        }
    }
    for adjacent in &mut neighbors {
        adjacent.sort_unstable();
        adjacent.dedup();
    }
    for _ in 0..4 {
        if cancel() {
            return Err("Mesh operation cancelled".into());
        }
        let before = mesh.vertices.clone();
        for (v, adjacent) in neighbors.iter().enumerate() {
            let mean = adjacent.iter().fold(Vec3::default(), |p, &i| p + before[i])
                * (1. / adjacent.len() as f32);
            let next = before[v] + (mean - before[v]) * 0.5;
            let delta = next - skin[v];
            mesh.vertices[v] = skin[v] + delta * (0.5 / delta.len().max(0.5));
        }
        mesh.limit_smoothing_deformation(&before);
    }
    let reference = Mesh {
        vertices: source
            .vertices
            .iter()
            .map(|&p| (p - origin) * (1. / step))
            .collect(),
        triangles: source.triangles.clone(),
        mask: vec![0.; source.vertices.len()],
        ..Default::default()
    };
    let (mut projected, _) = mesh.project_details_locally(
        &reference,
        ProjectionOptions {
            strength: 1.,
            max_distance: 1.5,
        },
        &cancel,
    )?;
    projected.require_adaptive_quads()?;
    for p in &mut projected.vertices {
        *p = *p * step + origin;
    }
    Ok(projected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, QuadRemeshAlgorithm, Topology};

    #[test]
    fn voxel_skin_exposes_boundary_squares_and_separates_touching_voxels() {
        let grid = Grid {
            dims: [6; 3],
            origin: Vec3::default(),
            step: 1.,
        };
        let mut inside = vec![false; 216];
        inside[grid.index(2, 2, 2)] = true;
        let cube = extract(&grid, &inside, &|| false).unwrap();
        assert_eq!((cube.vertices.len(), cube.quads.len()), (8, 6));
        inside[grid.index(3, 2, 2)] = true;
        let pair = extract(&grid, &inside, &|| false).unwrap();
        assert_eq!((pair.vertices.len(), pair.quads.len()), (12, 10));
        inside[grid.index(3, 2, 2)] = false;
        for touching in [[3, 3, 2], [3, 3, 3]] {
            inside[grid.index(touching[0], touching[1], touching[2])] = true;
            let touching_skin = extract(&grid, &inside, &|| false).unwrap();
            assert_eq!(
                (touching_skin.vertices.len(), touching_skin.quads.len()),
                (16, 12)
            );
            touching_skin.require_adaptive_quads().unwrap();
            inside[grid.index(touching[0], touching[1], touching[2])] = false;
        }
        assert!(
            extract(&grid, &inside, &|| true)
                .unwrap_err()
                .contains("cancelled")
        );
    }

    #[test]
    fn voxel_skin_all_corner_configurations_have_closed_fans_and_outward_squares() {
        let grid = Grid {
            dims: [6; 3],
            origin: Vec3::default(),
            step: 1.,
        };
        for pattern in 1u32..256 {
            let mut inside = vec![false; 216];
            for i in 0..8 {
                inside[grid.index(2 + i % 2, 2 + (i / 2) % 2, 2 + i / 4)] = pattern & (1 << i) != 0;
            }
            let mesh =
                extract(&grid, &inside, &|| false).unwrap_or_else(|e| panic!("{pattern:08b}: {e}"));
            let mut volume = 0.;
            for q in &mesh.quads {
                let [a, b, c, d] = q.map(|v| mesh.vertices[v]);
                for edge in [b - a, c - b, d - c, a - d] {
                    assert_eq!(edge.len(), 1.);
                }
                assert_eq!((b - a).dot(d - a), 0.);
                volume += (a.dot(b.cross(c)) + a.dot(c.cross(d))) / 6.;
            }
            assert!(
                (volume - pattern.count_ones() as f32).abs() < 1e-4,
                "{pattern:08b}"
            );
        }
    }

    #[test]
    fn voxel_skin_projection_scales_cancels_and_preserves_transferred_attributes() {
        let mut source = Mesh::starting_sphere();
        source.mask.fill(1.);
        let raw = source.remesh_stepped_cancellable(16, &|| false).unwrap();
        let fitted = source
            .remesh_algorithm_cancellable(16, Topology::Quads, QuadRemeshAlgorithm::Fast, &|| false)
            .unwrap();
        assert_eq!(fitted.vertex_groups.len(), source.vertex_groups.len());
        assert_eq!(fitted.vertex_groups[0].name, source.vertex_groups[0].name);
        assert!(fitted.mask.iter().all(|&m| m == 0.));
        assert!(
            fitted
                .polygroups
                .as_chunks::<2>()
                .0
                .iter()
                .all(|p| p[0] == p[1])
        );
        let expected = project(raw.clone(), &source, 2. / 16., &|| false).unwrap();
        for scale in [1. / 1024., 1024.] {
            let mut scaled_source = source.clone();
            let mut scaled_raw = raw.clone();
            for p in &mut scaled_source.vertices {
                *p = *p * scale;
            }
            for p in &mut scaled_raw.vertices {
                *p = *p * scale;
            }
            let scaled = project(scaled_raw, &scaled_source, scale * 2. / 16., &|| false).unwrap();
            assert_eq!(scaled.quads, expected.quads);
            for (p, q) in scaled.vertices.iter().zip(&expected.vertices) {
                assert!((*p * (1. / scale) - *q).len() < 1e-5);
            }
        }
        let calls = std::cell::Cell::new(0);
        assert!(
            project(raw, &source, 2. / 16., &|| {
                calls.set(calls.get() + 1);
                calls.get() > 5
            })
            .unwrap_err()
            .contains("cancelled")
        );
    }

    #[test]
    fn voxel_skin_projection_keeps_quads_and_improves_surface_fit() {
        for kind in [
            PrimitiveKind::UvSphere,
            PrimitiveKind::Cube,
            PrimitiveKind::Torus,
        ] {
            let source = Primitive::for_kind(kind).mesh().unwrap();
            let raw = source
                .remesh_stepped_cancellable(24, &|| false)
                .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            let fitted = source
                .remesh_algorithm_cancellable(
                    24,
                    Topology::Quads,
                    QuadRemeshAlgorithm::Fast,
                    &|| false,
                )
                .unwrap();
            assert_eq!(raw.quads, fitted.quads);
            assert_eq!(raw.triangles, fitted.triangles);
            fitted.require_adaptive_quads().unwrap();
            let surface = crate::sculpt::projection::Surface::new(&source, &|| false).unwrap();
            let error = |mesh: &Mesh| {
                let mut budget = 10_000_000;
                mesh.vertices
                    .iter()
                    .map(|&p| {
                        surface
                            .nearest(p, 10., None, &mut budget)
                            .unwrap()
                            .unwrap()
                            .2
                    })
                    .sum::<f32>()
                    / mesh.vertices.len() as f32
            };
            let (before, after) = (error(&raw), error(&fitted));
            eprintln!(
                "{kind:?}: {} quads, mean vertex distance {before} -> {after}",
                raw.quads.len()
            );
            assert!(after < before * 0.5, "{kind:?}: {before} -> {after}");
        }
    }

    #[test]
    #[ignore = "manual voxel skin comparison; requires CLAY_REVIEW_PROJECT and CLAY_GUIDE_PREVIEW"]
    fn fast_quads_project_comparison() {
        let input = std::env::var("CLAY_REVIEW_PROJECT").unwrap();
        let directory = std::path::PathBuf::from(std::env::var("CLAY_GUIDE_PREVIEW").unwrap());
        std::fs::create_dir_all(&directory).unwrap();
        let source: crate::sculpt::Project =
            serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
        for projected in [false, true] {
            let mut project = source.clone();
            let now = std::time::Instant::now();
            project.mesh = if projected {
                source.mesh.remesh_algorithm_cancellable(
                    source.mesh_settings.resolution,
                    Topology::Quads,
                    QuadRemeshAlgorithm::Fast,
                    &|| false,
                )
            } else {
                source
                    .mesh
                    .remesh_stepped_cancellable(source.mesh_settings.resolution, &|| false)
            }
            .unwrap();
            eprintln!(
                "Voxel skin projected={projected}: {} quads in {:?}",
                project.mesh.quads.len(),
                now.elapsed()
            );
            let seconds = now.elapsed().as_secs_f64();
            let step = {
                let mut lo = [f32::INFINITY; 3];
                let mut hi = [f32::NEG_INFINITY; 3];
                for p in &source.mesh.vertices {
                    for (i, x) in [p.0, p.1, p.2].into_iter().enumerate() {
                        lo[i] = lo[i].min(x);
                        hi[i] = hi[i].max(x);
                    }
                }
                (0..3).map(|i| hi[i] - lo[i]).fold(0f32, f32::max)
                    / source.mesh_settings.resolution as f32
            };
            let mut errors = Vec::new();
            for (from, to) in [(&project.mesh, &source.mesh), (&source.mesh, &project.mesh)] {
                let surface = crate::sculpt::projection::Surface::new(to, &|| false).unwrap();
                let mut budget = 100_000_000;
                let mut maximum = 0f32;
                for t in &from.triangles {
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
                        maximum = maximum.max(
                            surface
                                .nearest(p, step * 5., None, &mut budget)
                                .unwrap()
                                .expect("sample lies over five voxels away")
                                .2
                                / step,
                        );
                    }
                }
                errors.push(maximum);
            }
            let profile = serde_json::json!({"projected":projected,"seconds":seconds,"vertices":project.mesh.vertices.len(),"quads":project.mesh.quads.len(),"max_sampled_deviation_voxels":errors});
            eprintln!("{profile}");
            std::fs::write(
                directory.join(if projected {
                    "projected-profile.json"
                } else {
                    "stepped-profile.json"
                }),
                serde_json::to_vec_pretty(&profile).unwrap(),
            )
            .unwrap();
            project.mesh_settings.quad_remesh_algorithm = QuadRemeshAlgorithm::Fast;
            std::fs::write(
                directory.join(if projected {
                    "projected.clay"
                } else {
                    "stepped.clay"
                }),
                serde_json::to_vec(&project).unwrap(),
            )
            .unwrap();
        }
    }
}

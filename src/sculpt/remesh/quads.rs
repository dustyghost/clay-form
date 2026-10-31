//! Surface nets with separate vertices for disconnected contour loops in a voxel.
//! Shared-face asymptotic decisions keep adjacent cells consistent. Singular
//! saddle ties and non-manifold/folded reconstructions are still rejected.
use super::{Grid, Mesh, Vec3, max_triangles, max_vertices};
use std::collections::HashMap;

const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];
const FACES: [[usize; 4]; 6] = [
    [0, 1, 2, 3],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [3, 2, 6, 7],
    [0, 3, 7, 4],
    [1, 2, 6, 5],
];
const AMBIGUOUS: &str = "Quad remesh cannot resolve nearby sheets at this resolution. Increase Resolution or choose Triangles; the original model is unchanged.";

fn unambiguous(signs: [bool; 8]) -> bool {
    // Four crossings on a face require an asymptotic-decider/multi-vertex cell.
    // Two disconnected corner components also require more than one dual vertex.
    if FACES
        .iter()
        .any(|&[a, b, c, d]| signs[a] == signs[c] && signs[b] == signs[d] && signs[a] != signs[b])
    {
        return false;
    }
    for sign in [false, true] {
        let Some(seed) = signs.iter().position(|&s| s == sign) else {
            continue;
        };
        let mut seen = [false; 8];
        seen[seed] = true;
        for _ in 0..8 {
            for (a, b) in EDGES {
                if signs[a] == sign && signs[b] == sign && (seen[a] || seen[b]) {
                    seen[a] = true;
                    seen[b] = true;
                }
            }
        }
        if (0..8).any(|i| signs[i] == sign && !seen[i]) {
            return false;
        }
    }
    true
}

// Connect crossed cube edges through their face-contour arcs. Each connected
// cycle gets its own representative, as in dual marching cubes (Grosso & Zint,
// 2021, section 3.1.1, https://doi.org/10.1007/s00371-021-02139-w).
// This is not a guarantee for sub-voxel tunnels: the final
// manifold and orientation checks remain mandatory.
fn contour_components(values: [f32; 8]) -> Result<[usize; 12], String> {
    let mut parent = std::array::from_fn(|i| i);
    fn root(parent: &[usize; 12], mut i: usize) -> usize {
        while parent[i] != i {
            i = parent[i];
        }
        i
    }
    for face in FACES {
        let ids: [usize; 4] = std::array::from_fn(|i| {
            let (a, b) = (face[i], face[(i + 1) % 4]);
            EDGES
                .iter()
                .position(|&(x, y)| (x == a && y == b) || (x == b && y == a))
                .unwrap()
        });
        let crossed: Vec<_> = ids
            .into_iter()
            .filter(|&e| {
                let (a, b) = EDGES[e];
                (values[a] < 0.) != (values[b] < 0.)
            })
            .collect();
        let pairs = match crossed.as_slice() {
            [] => Vec::new(),
            &[a, b] => vec![(a, b)],
            &[a, b, c, d] => {
                let [v0, v1, v2, v3] = face.map(|i| values[i] as f64);
                let x = v0 * v2;
                let y = v1 * v3;
                let determinant = x - y;
                // A saddle exactly on the iso-value has no unique connection.
                // Reject rather than silently choosing a diagonal or welding sheets.
                if determinant.abs() <= (x.abs() + y.abs()) * 1e-8 {
                    return Err(AMBIGUOUS.into());
                }
                if determinant > 0. {
                    vec![(a, b), (c, d)]
                } else {
                    vec![(d, a), (b, c)]
                }
            }
            _ => return Err(AMBIGUOUS.into()),
        };
        for (a, b) in pairs {
            let a = root(&parent, a);
            let b = root(&parent, b);
            parent[b] = a;
        }
    }
    Ok(std::array::from_fn(|i| root(&parent, i)))
}

pub(super) fn extract(
    grid: &Grid,
    field: &[f32],
    cancelled: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    let [nx, ny, nz] = grid.dims;
    let mut cells = vec![usize::MAX; field.len()];
    let mut mesh = Mesh::default();
    let mut branches: HashMap<usize, [usize; 12]> = HashMap::new();
    for z in 0..nz - 1 {
        if cancelled() {
            return Err("Mesh operation cancelled".into());
        }
        for y in 0..ny - 1 {
            for x in 0..nx - 1 {
                let corners = [
                    grid.index(x, y, z),
                    grid.index(x + 1, y, z),
                    grid.index(x + 1, y + 1, z),
                    grid.index(x, y + 1, z),
                    grid.index(x, y, z + 1),
                    grid.index(x + 1, y, z + 1),
                    grid.index(x + 1, y + 1, z + 1),
                    grid.index(x, y + 1, z + 1),
                ];
                let signs = corners.map(|i| field[i] < 0.);
                if signs.iter().all(|&s| s == signs[0]) {
                    continue;
                }
                let split = !unambiguous(signs);
                let components = if split {
                    contour_components(corners.map(|i| field[i]))?
                } else {
                    [0; 12]
                };
                let mut ids = [usize::MAX; 12];
                let low = grid.point(grid.index(x, y, z));
                let margin = grid.step * 0.02;
                // Preserve the old vertex order and arithmetic for ordinary cells.
                for e in 0..12 {
                    let (a, b) = EDGES[e];
                    if signs[a] == signs[b] || ids[e] != usize::MAX {
                        continue;
                    }
                    let mut sum = Vec3::default();
                    let mut count = 0;
                    let id = mesh.vertices.len();
                    for (j, (a, b)) in EDGES.into_iter().enumerate() {
                        if signs[a] == signs[b] || components[j] != components[e] {
                            continue;
                        }
                        let (a, b) = (corners[a], corners[b]);
                        let t = field[a] / (field[a] - field[b]);
                        sum = sum + grid.point(a) + (grid.point(b) - grid.point(a)) * t;
                        count += 1;
                        ids[j] = id;
                    }
                    let p = sum * (1. / count as f32);
                    mesh.vertices.push(Vec3(
                        p.0.clamp(low.0 + margin, low.0 + grid.step - margin),
                        p.1.clamp(low.1 + margin, low.1 + grid.step - margin),
                        p.2.clamp(low.2 + margin, low.2 + grid.step - margin),
                    ));
                    mesh.mask.push(0.);
                    if mesh.vertices.len() > max_vertices() {
                        return Err(
                            "Quad remesh exceeds mesh limits; choose a lower resolution".into()
                        );
                    }
                }
                let cell = grid.index(x, y, z);
                cells[cell] = *ids.iter().filter(|&&id| id != usize::MAX).min().unwrap();
                if split {
                    branches.insert(cell, ids);
                }
            }
        }
    }
    for z in 1..nz - 1 {
        if cancelled() {
            return Err("Mesh operation cancelled".into());
        }
        for y in 1..ny - 1 {
            for x in 1..nx - 1 {
                let start = grid.index(x, y, z);
                for (end, offsets, local_edges) in [
                    (
                        grid.index(x + 1, y, z),
                        [[x, y - 1, z - 1], [x, y, z - 1], [x, y, z], [x, y - 1, z]],
                        [6, 4, 0, 2],
                    ),
                    (
                        grid.index(x, y + 1, z),
                        [[x - 1, y, z - 1], [x - 1, y, z], [x, y, z], [x, y, z - 1]],
                        [5, 1, 3, 7],
                    ),
                    (
                        grid.index(x, y, z + 1),
                        [[x - 1, y - 1, z], [x, y - 1, z], [x, y, z], [x - 1, y, z]],
                        [10, 11, 8, 9],
                    ),
                ] {
                    if (field[start] < 0.) == (field[end] < 0.) {
                        continue;
                    }
                    let mut q = std::array::from_fn(|i| {
                        let [x, y, z] = offsets[i];
                        let cell = grid.index(x, y, z);
                        branches
                            .get(&cell)
                            .map_or(cells[cell], |ids| ids[local_edges[i]])
                    });
                    if q.contains(&usize::MAX) {
                        return Err(AMBIGUOUS.into());
                    }
                    if field[start] >= 0. {
                        q.reverse();
                    }
                    let [a, b, c, d] = q.map(|i| mesh.vertices[i]);
                    let n0 = (b - a).cross(c - a);
                    let n1 = (c - a).cross(d - a);
                    if n0.len().min(n1.len()) < grid.step * grid.step * 1e-8 || n0.dot(n1) <= 0. {
                        return Err("Quad remesh would create folded or degenerate faces. Increase Resolution or choose Triangles; the original model is unchanged.".into());
                    }
                    mesh.quads.push(q);
                    let [a, b, c, d] = q;
                    mesh.triangles.extend([[a, b, c], [a, c, d]]);
                    if mesh.triangles.len() > max_triangles() {
                        return Err(
                            "Quad remesh exceeds mesh limits; choose a lower resolution".into()
                        );
                    }
                }
            }
        }
    }
    if mesh.quads.is_empty() {
        return Err("No surface found at this resolution; increase resolution".into());
    }
    mesh.require_adaptive_quads()
        .map_err(|_| AMBIGUOUS.to_string())?;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, Topology};

    #[test]
    fn quad_remesh_closed_shapes_and_repeated_reconstruction() {
        for kind in [
            PrimitiveKind::Cube,
            PrimitiveKind::Cylinder,
            PrimitiveKind::Torus,
        ] {
            let source = Primitive::for_kind(kind).mesh().unwrap();
            let output = source
                .remesh_topology_cancellable(20, Topology::Quads, &|| false)
                .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            output.validate_closed().unwrap();
            assert_eq!(output.quads.len() * 2, output.triangles.len());
            assert!(output.polygons().iter().all(|p| p.vertices.len() == 4));
            assert!(output.mask.iter().all(|&w| w == 0.));
            let again = output
                .remesh_topology_cancellable(20, Topology::Quads, &|| false)
                .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            again.validate_closed().unwrap();
            assert_eq!(again.quads.len() * 2, again.triangles.len());
        }
    }
    #[test]
    fn quad_remesh_rejects_ambiguous_cells_and_cancellation() {
        let grid = Grid {
            dims: [5; 3],
            origin: Vec3::default(),
            step: 1.,
        };
        let mut field = vec![1.; 125];
        field[grid.index(1, 1, 1)] = -1.;
        field[grid.index(2, 2, 1)] = -1.;
        assert!(
            extract(&grid, &field, &|| false)
                .unwrap_err()
                .contains("nearby sheets")
        );
        let source = Mesh::sphere();
        assert!(
            source
                .remesh_topology_cancellable(16, Topology::Quads, &|| true)
                .unwrap_err()
                .contains("cancelled")
        );
    }
    #[test]
    fn quad_remesh_resolves_separate_branches_across_a_shared_face() {
        // These two enclosed components share a checkerboard voxel face.
        // A single cell vertex would weld them, but the positive saddle keeps
        // the negative regions separate. The old extractor rejected this case.
        let grid = Grid {
            dims: [7; 3],
            origin: Vec3::default(),
            step: 1.,
        };
        for scale in [1e-6, 1., 1e6] {
            let mut field = vec![scale; 343];
            field[grid.index(2, 2, 2)] = -0.25 * scale;
            field[grid.index(3, 3, 2)] = -0.25 * scale;
            let mesh = extract(&grid, &field, &|| false).unwrap();
            mesh.require_adaptive_quads().unwrap();
            assert_eq!(mesh.quads.len(), 12);
            assert_eq!(mesh.vertices.len(), 16);
            let surface = crate::sculpt::projection::Surface::new(&mesh, &|| false).unwrap();
            assert_eq!(
                surface
                    .components
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                2
            );
            assert!(
                mesh.quads.iter().all(|q| {
                    let first = mesh.vertices[q[0]];
                    q.iter().all(|&v| (mesh.vertices[v] - first).len() < 1.)
                }),
                "separate regions must not be bridged"
            );
        }
    }
    #[test]
    fn quad_remesh_still_rejects_a_nonmanifold_two_cell_tunnel() {
        // The other saddle connection makes one contour loop in each of two
        // neighbouring cells, with four faces sharing the resulting dual edge.
        // Face consistency alone is insufficient: never return that mesh.
        let grid = Grid {
            dims: [7; 3],
            origin: Vec3::default(),
            step: 1.,
        };
        let mut field = vec![1.; 343];
        field[grid.index(2, 2, 2)] = -4.;
        field[grid.index(3, 3, 2)] = -4.;
        assert!(
            extract(&grid, &field, &|| false)
                .unwrap_err()
                .contains("nearby sheets")
        );
    }
    #[test]
    fn quad_remesh_preserves_sphere_shape_and_attributes() {
        let mut mesh = Mesh::sphere();
        for (v, w) in mesh.vertices.iter().zip(&mut mesh.mask) {
            if v.0 > 0. {
                *w = 1.;
            }
        }
        mesh.create_vertex_group("Right".into(), 0.5, false)
            .unwrap();
        mesh.materials = vec![
            crate::sculpt::Material::from_srgb([0.9, 0.1, 0.2], 0.3, 0.7);
            mesh.vertices.len()
        ];
        let start = std::time::Instant::now();
        for iteration in 0..3 {
            mesh = mesh
                .remesh_topology_cancellable(24, Topology::Quads, &|| false)
                .unwrap();
            let error = mesh
                .vertices
                .iter()
                .map(|v| (v.len() - 1.).abs())
                .sum::<f32>()
                / mesh.vertices.len() as f32;
            let normal_error = mesh
                .vertices
                .iter()
                .zip(mesh.normals())
                .map(|(p, n)| 1. - p.norm().dot(n))
                .sum::<f32>()
                / mesh.vertices.len() as f32;
            eprintln!(
                "quad sphere pass {iteration}: {} quads, mean radius error {error:.6}, mean normal error {normal_error:.6}, elapsed {:?}",
                mesh.quads.len(),
                start.elapsed()
            );
            assert!(error < 0.015);
            assert!(normal_error < 0.01);
            assert_eq!(mesh.vertex_groups[0].name, "Right");
            assert!(mesh.vertex_groups[0].weights.iter().any(|&w| w > 0.9));
            assert!(mesh.mask.iter().all(|&w| w == 0.));
            assert!(!mesh.materials.is_empty());
            assert!(
                mesh.materials
                    .iter()
                    .all(|m| (m.metallic - 0.7).abs() < 1e-5 && (m.roughness - 0.3).abs() < 1e-5)
            );
            mesh.validate_closed().unwrap();
        }
    }
}

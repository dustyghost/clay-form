use super::*;
use crate::sculpt::{Primitive, PrimitiveKind, Symmetry};

fn shells(closed: bool) -> Mesh {
    let (vertices, triangles) = if closed {
        (
            vec![
                Vec3(-0.1, 0., 0.),
                Vec3(-1., -1., -1.),
                Vec3(-1., 1., -1.),
                Vec3(-1., 0., 1.),
                Vec3(0.1, 0., 0.),
                Vec3(1., -1., -1.),
                Vec3(1., 1., -1.),
                Vec3(1., 0., 1.),
            ],
            vec![
                [0, 1, 2],
                [0, 2, 3],
                [0, 3, 1],
                [1, 3, 2],
                [4, 6, 5],
                [4, 7, 6],
                [4, 5, 7],
                [5, 6, 7],
            ],
        )
    } else {
        (
            vec![
                Vec3(-0.1, 0., 0.),
                Vec3(-1., -1., 0.),
                Vec3(-1., 1., 0.),
                Vec3(0.1, 0., 0.),
                Vec3(1., 1., 0.),
                Vec3(1., -1., 0.),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
        )
    };
    Mesh {
        mask: vec![0.; vertices.len()],
        vertices,
        triangles,
        ..Default::default()
    }
}

// Independent, intentionally simple oracle for tiny generated meshes. It scans
// triangles and floods undirected links, rather than using production corner
// adjacency or the directed-link walk in manifold().
fn surface_invariants(mesh: &Mesh) -> bool {
    let mut faces = BTreeSet::new();
    let mut incidence = BTreeMap::<[usize; 2], (usize, i32)>::new();
    for &t in &mesh.triangles {
        let mut key = t;
        key.sort_unstable();
        if !faces.insert(key) || key.windows(2).any(|p| p[0] == p[1]) {
            return false;
        }
        let [a, b, c] = t.map(|v| mesh.vertices[v]);
        if (b - a).cross(c - a).len() <= 1e-12 {
            return false;
        }
        for i in 0..3 {
            let (a, b) = (t[i], t[(i + 1) % 3]);
            let entry = incidence.entry([a.min(b), a.max(b)]).or_default();
            entry.0 += 1;
            entry.1 += if a < b { 1 } else { -1 };
        }
    }
    if incidence
        .values()
        .any(|&(count, winding)| count > 2 || (count == 2 && winding != 0))
    {
        return false;
    }
    for v in 0..mesh.vertices.len() {
        let mut link = BTreeMap::<usize, BTreeSet<usize>>::new();
        for t in &mesh.triangles {
            if t.contains(&v) {
                let others: Vec<_> = t.iter().copied().filter(|&w| w != v).collect();
                link.entry(others[0]).or_default().insert(others[1]);
                link.entry(others[1]).or_default().insert(others[0]);
            }
        }
        if link.values().any(|neighbors| neighbors.len() > 2) {
            return false;
        }
        let ends = link
            .values()
            .filter(|neighbors| neighbors.len() == 1)
            .count();
        if ends != 0 && ends != 2 {
            return false;
        }
        if let Some(&start) = link.keys().next() {
            let mut visited = BTreeSet::new();
            let mut stack = vec![start];
            while let Some(w) = stack.pop() {
                if visited.insert(w) {
                    stack.extend(&link[&w]);
                }
            }
            if visited.len() != link.len() {
                return false;
            }
        }
    }
    true
}

#[test]
fn modeling_fan_gate_rejects_closed_and_open_pinched_welds() {
    for closed in [false, true] {
        let mesh = shells(closed);
        let other = if closed { 4 } else { 3 };
        assert!(surface_invariants(&mesh));
        manifold(&mesh).unwrap();
        let symmetry = ModelingSymmetry::new(&mesh, &Symmetry::default()).unwrap();
        let error = mesh
            .edit_model_topology(TopologyEdit::Weld, &[], &[], &[0, other], &symmetry)
            .unwrap_err();
        assert!(error.contains("connected surface fan"), "{error}");

        // Demonstrate that the independent oracle detects the old accepted result.
        let mut pinched = mesh.clone();
        pinched.vertices[0] = Vec3::default();
        for t in &mut pinched.triangles {
            *t = t.map(|v| if v == other { 0 } else { v });
        }
        assert!(!surface_invariants(&pinched));
        assert!(
            manifold(&pinched)
                .unwrap_err()
                .contains("connected surface fan")
        );
    }
}

#[test]
fn modeling_fan_gate_keeps_open_boundary_welds_and_explicit_wire_branches() {
    let mesh = Mesh {
        vertices: vec![
            Vec3(-1., -1., 0.),
            Vec3(1., -1., 0.),
            Vec3(1., 1., 0.),
            Vec3(-1., 1., 0.),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        mask: vec![0.; 4],
        ..Default::default()
    };
    let symmetry = ModelingSymmetry::new(&mesh, &Symmetry::default()).unwrap();
    let result = mesh
        .edit_model_topology(TopologyEdit::Weld, &[], &[], &[0, 1], &symmetry)
        .unwrap();
    assert_eq!(result.triangles.len(), 1);
    assert!(surface_invariants(&result));
    manifold(&result).unwrap();

    let mut mixed = mesh;
    mixed.vertices.extend([
        Vec3(2., 2., 0.),
        Vec3(3., 2., 0.),
        Vec3(2., 3., 0.),
        Vec3(4., 4., 0.),
    ]);
    mixed.mask.resize(8, 0.);
    mixed.loose_edges = vec![[2, 4], [4, 5], [4, 6]];
    mixed.loose_vertices = vec![7];
    manifold(&mixed).unwrap();
    mixed.triangles.clear();
    mixed.loose_vertices = vec![0, 1, 3, 7];
    manifold(&mixed).unwrap();
}

fn fan(sides: usize) -> Mesh {
    let mut vertices = vec![Vec3::default()];
    for i in 0..sides {
        let angle = std::f32::consts::TAU * i as f32 / sides as f32;
        vertices.push(Vec3(angle.cos(), angle.sin(), 0.));
    }
    Mesh {
        mask: vec![0.; vertices.len()],
        vertices,
        triangles: (0..sides)
            .map(|i| [0, i + 1, (i + 1) % sides + 1])
            .collect(),
        ..Default::default()
    }
}

#[test]
fn modeling_bounded_edit_sequences_preserve_independent_surface_invariants() {
    let mut grid = Primitive::for_kind(PrimitiveKind::Grid);
    grid.divisions = [3, 3, 1];
    let sources = [
        Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap(),
        grid.mesh().unwrap(),
        fan(32),
    ];
    // 36 tiny cases, six edits each: closed/open, ordinary/high valence,
    // reversed face order, three scales and exact mirror correspondence.
    for (fixture, source) in sources.into_iter().enumerate() {
        for scale in [0.01, 1., 100.] {
            for reverse in [false, true] {
                for mirrored in [false, true] {
                    let mut mesh = source.clone();
                    for p in &mut mesh.vertices {
                        *p = *p * scale;
                    }
                    if reverse {
                        mesh.triangles.reverse();
                        mesh.quads.reverse();
                    }
                    mesh.mask.fill(0.25);
                    mesh.create_vertex_group("Sequence".into(), 0., true)
                        .unwrap();
                    let settings = Symmetry {
                        enabled: mirrored,
                        ..Default::default()
                    };
                    let mut seed = 31 + fixture;
                    let mut accepted = 0;
                    for step in 0..6 {
                        mesh.validate().unwrap();
                        assert!(surface_invariants(&mesh));
                        let symmetry = ModelingSymmetry::new(&mesh, &settings).unwrap();
                        let edges = mesh.polygon_edges();
                        seed = (seed * 17 + 11) % 997;
                        let edge = edges[seed % edges.len()];
                        let edit = if step % 2 == 0 {
                            TopologyEdit::Split
                        } else {
                            TopologyEdit::Weld
                        };
                        if let Ok(next) =
                            mesh.edit_model_topology(edit, &[], &[edge], &edge, &symmetry)
                        {
                            next.validate().unwrap();
                            assert!(
                                surface_invariants(&next),
                                "fixture={fixture}, scale={scale}, reverse={reverse}, mirrored={mirrored}, step={step}"
                            );
                            assert!(next.mask.iter().all(|&m| m == 0.25));
                            assert!(next.vertex_groups[0].weights.iter().all(|&w| w == 1.));
                            let restored: Mesh =
                                serde_json::from_slice(&serde_json::to_vec(&next).unwrap())
                                    .unwrap();
                            assert_eq!(restored, next);
                            assert!(surface_invariants(&restored));
                            mesh = next;
                            accepted += 1;
                        }
                    }
                    assert!(
                        accepted >= 1,
                        "each generated case must exercise a successful edit"
                    );
                }
            }
        }
    }
}

use super::*;
use crate::sculpt::{Primitive, PrimitiveKind};
fn cube(n: u32) -> Mesh {
    let mut p = Primitive::for_kind(PrimitiveKind::Cube);
    p.divisions = [n; 3];
    p.mesh().unwrap()
}
fn front_center(mesh: &Mesh) -> usize {
    mesh.polygons()
        .iter()
        .enumerate()
        .filter(|(_, p)| normal(mesh, p).2 > 0.9)
        .min_by(|(_, a), (_, b)| {
            let center = |p: &Polygon| {
                p.vertices
                    .iter()
                    .fold(Vec3::default(), |s, &i| s + mesh.vertices[i])
                    * (1. / p.vertices.len() as f32)
            };
            center(a).len().total_cmp(&center(b).len())
        })
        .unwrap()
        .0
}
#[test]
fn modeling_extrudes_closed_quads_with_attributes() {
    let mut mesh = cube(1);
    mesh.mask.fill(0.4);
    mesh.create_vertex_group("Base".into(), 0., true).unwrap();
    mesh.creases = mesh.polygon_edges();
    let face = front_center(&mesh);
    let result = mesh.extrude_faces(&[face], 0.5, false).unwrap();
    assert!(!result.joined);
    assert_eq!(result.mesh.quads.len(), mesh.quads.len() + 4);
    result.mesh.validate_closed().unwrap();
    manifold(&result.mesh).unwrap();
    assert!(result.mesh.mask.iter().all(|&m| m == 0.4));
    assert!(
        result.mesh.vertex_groups[0]
            .weights
            .iter()
            .all(|&w| w == 1.)
    );
    assert_eq!(mesh.extrude_faces(&[face], 0., true).unwrap().mesh, mesh);
}
#[test]
fn modeling_tunnel_removes_caps_and_welds_closed_surface() {
    let mesh = cube(3);
    let face = front_center(&mesh);
    let result = mesh.extrude_faces(&[face], -2.1, true).unwrap();
    assert!(result.joined);
    assert!((result.distance + 2.).abs() < 1e-5);
    assert_eq!(result.mesh.vertices.len(), mesh.vertices.len());
    assert_eq!(result.mesh.quads.len(), mesh.quads.len() + 2);
    result.mesh.validate_closed().unwrap();
    manifold(&result.mesh).unwrap();
    // A tunnel changes Euler characteristic from 2 (sphere) to 0 (torus).
    assert_eq!(
        result.mesh.vertices.len() + result.mesh.quads.len(),
        result.mesh.polygon_edges().len()
    );
    let no_join = mesh.extrude_faces(&[face], -0.5, true).unwrap();
    assert!(!no_join.joined);
    let disabled = mesh.extrude_faces(&[face], -1., false).unwrap();
    assert!(!disabled.joined);
}
#[test]
fn modeling_rejects_collapsed_cube_and_invalid_regions() {
    let mesh = cube(1);
    assert!(
        mesh.extrude_faces(&[front_center(&mesh)], -2., true)
            .is_err()
    );
    assert!(mesh.extrude_faces(&[usize::MAX], 1., true).is_err());
    assert!(mesh.extrude_faces(&[0], f32::NAN, false).is_err());
    assert!(
        mesh.extrude_faces(&(0..6).collect::<Vec<_>>(), 1., false)
            .is_err()
    );
}
#[test]
fn modeling_loop_cut_reuses_midpoints_and_preserves_creases() {
    let mut mesh = cube(1);
    mesh.creases = mesh.polygon_edges();
    let out = mesh.insert_edge_loop(mesh.polygon_edges()[0]).unwrap();
    assert_eq!(out.vertices.len(), 12);
    assert_eq!(out.quads.len(), 10);
    assert_eq!(out.creases.len(), 16);
    out.validate_closed().unwrap();
    manifold(&out).unwrap();
    let mut hidden = mesh.clone();
    hidden.hidden_faces = vec![true; mesh.triangles.len()];
    assert!(hidden.insert_edge_loop(mesh.polygon_edges()[0]).is_err());
    let mut triangles = mesh.clone();
    triangles.quads.clear();
    assert!(triangles.insert_edge_loop(mesh.polygon_edges()[0]).is_err());
}

#[test]
fn modeling_flat_region_has_no_internal_walls_and_corner_contacts_are_rejected() {
    let mut p = Primitive::for_kind(PrimitiveKind::Grid);
    p.divisions = [2, 2, 1];
    let mesh = p.mesh().unwrap();
    let faces = mesh.polygons();
    let adjacent = (1..faces.len())
        .find(|&i| {
            faces[i]
                .vertices
                .iter()
                .filter(|v| faces[0].vertices.contains(v))
                .count()
                == 2
        })
        .unwrap();
    let corner = (1..faces.len())
        .find(|&i| {
            faces[i]
                .vertices
                .iter()
                .filter(|v| faces[0].vertices.contains(v))
                .count()
                == 1
        })
        .unwrap();
    let out = mesh.extrude_faces(&[0, adjacent], 0.3, false).unwrap().mesh;
    assert_eq!(out.quads.len(), mesh.quads.len() + 6);
    manifold(&out).unwrap();
    assert!(mesh.extrude_faces(&[0, corner], 0.3, false).is_err());
    let all = mesh
        .extrude_faces(&(0..faces.len()).collect::<Vec<_>>(), 0.3, false)
        .unwrap()
        .mesh;
    // The original interior vertex is removed; all emitted vertices are used.
    assert_eq!(
        all.triangles
            .iter()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        all.vertices.len()
    );
}

fn front_at(mesh: &Mesh, x: f32, y: f32) -> usize {
    mesh.polygons()
        .iter()
        .position(|p| {
            let c = p
                .vertices
                .iter()
                .fold(Vec3::default(), |s, &i| s + mesh.vertices[i])
                * (1. / p.vertices.len() as f32);
            normal(mesh, p).2 > 0.99 && (c.0 - x).abs() < 1e-5 && (c.1 - y).abs() < 1e-5
        })
        .expect("front face")
}
fn slab() -> Mesh {
    let mut mesh = cube(4);
    for p in &mut mesh.vertices {
        p.2 *= 0.1;
    }
    mesh
}
#[test]
fn modeling_boundary_join_removes_subdivided_outside_wall() {
    let mesh = slab();
    let joined = mesh
        .extrude_faces(&[front_at(&mesh, 0.25, -0.75)], -0.21, true)
        .unwrap();
    assert!(joined.joined);
    joined.mesh.validate_closed().unwrap();
    manifold(&joined.mesh).unwrap();
    assert!(
        !joined.mesh.polygons().iter().any(|p| {
            p.vertices.iter().all(|&i| {
                let v = joined.mesh.vertices[i];
                (v.1 + 1.).abs() < 1e-5 && v.0 >= -1e-5 && v.0 <= 0.5 + 1e-5
            })
        }),
        "the entire bottom wall of the notch must disappear, including its depth loops"
    );
}
#[test]
fn modeling_adjacent_tunnel_joins_remove_shared_wall() {
    let mesh = slab();
    let first = mesh
        .extrude_faces(&[front_at(&mesh, 0.25, 0.25)], -0.21, true)
        .unwrap()
        .mesh;
    let second = first
        .extrude_faces(&[front_at(&first, -0.25, 0.25)], -0.21, true)
        .unwrap()
        .mesh;
    second.validate_closed().unwrap();
    manifold(&second).unwrap();
    assert!(
        !second.polygons().iter().any(|p| p
            .vertices
            .iter()
            .all(|&i| second.vertices[i].0.abs() < 1e-5
                && second.vertices[i].1 >= -1e-5
                && second.vertices[i].1 <= 0.5 + 1e-5)),
        "no shared wall between adjacent holes"
    );
}

fn check_joined_surface(mesh: &Mesh, removed_cells: usize) {
    mesh.validate_closed().unwrap();
    manifold(mesh).unwrap();
    let volume: f32 = mesh
        .triangles
        .iter()
        .map(|&[a, b, c]| mesh.vertices[a].dot(mesh.vertices[b].cross(mesh.vertices[c])) / 6.)
        .sum();
    assert!(
        (volume - (0.8 - 0.05 * removed_cells as f32)).abs() < 1e-5,
        "volume {volume}"
    );
    assert_eq!(
        mesh.triangles
            .iter()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        mesh.vertices.len()
    );
}
#[test]
fn modeling_corner_and_repeated_boundary_joins_stay_closed_and_quad_based() {
    let mut mesh = slab();
    for (i, (x, y)) in [(0.75, -0.75), (0.25, -0.75), (-0.25, -0.75), (-0.75, -0.75)]
        .into_iter()
        .enumerate()
    {
        let result = mesh
            .extrude_faces(&[front_at(&mesh, x, y)], -0.21, true)
            .unwrap();
        assert!(result.joined);
        mesh = result.mesh;
        check_joined_surface(&mesh, i + 1);
        assert_eq!(
            mesh.quads.len() * 2,
            mesh.triangles.len(),
            "regular depth strips should stay quads"
        );
        assert_eq!(
            mesh.vertices.len() + mesh.polygons().len(),
            mesh.polygon_edges().len() + 2,
            "notches must not leave tunnel walls or handles"
        );
    }
    assert!(
        mesh.vertices.iter().all(|v| v.1 >= -0.5 - 1e-5),
        "the removed bottom row must leave no rim vertices"
    );
}
#[test]
fn modeling_join_conforms_to_unequal_depth_loops_and_preserves_attributes() {
    let mut mesh = slab();
    for v in &mut mesh.vertices {
        if v.0.abs() < 1e-5 && (v.1 + 1.).abs() < 1e-5 && (v.2 + 0.05).abs() < 1e-5 {
            v.2 = -0.04;
        }
    }
    for (mask, v) in mesh.mask.iter_mut().zip(&mesh.vertices) {
        *mask = (v.2 + 0.1) / 0.2;
    }
    mesh.create_vertex_group("Depth".into(), 0., false).unwrap();
    mesh.creases = mesh.polygon_edges();
    let result = mesh
        .extrude_faces(&[front_at(&mesh, 0.25, -0.75)], -0.21, true)
        .unwrap();
    check_joined_surface(&result.mesh, 1);
    assert!(!result.mesh.creases.is_empty());
    for ((v, mask), weight) in result
        .mesh
        .vertices
        .iter()
        .zip(&result.mesh.mask)
        .zip(&result.mesh.vertex_groups[0].weights)
    {
        let expected = (v.2 + 0.1) / 0.2;
        assert!((*mask - expected).abs() < 1e-5);
        assert!((*weight - expected).abs() < 1e-5);
    }
}
#[test]
fn modeling_join_will_not_remove_hidden_side_patches() {
    let mut mesh = slab();
    mesh.hidden_faces = mesh
        .triangles
        .iter()
        .map(|t| t.iter().all(|&v| (mesh.vertices[v].1 + 1.).abs() < 1e-5))
        .collect();
    let error = mesh
        .extrude_faces(&[front_at(&mesh, 0.25, -0.75)], -0.21, true)
        .err()
        .unwrap();
    assert!(error.contains("Show the adjoining"), "{error}");
}

#[test]
fn modeling_join_connects_holes_with_different_wall_subdivisions() {
    let mut mesh = slab();
    for (i, x) in [-0.75, 0.25, -0.25].into_iter().enumerate() {
        mesh = mesh
            .extrude_faces(&[front_at(&mesh, x, 0.25)], -0.21, true)
            .unwrap()
            .mesh;
        check_joined_surface(&mesh, i + 1);
    }
    assert_eq!(
        mesh.vertices.len() + mesh.polygons().len(),
        mesh.polygon_edges().len() + 2
    );
}

#[test]
fn modeling_multiple_sliding_loops_follow_oriented_ring_and_interpolate_attributes() {
    let mut mesh = cube(1);
    mesh.creases = mesh.polygon_edges();
    let seed = mesh.creases[0];
    let a = mesh.vertices[seed[0]];
    let axis = (mesh.vertices[seed[1]] - a) * 0.5;
    mesh.mask = mesh
        .vertices
        .iter()
        .map(|p| ((*p - a).dot(axis) * 0.5).clamp(0., 1.))
        .collect();
    mesh.create_vertex_group("Gradient".into(), 0., true)
        .unwrap();
    mesh.vertex_groups[0].weights = mesh.mask.clone();
    let cut = mesh.insert_edge_loops(seed, 3, 0.6).unwrap();
    assert_eq!(
        (
            cut.mesh.vertices.len(),
            cut.mesh.quads.len(),
            cut.mesh.creases.len(),
            cut.cuts.len()
        ),
        (20, 18, 24, 12)
    );
    cut.mesh.validate_closed().unwrap();
    manifold(&cut.mesh).unwrap();
    for (i, p) in cut.mesh.vertices.iter().enumerate().skip(8) {
        let t = (*p - a).dot(axis) * 0.5;
        assert!([0.35, 0.6, 0.85].iter().any(|x| (t - x).abs() < 1e-5));
        assert!((cut.mesh.mask[i] - t).abs() < 1e-5);
        assert!((cut.mesh.vertex_groups[0].weights[i] - t).abs() < 1e-5);
    }
    let reverse = mesh.insert_edge_loops([seed[1], seed[0]], 3, 0.4).unwrap();
    for p in &cut.mesh.vertices {
        assert!(reverse.mesh.vertices.iter().any(|q| (*p - *q).len() < 1e-5));
    }
    for position in [-5., 5.] {
        mesh.insert_edge_loops(seed, 16, position)
            .unwrap()
            .mesh
            .validate_closed()
            .unwrap();
    }
    for count in [0, 17] {
        assert!(mesh.insert_edge_loops(seed, count, 0.5).is_err());
    }
    assert!(mesh.insert_edge_loops(seed, 1, f32::NAN).is_err());
}

#[test]
fn modeling_selection_loops_and_parallel_rings_choose_distinct_directions() {
    let mut p = Primitive::for_kind(PrimitiveKind::Grid);
    p.divisions = [4, 3, 1];
    let mesh = p.mesh().unwrap();
    let seed = [7, 12]; // Interior vertical edge in the middle row.
    assert_eq!(
        mesh.select_edge_ring(seed).unwrap(),
        vec![[5, 10], [6, 11], [7, 12], [8, 13], [9, 14]]
    );
    assert_eq!(
        mesh.expand_edge_loop(&[seed]).unwrap(),
        vec![[2, 7], [7, 12], [12, 17]]
    );
    assert_eq!(mesh.select_face_loop(5, seed).unwrap(), vec![4, 5, 6, 7]);
    assert_eq!(mesh.select_face_loop(5, [6, 7]).unwrap(), vec![1, 5, 9]);
    assert!(mesh.select_face_loop(5, [0, 1]).is_err());
    let mut hidden = mesh.clone();
    hidden.hidden_faces = vec![false; mesh.triangles.len()];
    hidden.hidden_faces[12] = true;
    hidden.hidden_faces[13] = true; // Face 6 blocks the rightward path.
    assert_eq!(hidden.select_face_loop(5, seed).unwrap(), vec![4, 5]);
    assert_eq!(
        hidden.select_edge_ring(seed).unwrap(),
        vec![[5, 10], [6, 11], [7, 12]]
    );
    let mut triangles = mesh.clone();
    triangles.quads.remove(6);
    assert_eq!(
        triangles.select_edge_ring(seed).unwrap(),
        vec![[5, 10], [6, 11], [7, 12]]
    );
    let cube = cube(3);
    let face = front_center(&cube);
    let p = &cube.polygons()[face];
    let seed = [p.vertices[0], p.vertices[1]];
    assert_eq!(cube.select_face_loop(face, seed).unwrap().len(), 12);
    assert_eq!(cube.select_edge_ring(seed).unwrap().len(), 12);
}

#[test]
fn modeling_dissolve_reverses_single_and_multiple_loop_cuts_with_attributes() {
    let mut base = cube(1);
    base.creases = base.polygon_edges();
    base.mask.fill(0.4);
    base.create_vertex_group("Base".into(), 0., true).unwrap();
    for count in [1, 3] {
        let result = base
            .insert_edge_loops(base.polygon_edges()[0], count, 0.5)
            .unwrap();
        let cut = result.mesh;
        let selected: Vec<_> = cut
            .polygon_edges()
            .into_iter()
            .filter(|e| {
                result.cuts.iter().any(|segment| {
                    segment.contains(&cut.vertices[e[0]]) && segment.contains(&cut.vertices[e[1]])
                })
            })
            .collect();
        let out = cut.dissolve_loops(&selected).unwrap();
        assert_eq!(out.vertices, base.vertices);
        assert_eq!(out.quads.len(), base.quads.len());
        assert_eq!(out.mask, base.mask);
        assert_eq!(out.vertex_groups, base.vertex_groups);
        assert_eq!(out.creases, base.creases);
        out.validate_closed().unwrap();
        manifold(&out).unwrap();
        assert!(cut.dissolve_loops(&selected[..1]).is_err());
        let mut hidden = cut.clone();
        hidden.hidden_faces.fill(true);
        assert!(hidden.dissolve_loops(&selected).is_err());
    }
    let mut p = Primitive::for_kind(PrimitiveKind::Grid);
    p.divisions = [4, 4, 1];
    let grid = p.mesh().unwrap();
    let selected: Vec<_> = grid
        .polygon_edges()
        .into_iter()
        .filter(|e| e.iter().all(|&v| grid.vertices[v].0.abs() < 1e-6))
        .collect();
    let out = grid.dissolve_loops(&selected).unwrap();
    assert_eq!((out.vertices.len(), out.quads.len()), (20, 12));
    manifold(&out).unwrap();
    let mut crossed = selected;
    crossed.extend(
        grid.polygon_edges()
            .into_iter()
            .filter(|e| e.iter().all(|&v| grid.vertices[v].1.abs() < 1e-6)),
    );
    assert!(grid.dissolve_loops(&crossed).is_err());
    assert!(grid.dissolve_loops(&[]).is_err());
    assert!(grid.dissolve_loops(&[[0, 1]]).is_err());
}
fn mirror_x() -> crate::sculpt::Symmetry {
    crate::sculpt::Symmetry {
        enabled: true,
        ..Default::default()
    }
}
#[test]
fn modeling_symmetry_checks_geometry_topology_visibility_and_supports_radial() {
    let mesh = cube(2);
    let settings = mirror_x();
    let symmetry = ModelingSymmetry::new(&mesh, &settings).unwrap();
    for (i, &p) in mesh.vertices.iter().enumerate() {
        assert!(
            symmetry
                .vertex_copies(i)
                .iter()
                .any(|&v| (mesh.vertices[v] - Vec3(-p.0, p.1, p.2)).len() < 1e-5)
        );
    }
    let mut asymmetric = mesh.clone();
    asymmetric.vertices[0].0 += 0.1;
    assert!(ModelingSymmetry::new(&asymmetric, &settings).is_err());
    let mut topology = mesh.clone();
    topology.quads.remove(0);
    assert!(ModelingSymmetry::new(&topology, &settings).is_err());
    let mut hidden = mesh.clone();
    hidden.hidden_faces = vec![false; mesh.triangles.len()];
    hidden.hidden_faces[0] = true;
    assert!(ModelingSymmetry::new(&hidden, &settings).is_err());
    let radial = crate::sculpt::Symmetry {
        enabled: true,
        mirror: [false; 3],
        radial: true,
        radial_axis: crate::sculpt::Axis::Z,
        radial_count: 4,
        ..Default::default()
    };
    assert_eq!(
        ModelingSymmetry::new(&mesh, &radial)
            .unwrap()
            .face_copies(0)
            .len(),
        4
    );
    let mut shifted = mesh.clone();
    for p in &mut shifted.vertices {
        *p = *p + Vec3(2., 3., 4.);
    }
    ModelingSymmetry::new(
        &shifted,
        &crate::sculpt::Symmetry {
            center: Vec3(2., 3., 4.),
            ..settings
        },
    )
    .unwrap();
}
#[test]
fn modeling_symmetric_extrusions_cuts_and_dissolve_remain_symmetric() {
    let mesh = cube(3);
    let settings = mirror_x();
    let symmetry = ModelingSymmetry::new(&mesh, &settings).unwrap();
    let face = front_at(&mesh, 2. / 3., 0.);
    let extruded = mesh
        .extrude_symmetric(&[face], 0.3, false, &symmetry)
        .unwrap();
    assert_eq!(extruded.mesh.quads.len(), mesh.quads.len() + 8);
    symmetry.validate_result(&extruded.mesh).unwrap();
    extruded.mesh.validate_closed().unwrap();
    let tunnel = mesh
        .extrude_symmetric(&[face], -2., true, &symmetry)
        .unwrap();
    assert!(tunnel.joined);
    symmetry.validate_result(&tunnel.mesh).unwrap();
    tunnel.mesh.validate_closed().unwrap();
    // A cut through the symmetry plane generates both off-center positions,
    // and a centered cut is deduplicated instead of cut twice.
    let base = cube(1);
    let symmetry = ModelingSymmetry::new(&base, &settings).unwrap();
    let seed = base
        .polygon_edges()
        .into_iter()
        .find(|e| (base.vertices[e[0]].0 - base.vertices[e[1]].0).abs() > 1.)
        .unwrap();
    for (position, vertices) in [(0.5, 12), (0.3, 16)] {
        let cut = base
            .insert_symmetric_edge_loops(seed, 1, position, &symmetry)
            .unwrap()
            .mesh;
        assert_eq!(cut.vertices.len(), vertices);
        cut.validate_closed().unwrap();
        let selected: Vec<_> = cut
            .polygon_edges()
            .into_iter()
            .filter(|e| e.iter().all(|&v| v >= 8 && cut.vertices[v].0 <= 0.01))
            .collect();
        let maps = ModelingSymmetry::new(&cut, &settings).unwrap();
        let dissolved = cut.dissolve_loops(&maps.edges(&selected)).unwrap();
        assert_eq!((dissolved.vertices.len(), dissolved.quads.len()), (8, 6));
        maps.validate_result(&dissolved).unwrap();
    }
    let radial = crate::sculpt::Symmetry {
        enabled: true,
        mirror: [false; 3],
        radial: true,
        radial_axis: crate::sculpt::Axis::Z,
        radial_count: 4,
        ..Default::default()
    };
    let maps = ModelingSymmetry::new(&base, &radial).unwrap();
    let cut = base
        .insert_symmetric_edge_loops(seed, 1, 0.3, &maps)
        .unwrap();
    cut.mesh.validate_closed().unwrap();
    maps.validate_result(&cut.mesh).unwrap();
}

#[test]
fn modeling_symmetric_adjacent_caps_join_and_center_regions_extrude_once() {
    let mesh = slab();
    let symmetry = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let face = front_at(&mesh, 0.25, 0.25);
    let result = mesh
        .extrude_symmetric(&[face], -0.21, true, &symmetry)
        .unwrap();
    assert!(result.joined);
    result.mesh.validate_closed().unwrap();
    symmetry.validate_result(&result.mesh).unwrap();
    let result = mesh
        .extrude_symmetric(&[face], 0.2, false, &symmetry)
        .unwrap();
    assert_eq!(result.mesh.quads.len(), mesh.quads.len() + 6);
    result.mesh.validate_closed().unwrap();
    let mut settings = mirror_x();
    settings.mirror = [true, true, true];
    let mesh = cube(3);
    let symmetry = ModelingSymmetry::new(&mesh, &settings).unwrap();
    let result = mesh
        .extrude_symmetric(&[front_center(&mesh)], -2., true, &symmetry)
        .unwrap();
    assert!(result.joined);
    result.mesh.validate_closed().unwrap();
    symmetry.validate_result(&result.mesh).unwrap();
}

#[test]
fn component_transforms_preserve_attributes_and_mirror_displacements() {
    let mesh = cube(3);
    let sym = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let selected: Vec<_> = mesh
        .vertices
        .iter()
        .enumerate()
        .filter(|(_, p)| p.0 > 0.9 && p.1 > 0.2)
        .map(|(v, _)| v)
        .collect();
    let out = mesh
        .transform_components(
            &selected,
            Vec3(1., 1., 0.),
            ComponentTransform::Move(Vec3(0.15, 0., 0.)),
            false,
            &sym,
        )
        .unwrap();
    sym.validate_result(&out).unwrap();
    for &v in &selected {
        assert!((out.vertices[v].0 - mesh.vertices[v].0 - 0.15).abs() < 1e-5);
    }
    assert_eq!(out.quads, mesh.quads);
    assert_eq!(out.mask, mesh.mask);
    let face = front_center(&mesh);
    let vertices = mesh.polygons()[face].vertices.clone();
    let identity = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    for op in [
        ComponentTransform::Rotate(Vec3(0., 0., 1.), 0.1),
        ComponentTransform::Scale(Vec3::default(), 0.8),
    ] {
        let out = mesh
            .transform_components(&vertices, Vec3(0., 0., 1.), op, false, &identity)
            .unwrap();
        manifold(&out).unwrap();
        assert_ne!(out.vertices, mesh.vertices);
    }
    assert!(
        mesh.transform_components(
            &vertices,
            Vec3::default(),
            ComponentTransform::Scale(Vec3::default(), 0.),
            false,
            &identity
        )
        .is_err()
    );
}
#[test]
fn modeling_protection_rejects_whole_edits() {
    let mut mesh = cube(3);
    let f = front_center(&mesh);
    let vertices = mesh.polygons()[f].vertices.clone();
    mesh.mask[vertices[0]] = 1.;
    let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    assert!(
        mesh.transform_components(
            &vertices,
            Vec3::default(),
            ComponentTransform::Move(Vec3(0., 0., 0.1)),
            true,
            &sym
        )
        .is_err()
    );
    let extruded = mesh.extrude_faces(&[f], 0.1, false).unwrap();
    assert!(mesh.check_model_protection(&extruded.mesh, true).is_err());
    mesh.check_model_protection(&extruded.mesh, false).unwrap();
}

#[test]
fn modeling_inset_bevel_and_region_extrude_are_closed() {
    let mesh = cube(1);
    let f = mesh
        .polygons()
        .iter()
        .position(|p| normal(&mesh, p).2 > 0.99)
        .unwrap();
    for bevel in [false, true] {
        let out = mesh.inset_faces(&[f], 0.1, bevel).unwrap();
        manifold(&out).unwrap();
        assert_eq!(out.polygons().len(), 10);
        let mut incidence = BTreeMap::<_, usize>::new();
        for p in out.polygons() {
            for (a, b) in p.edges() {
                *incidence.entry(edge(a, b)).or_default() += 1;
            }
        }
        assert!(incidence.values().all(|&n| n == 2));
    }
    assert!(mesh.inset_faces(&[f], 2., false).is_err());
    let faces: Vec<_> = mesh
        .polygons()
        .iter()
        .enumerate()
        .filter(|(_, p)| normal(&mesh, p).2 > 0.99 || normal(&mesh, p).0 > 0.99)
        .map(|(f, _)| f)
        .collect();
    let out = mesh.extrude_region(&faces, 0.1).unwrap();
    manifold(&out).unwrap();
    let sym = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let f = mesh
        .polygons()
        .iter()
        .position(|p| normal(&mesh, p).0 > 0.99)
        .unwrap();
    let out = mesh
        .construct_symmetric(&[f], 0.1, Construction::Inset, false, &sym)
        .unwrap();
    sym.validate_result(&out).unwrap();
}
#[test]
fn modeling_fuse_stops_at_intermediate_slab_loop() {
    let mut primitive = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
    primitive.divisions = [4, 4, 1];
    let mut mesh = primitive.mesh().unwrap();
    for v in &mut mesh.vertices {
        v.2 *= 0.1;
    }
    let f = mesh
        .polygons()
        .iter()
        .position(|p| {
            normal(&mesh, p).0 > 0.99
                && p.vertices.iter().map(|&v| mesh.vertices[v].1).sum::<f32>()
                    / (p.vertices.len() as f32)
                    < -0.5
        })
        .unwrap();
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    let result = mesh.extrude_contact(&[f], -0.8, contact).unwrap();
    assert!(result.collapsed);
    assert!(!result.joined);
    assert!((result.distance + 0.5).abs() < 1e-5);
    manifold(&result.mesh).unwrap();
    assert!(result.mesh.polygons().len() < mesh.polygons().len() + 4);
    assert!(!mesh.extrude_faces(&[f], -0.8, true).unwrap().joined);
    assert!(!mesh.extrude_contact(&[f], -0.2, contact).unwrap().collapsed);
    let sym = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let out = mesh
        .extrude_symmetric_contact(&[f], -0.8, contact, &sym)
        .unwrap();
    assert!(out.collapsed);
    sym.validate_result(&out.mesh).unwrap();
}

#[test]
fn modeling_topology_open_fill_bridge_connect_split_and_weld() {
    let mesh = cube(1);
    let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let f = mesh
        .polygons()
        .iter()
        .position(|p| normal(&mesh, p).2 > 0.99)
        .unwrap();
    let open = mesh
        .edit_model_topology(TopologyEdit::DeleteFaces, &[f], &[], &[], &sym)
        .unwrap();
    let border = open
        .border_edges(crate::sculpt::BorderTarget::Holes, 0.5, &[])
        .unwrap();
    let open_sym = ModelingSymmetry::new(&open, &Default::default()).unwrap();
    let filled = open
        .edit_model_topology(TopologyEdit::Fill, &[], &border, &[], &open_sym)
        .unwrap();
    assert_eq!(filled.quads.len(), 6);
    manifold(&filled).unwrap();
    assert!(
        open.edit_model_topology(TopologyEdit::Fill, &[], &border[..1], &[], &open_sym)
            .is_err()
    );
    let sides: Vec<_> = mesh
        .polygons()
        .iter()
        .enumerate()
        .filter(|(_, p)| normal(&mesh, p).2.abs() < 0.5)
        .map(|(f, _)| f)
        .collect();
    let caps = mesh
        .edit_model_topology(TopologyEdit::DeleteFaces, &sides, &[], &[], &sym)
        .unwrap();
    let edges = caps
        .border_edges(crate::sculpt::BorderTarget::Holes, 0.5, &[])
        .unwrap();
    let caps_sym = ModelingSymmetry::new(&caps, &Default::default()).unwrap();
    let bridge = caps
        .edit_model_topology(TopologyEdit::Bridge, &[], &edges, &[], &caps_sym)
        .unwrap();
    manifold(&bridge).unwrap();
    assert_eq!(bridge.quads.len(), 6);
    let p = &mesh.polygons()[f];
    let connected = mesh
        .edit_model_topology(
            TopologyEdit::Connect,
            &[],
            &[],
            &[p.vertices[0], p.vertices[2]],
            &sym,
        )
        .unwrap();
    assert_eq!(connected.quads.len(), 5);
    assert_eq!(connected.polygons().len(), 7);
    let e = [p.vertices[0], p.vertices[1]];
    let split = mesh
        .edit_model_topology(TopologyEdit::Split, &[], &[e], &[], &sym)
        .unwrap();
    assert_eq!(split.vertices.len(), mesh.vertices.len() + 1);
    manifold(&split).unwrap();
    for edit in [TopologyEdit::Weld, TopologyEdit::Collapse] {
        let welded = mesh.edit_model_topology(edit, &[], &[e], &e, &sym).unwrap();
        assert_eq!(welded.vertices.len(), mesh.vertices.len() - 1);
        manifold(&welded).unwrap();
    }
}
#[test]
fn modeling_slide_preserves_loops_and_mirrors_the_rails() {
    let mut p = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Grid);
    p.divisions = [5, 5, 1];
    let mesh = p.mesh().unwrap();
    let seed = mesh
        .polygon_edges()
        .into_iter()
        .find(|e| e.iter().all(|&v| (mesh.vertices[v].0 - 0.6).abs() < 1e-5))
        .unwrap();
    let edges = mesh.expand_edge_loop(&[seed]).unwrap();
    assert_eq!(edges.len(), 5);
    let sym = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let out = mesh
        .slide_model_edges(&edges, 0.25, Vec3(1., 0., 0.), Vec3(1., 0., 0.), &sym)
        .unwrap();
    sym.validate_result(&out).unwrap();
    assert_eq!(out.quads, mesh.quads);
    for v in edges.iter().flatten() {
        assert!((out.vertices[*v].0 - 0.7).abs() < 1e-5);
    }
    assert!(
        mesh.slide_model_edges(&edges[..1], 0.2, Vec3(1., 0., 0.), Vec3::default(), &sym)
            .is_err()
    );
    assert!(
        mesh.slide_model_edges(&edges, 1., Vec3(1., 0., 0.), Vec3::default(), &sym)
            .is_err()
    );
}
#[test]
fn modeling_connect_accepts_expanded_symmetry_selection() {
    let mesh = cube(3);
    let sym = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let f = mesh
        .polygons()
        .iter()
        .position(|p| {
            normal(&mesh, p).2 > 0.99 && p.vertices.iter().all(|&v| mesh.vertices[v].0 > 0.2)
        })
        .unwrap();
    let p = &mesh.polygons()[f];
    let selected: BTreeSet<_> = [p.vertices[0], p.vertices[2]]
        .into_iter()
        .flat_map(|v| sym.vertex_copies(v))
        .collect();
    let out = mesh
        .edit_model_topology(
            TopologyEdit::Connect,
            &[],
            &[],
            &selected.into_iter().collect::<Vec<_>>(),
            &sym,
        )
        .unwrap();
    sym.validate_result(&out).unwrap();
    assert_eq!(out.quads.len(), mesh.quads.len() - 2);
}

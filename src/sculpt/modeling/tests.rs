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
    assert!(result.preview.targets.is_empty());
    assert_eq!(result.preview.loop_edges, result.rim);
    assert!(!result.preview.walls.is_empty());
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

fn component_options(amount: f32) -> ComponentBuildOptions {
    ComponentBuildOptions {
        amount,
        width: 0.1,
        round: false,
        double_ring: false,
        triangulate: false,
        crease: false,
        other_side: false,
    }
}
#[test]
fn modeling_component_construction_preserves_closed_surfaces_and_attributes() {
    let mut mesh = cube(3);
    mesh.mask.fill(0.3);
    mesh.create_vertex_group("Surface".into(), 0., true)
        .unwrap();
    let f = front_center(&mesh);
    let p = &mesh.polygons()[f];
    let e = [p.vertices[0], p.vertices[1]];
    let v = p.vertices[0];
    let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    for kind in [
        ComponentBuild::FacePoint,
        ComponentBuild::FaceSplit,
        ComponentBuild::EdgeSplit,
        ComponentBuild::EdgeCollapse,
        ComponentBuild::EdgeExtrude,
        ComponentBuild::PointSplit,
        ComponentBuild::PointExtrude,
    ] {
        let amount = if matches!(
            kind,
            ComponentBuild::EdgeSplit | ComponentBuild::EdgeCollapse
        ) {
            0.25
        } else {
            0.1
        };
        let out = mesh
            .build_components(
                kind,
                &[f],
                &[e],
                &[v],
                mesh.vertices[v],
                component_options(amount),
                &sym,
            )
            .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        out.validate_closed().unwrap();
        manifold(&out).unwrap();
        assert_ne!(out, mesh);
        assert!(out.mask.iter().all(|&w| (w - 0.3).abs() < 1e-6));
        assert!(
            out.vertex_groups[0]
                .weights
                .iter()
                .all(|&w| (w - 1.).abs() < 1e-6)
        );
        assert!(out.polygroups.iter().all(|&g| g == 1));
    }
}
#[test]
fn modeling_face_split_propagates_edge_points_and_edge_positions_are_exact() {
    let mesh = cube(1);
    let f = front_center(&mesh);
    let p = &mesh.polygons()[f];
    let [a, b] = [p.vertices[0], p.vertices[1]];
    let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let split = mesh
        .build_components(
            ComponentBuild::FaceSplit,
            &[f],
            &[],
            &[],
            Vec3::default(),
            component_options(0.1),
            &sym,
        )
        .unwrap();
    assert_eq!(split.quads.len(), 5); // Four new quads and the unchanged opposite face.
    split.validate_closed().unwrap();
    for amount in [0., 0.25, 0.5, 1.] {
        let kind = if amount == 0. || amount == 1. {
            ComponentBuild::EdgeCollapse
        } else {
            ComponentBuild::EdgeSplit
        };
        let out = mesh
            .build_components(
                kind,
                &[],
                &[[a, b]],
                &[],
                mesh.vertices[a],
                component_options(amount),
                &sym,
            )
            .unwrap();
        let target = mesh.vertices[a] * (1. - amount) + mesh.vertices[b] * amount;
        assert!(out.vertices.iter().any(|&p| (p - target).len() < 1e-6));
        out.validate_closed().unwrap();
    }
}
#[test]
fn modeling_point_ring_and_surface_edge_construction_are_symmetric() {
    let mesh = cube(4);
    let sym = ModelingSymmetry::new(
        &mesh,
        &crate::sculpt::Symmetry {
            enabled: true,
            ..Default::default()
        },
    )
    .unwrap();
    let v = mesh
        .vertices
        .iter()
        .position(|&v| (v - Vec3(0.5, 0.5, 1.)).len() < 1e-6)
        .unwrap();
    for kind in [ComponentBuild::PointSplit, ComponentBuild::PointExtrude] {
        for round in [false, true] {
            let mut options = component_options(0.1);
            options.round = round;
            options.double_ring = kind == ComponentBuild::PointSplit;
            options.crease = true;
            let out = mesh
                .build_components(kind, &[], &[], &[v], mesh.vertices[v], options, &sym)
                .unwrap();
            out.validate_closed().unwrap();
            sym.validate_result(&out).unwrap();
            assert!(!out.creases.is_empty());
        }
    }
    let w = mesh
        .vertices
        .iter()
        .position(|&p| (p - Vec3(1., 0.5, 1.)).len() < 1e-6)
        .unwrap();
    let out = mesh
        .build_components(
            ComponentBuild::EdgeExtrude,
            &[],
            &[[v, w]],
            &[],
            Vec3(0.75, 0.6, 1.),
            component_options(0.1),
            &sym,
        )
        .unwrap();
    out.validate_closed().unwrap();
    sym.validate_result(&out).unwrap();
}
#[test]
fn modeling_boundary_extrusion_extends_a_shared_strip_without_cracks() {
    let mut p = Primitive::for_kind(PrimitiveKind::Plane);
    p.divisions = [3; 3];
    let mesh = p.mesh().unwrap();
    let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let mut incidence = BTreeMap::<_, usize>::new();
    for p in mesh.polygons() {
        for (a, b) in p.edges() {
            *incidence.entry(edge(a, b)).or_default() += 1;
        }
    }
    let edges: Vec<_> = incidence
        .into_iter()
        .filter(|(_, n)| *n == 1)
        .map(|(e, _)| e)
        .collect();
    let out = mesh
        .build_components(
            ComponentBuild::EdgeExtrude,
            &[],
            &edges,
            &[],
            Vec3::default(),
            component_options(0.1),
            &sym,
        )
        .unwrap();
    manifold(&out).unwrap();
    assert_eq!(out.quads.len(), mesh.quads.len() + edges.len());
}
#[test]
fn modeling_point_split_patch_can_be_extruded_and_surface_sides_differ() {
    let mesh = cube(4);
    let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let v = mesh
        .vertices
        .iter()
        .position(|&p| (p - Vec3(0.5, 0.5, 1.)).len() < 1e-6)
        .unwrap();
    let split = mesh
        .build_components(
            ComponentBuild::PointSplit,
            &[],
            &[],
            &[v],
            mesh.vertices[v],
            component_options(0.1),
            &sym,
        )
        .unwrap();
    let cap = split
        .polygons()
        .iter()
        .position(|p| {
            p.vertices.len() == 4
                && p.vertices
                    .iter()
                    .all(|&w| (split.vertices[w] - mesh.vertices[v]).len() < 0.15)
        })
        .unwrap();
    let symmetry = ModelingSymmetry::new(&split, &Default::default()).unwrap();
    let tube = split
        .extrude_model(&[cap], 0.3, false, false, &symmetry)
        .unwrap();
    tube.mesh.validate_closed().unwrap();
    let p = &mesh.polygons()[front_center(&mesh)];
    let e = [p.vertices[0], p.vertices[1]];
    let a = mesh
        .build_components(
            ComponentBuild::EdgeExtrude,
            &[],
            &[e],
            &[],
            mesh.vertices[e[0]],
            component_options(0.1),
            &sym,
        )
        .unwrap();
    let mut o = component_options(0.1);
    o.other_side = true;
    let b = mesh
        .build_components(
            ComponentBuild::EdgeExtrude,
            &[],
            &[e],
            &[],
            mesh.vertices[e[0]],
            o,
            &sym,
        )
        .unwrap();
    assert_ne!(a, b);
    a.validate_closed().unwrap();
    b.validate_closed().unwrap();
    let mut triangles = cube(1);
    triangles.quads.clear();
    let sym = ModelingSymmetry::new(&triangles, &Default::default()).unwrap();
    let out = triangles
        .build_components(
            ComponentBuild::PointSplit,
            &[],
            &[],
            &[0],
            triangles.vertices[0],
            component_options(0.1),
            &sym,
        )
        .unwrap();
    out.validate_closed().unwrap();
}

#[test]
fn modeling_fuse_connected_region_stops_at_intermediate_slab_loop() {
    let mut primitive = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
    primitive.divisions = [4, 4, 1];
    let mut mesh = primitive.mesh().unwrap();
    for v in &mut mesh.vertices {
        v.2 *= 0.1;
    }
    let faces: Vec<_> = mesh
        .polygons()
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            normal(&mesh, p).0 > 0.99
                && p.vertices.iter().map(|&v| mesh.vertices[v].1).sum::<f32>()
                    / (p.vertices.len() as f32)
                    < 0.
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(faces.len(), 2);
    let result = mesh
        .extrude_contact(
            &faces,
            -0.8,
            ExtrudeContact {
                join: true,
                loops: true,
                snap: 0.04,
            },
        )
        .unwrap();
    assert!(
        result.collapsed,
        "multi-face region passed the first compatible loop"
    );
    assert!(!result.joined);
    assert!((result.distance + 0.5).abs() < 1e-5);
    manifold(&result.mesh).unwrap();
    result.mesh.validate_closed().unwrap();
    // A farther hidden cap must not block a safe intermediate contact.
    let mut hidden = mesh.clone();
    hidden.hidden_faces = hidden
        .triangles
        .iter()
        .map(|t| t.iter().all(|&v| (hidden.vertices[v].0 + 1.).abs() < 1e-5))
        .collect();
    let nearest = hidden
        .extrude_contact(
            &faces,
            -3.,
            ExtrudeContact {
                join: true,
                loops: true,
                snap: 0.04,
            },
        )
        .unwrap();
    assert!(nearest.collapsed);
    assert!((nearest.distance + 0.5).abs() < 1e-5);
    nearest.mesh.validate_closed().unwrap();
}

#[test]
fn modeling_fuse_skips_a_partial_loop_between_notches_to_join_the_opposite_face() {
    let mut mesh = cube(3);
    // Two notches beside the selected face leave vertices at the first depth,
    // while the notch on the opposite side supplies the actual matching cap.
    for midpoint in [Vec3(1., -1., 0.), Vec3(1., 1., 0.), Vec3(-1., 0., 1.)] {
        let ridge = mesh
            .polygon_edges()
            .into_iter()
            .find(|&[a, b]| ((mesh.vertices[a] + mesh.vertices[b]) * 0.5 - midpoint).len() < 1e-5)
            .unwrap();
        let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
        mesh = mesh
            .cell_notch(ridge)
            .unwrap()
            .apply(&mesh, true, &symmetry)
            .unwrap()
            .0;
    }
    let source = mesh
        .quads
        .iter()
        .position(|q| {
            let center = q
                .iter()
                .fold(Vec3::default(), |sum, &v| sum + mesh.vertices[v])
                * 0.25;
            (center - Vec3(1., 0., 2. / 3.)).len() < 1e-5
        })
        .unwrap();
    let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    let halfway = mesh
        .extrude_symmetric_contact(&[source], -2. / 3., contact, &symmetry)
        .unwrap();
    assert!(!halfway.joined && !halfway.collapsed);
    let fused = mesh
        .extrude_symmetric_contact(&[source], -2., contact, &symmetry)
        .unwrap();
    assert!(fused.joined && !fused.collapsed);
    assert!((fused.distance + 4. / 3.).abs() < 1e-5);
    assert_eq!(
        fused.mesh,
        mesh.extrude_faces(&[source], -2., true).unwrap().mesh
    );
    fused.mesh.validate_closed().unwrap();
}

fn face_at(mesh: &Mesh, center: Vec3) -> usize {
    mesh.quads
        .iter()
        .position(|q| {
            let midpoint = q
                .iter()
                .fold(Vec3::default(), |sum, &v| sum + mesh.vertices[v])
                * 0.25;
            (midpoint - center).len() < 1e-5
        })
        .unwrap()
}

#[test]
fn modeling_fuse_joins_a_cap_bordered_by_touching_tunnels() {
    let mut mesh = cube(3);
    let midpoint = Vec3(0., 1., 1.);
    let ridge = mesh
        .polygon_edges()
        .into_iter()
        .find(|&[a, b]| ((mesh.vertices[a] + mesh.vertices[b]) * 0.5 - midpoint).len() < 1e-5)
        .unwrap();
    let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    mesh = mesh
        .cell_notch(ridge)
        .unwrap()
        .apply(&mesh, true, &symmetry)
        .unwrap()
        .0;
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    // Successive tunnels leave separate sheets touching along the inner grid edges.
    for center in [
        Vec3(2. / 3., 0., 1.),
        Vec3(-2. / 3., 0., 1.),
        Vec3(0., -2. / 3., 1.),
        Vec3(0., 2. / 3., 1. / 3.),
    ] {
        let face = face_at(&mesh, center);
        let result = mesh.extrude_contact(&[face], -2., contact).unwrap();
        assert!(result.joined, "setup tunnel at {center:?}");
        mesh = result.mesh;
    }
    let volume = |m: &Mesh| {
        m.triangles
            .iter()
            .map(|&[a, b, c]| m.vertices[a].dot(m.vertices[b].cross(m.vertices[c])) / 6.)
            .sum::<f32>()
    };
    for y in [1., 1. / 3.] {
        let face = face_at(&mesh, Vec3(2. / 3., y, 0.));
        let result = mesh.extrude_contact(&[face], -0.67, contact).unwrap();
        assert!(result.joined && !result.collapsed);
        assert!((result.distance + 2. / 3.).abs() < 1e-5);
        result.mesh.validate_closed().unwrap();
        assert_eq!(result.mesh.quads.len() + 2, mesh.quads.len());
        assert!((volume(&mesh) - volume(&result.mesh) - 8. / 27.).abs() < 1e-5);
    }
}

#[test]
fn modeling_fuse_removes_a_column_between_crossing_tunnels() {
    let mut mesh = cube(3);
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    // Cut the four neighboring columns all the way through a three-span cube.
    let step = 2. / 3.;
    for (x, z) in [(-step, 0.), (0., step), (step, 0.), (0., -step)] {
        let face = face_at(&mesh, Vec3(x, -1., z));
        let cut = mesh.extrude_contact(&[face], -2., contact).unwrap();
        assert!(cut.joined, "setup tunnel at ({x}, {z})");
        mesh = cut.mesh;
    }
    for (x, z) in [
        (-step, -step),
        (0., 0.),
        (step, step),
        (-step, step),
        (step, -step),
    ] {
        for y in [-1., 1.] {
            let face = face_at(&mesh, Vec3(x, y, z));
            let cut = mesh.extrude_contact(&[face], -2., contact).unwrap();
            assert!(cut.joined && !cut.collapsed);
            assert!((cut.distance + 2.).abs() < 1e-5);
            cut.mesh.validate_closed().unwrap();
            let points = |m: &Mesh, q: &[usize; 4]| {
                q.map(|v| {
                    let Vec3(a, b, c) = m.vertices[v];
                    [a, b, c].map(|n| (n * 3.).round() as i32)
                })
            };
            let expected: BTreeSet<_> = mesh
                .quads
                .iter()
                .filter(|q| {
                    !q.iter().all(|&v| {
                        let p = mesh.vertices[v];
                        (p.0 - x).abs() <= step * 0.5 + 1e-5 && (p.2 - z).abs() <= step * 0.5 + 1e-5
                    })
                })
                .map(|q| points(&mesh, q))
                .collect();
            assert_eq!(expected.len(), mesh.quads.len() - 14);
            assert_eq!(cut.mesh.quads.len(), expected.len());
            assert_eq!(
                cut.mesh
                    .quads
                    .iter()
                    .map(|q| points(&cut.mesh, q))
                    .collect::<BTreeSet<_>>(),
                expected
            );
        }
    }
}

#[test]
fn modeling_fuse_does_not_choose_between_two_coincident_caps() {
    let original = cube(3);
    let mut mesh = original.clone();
    let front = front_center(&mesh);
    let back = mesh
        .quads
        .iter()
        .find(|q| {
            q.iter().all(|&v| {
                mesh.vertices[v].2 == -1.
                    && mesh.vertices[v].0.abs() < 0.5
                    && mesh.vertices[v].1.abs() < 0.5
            })
        })
        .unwrap();
    let duplicate = (*back).map(|v| mesh.append_blend(&original, &[(v, 1.)], original.vertices[v]));
    mesh.polygroups.resize(mesh.triangles.len(), 0);
    mesh.hidden_faces.resize(mesh.triangles.len(), false);
    mesh.push_polygon(&duplicate, 0, false);
    let result = mesh.extrude_faces(&[front], -2., true).unwrap();
    assert!(!result.joined);
}

#[test]
fn modeling_fuse_region_caps_join_without_internal_walls() {
    let mesh = slab();
    let faces = [front_at(&mesh, -0.25, -0.25), front_at(&mesh, 0.25, -0.25)];
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    let joined = mesh.extrude_contact(&faces, -0.21, contact).unwrap();
    assert!(joined.joined);
    assert!(!joined.collapsed);
    assert!((joined.distance + 0.2).abs() < 1e-5);
    check_joined_surface(&joined.mesh, 2);
    assert_eq!(
        joined.mesh,
        mesh.extrude_contact(&faces, -10., contact).unwrap().mesh
    );
    let free = mesh.extrude_contact(&faces, -0.1, contact).unwrap();
    assert!(!free.joined && !free.collapsed);
    let no_join = mesh
        .extrude_contact(
            &faces,
            -0.21,
            ExtrudeContact {
                join: false,
                ..contact
            },
        )
        .unwrap();
    assert!(!no_join.joined && !no_join.collapsed);
    assert!((no_join.distance + 0.21).abs() < 1e-5);
    assert!(mesh.extrude_contact(&faces, -0.19, contact).unwrap().joined);
    assert!(
        !mesh
            .extrude_contact(
                &faces,
                -0.19,
                ExtrudeContact {
                    snap: 0.,
                    ..contact
                }
            )
            .unwrap()
            .joined
    );
}

#[test]
fn modeling_fuse_tiled_cap_translates_interior_vertices_and_preserves_symmetry() {
    let mesh = cube(4);
    let faces: Vec<_> = mesh
        .polygons()
        .iter()
        .enumerate()
        .filter(|(_, p)| normal(&mesh, p).0 > 0.99)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(faces.len(), 16);
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    let out = mesh.extrude_contact(&faces, -0.8, contact).unwrap();
    assert!(out.collapsed);
    assert!((out.distance + 0.5).abs() < 1e-5);
    out.mesh.validate_closed().unwrap();
    manifold(&out.mesh).unwrap();
    assert_eq!(
        out.mesh,
        mesh.extrude_contact(&faces, -3., contact).unwrap().mesh
    );
    assert!(
        out.mesh
            .vertices
            .iter()
            .any(|p| (p.0 - 0.5).abs() < 1e-5 && p.1 == 0. && p.2 == 0.)
    );
    let symmetry = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let out = mesh
        .extrude_symmetric_contact(&faces, -0.8, contact, &symmetry)
        .unwrap();
    assert!(out.collapsed);
    out.mesh.validate_closed().unwrap();
    ModelingSymmetry::new(&out.mesh, &mirror_x()).unwrap();
}

#[test]
fn modeling_fuse_rejects_disconnected_nonplanar_and_hidden_targets_transactionally() {
    let mesh = slab();
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    let disconnected = [front_at(&mesh, -0.75, -0.25), front_at(&mesh, 0.75, -0.25)];
    assert!(
        mesh.extrude_contact(&disconnected, -0.21, contact)
            .err()
            .unwrap()
            .contains("connected")
    );
    let front = front_at(&mesh, 0.25, 0.25);
    let side = mesh
        .polygons()
        .iter()
        .position(|p| normal(&mesh, p).0 > 0.99)
        .unwrap();
    assert!(
        mesh.extrude_contact(&[front, side], -0.21, contact)
            .err()
            .unwrap()
            .contains("flat")
    );
    let faces = [front, front_at(&mesh, -0.25, 0.25)];
    let mut hidden = mesh.clone();
    let target = hidden
        .polygons()
        .iter()
        .position(|p| {
            normal(&hidden, p).2 < -0.99
                && p.vertices.iter().all(|&v| {
                    hidden.vertices[v].0 >= 0.
                        && hidden.vertices[v].0 <= 0.5
                        && hidden.vertices[v].1 >= 0.
                        && hidden.vertices[v].1 <= 0.5
                })
        })
        .unwrap();
    hidden.hidden_faces = vec![false; hidden.triangles.len()];
    hidden.hidden_faces[target * 2] = true;
    hidden.hidden_faces[target * 2 + 1] = true;
    let before = hidden.clone();
    assert!(
        hidden
            .extrude_contact(&faces, -0.21, contact)
            .err()
            .unwrap()
            .contains("target")
    );
    assert_eq!(hidden, before);
}

#[test]
fn modeling_fuse_separate_regions_make_independent_tunnels() {
    let mesh = slab();
    let faces = [
        front_at(&mesh, -0.75, -0.25),
        front_at(&mesh, -0.25, -0.25),
        front_at(&mesh, 0.75, -0.25),
    ];
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let out = mesh
        .extrude_symmetric_contact(&faces, -0.8, contact, &symmetry)
        .unwrap();
    assert!(out.joined && !out.collapsed && !out.partial_contact && !out.varying_distance);
    assert!((out.distance + 0.2).abs() < 1e-5);
    check_joined_surface(&out.mesh, 3);
    let reversed: Vec<_> = faces.into_iter().rev().collect();
    assert_eq!(
        out.mesh,
        mesh.extrude_symmetric_contact(&reversed, -4., contact, &symmetry)
            .unwrap()
            .mesh
    );
    let symmetry = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let out = mesh
        .extrude_symmetric_contact(&[faces[0]], -0.8, contact, &symmetry)
        .unwrap();
    check_joined_surface(&out.mesh, 2);
    ModelingSymmetry::new(&out.mesh, &mirror_x()).unwrap();
}

#[test]
fn modeling_fuse_separate_regions_stop_at_side_loops() {
    let mut p = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
    p.divisions = [4, 4, 1];
    let mesh = p.mesh().unwrap();
    let faces: Vec<_> = mesh
        .polygons()
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            normal(&mesh, p).0 > 0.99
                && p.vertices
                    .iter()
                    .map(|&v| mesh.vertices[v].1)
                    .sum::<f32>()
                    .abs()
                    > 2.
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(faces.len(), 2);
    let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let contact = ExtrudeContact {
        join: false,
        loops: true,
        snap: 0.04,
    };
    let out = mesh
        .extrude_symmetric_contact(&faces, -0.8, contact, &symmetry)
        .unwrap();
    assert!(out.collapsed && !out.joined && !out.partial_contact && !out.varying_distance);
    assert!((out.distance + 0.5).abs() < 1e-5);
    out.mesh.validate_closed().unwrap();
    manifold(&out.mesh).unwrap();
    assert_eq!(
        out.mesh,
        mesh.extrude_symmetric_contact(&faces, -3., contact, &symmetry)
            .unwrap()
            .mesh
    );
}

#[test]
fn modeling_fuse_separate_depths_report_mixed_contacts_and_fail_atomically() {
    let mut other = slab();
    for v in &mut other.vertices {
        v.0 += 3.;
        v.2 *= 2.;
    }
    let mesh = slab().merged_with(&other).unwrap();
    let faces = [front_at(&mesh, 0.25, 0.25), front_at(&mesh, 3.25, 0.25)];
    let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let contact = ExtrudeContact {
        join: true,
        loops: true,
        snap: 0.04,
    };
    let mixed = mesh
        .extrude_symmetric_contact(&faces, -0.3, contact, &symmetry)
        .unwrap();
    assert!(mixed.joined && mixed.partial_contact && mixed.varying_distance);
    assert!((mixed.distance + 0.3).abs() < 1e-5);
    mixed.mesh.validate_closed().unwrap();
    let both = mesh
        .extrude_symmetric_contact(&faces, -0.8, contact, &symmetry)
        .unwrap();
    assert!(both.joined && !both.partial_contact && both.varying_distance);
    assert!((both.distance + 0.8).abs() < 1e-5);
    both.mesh.validate_closed().unwrap();
    manifold(&both.mesh).unwrap();
    assert_eq!(
        both.mesh,
        mesh.extrude_symmetric_contact(&faces, -3., contact, &symmetry)
            .unwrap()
            .mesh
    );
    let mut hidden = mesh.clone();
    hidden.hidden_faces = hidden
        .triangles
        .iter()
        .map(|t| {
            t.iter()
                .all(|&v| hidden.vertices[v].0 > 2. && hidden.vertices[v].2 < -0.19)
        })
        .collect();
    let before = hidden.clone();
    let error = hidden
        .extrude_symmetric_contact(&faces, -0.8, contact, &symmetry)
        .err()
        .unwrap();
    assert!(error.contains("target"), "{error}");
    assert_eq!(hidden, before);
}

#[test]
fn modeling_fuse_both_selected_ends_of_a_tunnel_join_once() {
    for lateral_offset in [0., 2e-5] {
        let mut mesh = slab();
        let front = front_at(&mesh, 0.25, 0.25);
        let back = mesh
            .polygons()
            .iter()
            .position(|p| {
                normal(&mesh, p).2 < -0.99
                    && p.vertices.iter().all(|&v| {
                        mesh.vertices[v].0 >= 0.
                            && mesh.vertices[v].0 <= 0.5
                            && mesh.vertices[v].1 >= 0.
                            && mesh.vertices[v].1 <= 0.5
                    })
            })
            .unwrap();
        // Cap matching already permits this scale-relative lateral tolerance.
        for v in &mut mesh.vertices {
            v.0 += lateral_offset * (0.1 - v.2) / 0.2;
        }
        let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
        let out = mesh
            .extrude_symmetric_contact(
                &[front, back],
                -0.8,
                ExtrudeContact {
                    join: true,
                    loops: true,
                    snap: 0.04,
                },
                &symmetry,
            )
            .unwrap();
        assert!(out.joined && !out.partial_contact && !out.varying_distance);
        check_joined_surface(&out.mesh, 1);
    }
}

#[test]
fn modeling_double_point_ring_has_eight_exact_radius_vertices_and_repeats() {
    for divisions in [4, 6] {
        let mesh = cube(divisions);
        let v = mesh
            .vertices
            .iter()
            .position(|p| p.0.abs() < 1e-6 && p.1.abs() < 1e-6 && (p.2 - 1.).abs() < 1e-6)
            .unwrap();
        let sym = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
        let mut options = component_options(0.1);
        options.double_ring = true;
        options.crease = true;
        let out = mesh
            .build_components(
                ComponentBuild::PointSplit,
                &[],
                &[],
                &[v],
                mesh.vertices[v],
                options,
                &sym,
            )
            .unwrap();
        out.validate_closed().unwrap();
        assert_eq!(out.creases.len(), 8);
        let ring: BTreeSet<_> = out.creases.iter().flatten().copied().collect();
        assert_eq!(ring.len(), 8);
        for i in ring {
            assert!(((out.vertices[i] - mesh.vertices[v]).len() - 0.1).abs() < 1e-6);
        }
        options.amount = 1.;
        assert!(
            mesh.build_components(
                ComponentBuild::PointSplit,
                &[],
                &[],
                &[v],
                mesh.vertices[v],
                options,
                &sym
            )
            .unwrap_err()
            .contains("Radius")
        );
    }
}

#[test]
fn modeling_cell_notch_removes_one_cell_and_preserves_the_surrounding_grid() {
    let mut mesh = cube(3);
    let ridge = mesh
        .polygon_edges()
        .into_iter()
        .find(|e| {
            e.iter().all(|&v| {
                let p = mesh.vertices[v];
                p.1 == 1. && p.2 == 1. && p.0.abs() < 0.5
            })
        })
        .unwrap();
    mesh.creases = mesh.polygon_edges();
    mesh.create_vertex_group("Ridge".into(), 0., true).unwrap();
    let symmetry = ModelingSymmetry::new(&mesh, &mirror_x()).unwrap();
    let cell = mesh.cell_notch(ridge).unwrap();
    let delta = Vec3(0., -2. / 3., -2. / 3.);
    assert!((cell.travel() - delta).len() < 1e-5);
    let mut mirrored = mirror_x();
    mirrored.mirror = [true, true, false];
    let (both, _) = cell
        .apply(
            &mesh,
            true,
            &ModelingSymmetry::new(&mesh, &mirrored).unwrap(),
        )
        .unwrap();
    both.validate_closed().unwrap();
    assert_eq!(both.quads.len(), 58);
    let (cut, _) = cell.apply(&mesh, true, &symmetry).unwrap();
    cut.validate_closed().unwrap();
    assert_eq!(cut.vertices.len(), mesh.vertices.len() + 2);
    assert_eq!(cut.quads.len(), 56);
    assert_eq!(cut.triangles.len(), cut.quads.len() * 2);
    let mut original_quads = 0;
    for (q, out) in mesh.quads.iter().zip(&cut.quads) {
        if !ridge.iter().all(|v| q.contains(v)) {
            assert_eq!(q.map(|i| mesh.vertices[i]), out.map(|i| cut.vertices[i]));
            original_quads += 1;
        }
    }
    assert_eq!(original_quads, 52);
    let volume: f32 = cut
        .triangles
        .iter()
        .map(|&[a, b, c]| cut.vertices[a].dot(cut.vertices[b].cross(cut.vertices[c])) / 6.)
        .sum();
    assert!((volume - 8. * 26. / 27.).abs() < 1e-5);
    for &v in &ridge {
        assert!((cut.vertices[v] - mesh.vertices[v] - delta).len() < 1e-5);
    }
    assert!(cut.creases.contains(&ridge));
    assert!(cut.vertex_groups[0].weights.iter().all(|&w| w == 1.));

    // The same local grid is recoverable if the user releases the edge partway in.
    let partial = mesh
        .transform_components(
            &ridge,
            Vec3(0., 1., 1.),
            ComponentTransform::Move(delta * 0.35),
            true,
            &symmetry,
        )
        .unwrap();
    let remaining = partial.cell_notch(ridge).unwrap();
    assert!((remaining.travel() - delta * 0.65).len() < 1e-5);
    let (completed, _) = remaining.apply(&partial, true, &symmetry).unwrap();
    assert_eq!(completed.quads, cut.quads);
    assert!(
        completed
            .vertices
            .iter()
            .zip(&cut.vertices)
            .all(|(&a, &b)| (a - b).len() < 1e-5)
    );

    let affected = mesh
        .quads
        .iter()
        .find(|q| ridge.iter().all(|v| q.contains(v)))
        .unwrap();
    let rim = *affected.iter().find(|v| !ridge.contains(v)).unwrap();
    mesh.mask[rim] = 1.;
    assert!(cell.apply(&mesh, true, &symmetry).is_err());
    assert!(cell.apply(&mesh, false, &symmetry).is_ok());
    mesh.hidden_faces = mesh
        .triangles
        .iter()
        .map(|t| t.iter().all(|v| affected.contains(v)))
        .collect();
    assert!(cell.apply(&mesh, false, &symmetry).is_err());
}

#[test]
fn modeling_cell_notch_rejects_contact_with_the_far_wall() {
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [3, 3, 1];
    let mesh = primitive.mesh().unwrap();
    let ridge = mesh
        .polygon_edges()
        .into_iter()
        .find(|e| {
            e.iter().all(|&v| {
                let p = mesh.vertices[v];
                p.0.abs() < 0.5 && p.1 == 1. && p.2 == 1.
            })
        })
        .unwrap();
    let cell = mesh.cell_notch(ridge).unwrap();
    let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    assert!(
        cell.apply(&mesh, true, &symmetry)
            .unwrap_err()
            .contains("occupies this grid cell")
    );
}

#[test]
fn face_cell_notch_preserves_every_square_outside_the_chosen_cell() {
    let mesh = cube(3);
    let symmetry = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let surface = |mesh: &Mesh| {
        mesh.quads
            .iter()
            .map(|q| {
                let mut points = q.map(|v| {
                    let Vec3(x, y, z) = mesh.vertices[v];
                    [x, y, z].map(|p| {
                        let grid = (p + 1.) * 1.5;
                        assert!((grid - grid.round()).abs() < 1e-5);
                        grid.round() as i32
                    })
                });
                points.sort_unstable();
                points
            })
            .collect::<BTreeSet<_>>()
    };
    let original = surface(&mesh);
    for (f, q) in mesh.quads.iter().enumerate() {
        let [a, b, c, _] = q.map(|v| mesh.vertices[v]);
        let normal = (b - a).cross(c - a).norm();
        let Vec3(x, y, z) = (a + c) * 0.5 - normal * (1. / 3.);
        let voxel = [x, y, z].map(|p| ((p + 1.) * 1.5).floor() as i32);
        let cell = mesh.face_cell_notch(f).unwrap();
        let (cut, caps) = cell.apply(&mesh, true, &symmetry).unwrap();
        cut.validate_closed().unwrap();
        assert_eq!(cut.quads.len() * 2, cut.triangles.len());
        for (old, new) in mesh.quads[f].iter().zip(cut.quads[caps[&f]]) {
            assert!((cut.vertices[new] - mesh.vertices[*old] - cell.travel()).len() < 1e-5);
        }
        // Removing a voxel toggles exactly its six boundary squares.
        let mut expected = original.clone();
        for axis in 0..3 {
            for side in [0, 1] {
                let mut wall = [voxel; 4];
                for (corner, p) in wall.iter_mut().enumerate() {
                    p[axis] += side;
                    p[(axis + 1) % 3] += (corner % 2) as i32;
                    p[(axis + 2) % 3] += (corner / 2) as i32;
                }
                wall.sort_unstable();
                if !expected.remove(&wall) {
                    expected.insert(wall);
                }
            }
        }
        assert_eq!(surface(&cut), expected);
    }
}

#[test]
fn face_cell_notch_uses_grid_depth_and_respects_contacts_masks_and_symmetry() {
    let mut mesh = cube(3);
    for v in &mut mesh.vertices {
        v.2 *= 0.6;
    }
    let f = front_center(&mesh);
    let q = mesh.quads[f];
    let identity = ModelingSymmetry::new(&mesh, &Default::default()).unwrap();
    let notch = mesh.face_cell_notch(f).unwrap();
    assert!((notch.travel() - Vec3(0., 0., -0.4)).len() < 1e-5);
    let partial_delta = Vec3(0.02, 0., -0.15);
    let partial = mesh
        .transform_components(
            &q,
            mesh.vertices[q[0]],
            ComponentTransform::Move(partial_delta),
            false,
            &identity,
        )
        .unwrap();
    let finish = partial.face_cell_notch(f).unwrap();
    assert!((finish.travel() + partial_delta - notch.travel()).len() < 1e-5);
    let (cut, _) = finish.apply(&partial, true, &identity).unwrap();
    let (direct, _) = notch.apply(&mesh, true, &identity).unwrap();
    assert_eq!(cut.quads, direct.quads);
    assert!(
        cut.vertices
            .iter()
            .zip(&direct.vertices)
            .all(|(&a, &b)| (a - b).len() < 1e-5)
    );

    let mut mirror = mirror_x();
    mirror.mirror = [false, false, true];
    let symmetry = ModelingSymmetry::new(&mesh, &mirror).unwrap();
    let (both, caps) = notch.apply(&mesh, true, &symmetry).unwrap();
    both.validate_closed().unwrap();
    assert_eq!(caps.len(), 2);
    assert_eq!(both.quads.len(), mesh.quads.len() + 8);
    mesh.mask[q[0]] = 1.;
    assert!(notch.apply(&mesh, true, &identity).is_err());
    assert!(notch.apply(&mesh, false, &identity).is_ok());
    mesh.hidden_faces = mesh.triangles.iter().map(|t| t.contains(&q[1])).collect();
    assert!(notch.apply(&mesh, false, &identity).is_err());

    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [3, 3, 1];
    let thin = primitive.mesh().unwrap();
    assert!(
        thin.face_cell_notch(front_center(&thin))
            .unwrap()
            .apply(
                &thin,
                true,
                &ModelingSymmetry::new(&thin, &Default::default()).unwrap()
            )
            .unwrap_err()
            .contains("occupies this grid cell")
    );
}

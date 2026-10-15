use super::*;
use crate::sculpt::{
    Axis, MirrorOptions, ModelMetadata, Primitive, PrimitiveKind, SliceMode, SliceOptions,
    SlicePlane, VertexGroup,
};
fn cube() -> Mesh {
    let mut p = Primitive::for_kind(PrimitiveKind::Cube);
    p.divisions = [1; 3];
    p.mesh().unwrap()
}
fn plane() -> Mesh {
    Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap()
}
fn grid() -> Mesh {
    let mut p = Primitive::for_kind(PrimitiveKind::Grid);
    p.divisions = [6, 6, 1];
    p.mesh().unwrap()
}
fn oriented(mesh: &Mesh) {
    mesh.validate().unwrap();
    for &[a, b, c] in &mesh.triangles {
        assert!(
            (mesh.vertices[b] - mesh.vertices[a])
                .cross(mesh.vertices[c] - mesh.vertices[a])
                .len()
                > 1e-8
        );
    }
}

#[test]
fn edge_smoothness_rounds_uncreased_cube_and_preserves_creased_corners() {
    let original = cube();
    let linear = original.smooth_subdivide(0., &|| false).unwrap();
    let smooth = original.smooth_subdivide(1., &|| false).unwrap();
    let half = original.smooth_subdivide(0.5, &|| false).unwrap();
    for i in 0..original.vertices.len() {
        assert_eq!(linear.vertices[i], original.vertices[i]);
        assert!((half.vertices[i] - (linear.vertices[i] + smooth.vertices[i]) * 0.5).len() < 1e-6);
        assert!(smooth.vertices[i].len() < original.vertices[i].len());
    }
    let mut creased = original.clone();
    creased.creases = creased.polygon_edges();
    let hard = creased.smooth_subdivide(1., &|| false).unwrap();
    assert_eq!(
        &hard.vertices[..original.vertices.len()],
        &original.vertices
    );
    assert_eq!(hard.creases.len(), creased.creases.len() * 2);
    hard.validate_closed().unwrap();
    oriented(&hard);
    assert!(creased.surface_shading().vertices.len() >= creased.vertices.len());
    let mut clear = Geometry::new(creased);
    clear
        .edit_edges(&EdgeEdit::ClearCreases, &|| false)
        .unwrap();
    assert!(clear.mesh.creases.is_empty());
    assert_eq!(clear.mesh.vertices, original.vertices);
    assert_eq!(clear.mesh.smooth_subdivide(1., &|| false).unwrap(), smooth);
}

#[test]
fn edge_support_loops_preserve_holes_attributes_and_closed_polygroup_surfaces() {
    let mut source = grid();
    for (i, p) in source.vertices.iter().enumerate() {
        source.mask[i] = if p.0.abs() < 0.4 && p.1.abs() < 0.4 {
            1.
        } else {
            0.
        };
    }
    let mut hole = source.delete_masked().unwrap().unwrap();
    for (i, p) in hole.vertices.iter().enumerate() {
        hole.mask[i] = (p.0 + 1.) * 0.5;
    }
    let group = hole
        .create_vertex_group("Interpolated".into(), 0., false)
        .unwrap();
    let before = hole.border_edges(BorderTarget::Holes, 0.5, &[]).unwrap();
    assert!(!before.is_empty());
    let loops = hole
        .support_loops(BorderTarget::Holes, 0.5, &[], 2, 0.15, &|| false)
        .unwrap();
    assert_eq!(
        before,
        loops.border_edges(BorderTarget::Holes, 0.5, &[]).unwrap()
    );
    assert_eq!(&loops.vertices[..hole.vertices.len()], &hole.vertices);
    oriented(&loops);
    for (i, p) in loops.vertices.iter().enumerate() {
        assert!((loops.mask[i] - (p.0 + 1.) * 0.5).abs() < 1e-5);
        assert!((loops.vertex_groups[group].weights[i] - loops.mask[i]).abs() < 1e-5);
    }
    assert!(loops.validate_closed().is_err());
    assert_eq!(loops.vertices.len(), hole.vertices.len() + before.len() * 2);

    let masked = source
        .support_loops(BorderTarget::Masked, 0.5, &[], 1, 0.2, &|| false)
        .unwrap();
    oriented(&masked);
    assert!(masked.vertices.len() > source.vertices.len());
    assert_eq!(
        masked.border_edges(BorderTarget::Holes, 0.5, &[]).unwrap(),
        source.border_edges(BorderTarget::Holes, 0.5, &[]).unwrap()
    );

    let mut grouped = cube();
    grouped.vertex_groups = (0..6)
        .map(|i| VertexGroup {
            name: format!("Face {i}"),
            weights: vec![1.; grouped.vertices.len()],
            visible: true,
        })
        .collect();
    grouped.polygroups = (0..grouped.triangles.len()).map(|i| i / 2 + 1).collect();
    let loops = grouped
        .support_loops(BorderTarget::VisibleGroups, 0.5, &[], 2, 0.2, &|| false)
        .unwrap();
    oriented(&loops);
    loops.validate_closed().unwrap();
    assert_eq!(loops.vertex_groups.len(), 6);
    assert!(loops.polygroups.iter().all(|&g| (1..=6).contains(&g)));
    grouped.vertex_groups[0].visible = false;
    let hidden: Vec<_> = grouped
        .triangles
        .iter()
        .enumerate()
        .filter(|(i, _)| !grouped.face_visible(*i))
        .map(|(_, &t)| t)
        .collect();
    let out = grouped
        .support_loops(BorderTarget::VisibleGroups, 0.5, &[], 1, 0.2, &|| false)
        .unwrap();
    out.validate_closed().unwrap();
    for t in hidden {
        assert!(out.triangles.contains(&t));
        for v in t {
            assert_eq!(out.vertices[v], grouped.vertices[v]);
        }
    }
}

#[test]
fn edge_crease_selection_uncrease_and_existing_topology_operations_preserve_tags() {
    let mut mesh = plane();
    mesh.creases = mesh.polygon_edges();
    assert_eq!(
        mesh.expand_edge_loop(&mesh.creases[..1]).unwrap(),
        mesh.creases
    );
    let refined = mesh.subdivide().unwrap();
    assert_eq!(refined.creases.len(), mesh.creases.len() * 2);
    refined.validate().unwrap();
    let mirrored = mesh
        .mirror_half(
            MirrorOptions {
                axis: Axis::X,
                source_positive: true,
                offset: 0.,
            },
            &|| false,
        )
        .unwrap();
    mirrored.validate().unwrap();
    assert!(!mirrored.creases.is_empty());
    let (cut, _) = mesh
        .slice(
            SlicePlane {
                origin: Vec3::default(),
                normal: Vec3(1., 0., 0.),
            },
            SliceOptions {
                mode: SliceMode::Trim,
                cap: false,
                flip: false,
            },
            &|| false,
        )
        .unwrap();
    cut.validate().unwrap();
    assert!(!cut.creases.is_empty());
    let merged = mesh.merged_with(&mesh).unwrap();
    assert_eq!(merged.creases.len(), mesh.creases.len() * 2);
    let mut geometry = Geometry::new(mesh.clone());
    geometry
        .edit_edges(
            &EdgeEdit::Crease {
                target: BorderTarget::Selected,
                selected: mesh.creases[..1].to_vec(),
                remove: true,
            },
            &|| false,
        )
        .unwrap();
    assert_eq!(geometry.mesh.creases.len(), mesh.creases.len() - 1);
    assert_eq!(geometry.mesh.vertices, mesh.vertices);
    let json = serde_json::to_vec(&geometry.mesh).unwrap();
    let loaded: Mesh = serde_json::from_slice(&json).unwrap();
    assert_eq!(loaded, geometry.mesh);
}

#[test]
fn edge_thickness_modifier_loops_and_creases_match_applied_geometry() {
    let source = plane();
    let metadata = ModelMetadata {
        temporary_thickness: 0.2,
        thickness_loops: 2,
        thickness_crease: true,
        thickness_groups: true,
        thickness_mask_rim: true,
        ..Default::default()
    };
    let shell = source.solidify_with_metadata(&metadata).unwrap();
    let again = source.solidify_with_metadata(&metadata).unwrap();
    assert_eq!(shell, again);
    shell.validate_closed().unwrap();
    oriented(&shell);
    assert_eq!(shell.vertices.len(), 16);
    assert_eq!(shell.creases.len(), 8);
    assert!(shell.mask.iter().all(|&m| m == 1.));
    let smooth = shell.smooth_subdivide(1., &|| false).unwrap();
    smooth.validate_closed().unwrap();
    assert_eq!(smooth.creases.len(), 16);
}

#[test]
fn edge_invalid_targets_cancel_and_hidden_faces_are_transactional() {
    for edit in [
        EdgeEdit::Loops {
            target: BorderTarget::Holes,
            selected: vec![],
            count: 1,
            width: 0.1,
        },
        EdgeEdit::Loops {
            target: BorderTarget::Selected,
            selected: vec![[0, 1]],
            count: 0,
            width: 0.1,
        },
        EdgeEdit::SmoothSubdivide(f32::NAN),
    ] {
        let mut geometry = Geometry::new(cube());
        let before = geometry.clone();
        assert!(geometry.edit_edges(&edit, &|| false).is_err());
        assert_eq!(geometry, before);
    }
    let mut geometry = Geometry::new(plane());
    let before = geometry.clone();
    assert!(
        geometry
            .edit_edges(&EdgeEdit::SmoothSubdivide(1.), &|| true)
            .is_err()
    );
    assert_eq!(geometry, before);
    geometry.mesh.hidden_faces = vec![true; geometry.mesh.triangles.len()];
    let before = geometry.clone();
    assert!(
        geometry
            .edit_edges(&EdgeEdit::SmoothSubdivide(1.), &|| false)
            .is_err()
    );
    assert_eq!(geometry, before);
    let mut invalid = plane();
    invalid.creases = vec![[usize::MAX, usize::MAX]];
    assert!(invalid.validate().is_err());
}

#[test]
fn edge_relaxation_moves_uneven_borders_without_changing_topology() {
    let mut mesh = grid();
    let edges = mesh.border_edges(BorderTarget::Holes, 0.5, &[]).unwrap();
    let v = edges
        .iter()
        .flatten()
        .copied()
        .find(|&v| mesh.vertices[v].0.abs() < 0.5 && mesh.vertices[v].1 > 0.9)
        .unwrap();
    mesh.vertices[v].0 += 0.08;
    let out = mesh
        .relax_borders(BorderTarget::Holes, 0.5, &[], 0.5, &|| false)
        .unwrap();
    assert_ne!(out.vertices, mesh.vertices);
    assert_eq!(out.triangles, mesh.triangles);
    assert_eq!(out.quads, mesh.quads);
    assert_eq!(out.mask, mesh.mask);
    oriented(&out);
}

#[test]
fn edge_limits_and_legacy_data_are_checked() {
    let mesh = plane();
    let mut legacy = serde_json::to_value(&mesh).unwrap();
    legacy.as_object_mut().unwrap().remove("creases");
    let loaded: Mesh = serde_json::from_value(legacy).unwrap();
    assert!(loaded.creases.is_empty());
    let mut invalid = mesh.clone();
    let q = invalid.quads[0];
    invalid.creases = vec![edge(q[0], q[2])];
    assert!(invalid.validate().is_err());
    let mut large = mesh;
    large.vertices.resize(MAX_VERTICES, Vec3(0., 0., 0.));
    large.mask.resize(MAX_VERTICES, 0.);
    for g in &mut large.vertex_groups {
        g.weights.resize(MAX_VERTICES, 0.);
    }
    large.validate().unwrap();
    assert!(
        large
            .smooth_subdivide(1., &|| false)
            .unwrap_err()
            .contains("size limit")
    );
    assert!(
        large
            .support_loops(BorderTarget::Holes, 0.5, &[], 1, 0.1, &|| false)
            .unwrap_err()
            .contains("size limit")
    );
}

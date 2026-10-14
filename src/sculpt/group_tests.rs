use super::*;
fn plane() -> Mesh {
    Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap()
}
#[test]
fn weighted_groups_are_independent_and_quads_require_all_corners() {
    let mut m = plane();
    m.mask = vec![0., 0.4, 0.7, 1.];
    m.create_vertex_group("Detail".into(), 0.5, false).unwrap();
    assert_eq!(m.vertex_groups[0].weights, vec![0., 0., 0.7, 1.]);
    assert!(m.polygroups.is_empty());
    m.mask.fill(0.8);
    m.assign_mask_to_group(0, 0.5, false).unwrap();
    assert_eq!(m.polygroups, vec![1; 2]);
    m.create_vertex_group("Surface".into(), 0.5, true).unwrap();
    assert_eq!(m.polygroups, vec![2; 2]);
    m.mask.fill(0.);
    m.mask_from_group(0).unwrap();
    assert_eq!(m.mask, vec![0.8, 0.8, 0.8, 1.]);
    m.rename_vertex_group(0, "Feature".into()).unwrap();
    assert!(m.rename_vertex_group(1, "Feature".into()).is_err());
    m.remove_vertex_group(0).unwrap();
    assert_eq!(m.polygroups, vec![1; 2]);
    m.mask.fill(1.);
    m.assign_mask_to_group(0, 0.5, true).unwrap();
    assert!(m.polygroups.is_empty());
    assert!(m.vertex_groups[0].weights.iter().all(|&w| w == 0.));
    m.validate().unwrap();
}
#[test]
fn mask_ring_expansion_ignores_quad_diagonal_and_preserves_softness() {
    let mut m = plane();
    let [a, b, c, d] = m.quads[0];
    m.mask[a] = 0.6;
    m.grow_mask(true);
    assert_eq!(m.mask[a], 0.6);
    assert_eq!(m.mask[b], 0.6);
    assert_eq!(m.mask[d], 0.6);
    assert_eq!(m.mask[c], 0.);
    m.grow_mask(false);
    assert_eq!(m.mask[a], 0.6);
    assert_eq!(m.mask[b], 0.);
    assert_eq!(m.mask[d], 0.);
    assert!(!mask_selected(0., 0.));
    assert!(mask_selected(0.4, 0.4));
    assert!(!mask_selected(0.4, 0.5));
}
#[test]
fn mask_blur_softens_one_ring_without_quad_diagonal_or_face_order_bias() {
    let mut mesh = plane();
    let [a, b, c, d] = mesh.quads[0];
    mesh.mask[a] = 1.;
    let mut reversed = mesh.clone();
    reversed.triangles.reverse();
    reversed.blur_mask();
    mesh.blur_mask();
    assert_eq!(mesh.mask, reversed.mask);
    assert_eq!(mesh.mask[a], 0.5);
    assert_eq!(mesh.mask[b], 0.25);
    assert_eq!(mesh.mask[c], 0.);
    assert_eq!(mesh.mask[d], 0.25);

    // In a triangle-only mesh the shared diagonal is a real edge, counted once.
    mesh.quads.clear();
    mesh.mask.fill(0.);
    let [a, _, c] = mesh.triangles[0];
    mesh.mask[a] = 1.;
    mesh.blur_mask();
    assert!((mesh.mask[c] - 1. / 6.).abs() < 1e-6);
    assert_eq!(mesh.mask[a], 0.5);
}

#[test]
fn mask_filters_handle_uniform_empty_isolated_and_repeated_values() {
    let mut mesh = plane();
    for value in [0., 0.3, 0.5, 1.] {
        mesh.mask.fill(value);
        let before = mesh.mask.clone();
        mesh.blur_mask();
        assert_eq!(mesh.mask, before);
    }
    mesh.mask = vec![0.1, 0.4, 0.6, 0.9];
    mesh.sharpen_mask();
    for (&actual, expected) in mesh.mask.iter().zip([0., 0.3, 0.7, 1.]) {
        assert!((actual - expected).abs() < 1e-6);
    }
    for _ in 0..10 {
        mesh.sharpen_mask();
    }
    assert_eq!(mesh.mask, vec![0., 0., 1., 1.]);
    mesh.mask.fill(0.5);
    mesh.sharpen_mask();
    assert_eq!(mesh.mask, vec![0.5; 4]);
    mesh.triangles.clear();
    mesh.quads.clear();
    mesh.mask[0] = 0.3;
    let before = mesh.mask.clone();
    mesh.blur_mask();
    assert_eq!(
        mesh.mask, before,
        "isolated vertices have no neighbours to average"
    );
    mesh.vertices.clear();
    mesh.mask.clear();
    mesh.blur_mask();
    mesh.sharpen_mask();
    assert!(mesh.mask.is_empty());
}
#[test]
fn subdivision_cut_and_extraction_keep_group_membership() {
    let mut m = plane();
    m.mask.fill(0.75);
    m.create_vertex_group("Surface".into(), 0.5, false).unwrap();
    let mut refined = m.subdivide().unwrap();
    assert_eq!(refined.polygroups, vec![1; 8]);
    assert!(refined.vertex_groups[0].weights.iter().all(|&w| w == 0.75));
    assert!(refined.extract_masked_threshold(0.8).is_err());
    let extracted = refined.extract_masked_threshold(0.7).unwrap();
    assert_eq!(extracted.vertex_groups, refined.vertex_groups);
    assert_eq!(extracted.polygroups, refined.polygroups);
    assert!(extracted.mask.iter().all(|&w| w == 0.));
    let q = refined.quads[0];
    refined.mask.fill(0.);
    for i in q {
        refined.mask[i] = 0.9;
    }
    let before = refined.clone();
    refined = refined.delete_masked_threshold(0.8).unwrap().unwrap();
    refined.validate().unwrap();
    assert!(refined.vertex_groups[0].weights.iter().all(|&w| w == 0.75));
    assert!(refined.polygroups.iter().all(|&id| id == 1));
    assert!(refined.triangles.len() < before.triangles.len());
    let loaded: Mesh = serde_json::from_str(&serde_json::to_string(&refined).unwrap()).unwrap();
    assert_eq!(loaded, refined);
}
#[test]
fn thickness_groups_classify_parts_and_can_group_the_rim_mask() {
    let mut m = plane();
    m.create_vertex_group("Original".into(), 0.5, true).unwrap();
    let mut options = ModelMetadata {
        temporary_thickness: 0.2,
        thickness_groups: true,
        ..Default::default()
    };
    let shell = m.solidify_with_metadata(&options).unwrap();
    assert_eq!(&shell.polygroups[..2], &[2; 2]);
    assert_eq!(&shell.polygroups[2..4], &[3; 2]);
    assert_eq!(&shell.polygroups[4..], &[4; 8]);
    assert_eq!(
        shell.vertex_groups[1].weights,
        vec![1., 1., 1., 1., 0., 0., 0., 0.]
    );
    assert_eq!(
        shell.vertex_groups[2].weights,
        vec![0., 0., 0., 0., 1., 1., 1., 1.]
    );
    options.thickness_mask_rim = true;
    options.thickness_group_mask = true;
    let masked = m.solidify_with_metadata(&options).unwrap();
    assert_eq!(masked.vertex_groups[4].name, "Thickness masked");
    assert!(masked.mask.iter().all(|&w| w == 1.));
    assert!(masked.polygroups.iter().all(|&id| id == 5));
    masked.validate_closed().unwrap();
}
#[test]
fn remeshing_retains_named_weights_and_face_categories() {
    let mut m = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
    m.create_vertex_group("Whole".into(), 0.5, true).unwrap();
    let result = m.remesh(16).unwrap();
    result.validate_closed().unwrap();
    assert_eq!(result.vertex_groups[0].name, "Whole");
    assert!(result.vertex_groups[0].weights.iter().all(|&w| w == 1.));
    assert_eq!(result.polygroups, vec![1; result.triangles.len()]);
}
#[test]
fn malformed_group_data_is_rejected() {
    let mut m = plane();
    m.create_vertex_group("Whole".into(), 0.5, true).unwrap();
    m.vertex_groups[0].weights[0] = f32::NAN;
    assert!(m.validate().is_err());
    m.vertex_groups[0].weights[0] = 1.;
    m.polygroups[0] = 2;
    assert!(m.validate().is_err());
    m.polygroups[0] = 0;
    assert!(m.validate().is_err(), "quad halves must have one category");
}

#[test]
fn spatial_transfer_tracks_positions_after_vertex_and_face_reordering() {
    let mut primitive = Primitive::for_kind(PrimitiveKind::Plane);
    primitive.divisions = [2, 1, 1];
    let mut source = primitive.mesh().unwrap();
    source
        .create_vertex_group("Whole".into(), 0.5, true)
        .unwrap();
    source.mask = source
        .vertices
        .iter()
        .map(|p| if p.0 >= 0. { 0.8 } else { 0. })
        .collect();
    source
        .create_vertex_group("Right".into(), 0.5, false)
        .unwrap();
    assert!(source.polygroups.contains(&1) && source.polygroups.contains(&2));
    let n = source.vertices.len();
    let mut dest = source.clone();
    dest.vertex_groups.clear();
    dest.polygroups.clear();
    dest.quads.clear();
    dest.vertices.reverse();
    dest.mask.reverse();
    dest.triangles.reverse();
    for t in &mut dest.triangles {
        *t = t.map(|i| n - 1 - i);
    }
    source.transfer_groups_to(&mut dest);
    assert_eq!(
        dest.polygroups,
        source.polygroups.iter().rev().copied().collect::<Vec<_>>()
    );
    for (a, b) in dest.vertex_groups.iter().zip(&source.vertex_groups) {
        assert_eq!(
            a.weights,
            b.weights.iter().rev().copied().collect::<Vec<_>>()
        );
    }
    dest.validate().unwrap();
}

#[test]
fn group_palette_is_muted_distinct_and_independent_of_names() {
    let mut colors = std::collections::HashSet::new();
    for i in 0..64 {
        let color = Mesh::group_color(i);
        let min = color.into_iter().fold(f32::INFINITY, f32::min);
        let max = color.into_iter().fold(f32::NEG_INFINITY, f32::max);
        assert!((max - min) / max < 0.6, "avoid neon saturation");
        assert!(
            colors.insert(color.map(|c| (c * 255.) as u8)),
            "every supported group has a different display color"
        );
    }
    for i in 0..4 {
        for j in i + 1..4 {
            let a = Mesh::group_color(i);
            let b = Mesh::group_color(j);
            let distance = (0..3).map(|c| (a[c] - b[c]).powi(2)).sum::<f32>().sqrt();
            assert!(
                distance > 0.23,
                "the first groups must have clearly separated hues"
            );
        }
    }
}

#[test]
fn hidden_group_faces_are_protected_from_brushes_and_legacy_groups_stay_visible() {
    let mut mesh = plane();
    mesh.create_vertex_group("Surface".into(), 0.5, true)
        .unwrap();
    let legacy = serde_json::to_string(&mesh).unwrap();
    assert!(!legacy.contains("visible"));
    let loaded: Mesh = serde_json::from_str(&legacy).unwrap();
    assert!(loaded.vertex_groups[0].visible);
    mesh.vertex_groups[0].visible = false;
    let before = mesh.clone();
    let dab = Dab {
        center: Vec3::default(),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 10.,
        pressure: 1.,
        delta: Vec3(0., 0., 0.3),
        invert: false,
    };
    for kind in [Kind::Standard, Kind::Mask, Kind::Smooth, Kind::Snake] {
        mesh.stamp(&Brush::new(kind), dab);
        assert_eq!(mesh, before);
    }
    let refined = mesh.subdivide().unwrap();
    assert!((0..refined.triangles.len()).all(|i| !refined.face_visible(i)));
    let shell = mesh.solidify(0.2).unwrap();
    assert!(!shell.vertex_groups[0].visible);
}

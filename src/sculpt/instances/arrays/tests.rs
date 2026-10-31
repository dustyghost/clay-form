use super::*;

fn part() -> Piece {
    Piece {
        source: Source::Model(1),
        pivot: Vec3::default(),
        forward: 0,
        up: 2,
    }
}
fn set(id: u64, a: Array) -> Set {
    Set {
        id,
        name: format!("Array {id}"),
        visible: true,
        layout: Layout::Array(a),
        adjustments: Vec::new(),
    }
}
fn near(a: Vec3, b: Vec3) {
    assert!((a - b).len() < 1e-4, "{a:?} != {b:?}");
}

#[test]
fn layered_array_composes_xyz_transforms_and_surface_normals() {
    let mut a = Array::new(part());
    a.count = 2;
    a.spacing = Vec3(2., 0., 0.);
    a.step_rotation = Vec3(0., 0., 90.);
    a.step_scale = Vec3(2., 1., 1.);
    let mut outer = ArrayLayer::repeat();
    outer.spacing = Vec3(0., 4., 0.);
    outer.step_rotation = Vec3(0., 0., 90.);
    outer.step_scale = Vec3(1., 2., 3.);
    a.layers.push(outer);
    let parts = a.placements().unwrap();
    assert_eq!(parts.len(), 4);
    let point = Vec3(1., 1., 0.);
    near(parts[0].point(point), point);
    near(parts[1].point(point), Vec3(1., 2., 0.));
    near(parts[2].point(point), Vec3(-2., 5., 0.));
    near(parts[3].point(point), Vec3(-4., 5., 0.));
    assert!(parts.iter().all(|p| p.piece.source == part().source));
    // A non-right-angle rotation followed by nonuniform scaling creates shear.
    // Compare shading normals to independently transformed triangle edges.
    a.base.step_rotation = Vec3(23., 41., 37.);
    a.layers[0].rotation = Vec3(17., -19., 32.);
    a.layers[0].size = Vec3(0.6, 1.4, 1.9);
    let p = a.placements().unwrap()[3];
    let x = Vec3(0.8, 0.3, 0.5);
    let y = Vec3(-0.4, 0.2, 0.6);
    let n = (p.point(x) - p.point(Vec3::default()))
        .cross(p.point(y) - p.point(Vec3::default()))
        .norm();
    near(n, p.normal_basis().vector(x.cross(y).norm()).norm());
    let mut independent = Adjustment::new(3);
    independent.axis_scale = Vec3(1., 0.5, 2.);
    independent.rotation = Vec3(0., 0., 90.);
    let mut s = set(1, a);
    s.adjustments.push(independent);
    let adjusted = s.placements(&[]).unwrap()[3].1;
    let v = p.point(point) - p.transform.origin;
    near(
        adjusted.point(point),
        p.transform.origin + Vec3(-v.1 * 0.5, v.0, v.2 * 2.),
    );
}

#[test]
fn layered_array_radial_repeats_whole_rows_around_their_pivot() {
    let mut a = Array::new(part());
    a.center = Vec3(9., 3., -2.);
    a.count = 2;
    a.spacing = Vec3(1., 0., 0.);
    let mut ring = ArrayLayer::repeat();
    ring.kind = ArrayKind::Radial;
    ring.count = 4;
    ring.radius = 3.;
    ring.step_offset = Vec3(0., 0., 0.5);
    a.layers.push(ring);
    let out = a.placements().unwrap();
    near(out[0].transform.origin, Vec3(12., 3., -2.));
    near(out[1].transform.origin, Vec3(13., 3., -2.));
    near(out[2].transform.origin, Vec3(9., 6., -1.5));
    near(out[3].transform.origin, Vec3(9., 7., -1.5));
    assert_eq!(out.len(), 8);
}

#[test]
fn layered_array_checks_limits_cycles_and_missing_sources_without_compacting_slots() {
    let mut base = Array::new(part());
    base.count = 3;
    let mut source = set(1, base.clone());
    let mut hidden = Adjustment::new(1);
    hidden.visible = false;
    source.adjustments.push(hidden);
    source.visible = false;
    let mut link = Array::new(part());
    link.source_array = Some(1);
    link.count = 2;
    let mut library = Library {
        sets: vec![source, set(2, link.clone())],
        ..Default::default()
    };
    let list = ArrayResolver::new(&library)
        .placements(&library.sets[1], &[])
        .unwrap();
    assert_eq!(
        list.iter().map(|(slot, _)| *slot).collect::<Vec<_>>(),
        vec![0, 2, 3, 5]
    );
    assert!(library.ensure_array_unreferenced(1).is_err());
    let Layout::Array(first) = &mut library.sets[0].layout else {
        unreachable!()
    };
    first.source_array = Some(2);
    assert!(
        ArrayResolver::new(&library)
            .placements(&library.sets[1], &[])
            .unwrap_err()
            .contains("cannot repeat")
    );
    library.sets.remove(0);
    assert!(
        ArrayResolver::new(&library)
            .placements(&library.sets[0], &[])
            .unwrap_err()
            .contains("missing")
    );
    base.count = 17;
    let mut layer = ArrayLayer::repeat();
    layer.count = 16;
    base.layers.push(layer);
    assert!(base.placements().unwrap_err().contains("256"));
    base.layers.clear();
    base.step_scale = Vec3(2., 1., 1.);
    assert!(base.placements().is_err());
    base.step_scale = ones();
    base.layers = vec![ArrayLayer::repeat(); MAX_ARRAY_LAYERS];
    assert!(base.placements().is_err());
    base.layers.clear();
    let legacy = serde_json::to_value(&base).unwrap();
    for extra in ["layers", "base", "source_array", "size", "step_scale"] {
        assert!(legacy.get(extra).is_none())
    }
    assert_eq!(serde_json::from_value::<Array>(legacy).unwrap(), base);
}

use super::*;
#[test]
fn sphere_is_closed_and_outward() {
    let m = Mesh::sphere();
    m.validate().unwrap();
    let n = m.normals();
    assert!(m.vertices.iter().zip(n).all(|(v, n)| v.dot(n) > 0.99));
    let mut edges = std::collections::HashMap::new();
    for t in m.triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    assert!(edges.values().all(|&n| n == 2));
}
#[test]
fn initial_sphere_is_all_quads_and_preserves_them_through_subdivision() {
    let sphere = Mesh::sphere();
    assert_eq!(sphere.vertices.len(), 4706);
    assert_eq!(sphere.quads.len(), 4704);
    assert_eq!(sphere.triangles.len(), sphere.quads.len() * 2);
    assert!(sphere.vertices.iter().all(|v| (v.len() - 1.).abs() < 1e-6));
    let mut faces = std::collections::HashSet::new();
    for &[a, b, c, d] in &sphere.quads {
        for mut triangle in [[a, b, c], [a, c, d]] {
            triangle.sort_unstable();
            assert!(faces.insert(triangle));
        }
    }
    for mut triangle in sphere.triangles.clone() {
        triangle.sort_unstable();
        assert!(
            faces.remove(&triangle),
            "every rendering triangle belongs to a quad"
        );
    }
    assert!(faces.is_empty());
    let refined = sphere.subdivide().unwrap();
    assert_eq!(refined.quads.len(), sphere.quads.len() * 4);
    assert_eq!(refined.triangles.len(), refined.quads.len() * 2);
    refined.validate_closed().unwrap();
    let loaded: Mesh = serde_json::from_str(&serde_json::to_string(&sphere).unwrap()).unwrap();
    assert_eq!(sphere, loaded);
}

#[test]
fn add_subtract_and_mask() {
    let mut m = Mesh::sphere();
    let original = m.vertices[0];
    let b = Brush::new(Kind::Standard);
    m.stamp(
        &b,
        Dab {
            center: original,
            normal: original,
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        },
    );
    assert!(m.vertices[0].1 > original.1);
    let mut m = Mesh::sphere();
    m.stamp(
        &b,
        Dab {
            center: original,
            normal: original,
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: true,
        },
    );
    assert!(m.vertices[0].1 < original.1);
    m.mask[0] = 1.;
    let before = m.vertices[0];
    m.stamp(
        &b,
        Dab {
            center: before,
            normal: original,
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        },
    );
    assert_eq!(m.vertices[0].1, before.1);
}
#[test]
fn black_alpha_protects_from_every_brush() {
    for kind in Kind::ALL {
        let mut mesh = Mesh::sphere();
        let old = mesh.vertices.clone();
        let mut brush = Brush::new(kind);
        brush.alpha = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.].into(),
        };
        mesh.stamp(
            &brush,
            Dab {
                center: Vec3(0., 1., 0.),
                normal: Vec3(0., 1., 0.),
                right: Vec3(1., 0., 0.),
                up: Vec3(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: Vec3(0.1, 0., 0.),
                invert: false,
            },
        );
        assert!(
            mesh.vertices
                .iter()
                .zip(old)
                .all(|(a, b)| (a.0, a.1, a.2) == (b.0, b.1, b.2))
        );
        assert!(mesh.mask.iter().all(|&m| m == 0.));
    }
}

#[test]
fn project_roundtrip() {
    let p = Project {
        scene: Default::default(),
        model_name: "Model".into(),
        submodels: Vec::new(),
        mesh_settings: crate::sculpt::MeshSettings::default(),
        version: 1,
        mesh: Mesh::sphere(),
        brushes: Kind::ALL.into_iter().map(Brush::new).collect(),
        symmetry: super::Symmetry::default(),
    };
    let bytes = serde_json::to_vec(&p).unwrap();
    let q: Project = serde_json::from_slice(&bytes).unwrap();
    q.mesh.validate().unwrap();
    for b in q.brushes {
        validate_brush(&b).unwrap();
    }
}

#[test]
fn snake_brush_pulls_with_alpha_pressure_mask_and_reverse() {
    let original = Mesh::sphere();
    let mut brush = Brush::new(Kind::Snake);
    let dab = Dab {
        center: Vec3(0., 1., 0.),
        normal: Vec3(0., 1., 0.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 0., 1.),
        radius: 0.3,
        pressure: 1.,
        delta: Vec3(0.2, 0., 0.),
        invert: false,
    };
    let mut full = original.clone();
    full.stamp(&brush, dab);
    let motion = full.vertices[0] - original.vertices[0];
    assert!(motion.0 > 0.19);
    assert!(motion.1 > 0.);
    let mut low = original.clone();
    low.stamp(
        &brush,
        Dab {
            pressure: 0.5,
            ..dab
        },
    );
    assert!(((low.vertices[0] - original.vertices[0]) - motion * 0.5).len() < 1e-6);
    brush.subtract = true;
    let mut reverse = original.clone();
    reverse.stamp(&brush, dab);
    assert!(((reverse.vertices[0] - original.vertices[0]) + motion).len() < 1e-6);
    brush.subtract = false;
    let mut masked = original.clone();
    masked.mask.fill(1.);
    masked.stamp(&brush, dab);
    assert_eq!(masked.vertices, original.vertices);
    brush.alpha = Alpha::Image {
        width: 1,
        height: 1,
        pixels: vec![0.].into(),
    };
    let mut protected = original.clone();
    protected.stamp(&brush, dab);
    assert_eq!(protected, original);
}

#[test]
fn long_snake_drag_keeps_a_regular_surface() {
    check_long_snake_drag(false);
}

#[test]
fn curved_snake_drag_keeps_a_regular_surface() {
    check_long_snake_drag(true);
}

fn check_long_snake_drag(curved: bool) {
    let mut mesh = Mesh::sphere();
    let brush = Brush::new(Kind::Snake);
    let settings = super::AdaptiveSettings {
        enabled: true,
        detail: 5.,
        ..Default::default()
    };
    let mut dab = Dab {
        center: Vec3(0., 1., 0.),
        normal: Vec3(0., 1., 0.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 0., 1.),
        radius: 0.3,
        pressure: 1.,
        delta: Vec3(0.025, 0., 0.),
        invert: false,
    };
    for step in 0..160 {
        if curved {
            dab.delta = Vec3(0.025, 0., 0.018 * (step as f32 * 0.06).cos());
        }
        let mut budget = super::AdaptiveBudget::default();
        mesh.adapt(&brush, &[dab], &settings, &mut budget, true);
        mesh.stamp(&brush, dab);
        dab.center = dab.center + brush.snake_motion(dab);
        mesh.adapt(&brush, &[dab], &settings, &mut budget, false);
    }
    mesh.validate_closed().unwrap();
    assert!(
        mesh.vertices.iter().any(|v| v.0 > 3.5),
        "tip stopped following the long drag"
    );
    let mut poor = 0;
    let mut local = 0;
    for &[a, b, c] in &mesh.triangles {
        let [a, b, c] = [a, b, c].map(|i| mesh.vertices[i]);
        if (a.0 + b.0 + c.0) / 3. < 0.5 {
            continue;
        }
        local += 1;
        let sum = (b - a).dot(b - a) + (c - b).dot(c - b) + (a - c).dot(a - c);
        let quality = 2. * 3f32.sqrt() * (b - a).cross(c - a).len() / sum;
        if quality < 0.15 {
            poor += 1;
        }
    }
    assert!(local > 100, "no extended surface: {local}");
    // Allow a small transition band of thin faces at the original surface.
    assert!(poor * 50 < local, "{poor}/{local} stretched triangles");
}

#[test]
fn detail_carves_sharp_center_raised_shoulders_and_inverts_depth() {
    let mut plane = Mesh {
        vertex_groups: Vec::new(),
        hidden_faces: Vec::new(),
        polygroups: Vec::new(),
        quads: Vec::new(),
        vertices: vec![],
        triangles: vec![],
        mask: vec![0.; 41 * 41],
    };
    for y in 0..41 {
        for x in 0..41 {
            plane
                .vertices
                .push(Vec3(x as f32 / 20. - 1., y as f32 / 20. - 1., 0.));
        }
    }
    for y in 0..40 {
        for x in 0..40 {
            let a = y * 41 + x;
            plane
                .triangles
                .extend([[a, a + 1, a + 41], [a + 1, a + 42, a + 41]]);
        }
    }
    let brush = Brush::new(Kind::Detail);
    let dab = Dab {
        center: Vec3::default(),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 1.,
        pressure: 1.,
        delta: Vec3::default(),
        invert: false,
    };
    let center = 20 * 41 + 20;
    let shoulder = center + 8;
    let original = plane.clone();
    assert!(plane.stamp_many(&brush, &[dab]));
    assert!(plane.vertices[center].2 < -0.05);
    assert!(plane.vertices[shoulder].2 > 0.005);
    assert!(plane.vertices[shoulder].0 < original.vertices[shoulder].0);
    assert_eq!(plane.vertices[0], original.vertices[0]);
    let mut inverse = original.clone();
    inverse.stamp(
        &brush,
        Dab {
            invert: true,
            ..dab
        },
    );
    assert!((inverse.vertices[center].2 + plane.vertices[center].2).abs() < 1e-6);
    assert!((inverse.vertices[shoulder].2 + plane.vertices[shoulder].2).abs() < 1e-6);
    assert_eq!(inverse.vertices[shoulder].0, plane.vertices[shoulder].0);
    let mut protected = original.clone();
    protected.mask[center] = 1.;
    protected.stamp(&brush, dab);
    assert_eq!(protected.vertices[center], original.vertices[center]);
    let mut pressure = original;
    pressure.stamp(
        &brush,
        Dab {
            pressure: 0.5,
            ..dab
        },
    );
    assert!((pressure.vertices[center].2 - plane.vertices[center].2 * 0.5).abs() < 1e-6);
    let json = serde_json::to_vec(&brush).unwrap();
    assert!(serde_json::from_slice::<Brush>(&json).unwrap() == brush);
}

#[test]
fn drag_relief_has_visible_depth_and_preserves_pressure_and_mask_opacity() {
    let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
    plane.divisions = [10, 10, 1];
    let original = plane.mesh().unwrap();
    let dab = Dab {
        center: Vec3::default(),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 0.5,
        pressure: 1.,
        delta: Vec3::default(),
        invert: false,
    };
    let peak = |mesh: &Mesh| mesh.vertices.iter().map(|v| v.2).fold(0_f32, f32::max);
    let mut brush = Brush::new(Kind::Standard);
    let mut freehand = original.clone();
    freehand.stamp(&brush, dab);
    brush.stroke_mode = StrokeMode::DragRect;
    let mut stamp = original.clone();
    stamp.stamp(&brush, dab);
    assert!((peak(&stamp) - peak(&freehand) * 4.).abs() < 1e-6);
    assert!(
        peak(&stamp) > 0.05,
        "default-strength stamp must create clear relief"
    );
    let mut pressure = original.clone();
    pressure.stamp(
        &brush,
        Dab {
            pressure: 0.5,
            ..dab
        },
    );
    assert!((peak(&pressure) - peak(&stamp) * 0.5).abs() < 1e-6);
    brush.kind = Kind::Mask;
    let mut masked = original;
    masked.stamp(&brush, dab);
    assert!((masked.mask.iter().copied().fold(0_f32, f32::max) - brush.strength).abs() < 1e-6);
}

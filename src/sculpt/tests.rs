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
        version: 1,
        mesh: Mesh::sphere(),
        brushes: Kind::ALL.into_iter().map(Brush::new).collect(),
    };
    let bytes = serde_json::to_vec(&p).unwrap();
    let q: Project = serde_json::from_slice(&bytes).unwrap();
    q.mesh.validate().unwrap();
    for b in q.brushes {
        validate_brush(&b).unwrap();
    }
}

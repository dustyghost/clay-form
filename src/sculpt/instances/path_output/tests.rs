use super::*;
use crate::sculpt::{
    Material, Primitive, PrimitiveKind,
    paths::{PathMode, PathPoint},
};

fn bar() -> Mesh {
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [12, 2, 2];
    let mut mesh = cube.mesh().unwrap();
    for p in &mut mesh.vertices {
        p.1 *= 0.15;
        p.2 *= 0.15;
    }
    mesh.mask.fill(0.3);
    mesh.materials = vec![Material::from_srgb([0.2, 0.4, 0.6], 0.4, 0.1); mesh.vertices.len()];
    mesh
}
fn piece(id: u64) -> Piece {
    Piece {
        source: Source::Model(id),
        pivot: Vec3::default(),
        forward: 0,
        up: 2,
    }
}
fn path(closed: bool) -> SurfacePath {
    SurfacePath {
        id: 1,
        name: "Curve".into(),
        target: 1,
        mode: PathMode::Free,
        points: if closed {
            vec![
                Vec3(-2., 0., 0.),
                Vec3(0., 2., 0.),
                Vec3(2., 0., 0.),
                Vec3(0., -2., 0.),
            ]
        } else {
            vec![
                Vec3(-3., 0., 0.),
                Vec3(-1., 1.5, 0.),
                Vec3(1., 1.5, 0.),
                Vec3(3., 0., 0.),
            ]
        }
        .into_iter()
        .map(|position| PathPoint {
            position,
            normal: axis(2),
        })
        .collect(),
        smoothing: 1.,
        corner_rounding: Vec::new(),
        closed,
        seam: 0,
        surface_signature: 0,
        attached_route: None,
    }
}
fn set(join: JoinMode) -> Set {
    Set {
        id: 1,
        name: "Bent rail".into(),
        visible: true,
        adjustments: Vec::new(),
        layout: Layout::Path(PathAssembly {
            path: 1,
            start: None,
            middle: piece(1),
            end: None,
            count: 6,
            spacing: 2.,
            fit_count: true,
            twist: 0.,
            scale: 1.,
            bend: true,
            fit_sections: true,
            join,
            join_distance: 0.05,
        }),
    }
}
fn connected_closed(mesh: &Mesh) -> (usize, usize, usize) {
    let mut edges = HashMap::<_, usize>::new();
    let mut neighbours = vec![Vec::new(); mesh.vertices.len()];
    for &[a, b, c] in &mesh.triangles {
        for (a, b) in [(a, b), (b, c), (c, a)] {
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            neighbours[a].push(b);
            neighbours[b].push(a);
        }
    }
    assert!(
        edges.values().all(|&n| n == 2),
        "joined surface must have no boundary/non-manifold edges"
    );
    let mut seen = HashSet::new();
    let mut stack = vec![0];
    while let Some(i) = stack.pop() {
        if seen.insert(i) {
            stack.extend(neighbours[i].iter().copied());
        }
    }
    assert_eq!(
        seen.len(),
        mesh.vertices.len(),
        "all sections must belong to one surface"
    );
    (mesh.vertices.len(), edges.len(), mesh.triangles.len())
}
#[test]
fn reusable_path_bend_and_join_create_continuous_closed_geometry() {
    let mesh = bar();
    let before = mesh.clone();
    let path = path(false);
    let separate = set(JoinMode::Separate);
    let copies = separate
        .generated(std::slice::from_ref(&path), |_| Some(&mesh), &|| false)
        .unwrap()
        .unwrap();
    assert_eq!(copies.len(), 6);
    assert!(
        copies.iter().any(|c| c
            .mesh
            .vertices
            .iter()
            .zip(&mesh.vertices)
            .any(|(&b, &s)| (b - c.placement.point(s)).len() > 0.1)),
        "vertices must deform, not just rotate"
    );
    let joined = set(JoinMode::Weld)
        .generated(&[path], |_| Some(&mesh), &|| false)
        .unwrap()
        .unwrap();
    assert_eq!(joined.len(), 1);
    let result = &joined[0].mesh;
    let (v, e, f) = connected_closed(result);
    assert_eq!(v as isize - e as isize + f as isize, 2);
    assert!(result.triangles.len() < copies.iter().map(|c| c.mesh.triangles.len()).sum());
    assert!(result.mask.iter().all(|&m| (m - 0.3).abs() < 1e-5));
    assert!(
        result
            .materials
            .iter()
            .all(|m| (m.roughness - 0.4).abs() < 1e-5)
    );
    assert_eq!(mesh, before);
}
#[test]
fn reusable_path_join_distinct_caps_and_closed_seam() {
    let middle = bar();
    let mut start = middle.clone();
    let mut end = middle.clone();
    for p in &mut start.vertices {
        let scale = 0.75 + 0.25 * p.0;
        p.1 *= scale;
        p.2 *= scale;
    }
    for p in &mut end.vertices {
        let scale = 0.75 - 0.25 * p.0;
        p.1 *= scale;
        p.2 *= scale;
    }
    let mut s = set(JoinMode::Weld);
    if let Layout::Path(a) = &mut s.layout {
        a.start = Some(piece(2));
        a.end = Some(piece(3));
    }
    let resolve = |source| match source {
        Source::Model(2) => Some(&start),
        Source::Model(3) => Some(&end),
        _ => Some(&middle),
    };
    let output = s
        .generated(&[path(false)], resolve, &|| false)
        .unwrap()
        .unwrap();
    connected_closed(&output[0].mesh);
    let output = s
        .generated(&[path(true)], resolve, &|| false)
        .unwrap()
        .unwrap();
    let (v, e, f) = connected_closed(&output[0].mesh);
    assert_eq!(
        v as isize - e as isize + f as isize,
        0,
        "closed bent loop must weld its final seam once"
    );
}
#[test]
fn reusable_path_join_distance_is_bounded_and_rejects_incompatible_ends() {
    let mesh = bar();
    let mut line = path(false);
    line.points = vec![
        PathPoint {
            position: Vec3::default(),
            normal: axis(2),
        },
        PathPoint {
            position: Vec3(2.02, 0., 0.),
            normal: axis(2),
        },
    ];
    line.smoothing = 0.;
    let mut s = set(JoinMode::Weld);
    if let Layout::Path(a) = &mut s.layout {
        a.bend = false;
        a.count = 2;
    }
    let output = s
        .generated(std::slice::from_ref(&line), |_| Some(&mesh), &|| false)
        .unwrap()
        .unwrap();
    connected_closed(&output[0].mesh);
    if let Layout::Path(a) = &mut s.layout {
        a.join_distance = 0.001;
    }
    assert!(
        s.generated(std::slice::from_ref(&line), |_| Some(&mesh), &|| false)
            .err()
            .unwrap()
            .contains("farther apart")
    );
    let other = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
    if let Layout::Path(a) = &mut s.layout {
        a.join_distance = 0.05;
        a.start = Some(piece(2));
        a.count = 1;
    }
    assert!(
        s.generated(
            &[line],
            |s| if s == Source::Model(2) {
                Some(&other)
            } else {
                Some(&mesh)
            },
            &|| false
        )
        .is_err()
    );
}
#[test]
fn reusable_path_join_preserves_a_hollow_pipe_bore() {
    let n = 12;
    let mut pipe = Mesh::default();
    for x in [-1., 1.] {
        for radius in [0.2, 0.1] {
            for i in 0..n {
                let angle = std::f32::consts::TAU * i as f32 / n as f32;
                pipe.vertices
                    .push(Vec3(x, radius * angle.cos(), radius * angle.sin()));
            }
        }
    }
    for i in 0..n {
        let j = (i + 1) % n;
        for q in [
            [i, j, 2 * n + j, 2 * n + i],
            [n + i, 3 * n + i, 3 * n + j, n + j],
            [i, n + i, n + j, j],
            [2 * n + i, 2 * n + j, 3 * n + j, 3 * n + i],
        ] {
            let [a, b, c, d] = q;
            pipe.triangles.extend([[a, b, c], [a, c, d]]);
            pipe.quads.push(q);
        }
    }
    pipe.mask = vec![0.; pipe.vertices.len()];
    pipe.validate().unwrap();
    connected_closed(&pipe);
    let output = set(JoinMode::Weld)
        .generated(&[path(false)], |_| Some(&pipe), &|| false)
        .unwrap()
        .unwrap();
    let (v, e, f) = connected_closed(&output[0].mesh);
    assert_eq!(
        v as isize - e as isize + f as isize,
        0,
        "the bore must remain open through the full pipe, not be filled by internal caps"
    );
}
#[test]
fn reusable_path_bend_cancellation_limits_and_legacy_defaults() {
    let mesh = bar();
    let s = set(JoinMode::Weld);
    let path = path(false);
    let ticks = std::cell::Cell::new(0);
    assert!(
        s.generated(std::slice::from_ref(&path), |_| Some(&mesh), &|| {
            ticks.set(ticks.get() + 1);
            ticks.get() > 5
        })
        .is_err()
    );
    let mut hidden = s.clone();
    let mut a = Adjustment::new(2);
    a.visible = false;
    hidden.adjustments.push(a);
    assert!(
        hidden
            .generated(std::slice::from_ref(&path), |_| Some(&mesh), &|| false)
            .is_err()
    );
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [40; 3];
    let dense = cube.mesh().unwrap();
    let mut large = s.clone();
    if let Layout::Path(a) = &mut large.layout {
        a.count = 30;
    }
    assert!(
        large
            .generated(&[path], |_| Some(&dense), &|| false)
            .err()
            .unwrap()
            .contains("250,000")
    );
    let Layout::Path(recipe) = s.layout else {
        unreachable!()
    };
    let mut json = serde_json::to_value(recipe).unwrap();
    for key in ["bend", "join", "join_distance", "fit_sections"] {
        json.as_object_mut().unwrap().remove(key);
    }
    let legacy: PathAssembly = serde_json::from_value(json).unwrap();
    assert!(!legacy.bend);
    assert_eq!(legacy.join, JoinMode::Separate);
    assert_eq!(legacy.join_distance, 0.05);
}

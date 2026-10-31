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
            thickness: Default::default(),
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
    for key in ["bend", "join", "join_distance", "fit_sections", "thickness"] {
        json.as_object_mut().unwrap().remove(key);
    }
    let legacy: PathAssembly = serde_json::from_value(json).unwrap();
    assert!(!legacy.bend);
    assert_eq!(legacy.join, JoinMode::Separate);
    assert_eq!(legacy.join_distance, 0.05);
    assert_eq!(legacy.thickness, Thickness::default());
}

#[test]
fn reusable_path_thickness_changes_cross_section_without_changing_length() {
    let mesh = bar();
    let original = mesh.clone();
    let mut path = path(false);
    path.points = [Vec3::default(), Vec3(6., 0., 0.)]
        .map(|position| PathPoint {
            position,
            normal: axis(2),
        })
        .to_vec();
    path.smoothing = 0.;
    let mut s = set(JoinMode::Separate);
    let baseline = s
        .generated(std::slice::from_ref(&path), |_| Some(&mesh), &|| false)
        .unwrap()
        .unwrap();
    for mode in [ThicknessMode::Uniform, ThicknessMode::Taper] {
        let Layout::Path(a) = &mut s.layout else {
            unreachable!()
        };
        a.thickness = Thickness {
            mode,
            uniform: 2.,
            start: 0.5,
            end: 2.5,
            smooth: false,
            ..Default::default()
        };
        let result = s
            .generated(std::slice::from_ref(&path), |_| Some(&mesh), &|| false)
            .unwrap()
            .unwrap();
        for (before, after) in baseline.iter().zip(&result) {
            assert_eq!(before.mesh.triangles, after.mesh.triangles);
            for (&p, &q) in before.mesh.vertices.iter().zip(&after.mesh.vertices) {
                let factor = if mode == ThicknessMode::Uniform {
                    2.
                } else {
                    0.5 + 2. * p.0 / 6.
                };
                assert!(
                    (q.0 - p.0).abs() < 1e-6,
                    "thickness must not change section length"
                );
                assert!((q.1 - p.1 * factor).abs() < 1e-5);
                assert!((q.2 - p.2 * factor).abs() < 1e-5);
            }
        }
    }
    let Layout::Path(a) = &mut s.layout else {
        unreachable!()
    };
    a.join = JoinMode::Weld;
    let joined = s
        .generated(&[path], |_| Some(&mesh), &|| false)
        .unwrap()
        .unwrap();
    connected_closed(&joined[0].mesh);
    assert_eq!(mesh, original);
}

#[test]
fn reusable_path_thickness_profiles_and_closed_seams_join_continuously() {
    let mesh = bar();
    let mut s = set(JoinMode::Weld);
    for closed in [false, true] {
        for mode in [ThicknessMode::Taper, ThicknessMode::Profile] {
            let Layout::Path(a) = &mut s.layout else {
                unreachable!()
            };
            a.thickness = Thickness {
                mode,
                start: 0.5,
                end: 2.,
                points: vec![
                    ThicknessPoint {
                        position: 0.,
                        factor: 0.5,
                    },
                    ThicknessPoint {
                        position: 0.5,
                        factor: 2.,
                    },
                    ThicknessPoint {
                        position: 1.,
                        factor: 1.5,
                    },
                ],
                ..Default::default()
            };
            if closed {
                assert_eq!(a.thickness.at(0., true), a.thickness.at(1., true));
                assert!((a.thickness.at(-0.001, true) - a.thickness.at(0.001, true)).abs() < 1e-4);
            }
            let result = s
                .generated(&[path(closed)], |_| Some(&mesh), &|| false)
                .unwrap()
                .unwrap();
            let (v, e, f) = connected_closed(&result[0].mesh);
            assert_eq!(
                v as isize - e as isize + f as isize,
                if closed { 0 } else { 2 }
            );
            let max_z = result[0]
                .mesh
                .vertices
                .iter()
                .map(|p| p.2)
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                max_z > 0.28,
                "the middle should swell to twice the original section size"
            );
        }
    }
}

#[test]
fn reusable_path_thickness_validates_profiles_and_interpolation() {
    let mut thickness = Thickness {
        mode: ThicknessMode::Profile,
        smooth: false,
        ..Default::default()
    };
    thickness.points[0].factor = 0.5;
    thickness.points[1].factor = 2.;
    thickness.points[2].factor = 1.;
    thickness.validate().unwrap();
    assert_eq!(thickness.at(0.25, false), 1.25);
    assert_eq!(thickness.at(0.75, false), 1.5);
    assert_eq!(thickness.at(-1., false), 0.5);
    assert_eq!(thickness.at(2., false), 1.);
    assert_eq!(thickness.at(0.75, true), 1.25);
    for factor in [0., -1., f32::NAN, f32::INFINITY, 10.01] {
        let mut invalid = thickness.clone();
        invalid.points[1].factor = factor;
        assert!(invalid.validate().is_err());
    }
    for position in [0., 1., f32::NAN, f32::INFINITY] {
        let mut invalid = thickness.clone();
        invalid.points[1].position = position;
        assert!(invalid.validate().is_err());
    }
    let mut invalid = thickness.clone();
    invalid.points.clear();
    assert!(invalid.validate().is_err());
    invalid.points = (0..=MAX_THICKNESS_POINTS)
        .map(|i| ThicknessPoint {
            position: i as f32 / MAX_THICKNESS_POINTS as f32,
            factor: 1.,
        })
        .collect();
    assert!(invalid.validate().is_err());
    thickness.smooth = true;
    assert!((thickness.at(0.001, false) - 0.5).abs() < 0.0001);
    assert!((thickness.at(0.499, false) - 2.).abs() < 0.0001);
    let mut inline = Thickness::default();
    inline.edit_at(0., 0.5, false).unwrap();
    assert_eq!(inline.mode, ThicknessMode::Taper);
    assert_eq!(inline.at(1., false), 1.);
    inline.edit_at(0.4, 2., false).unwrap();
    assert_eq!(inline.mode, ThicknessMode::Profile);
    assert_eq!(inline.at(0., false), 0.5);
    assert_eq!(inline.at(0.4, false), 2.);
    inline.edit_at(1., 1.5, true).unwrap();
    assert_eq!(inline.at(0., true), inline.at(1., true));
    assert_eq!(inline.at(0., true), 1.5);
}

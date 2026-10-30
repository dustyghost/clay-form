use super::*;
use crate::sculpt::{Alpha, Axis, Brush, Primitive, PrimitiveKind, StrokeMode, Symmetry, alpha};
use std::cell::Cell;

thread_local! { pub(super) static FULL_NORMALS: Cell<bool> = const { Cell::new(false) }; }
fn full_normals<T>(f: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            FULL_NORMALS.set(self.0);
        }
    }
    let _reset = Reset(FULL_NORMALS.replace(true));
    f()
}
fn bits(v: Vec3) -> [u32; 3] {
    [v.0.to_bits(), v.1.to_bits(), v.2.to_bits()]
}
fn dab(radius: f32) -> Dab {
    Dab {
        center: Vec3(0.08, 0.03, 1.),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius,
        pressure: 0.75,
        delta: Vec3(0.12, 0., -0.04),
        invert: false,
    }
}

#[test]
fn selected_normals_match_full_accumulation_bit_for_bit() {
    let unusual = Mesh {
        vertices: vec![
            Vec3(-0., 0., 0.),
            Vec3(2., 0., 0.),
            Vec3(0., 3., 0.),
            Vec3(0., 0., 4.),
            Vec3(8., 8., 8.),
        ],
        // Unequal areas, opposing winding, repeated corners and an isolated vertex.
        triangles: vec![[0, 1, 2], [0, 2, 3], [2, 1, 0], [3, 3, 1], [1, 2, 3]],
        ..Default::default()
    };
    for mesh in [Mesh::default(), unusual, Mesh::sphere()] {
        let expected = mesh.normals();
        for pattern in 0..6 {
            let needed: Vec<_> = (0..mesh.vertices.len())
                .map(|i| match pattern {
                    0 => false,
                    1 => true,
                    2 => i % 2 == 0,
                    3 => i % 7 == 0,
                    4 => i == 0,
                    _ => i + 1 == mesh.vertices.len(),
                })
                .collect();
            let actual = mesh.normals_for(&needed);
            assert_eq!(actual.len(), expected.len());
            for (i, &needed) in needed.iter().enumerate() {
                if needed {
                    assert_eq!(
                        bits(actual[i]),
                        bits(expected[i]),
                        "pattern={pattern}, vertex={i}"
                    );
                }
            }
        }
    }
}

#[test]
fn stamp_normal_selection_preserves_radius_boundary_and_special_brushes() {
    let mut mesh = Mesh::sphere();
    let d = Dab {
        center: Vec3::default(),
        ..dab(0.5)
    };
    mesh.vertices.extend([
        Vec3(0.75, 0., 0.),
        Vec3(f32::from_bits(0.75_f32.to_bits() + 1), 0., 0.),
    ]);
    for dabs in [vec![], vec![d], vec![d, dab(0.65)], vec![dab(3.)]] {
        for kind in [Kind::Standard, Kind::Snake, Kind::Polish] {
            let mut needed = vec![true; mesh.vertices.len()];
            needed[0] = false;
            let original = needed.clone();
            let normals = mesh.stamp_normals(kind, &dabs, &mut needed);
            let expected = mesh.normals();
            if kind == Kind::Snake {
                assert!(normals.is_empty());
            }
            for i in 0..needed.len() {
                assert_eq!(
                    needed[i],
                    original[i]
                        && (kind == Kind::Polish
                            || !dabs
                                .iter()
                                .all(|d| (mesh.vertices[i] - d.center).len() > d.radius * 1.5))
                );
                if needed[i] && kind != Kind::Snake {
                    assert_eq!(bits(normals[i]), bits(expected[i]));
                }
            }
            if dabs == [d] && kind != Kind::Polish {
                assert!(needed[needed.len() - 2]);
                assert!(!needed[needed.len() - 1]);
            }
        }
    }
}

#[test]
fn broad_stamp_fallback_keeps_unsampled_outliers_safe() {
    let mut mesh = Mesh {
        vertices: vec![Vec3(0., 0., 0.); 260],
        mask: vec![0.; 260],
        triangles: vec![[0, 1, 2]],
        ..Default::default()
    };
    mesh.vertices[1] = Vec3(4., 0., 0.);
    mesh.vertices[2] = Vec3(0., 0.5, 0.);
    let d = Dab {
        center: Vec3::default(),
        ..dab(0.5)
    };
    let mut visible = vec![true; mesh.vertices.len()];
    let normals = mesh.stamp_normals(Kind::Standard, &[d], &mut visible);
    assert!(
        visible[1],
        "the full path leaves distance rejection to the stamp loop"
    );
    assert_eq!(
        normals.into_iter().map(bits).collect::<Vec<_>>(),
        mesh.normals().into_iter().map(bits).collect::<Vec<_>>()
    );
    let original = mesh.vertices[1];
    let mut expected = mesh.clone();
    let brush = Brush::new(Kind::Standard);
    assert_eq!(
        mesh.stamp(&brush, d),
        full_normals(|| expected.stamp(&brush, d))
    );
    assert_eq!(mesh, expected);
    assert_eq!(mesh.vertices[1], original);
}

#[test]
fn local_stamp_normals_match_full_mesh_brush_results_after_edits() {
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [6; 3];
    let cube = primitive.mesh().unwrap();
    let mut changed_cases = 0;
    for base in [cube, Mesh::sphere()] {
        for kind in Kind::ALL {
            if matches!(
                kind,
                Kind::Smooth | Kind::Paint | Kind::Relax | Kind::Decimate
            ) {
                continue;
            }
            for scenario in 0..6 {
                let mut actual = base.clone();
                for (i, p) in actual.vertices.iter_mut().enumerate() {
                    p.2 += 0.015 * (p.0 * 17.).sin() * (p.1 * 13.).cos();
                    actual.mask[i] = if i % 13 == 0 { 1. } else { 0.2 };
                }
                if scenario == 3 {
                    actual.hidden_faces = actual
                        .triangles
                        .iter()
                        .map(|t| t.iter().all(|&i| actual.vertices[i].0 < -0.05))
                        .collect();
                }
                let mut brush = Brush::new(kind);
                brush.strength = 0.35;
                brush.alpha = match scenario {
                    1 => Alpha::Hard,
                    2 => Alpha::Square,
                    4 => Alpha::Image {
                        width: 2,
                        height: 2,
                        pixels: [1., 0.1, 0.4, 0.8].into(),
                    },
                    _ => Alpha::Soft,
                };
                if scenario == 4 {
                    brush.stroke_mode = StrokeMode::DragRect;
                }
                let mut expected = actual.clone();
                for step in 0..3 {
                    if step == 1 && scenario == 3 {
                        actual = actual.subdivide().unwrap();
                        expected = expected.subdivide().unwrap();
                        actual.hidden_faces.clear();
                        expected.hidden_faces.clear();
                    }
                    let d = Dab {
                        invert: step == 2,
                        ..dab(if scenario == 5 { 3. } else { 0.65 })
                    };
                    let dabs = if scenario == 2 {
                        Symmetry {
                            enabled: true,
                            radial: true,
                            radial_count: 4,
                            radial_axis: Axis::Z,
                            ..Default::default()
                        }
                        .dabs(d)
                    } else if scenario == 1 {
                        Symmetry {
                            enabled: true,
                            ..Default::default()
                        }
                        .dabs(d)
                    } else {
                        vec![d]
                    };
                    let stencils = if scenario == 4 {
                        vec![alpha::Projection {
                            image: brush.alpha.clone(),
                            frame: alpha::Frame {
                                origin: Vec3::default(),
                                right: d.right,
                                up: d.up,
                            },
                            view: d.normal,
                            distance: 0.,
                            position: [0.; 2],
                            size: [1.2; 2],
                            rotation: 17.,
                        }]
                    } else {
                        vec![]
                    };
                    let changed = actual.stamp_many_stenciled(&brush, &dabs, &stencils);
                    let expected_changed =
                        full_normals(|| expected.stamp_many_stenciled(&brush, &dabs, &stencils));
                    assert_eq!(
                        changed, expected_changed,
                        "{kind:?} scenario={scenario} step={step}"
                    );
                    assert_eq!(actual, expected, "{kind:?} scenario={scenario} step={step}");
                    assert!(
                        actual
                            .vertices
                            .iter()
                            .zip(&expected.vertices)
                            .all(|(&a, &b)| bits(a) == bits(b))
                    );
                    assert!(
                        actual
                            .mask
                            .iter()
                            .zip(&expected.mask)
                            .all(|(&a, &b)| a.to_bits() == b.to_bits())
                    );
                    changed_cases += usize::from(changed);
                }
            }
        }
    }
    assert!(
        changed_cases > 100,
        "reference comparison must exercise deformation"
    );
}

#[test]
#[ignore = "opt-in comparison of local and full normal preparation; no wall-clock assertions"]
fn profile_local_stamp_normals() {
    use std::{hint::black_box, time::Instant};
    let mut mesh = Mesh::sphere();
    for _ in 0..3 {
        mesh = mesh.subdivide().unwrap();
    }
    let mut rows = Vec::new();
    for hidden in [false, true] {
        mesh.hidden_faces = if hidden {
            mesh.triangles
                .iter()
                .map(|t| t.iter().all(|&v| mesh.vertices[v].0 < -0.06))
                .collect()
        } else {
            vec![]
        };
        for radius in [0.18, 0.65, 2.] {
            for copies in [1, 8] {
                let dabs = if copies == 1 {
                    vec![dab(radius)]
                } else {
                    Symmetry {
                        enabled: true,
                        radial: true,
                        radial_count: 4,
                        radial_axis: Axis::Z,
                        ..Default::default()
                    }
                    .dabs(dab(radius))
                };
                for kind in [
                    Kind::Standard,
                    Kind::Clay,
                    Kind::Inflate,
                    Kind::Polish,
                    Kind::Snake,
                ] {
                    let brush = Brush::new(kind);
                    for trial in 0..6 {
                        let mut hashes = Vec::new();
                        // Alternate order to expose warm-cache and frequency bias.
                        for reference in [trial % 2 == 0, trial % 2 != 0] {
                            let mut candidate = mesh.clone();
                            let start = Instant::now();
                            let changed = if reference {
                                full_normals(|| candidate.stamp_many(&brush, &dabs))
                            } else {
                                candidate.stamp_many(&brush, &dabs)
                            };
                            let total_ms = start.elapsed().as_secs_f64() * 1000.;
                            let hash = crate::sculpt::brush_loop_profile::fingerprint(&candidate);
                            hashes.push((changed, hash.clone()));
                            let mut needed = mesh.sculptable_vertices();
                            let start = Instant::now();
                            let normals = if reference {
                                full_normals(|| mesh.stamp_normals(kind, &dabs, &mut needed))
                            } else {
                                mesh.stamp_normals(kind, &dabs, &mut needed)
                            };
                            black_box(&normals);
                            let preparation_ms = start.elapsed().as_secs_f64() * 1000.;
                            if trial > 0 {
                                rows.push(serde_json::json!({"hidden":hidden,"radius":radius,"copies":copies,"kind":format!("{kind:?}"),"trial":trial,"reference":reference,"total_ms":total_ms,"preparation_ms":preparation_ms,"changed":changed,"output_hash_fnv1a":hash}));
                            }
                        }
                        assert_eq!(hashes[0], hashes[1], "brush output changed");
                    }
                }
            }
        }
    }
    let output = serde_json::json!({"schema":1,"vertices":mesh.vertices.len(),"triangles":mesh.triangles.len(),"rows":rows});
    std::fs::write(
        std::env::var("CLAY_NORMAL_PROFILE").expect("output path"),
        serde_json::to_vec_pretty(&output).unwrap(),
    )
    .unwrap();
}

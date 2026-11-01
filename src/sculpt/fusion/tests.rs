use super::*;

pub(crate) fn tube(n: usize, bottom: f32, top: f32, radius: f32) -> Mesh {
    let mut mesh = Mesh::default();
    for z in [bottom, top] {
        for i in 0..n {
            let angle = i as f32 * std::f32::consts::TAU / n as f32;
            mesh.vertices
                .push(Vec3(radius * angle.cos(), radius * angle.sin(), z));
            mesh.mask.push(0.);
        }
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.push_polygon(&[i, j, j + n, i + n], 0, false);
    }
    mesh
}

#[test]
fn open_ends_preserve_sources_and_join_unequal_loops() {
    for (a_count, b_count) in [(8, 8), (8, 12), (7, 13)] {
        let a = tube(a_count, -2., -0.5, 0.7);
        let b = tube(b_count, 0.5, 2., 1.);
        let mesh = merge(&a, &b).unwrap();
        let loops = boundaries(&mesh).unwrap();
        let first = &loops
            .iter()
            .find(|l| (l.center.2 + 0.5).abs() < 0.01)
            .unwrap()
            .vertices;
        let second = &loops
            .iter()
            .find(|l| (l.center.2 - 0.5).abs() < 0.01)
            .unwrap()
            .vertices;
        for segments in [1, 4, 9] {
            let joined = bridge(
                &mesh,
                first,
                second,
                JoinOptions {
                    segments,
                    ..Default::default()
                },
                &|| false,
            )
            .unwrap();
            assert_eq!(boundaries(&joined).unwrap().len(), 2);
            assert_eq!(&joined.vertices[..mesh.vertices.len()], mesh.vertices);
            assert_eq!(&joined.triangles[..mesh.triangles.len()], mesh.triangles);
            assert_eq!(&joined.quads[..mesh.quads.len()], mesh.quads);
            assert_eq!(&joined.mask[..mesh.mask.len()], mesh.mask);
            let mut incident = BTreeMap::<_, usize>::new();
            for t in &joined.triangles {
                for i in 0..3 {
                    *incident.entry(edge(t[i], t[(i + 1) % 3])).or_default() += 1;
                }
            }
            for ring in [first, second] {
                for i in 0..ring.len() {
                    assert_eq!(incident[&edge(ring[i], ring[(i + 1) % ring.len()])], 2);
                }
            }
        }
    }
}

#[test]
fn open_ends_keep_uvs_materials_groups_and_reject_protected_rims() {
    let mut a = tube(8, -2., -0.5, 0.7);
    let mut b = tube(12, 0.5, 2., 0.9);
    for mesh in [&mut a, &mut b] {
        mesh.uv = UvMap {
            topology: mesh.triangles.clone(),
            corners: vec![[[0., 0.], [1., 0.], [0., 1.]]; mesh.triangles.len()],
            ..Default::default()
        };
        mesh.create_vertex_group("Source".into(), 0.5, true)
            .unwrap();
        mesh.materials = vec![super::super::Material::default(); mesh.vertices.len()];
        mesh.hidden_faces.resize(mesh.triangles.len(), false);
    }
    b.materials[0].base_color = [0.2, 0.7, 0.4];
    let mesh = merge(&a, &b).unwrap();
    let loops = boundaries(&mesh).unwrap();
    let first = &loops[1].vertices;
    let second = &loops[2].vertices;
    let joined = bridge(&mesh, first, second, JoinOptions::default(), &|| false).unwrap();
    assert!(joined.uv.current(&joined));
    assert_eq!(&joined.uv.corners[..mesh.triangles.len()], mesh.uv.corners);
    assert_eq!(&joined.materials[..mesh.vertices.len()], mesh.materials);
    for (old, new) in mesh.vertex_groups.iter().zip(&joined.vertex_groups) {
        assert_eq!(&new.weights[..mesh.vertices.len()], old.weights);
    }
    assert!(joined.uv.removed[mesh.triangles.len()..].iter().all(|x| *x));
    assert!(bridge(&mesh, first, second, JoinOptions::default(), &|| true).is_err());
    let mut protected = mesh.clone();
    protected.mask[first[0]] = 0.1;
    assert!(
        bridge(&protected, first, second, JoinOptions::default(), &|| false)
            .unwrap_err()
            .contains("mask")
    );
    let mut hidden = mesh.clone();
    hidden.hidden_faces[0] = true;
    let lower = &loops[0].vertices;
    assert!(
        bridge(&hidden, lower, second, JoinOptions::default(), &|| false)
            .unwrap_err()
            .contains("Show")
    );
}

#[test]
fn fusion_intersection_keeps_distant_faces_and_removes_buried_surfaces() {
    let mut a = super::super::Primitive::for_kind(super::super::PrimitiveKind::UvSphere)
        .mesh()
        .unwrap();
    let mut b = a.clone();
    for p in &mut a.vertices {
        *p = *p + Vec3(-0.3, 0., 0.);
    }
    for p in &mut b.vertices {
        *p = *p + Vec3(0.3, 0.03, 0.02);
    }
    let (joined, report) = fuse(&a, &b, 1, JoinOptions::default(), &|| false).unwrap();
    joined.validate_closed().unwrap();
    assert_eq!(report.joins, 1);
    assert!(report.removed_faces.iter().all(|n| *n > 0));
    assert!(report.retained_faces > 0);
    assert_eq!(&joined.vertices[..a.vertices.len()], a.vertices);
    assert_eq!(
        &joined.vertices[a.vertices.len()..a.vertices.len() + b.vertices.len()],
        b.vertices
    );
    let faces: BTreeSet<_> = joined.triangles.iter().copied().collect();
    for t in &a.triangles {
        if t.iter().all(|&v| a.vertices[v].0 < -0.5) {
            assert!(faces.contains(t));
        }
    }
    let mut far = b.clone();
    for p in &mut far.vertices {
        p.0 += 10.;
    }
    assert!(
        fuse(&a, &far, 0, JoinOptions::default(), &|| false)
            .unwrap_err()
            .contains("do not cross")
    );
}

#[test]
fn open_ends_reject_obstructing_surfaces_and_folded_connectors() {
    let a = tube(8, -2., -0.5, 0.7);
    let b = tube(12, 0.5, 2., 1.);
    let mut mesh = merge(&a, &b).unwrap();
    let loops = boundaries(&mesh).unwrap();
    let first = loops[1].vertices.clone();
    let second = loops[2].vertices.clone();
    let base = mesh.vertices.len();
    mesh.vertices.extend([
        Vec3(-3., -3., 0.),
        Vec3(3., -3., 0.),
        Vec3(3., 3., 0.),
        Vec3(-3., 3., 0.),
    ]);
    mesh.mask.resize(mesh.vertices.len(), 0.);
    mesh.polygroups.resize(mesh.triangles.len(), 0);
    mesh.hidden_faces.resize(mesh.triangles.len(), false);
    mesh.push_polygon(&[base, base + 1, base + 2, base + 3], 0, false);
    assert!(
        bridge(&mesh, &first, &second, JoinOptions::default(), &|| false)
            .unwrap_err()
            .contains("intersects")
    );
}

#[test]
fn open_ends_accept_offset_nonplanar_rims_and_keep_unselected_holes() {
    let a = tube(9, -2., -0.5, 0.7);
    let mut b = tube(14, 0.5, 2., 0.9);
    for p in &mut b.vertices {
        let wave = 0.035 * (p.0 * 4.).sin();
        *p = p.rotated(Vec3(0., 1., 0.), 0.15) + Vec3(0.15, 0.07, wave);
    }
    let mesh = merge(&a, &b).unwrap();
    let loops = boundaries(&mesh).unwrap();
    let joined = bridge(
        &mesh,
        &loops[1].vertices,
        &loops[2].vertices,
        JoinOptions::default(),
        &|| false,
    )
    .unwrap();
    let left = boundaries(&joined).unwrap();
    assert_eq!(left.len(), 2);
    assert_eq!(left[0].vertices, loops[0].vertices);
    assert_eq!(left[1].vertices, loops[3].vertices);
    assert_eq!(&joined.triangles[..mesh.triangles.len()], mesh.triangles);
    assert_eq!(&joined.vertices[..mesh.vertices.len()], mesh.vertices);
    assert!(
        bridge(
            &mesh,
            &loops[1].vertices,
            &loops[1].vertices,
            JoinOptions::default(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn fusion_rejects_branched_vertex_fans_and_invalid_settings() {
    let a = tube(6, -2., -0.5, 0.7);
    let b = tube(8, 0.5, 2., 0.9);
    let mesh = merge(&a, &b).unwrap();
    let loops = boundaries(&mesh).unwrap();
    for options in [
        JoinOptions {
            twist: i32::MIN,
            ..Default::default()
        },
        JoinOptions {
            segments: 0,
            ..Default::default()
        },
        JoinOptions {
            curvature: f32::NAN,
            ..Default::default()
        },
    ] {
        assert!(
            bridge(
                &mesh,
                &loops[1].vertices,
                &loops[2].vertices,
                options,
                &|| false
            )
            .is_err()
        );
    }
    let mut cube = super::super::Primitive::for_kind(super::super::PrimitiveKind::Cube)
        .mesh()
        .unwrap();
    let offset = cube.vertices.len();
    let extra = cube.clone();
    cube = merge(&cube, &extra).unwrap();
    for p in &mut cube.vertices[offset..] {
        *p = *p + Vec3(3., 3., 3.);
    }
    // Identify only a corner vertex between otherwise disconnected closed shells.
    for t in &mut cube.triangles {
        for v in t {
            if *v == offset {
                *v = 0;
            }
        }
    }
    for q in &mut cube.quads {
        for v in q {
            if *v == offset {
                *v = 0;
            }
        }
    }
    assert!(validate_surface(&cube).unwrap_err().contains("face fan"));
}

#[test]
fn fusion_checks_budgets_and_cancels_during_connector_work() {
    let mesh = merge(&tube(8, -2., -0.5, 0.7), &tube(12, 0.5, 2., 0.9)).unwrap();
    let loops = boundaries(&mesh).unwrap();
    super::super::capacity::with_test_capacity(
        super::super::capacity::MeshCapacity {
            vertices: mesh.vertices.len(),
            triangles: mesh.triangles.len(),
        },
        || {
            assert!(
                bridge(
                    &mesh,
                    &loops[1].vertices,
                    &loops[2].vertices,
                    JoinOptions::default(),
                    &|| false
                )
                .unwrap_err()
                .contains("budget")
            );
        },
    );
    let calls = std::cell::Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() > 5
    };
    assert!(
        bridge(
            &mesh,
            &loops[1].vertices,
            &loops[2].vertices,
            JoinOptions::default(),
            &cancel
        )
        .unwrap_err()
        .contains("cancelled")
    );
    assert!(calls.get() > 5);
    let rotated = bridge(
        &mesh,
        &loops[1].vertices,
        &loops[2].vertices,
        JoinOptions {
            twist: 1,
            curvature: 0.,
            ..Default::default()
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(&rotated.vertices[..mesh.vertices.len()], mesh.vertices);
    assert_eq!(&rotated.triangles[..mesh.triangles.len()], mesh.triangles);
}

fn spheres(segments: [u32; 2], separation: f32) -> [Mesh; 2] {
    std::array::from_fn(|side| {
        let mut p = super::super::Primitive::for_kind(super::super::PrimitiveKind::UvSphere);
        p.segments = segments[side];
        p.rings = segments[side] / 2;
        let mut mesh = p.mesh().unwrap();
        let shift = if side == 0 {
            Vec3(-separation * 0.5, 0., 0.)
        } else {
            Vec3(separation * 0.5, 0.03, 0.02)
        };
        for p in &mut mesh.vertices {
            *p = *p + shift;
        }
        mesh
    })
}

#[test]
fn fusion_preserves_retained_face_attributes_through_compaction() {
    let [mut a, mut b] = spheres([48, 48], 0.6);
    for (side, mesh) in [&mut a, &mut b].into_iter().enumerate() {
        mesh.uv = UvMap {
            topology: mesh.triangles.clone(),
            corners: vec![[[0.1, 0.2], [0.3, 0.4], [0.5, 0.6]]; mesh.triangles.len()],
            masks: vec![[0.1, 0.2, 0.3]; mesh.triangles.len()],
            ..Default::default()
        };
        mesh.create_vertex_group("Shared name".into(), 0.5, true)
            .unwrap();
        mesh.vertex_groups[0]
            .weights
            .fill(if side == 0 { 0.3 } else { 0.7 });
        mesh.materials = vec![super::super::Material::default(); mesh.vertices.len()];
        mesh.hidden_faces.resize(mesh.triangles.len(), false);
        for (i, p) in mesh.vertices.iter().enumerate() {
            mesh.materials[i].base_color = [(i % 7) as f32 / 7., 0.2, 0.8];
            if (side == 0 && p.0 < -1.) || (side == 1 && p.0 > 1.) {
                mesh.mask[i] = 0.6;
            }
        }
        for (i, t) in mesh.triangles.iter().enumerate() {
            if t.iter().all(|&v| mesh.mask[v] > 0.) {
                mesh.hidden_faces[i] = true;
                mesh.creases.push(edge(t[0], t[1]));
                mesh.uv.seams.push(edge(t[0], t[1]));
            }
        }
        mesh.creases.sort_unstable();
        mesh.creases.dedup();
        mesh.uv.seams.sort_unstable();
        mesh.uv.seams.dedup();
    }
    let source = merge(&a, &b).unwrap();
    let (joined, report) = fuse(&a, &b, 1, JoinOptions::default(), &|| false).unwrap();
    // Source indices are an explicit identity/offset correspondence until compact.
    let source_faces: BTreeMap<_, _> = source
        .triangles
        .iter()
        .enumerate()
        .map(|(i, &t)| (t, i))
        .collect();
    for (i, t) in joined.triangles[..report.retained_triangles]
        .iter()
        .enumerate()
    {
        let old = source_faces[t];
        assert_eq!(joined.polygroups[i], source.polygroups[old]);
        assert_eq!(joined.hidden_faces[i], source.hidden_faces[old]);
        assert_eq!(joined.uv.corners[i], source.uv.corners[old]);
        assert_eq!(joined.uv.masks[i], source.uv.masks[old]);
    }
    let used: BTreeSet<_> = joined.triangles.iter().flatten().copied().collect();
    let mapping: BTreeMap<_, _> = used
        .iter()
        .enumerate()
        .map(|(new, &old)| (old, new))
        .collect();
    let compacted = compact(joined.clone());
    compacted.validate().unwrap();
    assert!(compacted.uv.current(&compacted));
    for (&old, &new) in &mapping {
        assert_eq!(compacted.vertices[new], joined.vertices[old]);
        assert_eq!(compacted.mask[new], joined.mask[old]);
        assert_eq!(compacted.materials[new], joined.materials[old]);
        for (a, b) in compacted.vertex_groups.iter().zip(&joined.vertex_groups) {
            assert_eq!(a.weights[new], b.weights[old]);
        }
    }
    for (old, new) in joined.quads.iter().zip(&compacted.quads) {
        assert_eq!(old.map(|v| mapping[&v]), *new);
    }
    assert_eq!(compacted.uv.corners, joined.uv.corners);
    assert_eq!(
        compacted.creases,
        joined
            .creases
            .iter()
            .map(|e| e.map(|v| mapping[&v]))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        compacted.uv.seams,
        joined
            .uv
            .seams
            .iter()
            .map(|e| e.map(|v| mapping[&v]))
            .collect::<Vec<_>>()
    );
    a.mask.fill(0.1);
    assert!(
        fuse(&a, &b, 1, JoinOptions::default(), &|| false)
            .unwrap_err()
            .contains("masked")
    );
}

#[test]
fn fusion_handles_multiple_regions_mixed_density_and_shallow_overlap() {
    for (counts, separation, multiple) in [
        ([48, 48], 0.6, true),
        ([48, 36], 0.6, false),
        ([48, 48], 1.7, false),
    ] {
        let [mut a, mut b] = spheres(counts, separation);
        // One source is triangle-only; the other retains its mixed sphere poles/quads.
        a.quads.clear();
        if multiple {
            let mut extra_a = a.clone();
            let mut extra_b = b.clone();
            for mesh in [&mut extra_a, &mut extra_b] {
                for p in &mut mesh.vertices {
                    p.2 += 5.;
                }
            }
            a = merge(&a, &extra_a).unwrap();
            b = merge(&b, &extra_b).unwrap();
        }
        let (joined, r) = fuse(
            &a,
            &b,
            1,
            JoinOptions {
                segments: 2,
                curvature: 0.,
                twist: 0,
            },
            &|| false,
        )
        .unwrap_or_else(|e| {
            panic!("{counts:?}, separation {separation}, multiple {multiple}: {e}")
        });
        joined.validate_closed().unwrap();
        assert_eq!(r.joins, if multiple { 2 } else { 1 });
    }
    let [a, mut b] = spheres([24, 24], 0.6);
    b.quads.clear();
    for t in &mut b.triangles {
        t.swap(1, 2);
    }
    assert!(
        fuse(&a, &b, 1, JoinOptions::default(), &|| false)
            .unwrap_err()
            .contains("outward")
    );
}

#[test]
#[ignore = "manual dense-source timing and RSS measurement"]
fn fusion_dense_source_profile() {
    for count in [48, 96, 192] {
        let [a, b] = spheres([count, count], 1.7);
        let input = a.triangles.len() + b.triangles.len();
        let start = std::time::Instant::now();
        let (joined, r) = fuse(
            &a,
            &b,
            2,
            JoinOptions {
                segments: 2,
                curvature: 0.,
                twist: 0,
            },
            &|| false,
        )
        .unwrap();
        let seconds = start.elapsed().as_secs_f64();
        let patch = joined.triangles.len() - r.retained_triangles;
        let rss = std::fs::read_to_string("/proc/self/status")
            .unwrap_or_default()
            .lines()
            .find(|l| l.starts_with("VmHWM:"))
            .unwrap_or("peak RSS unavailable")
            .to_owned();
        eprintln!(
            "fusion input_triangles={input}, retained_triangles={}, patch_triangles={patch}, elapsed_seconds={seconds:.3}, {rss}",
            r.retained_triangles
        );
        joined.validate_closed().unwrap();
    }
}

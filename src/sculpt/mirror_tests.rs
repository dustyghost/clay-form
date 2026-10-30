use super::*;

fn options(axis: Axis, source_positive: bool, offset: f32) -> MirrorOptions {
    MirrorOptions {
        axis,
        source_positive,
        offset,
    }
}
fn cube() -> Mesh {
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [2; 3];
    primitive.mesh().unwrap()
}
fn symmetric_surface(mesh: &Mesh, axis: Axis, offset: f32, closed: bool) {
    mesh.validate().unwrap();
    if closed {
        mesh.validate_closed().unwrap();
    }
    for (i, &p) in mesh.vertices.iter().enumerate() {
        let reflected = p - axis.vector() * (2. * (p.dot(axis.vector()) - offset));
        let j = mesh
            .vertices
            .iter()
            .position(|&q| (q - reflected).len() < 1e-5)
            .unwrap();
        assert_eq!(mesh.mask[i], mesh.mask[j]);
        for group in &mesh.vertex_groups {
            assert_eq!(group.weights[i], group.weights[j]);
        }
        if (p.dot(axis.vector()) - offset).abs() < 1e-6 {
            assert_eq!(
                mesh.vertices.iter().filter(|&&q| q == p).count(),
                1,
                "duplicate seam vertex"
            );
        }
    }
    for &[a, b, c] in &mesh.triangles {
        assert!(
            (mesh.vertices[b] - mesh.vertices[a])
                .cross(mesh.vertices[c] - mesh.vertices[a])
                .len()
                > 1e-9
        );
    }
    if closed {
        let volume: f32 = mesh
            .triangles
            .iter()
            .map(|&[a, b, c]| mesh.vertices[a].dot(mesh.vertices[b].cross(mesh.vertices[c])) / 6.)
            .sum();
        assert!(volume > 0., "reflected winding must point outwards");
    }
}

#[test]
fn mirror_all_axes_directions_offsets_preserve_source_and_interpolate_seam_data() {
    for axis in Axis::ALL {
        for positive in [true, false] {
            for offset in [0., 0.13] {
                let mut mesh = cube();
                // Deliberately different source and destination shapes.
                for p in &mut mesh.vertices {
                    let d = p.dot(axis.vector());
                    *p = *p + axis.vector() * if d > 0. { d * 0.3 } else { d * 0.1 };
                }
                mesh.mask = mesh
                    .vertices
                    .iter()
                    .map(|p| (p.dot(axis.vector()) + 2.) / 4.)
                    .collect();
                mesh.create_vertex_group("Soft selection".into(), 0., false)
                    .unwrap();
                let out = mesh
                    .mirror_half(options(axis, positive, offset), &|| false)
                    .unwrap();
                symmetric_surface(&out, axis, offset, true);
                assert!(!out.quads.is_empty());
                assert_eq!(out.vertex_groups.len(), mesh.vertex_groups.len());
                assert_eq!(out.vertex_groups[0].name, "Soft selection");
                for (i, p) in out.vertices.iter().enumerate() {
                    let d = p.dot(axis.vector()) - offset;
                    let source_coordinate = offset + if positive { d.abs() } else { -d.abs() };
                    assert!((out.mask[i] - (source_coordinate + 2.) / 4.).abs() < 1e-6);
                    assert!((out.vertex_groups[0].weights[i] - out.mask[i]).abs() < 1e-6);
                }
                for p in &mesh.vertices {
                    if (p.dot(axis.vector()) > offset) == positive {
                        assert!(out.vertices.contains(p), "source geometry changed");
                    }
                }
                let (a, b) = out.polygroups.split_at(out.triangles.len() / 2);
                assert_eq!(a, b, "reflected face colors changed");
            }
        }
    }
}

#[test]
fn mirror_capped_half_removes_internal_wall_and_open_surface_stays_open() {
    let mut half = cube();
    for p in &mut half.vertices {
        p.0 = (p.0 + 1.) / 2.;
    }
    let out = half
        .mirror_half(options(Axis::X, true, 0.), &|| false)
        .unwrap();
    symmetric_surface(&out, Axis::X, 0., true);
    assert!(
        out.triangles
            .iter()
            .all(|t| t.iter().any(|&i| out.vertices[i].0 != 0.))
    );

    let mut open = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
    open.mask.fill(0.75);
    let out = open
        .mirror_half(options(Axis::X, true, 0.13), &|| false)
        .unwrap();
    symmetric_surface(&out, Axis::X, 0.13, false);
    assert!(out.validate_closed().is_err());
}

#[test]
fn mirror_failure_and_cancellation_leave_geometry_unchanged() {
    for scenario in 0..9 {
        let mut geometry = Geometry::new(cube());
        let mut opts = options(Axis::X, true, 0.);
        match scenario {
            0 => opts.offset = 10., // no source half
            1 => opts.offset = 1.,  // tangent / only coplanar faces
            2 => opts.offset = f32::NAN,
            3 => geometry.settings.primitive = Some(Primitive::for_kind(PrimitiveKind::Cube)),
            4 => geometry.scene.active_metadata.visible = false,
            5 => geometry.mesh.hidden_faces = vec![true; geometry.mesh.triangles.len()],
            6 => geometry.scene.active_metadata.temporary_thickness = 0.1,
            7 => geometry.mesh.mask.clear(),
            _ => (),
        }
        let before = geometry.clone();
        assert!(geometry.mirror(opts, &|| scenario == 8).is_err());
        assert_eq!(geometry, before);
    }
    // Cancellation after work has begun must also be transactional.
    let mut geometry = Geometry::new(Mesh::starting_sphere());
    let before = geometry.clone();
    let calls = std::cell::Cell::new(0);
    assert!(
        geometry
            .mirror(options(Axis::Z, false, 0.13), &|| {
                calls.set(calls.get() + 1);
                calls.get() > 8
            })
            .is_err()
    );
    assert_eq!(geometry, before);
}

#[test]
fn mirror_keeps_other_models_and_metadata_and_resets_active_mesh_levels() {
    let mut geometry = Geometry::new(cube());
    geometry.subdivide().unwrap();
    geometry.settings.resolution = 64;
    geometry.name = "Keep this name".into();
    geometry.submodels.push(Submodel {
        name: "Other".into(),
        mesh: cube(),
        mesh_settings: Default::default(),
        metadata: Default::default(),
    });
    let before = geometry.clone();
    geometry
        .mirror(options(Axis::Y, false, 0.13), &|| false)
        .unwrap();
    assert_eq!(geometry.submodels, before.submodels);
    assert_eq!(geometry.scene, before.scene);
    assert_eq!(geometry.name, before.name);
    assert_eq!(geometry.settings.levels(), 1);
    assert_eq!(geometry.settings.resolution, 64);
    geometry.settings.validate(&geometry.mesh).unwrap();
}

#[test]
fn mirror_rejects_results_over_vertex_and_face_limits() {
    for vertex_limit in [true, false] {
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        mesh.quads.clear();
        mesh.vertex_groups.clear();
        mesh.polygroups.clear();
        if vertex_limit {
            mesh.vertices.clear();
            mesh.triangles.clear();
            for i in 0..max_vertices() / 3 {
                let base = i * 3;
                mesh.vertices
                    .extend([Vec3(1., 0., 0.), Vec3(1., 1., 0.), Vec3(1., 0., 1.)]);
                mesh.triangles.push([base, base + 1, base + 2]);
            }
            mesh.mask = vec![0.; mesh.vertices.len()];
        } else {
            for p in &mut mesh.vertices {
                p.0 += 3.;
            }
            mesh.triangles = vec![mesh.triangles[0]; max_triangles() / 2 + 1];
        }
        mesh.validate().unwrap();
        assert!(
            mesh.mirror_half(options(Axis::X, true, 0.), &|| false)
                .unwrap_err()
                .contains("size limit")
        );
    }
}

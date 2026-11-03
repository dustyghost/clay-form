use super::*;

#[test]
fn layered_array_links_survive_edits_cancel_history_and_file_roundtrip() {
    let mut d = document();
    let mut row = array(&mut d, 3);
    let Layout::Array(a) = &mut row.layout else {
        unreachable!()
    };
    a.spacing = Vec3(1.5, 0., 0.);
    let mut outer = ArrayLayer::repeat();
    outer.spacing = Vec3(0., 1., 0.);
    a.layers.push(outer);
    let mut repeat = Array::new(a.piece);
    repeat.source_array = Some(row.id);
    repeat.count = 2;
    repeat.center = Vec3(0., 0., 5.);
    repeat.spacing = Vec3(0., 0., 3.);
    repeat.step_scale = Vec3(1.1, 0.9, 1.2);
    let linked = Set {
        id: 2,
        name: "Pair of racks".into(),
        visible: true,
        layout: Layout::Array(repeat),
        adjustments: Vec::new(),
    };
    row.visible = false;
    d.preview_instance_set(row.clone()).unwrap();
    d.apply_instance_preview().unwrap();
    d.preview_instance_set(linked.clone()).unwrap();
    d.apply_instance_preview().unwrap();
    let original = d.project();
    let placements = d.instance_placements().unwrap();
    assert_eq!(placements.len(), 12);
    assert!(placements.meshes.is_empty());
    assert!(
        placements
            .iter()
            .all(|(_, _, p)| std::ptr::eq(d.instance_source(p.piece.source).unwrap(), d.mesh()))
    );
    assert!(std::sync::Arc::ptr_eq(
        &placements,
        &d.instance_placements().unwrap()
    ));
    assert!(d.remove_instance_set(1).is_err());
    assert!(!d.instance_preview_active());
    assert!(
        d.geometry_snapshot()
            .bake_instances(BakeRequest::new(BakeScope::Set(1)), &|| false)
            .is_err()
    );
    let Layout::Array(a) = &mut row.layout else {
        unreachable!()
    };
    a.count = 4;
    d.preview_instance_set(row.clone()).unwrap();
    assert_eq!(d.instance_placements().unwrap().len(), 16);
    assert!(d.project() == original);
    d.cancel_instance_preview();
    assert_eq!(d.instance_placements().unwrap(), placements);
    d.preview_instance_set(row).unwrap();
    d.apply_instance_preview().unwrap();
    assert_eq!(d.instance_placements().unwrap().len(), 16);
    d.undo();
    assert!(d.project() == original);
    d.redo();
    let saved = d.project();
    saved.validate().unwrap();
    let mut reopened = document();
    reopened
        .replace_project(
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap(),
            "racks.clay",
        )
        .unwrap();
    assert_eq!(
        reopened.instance_placements().unwrap(),
        d.instance_placements().unwrap()
    );
    let before = d.instance_placements().unwrap()[15]
        .2
        .point(d.mesh().vertices[0]);
    let mut g = d.geometry_snapshot();
    g.mesh.vertices[0].0 += 0.1;
    d.replace_geometry(g).unwrap();
    let leaf = d.instance_placements().unwrap()[15].2;
    assert!((leaf.point(d.mesh().vertices[0]) - before).len() > 0.05);
    let expected = leaf.mesh(d.mesh()).unwrap();
    let mut baked = d.geometry_snapshot();
    baked
        .bake_instances(BakeRequest::new(BakeScope::Set(2)), &|| false)
        .unwrap();
    assert!(
        baked
            .submodels
            .iter()
            .any(|m| m.mesh.vertices == expected.vertices)
            || baked.mesh.vertices == expected.vertices
    );
    assert_eq!(baked.scene.instances.sets.len(), 1);
    d.detach_instance(2, 15).unwrap();
    assert_eq!(d.instance_placements().unwrap().len(), 15);
    assert_eq!(d.model(1).unwrap().mesh.vertices, expected.vertices);
    d.undo();
    assert_eq!(d.instance_placements().unwrap().len(), 16);
}

#[test]
fn bake_all_resolves_linked_sets_before_writing_any_destination() {
    let mut d = document();
    let source = array(&mut d, 2);
    let mut child = source.clone();
    child.id = 2;
    if let Layout::Array(a) = &mut child.layout {
        a.source_array = Some(source.id);
    }
    d.preview_instance_set(source).unwrap();
    d.apply_instance_preview().unwrap();
    d.preview_instance_set(child).unwrap();
    d.apply_instance_preview().unwrap();
    let before = d.geometry_snapshot();
    let copies = d.instance_placements().unwrap().len();
    assert_eq!(copies, 6);
    for (destination, models, triangles) in [
        (BakeDestination::PerCopy, 1 + copies, 1),
        (BakeDestination::New, 2, copies),
        (BakeDestination::Current, 1, 1 + copies),
    ] {
        let mut g = before.clone();
        g.bake_instances(
            BakeRequest {
                scope: BakeScope::All,
                destination,
                fuse: false,
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(g.submodels.len() + 1, models);
        assert_eq!(
            g.mesh.triangles.len(),
            before.mesh.triangles.len() * triangles
        );
        assert!(g.scene.instances.sets.is_empty());
        d.replace_geometry_named(g, "Bake instances").unwrap();
        d.project().validate().unwrap();
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
    }
}

#[test]
fn bake_into_current_at_model_limit_preserves_uvs_and_can_fuse_with_target() {
    let mut d = document();
    let set = array(&mut d, 1);
    d.preview_instance_set(set).unwrap();
    d.apply_instance_preview().unwrap();
    let mut base = d.geometry_snapshot();
    base.settings.resolution = 16;
    base.mesh.uv.corners = vec![[[0., 0.], [1., 0.], [0., 1.]]; base.mesh.triangles.len()];
    base.mesh.uv.topology.clone_from(&base.mesh.triangles);
    base.mesh.uv.seams = vec![[0, 1]];
    let target_id = base.scene.active_metadata.model_id;
    for index in 1..64 {
        base.submodels.push(crate::sculpt::Submodel {
            name: format!("Model {index}"),
            metadata: Default::default(),
            mesh: Default::default(),
            mesh_settings: Default::default(),
        });
    }
    let mut g = base.clone();
    let request = BakeRequest {
        scope: BakeScope::Set(1),
        destination: BakeDestination::Current,
        fuse: false,
    };
    assert!(
        g.bake_instances(
            BakeRequest {
                destination: BakeDestination::New,
                ..request
            },
            &|| false
        )
        .is_err()
    );
    assert_eq!(g, base);
    g.bake_instances(request, &|| false).unwrap();
    assert_eq!(g.submodels.len(), 63);
    assert_eq!(g.scene.active_metadata.model_id, target_id);
    assert!(g.mesh.uv.current(&g.mesh));
    assert_eq!(g.mesh.uv.seams.len(), 2);
    assert_eq!(g.mesh.uv.corners.len(), base.mesh.uv.corners.len() * 2);
    let mut fused = base.clone();
    fused
        .bake_instances(
            BakeRequest {
                fuse: true,
                ..request
            },
            &|| false,
        )
        .unwrap();
    assert_eq!(fused.submodels.len(), 63);
    assert!(fused.mesh.uv.is_empty());
    fused.mesh.validate_closed().unwrap();
    let mut open = base;
    open.mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
        .mesh()
        .unwrap();
    let unchanged = open.clone();
    assert!(
        open.bake_instances(
            BakeRequest {
                fuse: true,
                ..request
            },
            &|| false
        )
        .is_err()
    );
    assert_eq!(open, unchanged);
}

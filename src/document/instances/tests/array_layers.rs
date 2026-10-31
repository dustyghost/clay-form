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
            .bake_instances(1, false, &|| false)
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
    baked.bake_instances(2, false, &|| false).unwrap();
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

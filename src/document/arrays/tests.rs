use super::*;
use crate::sculpt::{Primitive, PrimitiveKind};
fn cube() -> Document {
    let mut d = Document::default();
    let mut p = Primitive::for_kind(PrimitiveKind::Cube);
    p.divisions = [1; 3];
    d.replace_geometry(Geometry::new(p.mesh().unwrap()))
        .unwrap();
    d
}
fn change(d: &mut Document, edit: impl FnOnce(&mut Array)) {
    let mut s = d.attached_array(d.active_model_index()).unwrap().clone();
    let Layout::Array(a) = &mut s.layout else {
        unreachable!()
    };
    edit(a);
    d.preview_instance_set(s).unwrap();
    d.apply_instance_preview().unwrap();
}
#[test]
fn attached_array_counts_source_once_and_survives_disable_history_and_reopen() {
    let mut d = cube();
    let before = d.project();
    let history = d.history_counts().0;
    let id = d.create_submodel_array().unwrap();
    assert_eq!(d.model_count(), 1);
    assert!(!d.source_is_displayed(0));
    assert_eq!(d.instance_placements().unwrap().len(), 2);
    assert_eq!(d.history_counts().0, history + 1);
    assert_eq!(d.create_submodel_array().unwrap(), id);
    assert_eq!(d.history_counts().0, history + 1);
    change(&mut d, |a| a.count = 1);
    let copies = d.instance_placements().unwrap();
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].2.mesh(d.mesh()).unwrap(), *d.mesh());
    let mut s = d.attached_array(0).unwrap().clone();
    s.visible = false;
    d.preview_instance_set(s).unwrap();
    d.apply_instance_preview().unwrap();
    assert!(d.source_is_displayed(0));
    assert!(d.instance_placements().unwrap().is_empty());
    d.undo();
    assert!(!d.source_is_displayed(0));
    assert_eq!(d.instance_placements().unwrap().len(), 1);
    let saved = d.project();
    saved.validate().unwrap();
    assert_eq!(saved.version, crate::sculpt::Project::VERSION);
    let decoded = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
    let mut reopened = cube();
    reopened.replace_project(decoded, "array.clay").unwrap();
    assert!(reopened.project() == saved);
    d.undo();
    d.undo();
    assert!(d.project() == before);
}
#[test]
fn attached_array_relative_spacing_follows_source_but_span_stays_fixed() {
    let mut d = cube();
    d.create_submodel_array().unwrap();
    let copies = d.instance_placements().unwrap();
    assert_eq!(
        copies[1].2.transform.origin - copies[0].2.transform.origin,
        Vec3(2., 0., 0.)
    );
    d.set_array_source_mode(true).unwrap();
    d.apply_transform(
        crate::sculpt::Transform {
            scale: Vec3(2., 1., 1.),
            ..Default::default()
        },
        false,
    )
    .unwrap();
    d.end_stroke();
    d.set_array_source_mode(false).unwrap();
    let copies = d.instance_placements().unwrap();
    assert_eq!(
        copies[1].2.transform.origin - copies[0].2.transform.origin,
        Vec3(4., 0., 0.)
    );
    change(&mut d, |a| {
        a.count = 6;
        a.spacing_mode = ArraySpacing::Span;
        a.spacing = Vec3(10., 0., 0.);
    });
    let copies = d.instance_placements().unwrap();
    assert_eq!(
        copies[5].2.transform.origin - copies[0].2.transform.origin,
        Vec3(10., 0., 0.)
    );
    change(&mut d, |a| a.count = 11);
    let copies = d.instance_placements().unwrap();
    assert_eq!(
        copies[10].2.transform.origin - copies[0].2.transform.origin,
        Vec3(10., 0., 0.)
    );
}
#[test]
fn attached_array_bake_replaces_owner_without_extra_source_and_restores_levels() {
    let mut d = cube();
    let mut g = d.geometry_snapshot();
    g.subdivide().unwrap();
    d.replace_geometry(g).unwrap();
    d.create_submodel_array().unwrap();
    change(&mut d, |a| a.count = 3);
    let before = d.geometry_snapshot();
    let id = d.attached_array(0).unwrap().id;
    let vertices = d.mesh().vertices.len();
    let mut g = before.clone();
    g.bake_instances(
        BakeRequest {
            scope: BakeScope::Set(id),
            destination: BakeDestination::Current,
            fuse: false,
        },
        &|| false,
    )
    .unwrap();
    d.replace_geometry(g).unwrap();
    assert!(d.attached_array(0).is_none());
    assert_eq!(d.mesh().vertices.len(), vertices * 3);
    assert_eq!(d.mesh_settings().levels(), 1);
    d.undo();
    assert_eq!(d.geometry_snapshot(), before);
}
#[test]
fn attached_array_whole_transform_keeps_source_and_composes_with_layers() {
    let mut d = cube();
    d.create_submodel_array().unwrap();
    change(&mut d, |a| {
        let mut l = ArrayLayer::repeat();
        l.spacing_mode = ArraySpacing::Relative;
        l.spacing = Vec3(0., 1., 0.);
        a.layers.push(l);
    });
    let source = d.mesh().clone();
    let before = d.instance_placements().unwrap();
    assert_eq!(before.len(), 4);
    d.apply_transform(
        crate::sculpt::Transform {
            translation: Vec3(1., 3., 2.),
            ..Default::default()
        },
        false,
    )
    .unwrap();
    d.apply_instance_preview().unwrap();
    assert_eq!(d.mesh(), &source);
    for (a, b) in d.instance_placements().unwrap().iter().zip(before.iter()) {
        assert!((a.2.transform.origin - b.2.transform.origin - Vec3(1., 3., 2.)).len() < 1e-5);
    }
    d.project().validate().unwrap();
}
#[test]
fn attached_array_legacy_scene_keeps_standalone_source_and_invalid_edits_are_atomic() {
    let mut d = cube();
    let piece = d.instance_piece(0).unwrap();
    let mut a = Array::new(piece);
    a.count = 3;
    let set = Set {
        id: 1,
        name: "Legacy".into(),
        visible: true,
        layout: Layout::Array(a),
        adjustments: Vec::new(),
    };
    d.preview_instance_set(set).unwrap();
    d.apply_instance_preview().unwrap();
    let mut saved = d.project();
    saved.version = 4;
    let bytes = serde_json::to_vec(&saved).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("\"owner\""));
    let mut d = cube();
    d.replace_project(serde_json::from_slice(&bytes).unwrap(), "old.clay")
        .unwrap();
    assert!(d.source_is_displayed(0));
    assert!(d.attached_array(0).is_none());
    d.create_submodel_array().unwrap();
    let before = d.instance_placements().unwrap();
    let mut bad = d.attached_array(0).unwrap().clone();
    if let Layout::Array(a) = &mut bad.layout {
        a.count = 257;
    }
    assert!(d.preview_instance_set(bad).is_err());
    assert_eq!(d.instance_placements().unwrap(), before);
    d.cancel_instance_preview();
    d.project().validate().unwrap();
}

#[test]
fn attached_array_duplicate_reorder_delete_and_hidden_bake_preserve_ownership() {
    let mut d = cube();
    let original = d.create_submodel_array().unwrap();
    d.duplicate_model(false).unwrap();
    let duplicate = d.attached_array(d.active_model_index()).unwrap().id;
    assert_ne!(original, duplicate);
    assert_eq!(d.instance_placements().unwrap().len(), 4);
    d.project().validate().unwrap();
    let active = d.active_model_index();
    d.reorder_model(active, 1 - active).unwrap();
    assert_eq!(
        d.attached_array(d.active_model_index()).unwrap().id,
        duplicate
    );
    d.project().validate().unwrap();
    let before = d.geometry_snapshot();
    assert!(d.delete_submodel());
    assert_eq!(d.model_count(), 1);
    assert_eq!(d.instances().sets.len(), 1);
    assert_eq!(d.attached_array(0).unwrap().id, original);
    d.project().validate().unwrap();
    d.undo();
    assert_eq!(d.geometry_snapshot(), before);
    let mut s = d.attached_array(d.active_model_index()).unwrap().clone();
    s.visible = false;
    d.preview_instance_set(s).unwrap();
    d.apply_instance_preview().unwrap();
    let mut g = d.geometry_snapshot();
    g.bake_instances(
        BakeRequest {
            scope: BakeScope::All,
            destination: BakeDestination::New,
            fuse: false,
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(g.scene.instances.sets.len(), 1);
    assert_eq!(g.scene.instances.sets[0].id, duplicate);
}

use super::*;
use crate::sculpt::{
    BooleanRole::*, Geometry, Primitive, PrimitiveKind, Transform, Vec3, boolean::evaluate,
};

fn cube(center: f32, size: f32) -> Mesh {
    Primitive {
        center: Vec3(center, 0., 0.),
        size: Vec3(size, size, size),
        divisions: [2; 3],
        ..Default::default()
    }
    .mesh()
    .unwrap()
}
fn stack() -> Document {
    // Rows top to bottom: regular plane, cutter, filler, base.
    let mut g = Geometry::new(Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap());
    for (name, role, mesh) in [
        ("Cutter", Subtract, cube(1., 1.4)),
        ("Filler", Add, cube(1., 1.)),
        ("Base", Add, cube(0., 2.)),
    ] {
        g.submodels.push(crate::sculpt::Submodel {
            name: name.into(),
            metadata: crate::sculpt::ModelMetadata {
                boolean_role: role,
                ..Default::default()
            },
            mesh,
            mesh_settings: Default::default(),
        });
    }
    let mut d = Document::default();
    d.replace_geometry(g).unwrap();
    d.set_boolean_resolution(16).unwrap();
    d
}
fn result(d: &Document) -> Mesh {
    evaluate(
        &d.boolean_operands().unwrap(),
        d.boolean_resolution(),
        &|| false,
    )
    .unwrap()
}
fn volume(mesh: &Mesh) -> f32 {
    mesh.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.vertices[i]);
            a.dot(b.cross(c)) / 6.
        })
        .sum::<f32>()
        .abs()
}
#[test]
fn boolean_stack_order_move_visibility_bake_undo_and_roundtrip() {
    let mut d = stack();
    let first = volume(&result(&d));
    d.reorder_model(1, 2).unwrap();
    let second = volume(&result(&d));
    assert!(second > first + 0.6, "{second} vs {first}");
    d.select_submodel(2).unwrap();
    d.apply_transform(
        Transform {
            translation: Vec3(4., 0., 0.),
            ..Default::default()
        },
        false,
    )
    .unwrap();
    d.end_stroke();
    let after_move = result(&d);
    assert!(volume(&after_move) > second + 0.2);
    let mut meta = d.model(1).unwrap().metadata.clone();
    meta.visible = false;
    d.set_model_metadata(1, "Filler".into(), meta).unwrap();
    let preview = result(&d);
    assert!(volume(&preview) < volume(&after_move) - 0.2);
    let before = d.geometry_snapshot();
    let mut loaded = Document::default();
    loaded
        .replace_project(
            serde_json::from_slice(&serde_json::to_vec(&d.project()).unwrap()).unwrap(),
            "stack.clay",
        )
        .unwrap();
    assert_eq!(before, loaded.geometry_snapshot());
    assert_eq!(result(&loaded), preview);
    let count = d.history_counts().0;
    d.bake_boolean(preview.clone(), d.display_revision())
        .unwrap();
    assert_eq!(d.history_counts().0, count + 1);
    assert_eq!(d.model_count(), 5);
    assert_eq!(d.mesh(), &preview);
    assert!(d.active_visible());
    assert!(d.model(0).unwrap().metadata.visible);
    assert!((1..4).all(|i| !d.model(i).unwrap().metadata.visible));
    d.mesh().validate_closed().unwrap();
    assert!(d.mesh().shading_seams.is_some());
    loaded
        .replace_project(
            serde_json::from_slice(&serde_json::to_vec(&d.project()).unwrap()).unwrap(),
            "baked.clay",
        )
        .unwrap();
    assert_eq!(loaded.mesh(), &preview);
    assert_eq!(
        loaded.mesh().surface_shading().vertices,
        preview.surface_shading().vertices
    );
    assert!(loaded.mesh().remesh(16).unwrap().shading_seams.is_none());
    d.undo();
    assert_eq!(d.geometry_snapshot(), before);
}
#[test]
fn boolean_stack_rejects_open_protected_empty_stale_cancelled_and_over_budget() {
    let mut d = stack();
    let before = d.geometry_snapshot();
    let rev = d.display_revision();
    let ops = d.boolean_operands().unwrap();
    assert!(evaluate(&ops, 16, &|| true).unwrap_err().contains("cancel"));
    let mut meta = d.model(0).unwrap().metadata.clone();
    meta.boolean_role = Add;
    d.set_model_metadata(0, "Plane".into(), meta).unwrap();
    assert!(
        evaluate(&d.boolean_operands().unwrap(), 16, &|| false)
            .unwrap_err()
            .contains("Plane")
    );
    assert!(d.bake_boolean(Mesh::default(), rev).is_err());
    d.undo();
    assert_eq!(d.geometry_snapshot(), before);
    let mut g = before;
    g.submodels[0].mesh.mask[0] = 1.;
    d.replace_geometry(g).unwrap();
    assert!(d.boolean_operands().err().unwrap().contains("mask"));
    d.undo();
    crate::sculpt::capacity::with_test_capacity(
        crate::sculpt::capacity::MeshCapacity {
            vertices: 10,
            triangles: 12,
        },
        || assert!(d.boolean_operands().err().unwrap().contains("capacity")),
    );
    let mut ops = d.boolean_operands().unwrap();
    ops.truncate(1);
    ops.push(crate::sculpt::boolean::Operand {
        name: "Identical cutter".into(),
        role: Subtract,
        mesh: ops[0].mesh.clone(),
    });
    assert!(evaluate(&ops, 16, &|| false).unwrap().triangles.is_empty());
}

#[test]
fn boolean_stack_group_scope_and_legacy_defaults() {
    let mut d = stack();
    d.add_group("Separate".into()).unwrap();
    let mut meta = d.model(1).unwrap().metadata.clone();
    meta.group = "Separate".into();
    d.set_model_metadata(1, "Cutter".into(), meta).unwrap();
    assert_eq!(d.boolean_operands().unwrap().len(), 2);
    let mesh = result(&d);
    d.bake_boolean(mesh, d.display_revision()).unwrap();
    assert!(d.model(1).unwrap().metadata.visible);
    let old = serde_json::to_value(Document::default().project()).unwrap();
    let old: Project = serde_json::from_value(old).unwrap();
    assert_eq!(old.scene.active_metadata.boolean_role, Regular);
    assert_eq!(old.scene.boolean_resolution, None);
}

#[test]
fn boolean_stack_intersect_keeps_only_shared_volume_at_its_position() {
    use crate::sculpt::boolean::Operand;
    let mut operands = vec![
        Operand {
            role: Add,
            name: "Base".into(),
            mesh: cube(0., 2.),
        },
        Operand {
            role: Intersect,
            name: "Overlap".into(),
            mesh: cube(1., 2.),
        },
    ];
    let intersection = evaluate(&operands, 24, &|| false).unwrap();
    assert!((volume(&intersection) - 4.).abs() < 0.4);
    assert!(
        intersection
            .vertices
            .iter()
            .all(|p| p.0 >= -0.09 && p.0 <= 1.09)
    );
    operands[1].mesh = cube(4., 2.);
    assert!(
        evaluate(&operands, 24, &|| false)
            .unwrap()
            .vertices
            .is_empty()
    );
    operands.push(Operand {
        role: Add,
        name: "Island".into(),
        mesh: cube(0., 1.),
    });
    assert!((volume(&evaluate(&operands, 24, &|| false).unwrap()) - 1.).abs() < 0.2);
}

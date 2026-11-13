use super::*;
use crate::{
    command::InstanceCommand as C,
    sculpt::{
        Geometry, Mesh, Primitive, PrimitiveKind, Vec3, VertexGroup,
        instances::*,
        paths::{PathMode, PathPoint},
    },
};

pub(super) fn source_mesh() -> Mesh {
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [12, 2, 2];
    let mut mesh = cube.mesh().unwrap();
    for p in &mut mesh.vertices {
        let radius = if p.0 < -0.5 {
            0.12 + 0.06 * (-0.5 - p.0) / 0.5
        } else if p.0 > 1. / 3. {
            0.12 - 0.08 * (p.0 - 1. / 3.) / (2. / 3.)
        } else {
            let t = (p.0 + 0.5) / (5. / 6.);
            0.12 + 0.06 * (std::f32::consts::PI * t).sin().powi(2)
        };
        p.1 *= radius;
        p.2 *= radius;
    }
    mesh.vertex_groups = ["Start", "Middle", "End"]
        .map(|name| VertexGroup {
            purpose: crate::sculpt::GroupPurpose::Selection,
            name: name.into(),
            visible: true,
            weights: vec![0.; mesh.vertices.len()],
        })
        .to_vec();
    mesh.polygroups = mesh
        .triangles
        .iter()
        .map(|t| {
            let x = t.iter().map(|&v| mesh.vertices[v].0).sum::<f32>() / 3.;
            if x < -0.5 {
                1
            } else if x < 1. / 3. {
                2
            } else {
                3
            }
        })
        .collect();
    for (t, &g) in mesh.triangles.iter().zip(&mesh.polygroups) {
        for &v in t {
            mesh.vertex_groups[g - 1].weights[v] = 1.;
        }
    }
    mesh
}
pub(super) fn prepare(app: &mut App) {
    app.document
        .replace_geometry(Geometry::new(source_mesh()))
        .unwrap();
    app.document
        .create_path(
            "Cable route".into(),
            PathMode::Free,
            [
                Vec3(-3., -0.7, 0.),
                Vec3(-1., 1., 0.),
                Vec3(1., 1., 0.),
                Vec3(3., -0.7, 0.),
            ]
            .map(|position| PathPoint {
                position,
                normal: axis(2),
            })
            .to_vec(),
            false,
            1.,
        )
        .unwrap();
}
#[test]
fn grouped_path_object_preview_history_source_edits_save_and_bake() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    prepare(&mut app);
    let source = app.document.mesh().clone();
    let before = app.document.project();
    let history = app.document.history_counts().0;
    app.dispatch(Command::Instances(C::NewPath), &ctx);
    assert!(!app.ui.instances.invalid, "{}", app.ui.status);
    assert_eq!(app.document.model_count(), 1);
    assert!(app.document.instances().sets[0].joined());
    assert!(
        app.document.project() == before,
        "private preview must not leak into saves"
    );
    let valid = app.document.instance_placements().unwrap();
    assert_eq!(valid.len(), 1);
    let mut bad = app.ui.instances.editor.clone().unwrap();
    let Layout::Path(a) = &mut bad.layout else {
        unreachable!()
    };
    a.regions.as_mut().unwrap().middle = "Missing".into();
    app.dispatch(Command::Instances(C::Preview(Box::new(bad))), &ctx);
    assert!(app.ui.instances.invalid);
    assert_eq!(
        app.document.instance_placements().unwrap().as_ref(),
        valid.as_ref()
    );
    let valid_set = app.document.instances().sets[0].clone();
    app.dispatch(Command::Instances(C::Preview(Box::new(valid_set))), &ctx);
    app.dispatch(Command::Instances(C::Apply), &ctx);
    assert!(!app.document.instance_preview_active());
    assert_eq!(app.document.history_counts().0, history + 1);
    assert_eq!(app.document.mesh(), &source);
    let applied = app.document.project();
    assert_eq!(applied.version, 6);
    app.dispatch(Command::Undo, &ctx);
    assert!(app.document.instances().sets.is_empty());
    app.dispatch(Command::Redo, &ctx);
    assert!(app.document.project() == applied);
    let mut reopened = Document::default();
    reopened
        .replace_project(
            serde_json::from_str(&serde_json::to_string(&applied).unwrap()).unwrap(),
            "regions.clay",
        )
        .unwrap();
    assert_eq!(
        reopened.instance_placements().unwrap().as_ref(),
        valid.as_ref()
    );
    let mut geometry = app.document.geometry_snapshot();
    for p in &mut geometry.mesh.vertices {
        p.1 *= 1.3;
        p.2 *= 1.3;
    }
    app.document
        .replace_geometry_named(geometry, "Edit source")
        .unwrap();
    let changed = app.document.instance_placements().unwrap();
    assert_ne!(valid.meshes, changed.meshes);
    app.document.undo();
    assert_eq!(
        app.document.instance_placements().unwrap().as_ref(),
        valid.as_ref()
    );
    let mut path = app.document.paths()[0].clone();
    path.points[1].position.1 += 0.4;
    app.document.update_path(path).unwrap();
    assert_ne!(
        app.document.instance_placements().unwrap().meshes,
        valid.meshes
    );
    app.document.undo();
    let snapshot = app.document.geometry_snapshot();
    let mut baked = snapshot.clone();
    baked
        .bake_instances(
            BakeRequest {
                scope: BakeScope::Set(1),
                destination: BakeDestination::New,
                fuse: false,
            },
            &|| false,
        )
        .unwrap();
    app.document
        .replace_geometry_named(baked, "Bake path object")
        .unwrap();
    assert_eq!(app.document.model_count(), 2);
    assert_eq!(app.document.mesh(), valid.meshes[&(1, 0)].as_ref());
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), snapshot);
}

#[test]
fn grouped_path_object_missing_groups_can_be_assigned_and_cancelled() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    prepare(&mut app);
    app.document.rename_vertex_group(0, "Foot".into()).unwrap();
    let before = app.document.project();
    app.ui
        .preferences
        .panels
        .iter_mut()
        .find(|p| p.tool == crate::preferences::Tool::Submodels)
        .unwrap()
        .dock = crate::preferences::Dock::Hidden;
    app.dispatch(Command::Instances(C::NewPath), &ctx);
    assert!(app.ui.instances.invalid);
    assert_ne!(
        app.ui
            .preferences
            .panels
            .iter()
            .find(|p| p.tool == crate::preferences::Tool::Submodels)
            .unwrap()
            .dock,
        crate::preferences::Dock::Hidden
    );
    assert!(app.document.instance_preview_active());
    let mut draft = app.ui.instances.editor.clone().unwrap();
    let Layout::Path(a) = &mut draft.layout else {
        unreachable!()
    };
    a.regions.as_mut().unwrap().start = "Foot".into();
    app.dispatch(Command::Instances(C::Preview(Box::new(draft))), &ctx);
    assert!(!app.ui.instances.invalid, "{}", app.ui.status);
    app.dispatch(Command::Instances(C::Cancel), &ctx);
    assert!(app.document.project() == before);
}

use super::*;
use crate::sculpt::{Geometry, Project, TopologyEdit};

#[test]
fn modeling_pinched_weld_rejection_preserves_selection_redo_and_document() {
    for closed in [false, true] {
        let vertices = vec![
            Vec3(-0.1, 0., 0.),
            Vec3(-1., -1., -1.),
            Vec3(-1., 1., -1.),
            Vec3(-1., 0., 1.),
            Vec3(0.1, 0., 0.),
            Vec3(1., -1., -1.),
            Vec3(1., 1., -1.),
            Vec3(1., 0., 1.),
        ];
        let triangles = if closed {
            vec![
                [0, 1, 2],
                [0, 2, 3],
                [0, 3, 1],
                [1, 3, 2],
                [4, 6, 5],
                [4, 7, 6],
                [4, 5, 7],
                [5, 6, 7],
            ]
        } else {
            vec![[0, 1, 2], [4, 6, 5]]
        };
        let mesh = Mesh {
            mask: vec![0.; vertices.len()],
            vertices,
            triangles,
            ..Default::default()
        };
        let mut document = Document::default();
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        let mut future = document.geometry_snapshot();
        future.mesh.mask[1] = 0.5;
        document.replace_geometry(future.clone()).unwrap();
        document.undo();
        assert!(document.can_redo());
        let mut state = ModelingState::default();
        state.sync(&document);
        state.selected = BTreeSet::from([Component::Vertex(0), Component::Vertex(4)]);
        let selected = state.selected.clone();
        let before = document.geometry_snapshot();
        let revision = document.display_revision();
        let history = document.history_counts();
        let project = serde_json::to_vec(&document.project()).unwrap();
        let error = state
            .topology_edit(&mut document, TopologyEdit::Weld, true, true, 1)
            .unwrap_err();
        assert!(error.contains("connected surface fan"), "{error}");
        assert_eq!(state.selected, selected);
        assert_eq!(document.geometry_snapshot(), before);
        assert_eq!(document.display_revision(), revision);
        assert_eq!(document.history_counts(), history);
        assert_eq!(serde_json::to_vec(&document.project()).unwrap(), project);
        document.redo();
        assert_eq!(document.geometry_snapshot(), future);
    }
}

#[test]
fn modeling_boundary_edit_sequence_keeps_exact_history_and_project_roundtrip() {
    let mesh = Mesh {
        vertices: vec![
            Vec3(-1., -1., 0.),
            Vec3(1., -1., 0.),
            Vec3(1., 1., 0.),
            Vec3(-1., 1., 0.),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        quads: vec![[0, 1, 2, 3]],
        mask: vec![0.; 4],
        ..Default::default()
    };
    let mut document = Document::default();
    document.replace_geometry(Geometry::new(mesh)).unwrap();
    let mut state = ModelingState::default();
    let before = document.geometry_snapshot();
    let history = document.history_counts();
    state.sync(&document);
    state.selected.insert(Component::Edge([0, 1]));
    state
        .topology_edit(&mut document, TopologyEdit::Split, true, false, 1)
        .unwrap();
    let split = document.geometry_snapshot();
    assert_eq!(split.mesh.vertices.len(), 5);
    state.selected = BTreeSet::from([Component::Vertex(0), Component::Vertex(4)]);
    state
        .topology_edit(&mut document, TopologyEdit::Weld, true, false, 1)
        .unwrap();
    let welded = document.geometry_snapshot();
    assert_eq!(welded.mesh.vertices.len(), 4);
    assert_eq!(document.history_counts(), (history.0 + 2, 0));
    document.undo();
    assert_eq!(document.geometry_snapshot(), split);
    document.undo();
    assert_eq!(document.geometry_snapshot(), before);
    document.redo();
    assert_eq!(document.geometry_snapshot(), split);
    document.redo();
    assert_eq!(document.geometry_snapshot(), welded);

    let project: Project =
        serde_json::from_slice(&crate::persistence::json_bytes(&document.project()).unwrap())
            .unwrap();
    let mut reopened = Document::default();
    reopened
        .replace_project(project, "modeling-invariant-sequence.clay")
        .unwrap();
    assert_eq!(reopened.geometry_snapshot(), welded);
}

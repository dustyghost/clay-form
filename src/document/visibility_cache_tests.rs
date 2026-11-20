use super::*;
use std::sync::atomic::Ordering;

fn check(document: &Document, expected: usize) {
    let scans = document.hidden_count_scans.load(Ordering::Relaxed);
    assert_eq!(document.mesh().hidden_face_count(), expected);
    for _ in 0..32 {
        assert_eq!(document.hidden_face_count(), expected);
    }
    assert_eq!(
        document.hidden_count_scans.load(Ordering::Relaxed),
        scans + 1
    );
}

#[test]
fn hidden_count_cache_tracks_visibility_topology_history_and_active_model() {
    let mut primitive = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
    primitive.divisions = [2, 1, 1];
    let mut mesh = primitive.mesh().unwrap();
    mesh.mask = mesh
        .vertices
        .iter()
        .map(|p| if p.0 >= 0. { 0.8 } else { 0.2 })
        .collect();
    mesh.create_vertex_group("Base".into(), 0.5, true).unwrap();
    mesh.create_vertex_group("Right".into(), 0.5, false)
        .unwrap();
    let mut d = Document::default();
    d.replace_geometry(Geometry::new(mesh)).unwrap();
    check(&d, 0);
    d.select_face_region(|p| p.0 > 0., true).unwrap();
    check(&d, 2);
    d.undo();
    check(&d, 0);
    d.redo();
    check(&d, 2);
    d.show_all_faces().unwrap();
    check(&d, 0);
    d.set_vertex_group_visible(1, false).unwrap();
    check(&d, 2);
    d.invert_visibility().unwrap();
    check(&d, 2);
    // Topology and source ownership change when the hidden part is split off.
    d.split_hidden().unwrap();
    check(&d, 0);
    let active = d.active_model_index();
    d.select_submodel(1 - active).unwrap();
    check(&d, d.mesh().hidden_face_count());
    d.select_submodel(active).unwrap();
    check(&d, d.mesh().hidden_face_count());
    d.undo();
    check(&d, d.mesh().hidden_face_count());
    let project = d.project();
    d.new_sphere();
    check(&d, 0);
    d.replace_project(project, "visibility.clay").unwrap();
    check(&d, d.mesh().hidden_face_count());
    d.select_face_region(|_| true, true).unwrap();
    check(&d, d.mesh().triangles.len());
    d.show_all_faces().unwrap();
    check(&d, 0);
    let scans = d.hidden_count_scans.load(Ordering::Relaxed);
    d.show_all_faces().unwrap();
    assert_eq!(d.hidden_face_count(), 0);
    assert_eq!(
        d.hidden_count_scans.load(Ordering::Relaxed),
        scans,
        "no-op keeps cached count"
    );
}

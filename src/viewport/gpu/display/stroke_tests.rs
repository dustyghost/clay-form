use super::*;
use crate::sculpt::{Brush, Dab, Kind, Material, Primitive, PrimitiveKind};

fn source() -> Mesh {
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [60; 3];
    let mut mesh = cube.mesh().unwrap();
    for p in &mut mesh.vertices {
        *p = p.norm();
    }
    mesh.create_vertex_group("Surface".into(), 0.5, true)
        .unwrap();
    mesh.hidden_faces = mesh
        .triangles
        .iter()
        .map(|t| t.iter().all(|&i| mesh.vertices[i].0 > 0.1))
        .collect();
    mesh.mask = mesh
        .vertices
        .iter()
        .map(|p| if p.0 < -0.5 { 1. } else { 0. })
        .collect();
    mesh.materials = vec![Material::from_srgb([0.2, 0.4, 0.6], 0.65, 0.); mesh.vertices.len()];
    mesh
}

fn document(mesh: Mesh) -> Document {
    let mut doc = Document::default();
    let mut project = doc.project();
    project.mesh = mesh;
    doc.replace_project(project, "hidden-stroke.clay").unwrap();
    doc
}

fn dab() -> Dab {
    Dab {
        center: Vec3(-0.2, 0., 0.98),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 0.4,
        pressure: 0.8,
        delta: Vec3::default(),
        invert: false,
    }
}

fn assert_visible_data(actual: &DisplayMesh, canonical: &DisplayMesh) {
    assert_eq!(actual.mesh, canonical.mesh);
    assert_eq!(actual.shading.triangles, canonical.shading.triangles);
    assert_eq!(actual.source_faces, canonical.source_faces);
    assert_eq!(actual.owners, canonical.owners);
    assert_eq!(actual.poly_colors, canonical.poly_colors);
    assert_eq!(actual.uv_corners, canonical.uv_corners);
    assert_eq!(actual.edge_colors, canonical.edge_colors);
    assert_eq!(actual.outlines, canonical.outlines);
}

#[test]
fn hidden_stroke_cache_preserves_filtered_faces_and_boundary_normals() {
    let initial = source();
    let mut doc = document(initial.clone());
    let mut display = DisplayMesh::new(&doc);
    let topology = display.hidden_stroke_topology.as_ref().unwrap().clone();
    let mut brush = Brush::new(Kind::Standard);
    brush.strength = 0.15;
    for _ in 0..3 {
        assert!(doc.apply_dab(&brush, dab()));
        display = display
            .during_stroke(&doc)
            .expect("hidden strokes must reuse topology");
        assert!(Arc::ptr_eq(
            &topology,
            display.hidden_stroke_topology.as_ref().unwrap()
        ));
        assert_visible_data(&display, &DisplayMesh::new(&doc));
        // Smoothing includes both sides of the hidden boundary. Summing only
        // filtered display triangles would produce different boundary normals.
        let full_normals = doc.mesh().normals();
        let filtered_normals = display.mesh.normals();
        assert!(
            full_normals
                .iter()
                .zip(&filtered_normals)
                .any(|(a, b)| (*a - *b).len() > 0.01)
        );
        for &(vertex, normal) in &display.shading.vertices {
            assert!((normal - full_normals[vertex]).len() < 1e-6);
        }
    }
    for (i, eligible) in initial.sculptable_vertices().into_iter().enumerate() {
        if !eligible || initial.mask[i] == 1. {
            assert_eq!(initial.vertices[i], doc.mesh().vertices[i]);
        }
    }
    assert_eq!(doc.history_counts(), (1, 0));
    let edited = doc.mesh().clone();
    doc.end_stroke();
    assert!(display.during_stroke(&doc).is_none());
    doc.undo();
    assert_eq!(doc.mesh(), &initial);
    doc.redo();
    assert_eq!(doc.mesh(), &edited);
}

#[test]
fn hidden_stroke_cache_paint_matches_canonical_attributes_and_normals() {
    let mut doc = document(source());
    let old = DisplayMesh::new(&doc);
    let mut brush = Brush::new(Kind::Paint);
    brush.set_paint_colour(false, [0.9, 0.1, 0.2]);
    assert!(doc.apply_dab(&brush, dab()));
    let updated = old.during_stroke(&doc).unwrap();
    let canonical = DisplayMesh::new(&doc);
    assert_visible_data(&updated, &canonical);
    assert_eq!(updated.shading.vertices, canonical.shading.vertices);
    assert_eq!(
        updated.vertices(updated.vertex_stride()),
        canonical.vertices(canonical.vertex_stride())
    );
    assert_ne!(updated.mesh.materials, old.mesh.materials);
}

#[test]
fn hidden_stroke_cache_rejects_visibility_group_and_topology_changes() {
    let mesh = source();
    let display = DisplayMesh::new(&document(mesh.clone()));
    for case in 0..7 {
        let mut changed = mesh.clone();
        match case {
            0 => {
                // Keep the visible count unchanged; counts alone are not a key.
                let hidden = changed.hidden_faces.iter().position(|&h| h).unwrap();
                let visible = changed.hidden_faces.iter().position(|&h| !h).unwrap();
                changed.hidden_faces.swap(hidden, visible);
            }
            1 => changed.vertex_groups[0].visible = false,
            2 => changed.polygroups[..2].fill(0),
            3 => {
                // A change only to hidden topology must also invalidate normals.
                let faces: Vec<_> = changed
                    .hidden_faces
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| **h)
                    .take(2)
                    .map(|(i, _)| i)
                    .collect();
                changed.triangles.swap(faces[0], faces[1]);
            }
            4 => changed.quads.swap(0, 1),
            5 => {
                changed.creases.push(changed.polygon_edges()[0]);
            }
            6 => {
                changed.uv.seams.push(changed.polygon_edges()[0]);
            }
            _ => unreachable!(),
        }
        let mut doc = document(changed);
        doc.begin_stroke();
        assert!(
            display.during_stroke(&doc).is_none(),
            "case {case} must rebuild"
        );
    }
    let mut doc = document(mesh);
    let mut project = doc.project();
    project.scene.active_metadata.model_id = 999;
    doc.replace_project(project, "different-model.clay")
        .unwrap();
    doc.begin_stroke();
    assert!(display.during_stroke(&doc).is_none());
    doc.end_stroke();
    doc.duplicate_model(false).unwrap();
    doc.begin_stroke();
    assert!(display.during_stroke(&doc).is_none());
}

#[test]
fn hidden_stroke_cache_supports_group_visibility_and_empty_displays() {
    let mut mesh = source();
    mesh.create_vertex_group("Hidden".into(), 0.5, true)
        .unwrap();
    for (pair, hidden) in mesh
        .polygroups
        .chunks_exact_mut(2)
        .zip(mesh.hidden_faces.chunks_exact(2))
    {
        pair.fill(if hidden.iter().any(|&h| h) { 2 } else { 1 });
    }
    mesh.vertex_groups[1].visible = false;
    mesh.hidden_faces.clear();
    let mut doc = document(mesh);
    let display = DisplayMesh::new(&doc);
    assert!(doc.apply_dab(&Brush::new(Kind::Standard), dab()));
    let live = display.during_stroke(&doc).unwrap();
    assert_visible_data(&live, &DisplayMesh::new(&doc));
    doc.end_stroke();
    let mut geometry = doc.geometry_snapshot();
    geometry.mesh.vertex_groups[0].visible = false;
    doc.replace_geometry(geometry).unwrap();
    let empty = DisplayMesh::new(&doc);
    doc.begin_stroke();
    let live = empty.during_stroke(&doc).unwrap();
    assert!(live.mesh.triangles.is_empty());
    assert!(live.mesh.quads.is_empty());
    assert!(live.source_faces.is_empty());
}

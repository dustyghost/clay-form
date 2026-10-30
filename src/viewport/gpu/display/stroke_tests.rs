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
    assert_eq!(actual.muted_vertices, canonical.muted_vertices);
    assert_eq!(actual.active_vertices, canonical.active_vertices);
    assert_eq!(actual.active, canonical.active);
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
    let topology = display.stroke_topology.as_ref().unwrap().clone();
    let mut brush = Brush::new(Kind::Standard);
    brush.strength = 0.15;
    for _ in 0..3 {
        assert!(doc.apply_dab(&brush, dab()));
        display = display
            .during_stroke(&doc)
            .expect("hidden strokes must reuse topology");
        assert!(Arc::ptr_eq(
            &topology,
            display.stroke_topology.as_ref().unwrap()
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
        for &(vertex, normal) in display.shading.vertices.iter() {
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
    let live = Arc::new(display);
    let previous_shading = live.shading.clone();
    let released = live.after_stroke(&doc).unwrap();
    // Renderers may still retain the live snapshot while release is prepared.
    assert!(!released.shares_geometry(&live));
    assert_eq!(live.shading.vertices, previous_shading.vertices);
    assert_eq!(live.shading.triangles, previous_shading.triangles);
    assert!(live.temporary_normals);
    assert_eq!(released.depth_extent, live.depth_extent);
    let canonical = DisplayMesh::new(&doc);
    assert_visible_data(&released, &canonical);
    assert_eq!(released.shading.vertices, canonical.shading.vertices);
    assert!(!released.temporary_normals);
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
    doc.end_stroke();
    let updated = Arc::new(updated);
    assert!(Arc::ptr_eq(&updated, &updated.after_stroke(&doc).unwrap()));
}

#[test]
fn hidden_stroke_cache_rejects_visibility_group_and_topology_changes() {
    let mesh = source();
    let display = Arc::new(DisplayMesh::new(&document(mesh.clone())));
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
        doc.end_stroke();
        assert!(display.after_stroke(&doc).is_none(), "release case {case}");
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

#[test]
fn hidden_stroke_cache_release_updates_fans_for_the_next_stroke() {
    let mut mesh = source();
    let original = mesh.clone();
    let mut display = Arc::new(DisplayMesh::new(&document(mesh.clone())));
    let topology = display.stroke_topology.as_ref().unwrap().source.clone();
    let index = mesh
        .vertices
        .iter()
        .position(|p| p.2 > 0.99 && p.0 <= 0.)
        .unwrap();
    let original_fans = display.shading.vertices.len();
    for (height, split) in [(0.8, true), (0., false)] {
        mesh.vertices[index].2 = original.vertices[index].2 + height;
        let mut doc = document(mesh.clone());
        doc.begin_stroke();
        let live = display.during_stroke(&doc).unwrap();
        assert!(live.temporary_normals);
        // Repeated frames and material-only dabs must not clear the dirty normals.
        let live = live.during_stroke(&doc).unwrap();
        assert!(live.temporary_normals);
        doc.end_stroke();
        display = Arc::new(live).after_stroke(&doc).unwrap();
        let canonical = DisplayMesh::new(&doc);
        assert_visible_data(&display, &canonical);
        assert_eq!(display.shading.vertices, canonical.shading.vertices);
        assert_eq!(display.shading.vertices.len() > original_fans, split);
        assert_eq!(
            *display.stroke_topology.as_ref().unwrap().fans,
            doc.mesh().surface_shading().triangles
        );
        assert!(Arc::ptr_eq(
            &topology,
            &display.stroke_topology.as_ref().unwrap().source
        ));
    }
}

#[test]
fn hidden_stroke_cache_release_rejects_unseen_final_dab() {
    for kind in [Kind::Standard, Kind::Paint, Kind::Mask] {
        let mut doc = document(source());
        let display = DisplayMesh::new(&doc);
        let mut brush = Brush::new(kind);
        brush.strength = 0.15;
        brush.set_paint_colour(false, [0.9, 0.1, 0.2]);
        // Mask edits attributes but apply_dab's return value reports deformation.
        let before_first = doc.mesh().clone();
        doc.apply_dab(&brush, dab());
        assert_ne!(doc.mesh(), &before_first, "first {kind:?} dab");
        let live = Arc::new(display.during_stroke(&doc).unwrap());
        let mut next = dab();
        next.center.0 -= 0.1;
        let before_final = doc.mesh().clone();
        doc.apply_dab(&brush, next);
        assert_ne!(doc.mesh(), &before_final, "final {kind:?} dab");
        doc.end_stroke();
        assert!(live.after_stroke(&doc).is_none());
    }
}

#[test]
fn hidden_stroke_cache_release_rejects_material_only_edits_and_history() {
    let mut mesh = source();
    for material in &mut mesh.materials {
        *material = material.with_link(7);
    }
    let mut doc = document(mesh);
    let display = Arc::new(DisplayMesh::new(&doc));
    for case in 0..5 {
        let mut geometry = doc.geometry_snapshot();
        let material = geometry.mesh.materials.last_mut().unwrap();
        match case {
            0 => material.emission_strength = 0.5,
            1 => material.layers[0].weight = 0.5,
            2 => material.layers[2].surface.roughness = 0.9,
            3 => Arc::make_mut(&mut material.links)[0].id = 8,
            4 => Arc::make_mut(&mut material.links)[0].weights[2] = 0.5,
            _ => unreachable!(),
        }
        doc.replace_geometry(geometry).unwrap();
        assert_eq!(doc.mesh().vertices, display.mesh.vertices);
        assert_eq!(doc.mesh().mask, display.mesh.mask);
        assert!(display.after_stroke(&doc).is_none(), "material edit {case}");
        let changed = Arc::new(DisplayMesh::new(&doc));
        doc.undo();
        assert!(display.after_stroke(&doc).is_some());
        assert!(changed.after_stroke(&doc).is_none());
        doc.redo();
        assert!(display.after_stroke(&doc).is_none());
        assert!(changed.after_stroke(&doc).is_some());
        doc.undo();
    }
}

#[test]
fn visible_strokes_share_immutable_payloads_and_preserve_snapshots() {
    for kind in [Kind::Standard, Kind::Paint] {
        let mut mesh = source();
        mesh.hidden_faces.clear();
        let mut doc = document(mesh);
        let initial = Arc::new(DisplayMesh::new(&doc));
        let original_bytes = initial.vertices(initial.vertex_stride());
        assert!(!initial.stroke_topology.as_ref().unwrap().source.filtered);
        assert!(Arc::ptr_eq(
            &initial.shading.triangles,
            &initial.stroke_topology.as_ref().unwrap().fans
        ));
        let mut display = initial.clone();
        let mut brush = Brush::new(kind);
        brush.strength = 0.15;
        brush.set_paint_colour(false, [0.9, 0.1, 0.2]);
        for _ in 0..3 {
            assert!(doc.apply_dab(&brush, dab()));
            let next = Arc::new(display.during_stroke(&doc).unwrap());
            assert!(Arc::ptr_eq(&next.owners, &display.owners));
            assert!(Arc::ptr_eq(&next.source_faces, &display.source_faces));
            assert!(Arc::ptr_eq(&next.muted_vertices, &display.muted_vertices));
            assert!(Arc::ptr_eq(&next.edge_colors, &display.edge_colors));
            assert!(Arc::ptr_eq(&next.outlines, &display.outlines));
            assert!(Arc::ptr_eq(
                &next.shading.triangles,
                &display.shading.triangles
            ));
            if kind == Kind::Paint {
                assert!(Arc::ptr_eq(
                    &next.shading.vertices,
                    &display.shading.vertices
                ));
            } else {
                assert!(!Arc::ptr_eq(
                    &next.shading.vertices,
                    &display.shading.vertices
                ));
                let normals = doc.mesh().normals();
                for &(vertex, normal) in next.shading.vertices.iter() {
                    assert_eq!(
                        [normal.0.to_bits(), normal.1.to_bits(), normal.2.to_bits()],
                        [
                            normals[vertex].0.to_bits(),
                            normals[vertex].1.to_bits(),
                            normals[vertex].2.to_bits()
                        ]
                    );
                }
            }
            assert_visible_data(&next, &DisplayMesh::new(&doc));
            display = next;
        }
        doc.end_stroke();
        let finished = display.after_stroke(&doc).unwrap();
        let canonical = DisplayMesh::new(&doc);
        assert_visible_data(&finished, &canonical);
        assert_eq!(
            finished.vertices(finished.vertex_stride()),
            canonical.vertices(canonical.vertex_stride())
        );
        assert!(Arc::ptr_eq(
            &finished.shading.triangles,
            &finished.stroke_topology.as_ref().unwrap().fans
        ));
        if kind == Kind::Paint {
            assert!(Arc::ptr_eq(&display, &finished));
        }
        assert_eq!(initial.vertices(initial.vertex_stride()), original_bytes);
        doc.undo();
        assert!(finished.after_stroke(&doc).is_none());
        doc.redo();
        assert!(finished.after_stroke(&doc).is_some());
    }
}

#[test]
fn visible_cache_rejects_topology_visibility_and_attribute_changes() {
    let mut mesh = source();
    mesh.hidden_faces.clear();
    let display = Arc::new(DisplayMesh::new(&document(mesh.clone())));
    for case in 0..7 {
        let mut changed = mesh.clone();
        match case {
            0 => {
                changed.hidden_faces = vec![false; changed.triangles.len()];
                changed.hidden_faces[0] = true;
            }
            1 => changed.vertex_groups[0].visible = false,
            2 => changed.polygroups[..2].fill(0),
            3 => changed.triangles.swap(0, 1),
            4 => changed.quads.swap(0, 1),
            5 => changed.creases.push(changed.polygon_edges()[0]),
            6 => changed.uv.seams.push(changed.polygon_edges()[0]),
            _ => unreachable!(),
        }
        let mut doc = document(changed);
        doc.begin_stroke();
        assert!(display.during_stroke(&doc).is_none(), "live case {case}");
        doc.end_stroke();
        assert!(display.after_stroke(&doc).is_none(), "release case {case}");
    }
    for kind in [Kind::Standard, Kind::Paint, Kind::Mask] {
        let mut doc = document(mesh.clone());
        let brush = Brush::new(kind);
        doc.apply_dab(&brush, dab());
        let live = Arc::new(display.during_stroke(&doc).unwrap());
        let mut next = dab();
        next.center.0 -= 0.1;
        doc.apply_dab(&brush, next);
        doc.end_stroke();
        assert!(live.after_stroke(&doc).is_none(), "unseen {kind:?} dab");
    }
}

#[test]
fn visible_release_rebuilds_exact_fans_after_sharpness_changes() {
    let mut mesh = source();
    mesh.hidden_faces.clear();
    let original = mesh.clone();
    let mut display = Arc::new(DisplayMesh::new(&document(mesh.clone())));
    let topology = display.stroke_topology.as_ref().unwrap().source.clone();
    let vertex = mesh
        .vertices
        .iter()
        .position(|p| p.2 > 0.99 && p.0 <= 0.)
        .unwrap();
    let original_fans = display.shading.vertices.len();
    for (height, split) in [(0.8, true), (0., false)] {
        mesh.vertices[vertex].2 = original.vertices[vertex].2 + height;
        let mut doc = document(mesh.clone());
        doc.begin_stroke();
        let live = display.during_stroke(&doc).unwrap();
        let live = live.during_stroke(&doc).unwrap();
        assert!(live.temporary_normals);
        doc.end_stroke();
        display = Arc::new(live).after_stroke(&doc).unwrap();
        let canonical = DisplayMesh::new(&doc);
        assert_visible_data(&display, &canonical);
        assert_eq!(
            display.vertices(display.vertex_stride()),
            canonical.vertices(canonical.vertex_stride())
        );
        assert_eq!(display.shading.vertices.len() > original_fans, split);
        assert!(Arc::ptr_eq(
            &topology,
            &display.stroke_topology.as_ref().unwrap().source
        ));
    }
}

fn assert_live_normal_bits(display: &DisplayMesh, mesh: &Mesh) {
    let fans = &display.stroke_topology.as_ref().unwrap().fans;
    let expected = super::live_normals::full_normals(mesh, fans, &display.shading.vertices);
    for (&(a, actual), &(b, expected)) in display.shading.vertices.iter().zip(&expected.vertices) {
        assert_eq!(a, b);
        assert_eq!(
            [actual.0.to_bits(), actual.1.to_bits(), actual.2.to_bits()],
            [
                expected.0.to_bits(),
                expected.1.to_bits(),
                expected.2.to_bits()
            ]
        );
    }
}

#[test]
fn local_display_normals_preserve_skipped_dabs_hidden_faces_and_snapshots() {
    for hidden in [false, true] {
        let mut mesh = source();
        if !hidden {
            mesh.hidden_faces.clear();
        }
        let mut doc = document(mesh);
        let mut display = Arc::new(DisplayMesh::new(&doc));
        let topology = display.stroke_topology.as_ref().unwrap().source.clone();
        let paint = Brush::new(Kind::Paint);
        doc.apply_dab(&paint, dab());
        display = Arc::new(display.during_stroke(&doc).unwrap());
        assert!(
            topology.normal_corners.get().is_none(),
            "Paint must not allocate adjacency"
        );
        let brush = Brush::new(Kind::Standard);
        for step in 0..5 {
            let old_bytes = display.vertices(display.vertex_stride());
            // Two edits before a display refresh, moving into another region.
            for offset in [0., 0.04] {
                let mut next = dab();
                next.center.0 += step as f32 * 0.07 + offset;
                assert!(doc.apply_dab(&brush, next));
            }
            let work = display
                .update_live_normals(doc.mesh(), &display.stroke_topology.as_ref().unwrap().fans);
            assert_eq!(work.local, step > 0);
            if step > 0 {
                assert!(work.evaluated_faces < doc.mesh().triangles.len() / 2);
            } else {
                assert!(topology.normal_corners.get().is_none());
            }
            let next = Arc::new(display.during_stroke(&doc).unwrap());
            assert_live_normal_bits(&next, doc.mesh());
            assert_eq!(display.vertices(display.vertex_stride()), old_bytes);
            display = next;
        }
        assert!(topology.normal_corners.get().unwrap().is_some());
        doc.end_stroke();
        let finished = display.after_stroke(&doc).unwrap();
        let canonical = DisplayMesh::new(&doc);
        assert_eq!(
            finished.vertices(finished.vertex_stride()),
            canonical.vertices(canonical.vertex_stride())
        );
        assert!(Arc::ptr_eq(
            &topology,
            &finished.stroke_topology.as_ref().unwrap().source
        ));
        doc.undo();
        assert!(finished.after_stroke(&doc).is_none());
        doc.redo();
        assert!(finished.after_stroke(&doc).is_some());
    }
}

#[test]
fn local_display_normals_reuse_adjacency_after_fans_split_and_merge() {
    let mut mesh = source();
    let original = mesh.clone();
    let mut display = Arc::new(DisplayMesh::new(&document(mesh.clone())));
    let topology = display.stroke_topology.as_ref().unwrap().source.clone();
    let sharp = mesh
        .vertices
        .iter()
        .position(|p| p.2 > 0.99 && p.0 <= 0.)
        .unwrap();
    let distant = mesh
        .vertices
        .iter()
        .position(|p| p.2 < -0.99 && p.0 <= 0.)
        .unwrap();
    let boundary = mesh
        .vertices
        .iter()
        .position(|p| p.2 > 0.98 && p.0 > 0.1 && p.0 < 0.15)
        .unwrap();
    let original_fans = display.shading.vertices.len();
    for (height, split) in [(0.8, true), (0., false)] {
        mesh.vertices[sharp].2 = original.vertices[sharp].2 + height;
        let mut doc = document(mesh.clone());
        doc.begin_stroke();
        let first = Arc::new(display.during_stroke(&doc).unwrap());
        assert_live_normal_bits(&first, doc.mesh());
        // Change a face across the hidden boundary, then a distant visible face.
        for vertex in [boundary, distant] {
            mesh.vertices[vertex].0 += 0.0001;
            doc = document(mesh.clone());
            doc.begin_stroke();
            let work = first
                .update_live_normals(doc.mesh(), &first.stroke_topology.as_ref().unwrap().fans);
            assert!(work.local);
            display = Arc::new(first.during_stroke(&doc).unwrap());
            assert_live_normal_bits(&display, doc.mesh());
        }
        doc.end_stroke();
        display = display.after_stroke(&doc).unwrap();
        assert_eq!(display.shading.vertices.len() > original_fans, split);
        let canonical = DisplayMesh::new(&doc);
        assert_eq!(
            display.vertices(display.vertex_stride()),
            canonical.vertices(canonical.vertex_stride())
        );
        assert!(Arc::ptr_eq(
            &topology,
            &display.stroke_topology.as_ref().unwrap().source
        ));
        assert!(topology.normal_corners.get().unwrap().is_some());
    }
}

#[test]
fn broad_display_deformation_preserves_full_normals_without_allocating_adjacency() {
    let mut mesh = source();
    let mut display = DisplayMesh::new(&document(mesh.clone()));
    let topology = display.stroke_topology.as_ref().unwrap().source.clone();
    for _ in 0..2 {
        for p in &mut mesh.vertices {
            p.0 *= 1.01;
            p.2 += 0.02;
        }
        let mut doc = document(mesh.clone());
        doc.begin_stroke();
        let work = display
            .update_live_normals(doc.mesh(), &display.stroke_topology.as_ref().unwrap().fans);
        assert!(!work.local);
        display = display.during_stroke(&doc).unwrap();
        assert_live_normal_bits(&display, doc.mesh());
        assert!(topology.normal_corners.get().is_none());
    }
}

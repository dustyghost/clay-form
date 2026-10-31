use super::*;
use crate::viewport::gpu::DisplayMesh;
use crate::{
    camera::Camera,
    document::Document,
    sculpt::{Geometry, Mesh},
};
use std::sync::Arc;

fn scene(mesh: Mesh, camera: &Camera, sort: bool) -> Scene {
    let visible_faces = mesh.triangles.len() - mesh.hidden_face_count();
    let mut doc = Document::default();
    doc.replace_geometry(Geometry::new(mesh)).unwrap();
    let display = Arc::new(DisplayMesh::new(&doc));
    assert_eq!(display.mesh.triangles.len(), visible_faces);
    Scene::from_geometry(
        egui::Rect::from_min_size(Pos2::new(13., 29.), egui::vec2(800., 600.)),
        camera,
        display,
        sort,
    )
}

fn bits(hit: Option<(Vec3, Vec3, usize)>) -> Option<([u32; 6], usize)> {
    hit.map(|(p, n, face)| ([p.0, p.1, p.2, n.0, n.1, n.2].map(f32::to_bits), face))
}

fn check(view: &Scene, pos: Pos2, radius: f32, budget: usize) -> Footprint {
    let grid = Footprint::with_budget(view, pos, radius, budget);
    for (i, &point) in grid.points.iter().enumerate() {
        let full = super::super::pick_surface(view, &view.all_faces, point);
        assert_eq!(
            bits(view.pick_surface(point)),
            bits(full),
            "ordinary pick at sample {i}"
        );
        assert_eq!(
            bits(grid.pick(view, i)),
            bits(full),
            "sample {i}: {point:?}, radius {radius}"
        );
    }
    if let Candidates::Bins(bins) = &grid.candidates {
        assert!(bins.iter().map(Vec::len).sum::<usize>() <= budget);
    }
    grid
}

#[test]
fn footprint_bins_match_full_picks_across_cameras_hidden_faces_and_grid_edges() {
    let mut mesh = Mesh::starting_sphere();
    for (i, p) in mesh.vertices.iter_mut().enumerate() {
        p.2 *= 1. + 0.02 * (i as f32 * 0.3).sin();
    }
    for hidden in [false, true] {
        mesh.hidden_faces = mesh
            .triangles
            .iter()
            .map(|t| hidden && t.iter().all(|&i| mesh.vertices[i].0 > 0.15))
            .collect();
        assert_eq!(mesh.has_hidden_faces(), hidden);
        for perspective in [false, true] {
            for (yaw, pitch, roll, zoom, fov) in [
                (0., 0., 0., 1., 45.),
                (0.4, -0.3, 0.7, 0.2, 20.),
                (1.5, 0.5, -0.4, 8., 75.),
            ] {
                let camera = Camera {
                    perspective,
                    yaw,
                    pitch,
                    roll,
                    zoom,
                    field_of_view: fov,
                    pan: egui::vec2(21., -17.),
                    ..Default::default()
                };
                for sort in [false, true] {
                    let view = scene(mesh.clone(), &camera, sort);
                    for (offset, radius) in [
                        (egui::Vec2::ZERO, 50.),
                        (egui::vec2(95., -30.), 70.),
                        (egui::vec2(1200., 800.), 30.),
                        (egui::Vec2::ZERO, 0.),
                        (egui::Vec2::ZERO, 0.000001),
                    ] {
                        check(&view, view.center + offset, radius, MAX_REFERENCES);
                    }
                    // Place an actual projected vertex on a grid sample.
                    let vertex =
                        view.positions[view.geometry.mesh.triangles[view.all_faces[0].0][0]];
                    check(&view, vertex, 50., MAX_REFERENCES);
                }
            }
        }
    }
}

#[test]
fn footprint_candidates_preserve_near_plane_crossings_and_depth_ties() {
    for perspective in [false, true] {
        let camera = Camera {
            perspective,
            ..Default::default()
        };
        let (r, u, v) = camera.basis();
        let depth = if perspective {
            camera.eye_distance() + 0.5
        } else {
            0.5
        };
        let back = if perspective { depth + 1. } else { -1. };
        // Open triangles, one crossing the near plane and one behind it.
        let vertices = vec![
            r * -1. - u,
            r - u,
            u + v * depth,
            r * -1. - u + v * back,
            r - u + v * back,
            u + v * back,
        ];
        let mesh = Mesh {
            mask: vec![0.; vertices.len()],
            vertices,
            triangles: vec![[0, 1, 2], [0, 1, 2], [3, 4, 5]],
            ..Default::default()
        };
        for sort in [false, true] {
            let view = scene(mesh.clone(), &camera, sort);
            for radius in [1., 60., 250.] {
                let indexed = check(&view, view.center, radius, MAX_REFERENCES);
                let fallback = check(&view, view.center, radius, 0);
                assert!(matches!(fallback.candidates, Candidates::Scan(_)));
                assert!(matches!(indexed.candidates, Candidates::Bins(_)));
            }
            let hit = super::super::pick_surface(&view, &view.all_faces, view.center).unwrap();
            assert_eq!(hit.2, 0, "equal-depth faces retain traversal order");
        }
    }
}

#[test]
fn footprint_lookup_is_local_bounded_and_rebuilt_for_changed_scenes() {
    let mut geometry = Geometry::new(Mesh::starting_sphere());
    geometry.subdivide().unwrap();
    let camera = Camera::default();
    let original = scene(geometry.mesh.clone(), &camera, false);
    let grid = check(&original, original.center, 50., MAX_REFERENCES);
    let Candidates::Bins(bins) = &grid.candidates else {
        panic!("dense local query should be indexed")
    };
    let references = bins.iter().map(Vec::len).sum::<usize>();
    assert!(
        references < original.all_faces.len(),
        "a dense footprint should test fewer faces than one full scan"
    );
    let fallback = check(&original, original.center, 50., 1);
    assert!(matches!(fallback.candidates, Candidates::Scan(_)));
    for p in &mut geometry.mesh.vertices {
        p.2 += 0.03;
    }
    let changed = scene(geometry.mesh, &Camera { yaw: 0.2, ..camera }, false);
    check(&changed, changed.center, 50., MAX_REFERENCES);
    // Building another view does not mutate a retained old lookup/snapshot.
    for (i, &point) in grid.points.iter().enumerate() {
        assert_eq!(
            bits(grid.pick(&original, i)),
            bits(super::super::pick_surface(
                &original,
                &original.all_faces,
                point
            ))
        );
    }
}

#[test]
fn footprint_bins_keep_occluding_inactive_models_and_source_face_order() {
    let mut doc = Document::default();
    doc.replace_geometry(Geometry::new(Mesh::starting_sphere()))
        .unwrap();
    doc.duplicate_model(false).unwrap();
    let camera = Camera::default();
    let (_, _, view) = camera.basis();
    doc.apply_transform(
        crate::sculpt::Transform {
            translation: view * -0.4,
            ..Default::default()
        },
        false,
    )
    .unwrap();
    doc.end_stroke();
    let geometry = Arc::new(DisplayMesh::new(&doc));
    let scene = Scene::from_geometry(
        egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.)),
        &camera,
        geometry,
        false,
    );
    let grid = check(&scene, scene.center, 50., MAX_REFERENCES);
    let (_, _, face) = grid.pick(&scene, SAMPLES / 2).unwrap();
    assert_ne!(scene.geometry.owners[face], scene.active);
    assert!(scene.pick(scene.center).is_none());
}

#[test]
fn footprint_wide_overlapping_faces_fall_back_without_changing_hits() {
    let mesh = Mesh {
        vertices: vec![Vec3(-10., -10., 0.), Vec3(10., -10., 0.), Vec3(0., 10., 0.)],
        mask: vec![0.; 3],
        triangles: vec![[0, 1, 2]; 300],
        ..Default::default()
    };
    for perspective in [false, true] {
        let view = scene(
            mesh.clone(),
            &Camera {
                perspective,
                ..Default::default()
            },
            false,
        );
        let grid = check(&view, view.center, 50., MAX_REFERENCES);
        assert!(matches!(grid.candidates, Candidates::Scan(_)));
        for radius in [-1., f32::INFINITY, f32::NAN] {
            check(&view, view.center, radius, MAX_REFERENCES);
        }
    }
}

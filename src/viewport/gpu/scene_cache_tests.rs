use super::*;
use crate::viewport::scene::cache::Cache;
use crate::{
    camera::Camera,
    sculpt::{Geometry, Vec3},
};

fn rect() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(13., 29.), egui::vec2(800., 600.))
}
fn bits(hit: Option<(Vec3, Vec3)>) -> Option<[u32; 6]> {
    hit.map(|(p, n)| [p.0, p.1, p.2, n.0, n.1, n.2].map(f32::to_bits))
}
fn check(scene: &Scene, camera: &Camera, rect: egui::Rect, sort: bool) {
    let reference = Scene::from_geometry(rect, camera, scene.geometry.clone(), sort);
    assert_eq!(scene.active_faces(), reference.active_faces());
    for offset in [
        egui::Vec2::ZERO,
        egui::vec2(20., -15.),
        egui::vec2(2000., 0.),
    ] {
        let p = rect.center() + offset;
        assert_eq!(bits(scene.pick(p)), bits(reference.pick(p)));
    }
}
#[test]
fn scene_cache_keys_every_camera_viewport_and_face_order_input() {
    let display = Arc::new(DisplayMesh::new(&Document::default()));
    let camera = Camera::default();
    let mut cache = Cache::default();
    let first = cache.get(rect(), &camera, display.clone(), false);
    check(&first, &camera, rect(), false);
    let second = cache.get(rect(), &camera, display.clone(), false);
    assert!(second.shares_preparation(&first));
    for case in 0..10 {
        let mut changed = camera;
        match case {
            0 => changed.yaw = 0.4,
            1 => changed.pitch = 0.3,
            2 => changed.roll = 0.2,
            3 => changed.zoom = 1.2,
            4 => changed.pan.x = 10.,
            5 => changed.pan.y = 10.,
            6 => changed.field_of_view = 60.,
            7 => changed.perspective_radius = 2.,
            8 => changed.perspective = true,
            _ => changed.pan.x = -0.,
        }
        cache.get(rect(), &camera, display.clone(), false);
        let view = cache.get(rect(), &changed, display.clone(), false);
        assert!(!view.shares_preparation(&first), "camera field {case}");
        check(&view, &changed, rect(), false);
    }
    for r in [rect().translate(egui::vec2(1., 0.)), rect().expand(1.)] {
        cache.get(rect(), &camera, display.clone(), false);
        let view = cache.get(r, &camera, display.clone(), false);
        assert!(!view.shares_preparation(&first));
        check(&view, &camera, r, false);
    }
    let sorted = cache.get(rect(), &camera, display.clone(), true);
    assert!(!sorted.shares_preparation(&first));
    check(&sorted, &camera, rect(), true);
    cache.clear();
    assert!(
        !cache
            .get(rect(), &camera, display, false)
            .shares_preparation(&sorted)
    );
    check(&first, &camera, rect(), false);
}
#[test]
fn scene_cache_rebinds_shading_and_materials_and_invalidates_geometry_inputs() {
    let display = Arc::new(DisplayMesh::new(&Document::default()));
    let camera = Camera::default();
    let mut cache = Cache::default();
    let original = cache.get(rect(), &camera, display.clone(), false);
    let old_hit = original.pick(rect().center()).unwrap();
    // A release can replace normals while reusing the same position/topology arrays.
    let mut changed = (*display).clone();
    let mut geometry = (*display.geometry).clone();
    geometry.shading.vertices = Arc::new(
        geometry
            .shading
            .vertices
            .iter()
            .map(|&(i, n)| (i, (n + Vec3(0.3, 0.2, 0.)).norm()))
            .collect(),
    );
    let material = crate::sculpt::Material::from_srgb([0.9, 0.1, 0.2], 0.3, 0.);
    geometry.mesh.materials = Arc::new(vec![material; geometry.mesh.vertices.len()]);
    changed.geometry = Arc::new(geometry);
    let changed = Arc::new(changed);
    let view = cache.get(rect(), &camera, changed, false);
    assert!(view.shares_preparation(&original));
    assert_ne!(view.pick(rect().center()).unwrap().1, old_hit.1);
    let reference = Scene::from_geometry(rect(), &camera, view.geometry.clone(), false);
    assert_eq!(
        view.sample_material(rect().center()),
        reference.sample_material(rect().center())
    );
    check(&view, &camera, rect(), false);
    assert_eq!(original.pick(rect().center()).unwrap(), old_hit);
    for case in 0..6 {
        cache.get(rect(), &camera, display.clone(), false);
        let mut next = (*display).clone();
        let mut data = (*display.geometry).clone();
        match case {
            0 => {
                let mut v = (*data.mesh.vertices).clone();
                v[0].0 += 0.02;
                data.mesh.vertices = Arc::new(v);
            }
            1 => {
                let mut t = (*data.mesh.triangles).clone();
                t.pop();
                data.mesh.triangles = Arc::new(t);
            }
            2 => data.owners = Arc::new((*data.owners).clone()),
            3 => data.active = 1,
            4 => data.active_vertices -= 1,
            _ => data.depth_extent += 1.,
        }
        next.geometry = Arc::new(data);
        let scene = cache.get(rect(), &camera, Arc::new(next), false);
        assert!(
            !scene.shares_preparation(&original),
            "geometry field {case}"
        );
        check(&scene, &camera, rect(), false);
    }
    // Real document visibility/model changes also replace the identity inputs.
    let mut doc = Document::default();
    doc.duplicate_model(false).unwrap();
    let view = cache.get(rect(), &camera, Arc::new(DisplayMesh::new(&doc)), false);
    assert!(!view.shares_preparation(&original));
    check(&view, &camera, rect(), false);
    let mut geometry = Geometry::new(doc.mesh().clone());
    geometry.mesh.hidden_faces = vec![true; geometry.mesh.triangles.len()];
    doc.replace_geometry(geometry).unwrap();
    let empty = cache.get(rect(), &camera, Arc::new(DisplayMesh::new(&doc)), false);
    assert!(empty.pick(rect().center()).is_none());
    assert!(!empty.shares_preparation(&view));
}
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Vulkan; validates separate base and preview preparation caches"]
fn gpu_scene_cache_keeps_preview_and_picking_roles_separate() {
    let headless = super::ui_capture::Headless::new();
    let mut renderer = Renderer::new(&headless.state).unwrap();
    let doc = Document::default();
    let camera = Camera::default();
    let base = renderer.geometry(&doc);
    let mut mesh = doc.mesh().clone();
    for p in &mut mesh.vertices {
        p.2 += 0.2;
    }
    let preview = Arc::new(DisplayMesh::with_active_mesh(&doc, &mesh));
    let original = renderer.prepare_scene(rect(), &camera, base.clone(), false);
    let displayed = renderer.prepare_scene(rect(), &camera, preview.clone(), true);
    assert_ne!(
        bits(original.pick(rect().center())),
        bits(displayed.pick(rect().center()))
    );
    assert!(
        renderer
            .prepare_scene(rect(), &camera, base.clone(), false)
            .shares_preparation(&original)
    );
    assert!(
        renderer
            .prepare_scene(rect(), &camera, preview.clone(), true)
            .shares_preparation(&displayed)
    );
    renderer.clear_scene_preview();
    assert!(
        !renderer
            .prepare_scene(rect(), &camera, preview, true)
            .shares_preparation(&displayed)
    );
    assert!(
        renderer
            .prepare_scene(rect(), &camera, base, false)
            .shares_preparation(&original)
    );
    renderer.destroy();
}

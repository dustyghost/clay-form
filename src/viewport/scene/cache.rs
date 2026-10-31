//! One prepared view per rendering role; immutable mesh identity controls reuse.
use super::{Arc, Camera, DisplayMesh, Mutex, Scene, egui};

#[derive(PartialEq, Eq)]
struct Key {
    bits: [u32; 12],
    perspective: bool,
    sort: bool,
}

impl Key {
    fn new(rect: egui::Rect, camera: &Camera, sort: bool) -> Self {
        Self {
            bits: [
                rect.min.x,
                rect.min.y,
                rect.max.x,
                rect.max.y,
                camera.yaw,
                camera.pitch,
                camera.roll,
                camera.zoom,
                camera.pan.x,
                camera.pan.y,
                camera.field_of_view,
                camera.perspective_radius,
            ]
            .map(f32::to_bits),
            perspective: camera.perspective,
            sort,
        }
    }
}
#[derive(Default)]
pub(in crate::viewport) struct Cache {
    view: Option<(Key, Scene)>,
    // Geometric face normals/origins depend on immutable positions and triangles,
    // not the camera. Keep one dense surface per rendering role.
    face_geometry: Vec<super::FaceGeometry>,
}
impl Cache {
    pub fn clear(&mut self) {
        self.view = None;
        self.face_geometry = Vec::new();
    }
    pub fn get(
        &mut self,
        rect: egui::Rect,
        camera: &Camera,
        geometry: Arc<DisplayMesh>,
        sort: bool,
    ) -> Scene {
        let key = Key::new(rect, camera, sort);
        if let Some((old_key, old)) = &self.view
            && *old_key == key
            && Arc::ptr_eq(&old.geometry.mesh.vertices, &geometry.mesh.vertices)
            && Arc::ptr_eq(&old.geometry.mesh.triangles, &geometry.mesh.triangles)
            && Arc::ptr_eq(&old.geometry.owners, &geometry.owners)
            && old.active == geometry.active
            && old.geometry.active_vertices == geometry.active_vertices
            && old.geometry.depth_extent.to_bits() == geometry.depth_extent.to_bits()
        {
            let mut scene = old.clone();
            // Projection/bounds depend only on geometry and view. A hit also
            // contains a shading normal, which can change on stroke release.
            if !Arc::ptr_eq(&old.geometry, &geometry) {
                scene.last_pick = Arc::new(Mutex::new(None));
            }
            scene.geometry = geometry;
            self.view = Some((key, scene.clone()));
            return scene;
        }
        let same_surface = self.view.as_ref().is_some_and(|(_, old)| {
            Arc::ptr_eq(&old.geometry.mesh.vertices, &geometry.mesh.vertices)
                && Arc::ptr_eq(&old.geometry.mesh.triangles, &geometry.mesh.triangles)
        });
        if !same_surface {
            self.face_geometry = Vec::new();
        }
        // Wait for another view of unchanged geometry. Deforming frames and
        // an unchanged camera never pay for a cache they cannot reuse.
        let face_geometry = (same_surface && geometry.mesh.triangles.len() >= 20_000)
            .then_some(&mut self.face_geometry);
        let scene = Scene::from_geometry_with_faces(rect, camera, geometry, sort, face_geometry);
        self.view = Some((key, scene.clone()));
        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::Document,
        sculpt::{Geometry, Mesh, Vec3},
    };

    fn check(scene: &Scene, camera: &Camera, sort: bool) {
        let mesh = &scene.geometry.mesh;
        // Frozen facing arithmetic: caching must preserve near-silhouette
        // decisions and source traversal, not just approximate the same pixels.
        let mut expected: Vec<_> = mesh
            .triangles
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let [a, b, c] = t.map(|v| mesh.vertices[v]);
                ((b - a).cross(c - a).dot(if scene.eye_distance == 0. {
                    scene.view
                } else {
                    scene.view * scene.eye_distance - a
                }) > 0.)
                    .then(|| {
                        (
                            i,
                            if sort {
                                (a + b + c).dot(scene.view)
                            } else {
                                0.
                            },
                        )
                    })
            })
            .collect();
        if sort {
            expected.sort_by(|a, b| a.1.total_cmp(&b.1));
        }
        assert_eq!(*scene.all_faces, expected);
        let fresh = Scene::from_geometry(scene.rect, camera, scene.geometry.clone(), sort);
        assert_eq!(scene.positions, fresh.positions);
        assert_eq!(scene.active_bounds, fresh.active_bounds);
        let bits = |hit: Option<(Vec3, Vec3)>| {
            hit.map(|(p, n)| [p.0, p.1, p.2, n.0, n.1, n.2].map(f32::to_bits))
        };
        for p in [
            scene.center,
            scene.center + egui::vec2(17., 9.),
            scene.center + egui::vec2(500., 0.),
        ] {
            assert_eq!(bits(scene.pick(p)), bits(fresh.pick(p)));
        }
    }

    #[test]
    fn dense_camera_face_geometry_reuse_exact_geometry_and_invalidate_on_edits() {
        let mut geometry = Geometry::new(Mesh::starting_sphere());
        geometry.subdivide().unwrap();
        let mut doc = Document::default();
        doc.replace_geometry(geometry).unwrap();
        let display = Arc::new(DisplayMesh::new(&doc));
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400., 300.));
        let mut cache = Cache::default();
        let first = cache.get(rect, &Camera::default(), display.clone(), false);
        assert!(
            cache.face_geometry.is_empty(),
            "one-off views avoid extra storage"
        );
        cache.get(
            rect,
            &Camera {
                yaw: 0.01,
                ..Default::default()
            },
            display.clone(),
            false,
        );
        assert_eq!(cache.face_geometry.len(), display.mesh.triangles.len());
        let storage = cache.face_geometry.as_ptr();
        for perspective in [false, true] {
            for sort in [false, true] {
                for frame in 0..5 {
                    let camera = Camera {
                        perspective,
                        yaw: frame as f32 * 0.3,
                        pitch: -0.2,
                        roll: 0.1,
                        perspective_radius: if frame == 4 { 0.1 } else { 1. },
                        ..Default::default()
                    };
                    let scene = cache.get(rect, &camera, display.clone(), sort);
                    assert_eq!(cache.face_geometry.as_ptr(), storage);
                    check(&scene, &camera, sort);
                }
            }
        }
        for hidden in [false, true] {
            let mut next = doc.geometry_snapshot();
            next.mesh.vertices[0].0 += 0.2;
            if hidden {
                next.mesh.hidden_faces = next
                    .mesh
                    .triangles
                    .iter()
                    .enumerate()
                    .map(|(i, _)| i % 7 == 0)
                    .collect();
            }
            doc.replace_geometry(next).unwrap();
            let scene = cache.get(
                rect,
                &Camera::default(),
                Arc::new(DisplayMesh::new(&doc)),
                false,
            );
            check(&scene, &Camera::default(), false);
            assert!(
                cache.face_geometry.is_empty(),
                "deformation does not build unused face_geometry"
            );
            let camera = Camera {
                yaw: 0.1,
                ..Default::default()
            };
            let scene = cache.get(rect, &camera, scene.geometry.clone(), false);
            check(&scene, &camera, false);
            assert_eq!(
                cache.face_geometry,
                scene
                    .geometry
                    .mesh
                    .triangles
                    .iter()
                    .map(|t| {
                        let [a, b, c] = t.map(|i| scene.geometry.mesh.vertices[i]);
                        super::super::FaceGeometry {
                            normal: (b - a).cross(c - a),
                            origin: a,
                        }
                    })
                    .collect::<Vec<_>>()
            );
        }
        check(&first, &Camera::default(), false);
        cache.clear();
        assert!(cache.face_geometry.is_empty());
        assert_eq!(cache.face_geometry.capacity(), 0);
    }
}

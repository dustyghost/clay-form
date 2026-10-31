//! Conservative bounds for contiguous groups in the existing face traversal.
use super::{Pos2, Scene, egui};

const GROUP: usize = 128;
const MIN_FACES: usize = 256;

pub(super) struct Index {
    bounds: Vec<egui::Rect>,
}
impl Index {
    pub fn new(scene: &Scene) -> Self {
        if scene.all_faces.len() < MIN_FACES {
            return Self { bounds: Vec::new() };
        }
        Self::build(scene)
    }
    fn build(scene: &Scene) -> Self {
        let check_near = may_cross_near(scene);
        let bounds = scene
            .all_faces
            .chunks(GROUP)
            .map(|faces| {
                faces
                    .iter()
                    .fold(egui::Rect::NOTHING, |bounds, &(face, _)| {
                        bounds.union(face_bounds(scene, face, check_near))
                    })
            })
            .collect();
        Self { bounds }
    }
    pub fn candidates<'a>(
        &'a self,
        scene: &'a Scene,
        bounds: egui::Rect,
    ) -> impl Iterator<Item = (usize, f32)> + 'a {
        // Filtering whole groups preserves the exact source traversal order,
        // including equal-depth hits, without a duplicated face-ID index.
        scene
            .all_faces
            .chunks(GROUP)
            .enumerate()
            .filter(move |(group, _)| self.bounds.get(*group).is_none_or(|b| b.intersects(bounds)))
            .flat_map(|(_, faces)| faces.iter().copied())
    }
}
pub(super) fn may_cross_near(scene: &Scene) -> bool {
    scene.eye_distance > 0. && scene.eye_distance - scene.geometry.depth_extent < scene.near()
}

pub(super) fn face_bounds(scene: &Scene, face: usize, check_near: bool) -> egui::Rect {
    let triangle = scene.geometry.mesh.triangles[face];
    if check_near
        && triangle
            .iter()
            .any(|&i| !scene.visible_depth(scene.geometry.mesh.vertices[i]))
    {
        // Clamped projections do not bound a triangle crossing the near plane.
        // Let the exact ray intersection and depth check decide every sample.
        return egui::Rect::EVERYTHING;
    }
    let [a, b, c] = triangle.map(|i| scene.positions[i]);
    if ![a, b, c].iter().all(|p| p.is_finite()) {
        return egui::Rect::EVERYTHING;
    }
    let bounds = egui::Rect::from_min_max(
        Pos2::new(a.x.min(b.x).min(c.x), a.y.min(b.y).min(c.y)),
        Pos2::new(a.x.max(b.x).max(c.x), a.y.max(b.y).max(c.y)),
    );
    if scene.eye_distance == 0. {
        bounds
    } else {
        // The perspective picker works in world space. Leave room for rounding
        // between ray arithmetic and screen projection at shared edges.
        let magnitude = [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
            .into_iter()
            .map(f32::abs)
            .fold(1_f32, f32::max);
        bounds.expand(magnitude * (32. * f32::EPSILON))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::gpu::DisplayMesh;
    use crate::{
        camera::Camera,
        document::Document,
        sculpt::{Geometry, Mesh, Vec3},
    };
    use std::sync::Arc;
    fn scene(mesh: Mesh, camera: &Camera, sort: bool) -> Scene {
        let mut doc = Document::default();
        doc.replace_geometry(Geometry::new(mesh)).unwrap();
        Scene::from_geometry(
            egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.)),
            camera,
            Arc::new(DisplayMesh::new(&doc)),
            sort,
        )
    }
    fn bits(hit: Option<(Vec3, Vec3, usize)>) -> Option<([u32; 6], usize)> {
        hit.map(|(p, n, f)| ([p.0, p.1, p.2, n.0, n.1, n.2].map(f32::to_bits), f))
    }
    #[test]
    fn grouped_bounds_skip_local_and_missed_queries_with_bounded_storage() {
        let mut geometry = Geometry::new(Mesh::starting_sphere());
        geometry.subdivide().unwrap();
        for perspective in [false, true] {
            let scene = scene(
                geometry.mesh.clone(),
                &Camera {
                    perspective,
                    ..Default::default()
                },
                false,
            );
            let point = scene.center;
            assert_eq!(
                bits(scene.pick_surface(point)),
                bits(super::super::pick_surface(&scene, &scene.all_faces, point))
            );
            assert!(scene.picking.get().is_none());
            scene.pick_surface(point); // Identical queries reuse the hit.
            assert!(scene.picking.get().is_none());
            scene.pick_surface(point + egui::vec2(1., 0.));
            let index = scene.picking.get().unwrap();
            assert_eq!(index.bounds.len(), scene.all_faces.len().div_ceil(GROUP));
            assert!(
                index
                    .candidates(&scene, egui::Rect::from_min_max(point, point))
                    .count()
                    < scene.all_faces.len() / 2
            );
            let point = point + egui::vec2(2000., 2000.);
            assert_eq!(
                index
                    .candidates(&scene, egui::Rect::from_min_max(point, point))
                    .count(),
                0
            );
            assert!(scene.pick_surface(point).is_none());
        }
    }
    #[test]
    fn grouped_bounds_preserve_near_crossings_and_ties_across_group_boundaries() {
        let camera = Camera {
            perspective: true,
            ..Default::default()
        };
        let mesh = Mesh {
            vertices: vec![
                Vec3(-1., -1., 0.),
                Vec3(1., -1., 0.),
                Vec3(0., 1., camera.eye_distance() + 0.5),
            ],
            mask: vec![0.; 3],
            triangles: vec![[0, 1, 2]; GROUP * 3],
            ..Default::default()
        };
        for sort in [false, true] {
            let scene = scene(mesh.clone(), &camera, sort);
            for x in -8..=8 {
                for y in -8..=8 {
                    let point = scene.center + egui::vec2(x as f32 * 45., y as f32 * 45.);
                    assert_eq!(
                        bits(scene.pick_surface(point)),
                        bits(super::super::pick_surface(&scene, &scene.all_faces, point))
                    );
                }
            }
            assert!(scene.picking.get().unwrap().bounds.len() >= 3);
            assert_eq!(scene.pick_surface(scene.center).unwrap().2, 0);
        }
    }
}

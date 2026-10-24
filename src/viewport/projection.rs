//! Shared camera projection for surface tools, picking, and control placement.
use super::scene::Scene;
use crate::sculpt::Vec3;
use eframe::egui::{Pos2, Vec2};

impl Scene {
    pub fn visible_depth(&self, point: Vec3) -> bool {
        self.eye_distance == 0. || self.eye_distance - point.dot(self.view) >= self.near()
    }
    pub fn near(&self) -> f32 {
        self.eye_distance * 0.001
    }
    pub fn scale_at(&self, point: Vec3) -> f32 {
        crate::camera::Camera::projection_scale(self.scale, self.eye_distance, point.dot(self.view))
    }
    pub fn project(&self, point: Vec3) -> Pos2 {
        let scale = self.scale_at(point);
        self.center + Vec2::new(point.dot(self.right), -point.dot(self.up)) * scale
    }
    /// Invert a screen point onto the plane through anchor, parallel to the view.
    pub fn at_depth(&self, point: Pos2, anchor: Vec3) -> Vec3 {
        let delta = (point - self.center) / self.scale_at(anchor);
        self.right * delta.x - self.up * delta.y + self.view * anchor.dot(self.view)
    }
    pub fn ray(&self, point: Pos2) -> (Vec3, Vec3) {
        let flat = self.at_depth(point, Vec3::default());
        if self.eye_distance == 0. {
            (flat, self.view * -1.)
        } else {
            let eye = self.view * self.eye_distance;
            (eye, (flat - eye).norm())
        }
    }
    /// Local projected axis, normalized by the scale at the control's anchor.
    pub fn axis_direction(&self, anchor: Vec3, axis: Vec3) -> Vec2 {
        let factor = if self.eye_distance == 0. {
            0.
        } else {
            axis.dot(self.view) / (self.eye_distance - anchor.dot(self.view)).max(self.near())
        };
        Vec2::new(
            axis.dot(self.right) + anchor.dot(self.right) * factor,
            -axis.dot(self.up) - anchor.dot(self.up) * factor,
        )
    }
    pub fn axis_parameter(&self, point: Pos2, anchor: Vec3, axis: Vec3) -> Option<f32> {
        let (origin, direction) = self.ray(point);
        let dot = direction.dot(axis);
        let denominator = 1. - dot * dot;
        let offset = origin - anchor;
        (denominator > 0.001)
            .then(|| (offset.dot(axis) - dot * offset.dot(direction)) / denominator)
    }
    pub fn clip_segment(&self, mut points: [Vec3; 2]) -> Option<[Pos2; 2]> {
        if self.eye_distance > 0. {
            let depths = points.map(|p| self.eye_distance - p.dot(self.view) - self.near());
            if depths.iter().all(|&d| d < 0.) {
                return None;
            }
            for i in 0..2 {
                if depths[i] < 0. {
                    let t = depths[i] / (depths[i] - depths[1 - i]);
                    points[i] = points[i] + (points[1 - i] - points[i]) * t;
                }
            }
        }
        Some(points.map(|p| self.project(p)))
    }
    pub fn screen_depth(&self, point: Vec3) -> f32 {
        if self.eye_distance == 0. {
            point.dot(self.view)
        } else {
            1. / (self.eye_distance - point.dot(self.view)).max(self.near())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::gpu::DisplayMesh;
    use super::*;
    use crate::{
        camera::Camera,
        document::Document,
        sculpt::{Geometry, Mesh},
    };
    use std::sync::Arc;

    fn scene(camera: &Camera, vertices: Vec<Vec3>) -> Scene {
        let mesh = Mesh {
            creases: Vec::new(),
            vertices,
            triangles: vec![[0, 1, 2]],
            quads: vec![],
            mask: vec![0.; 3],
            materials: Vec::new(),
            vertex_groups: vec![],
            polygroups: vec![],
            hidden_faces: vec![],
        };
        let mut document = Document::default();
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        Scene::from_geometry(
            eframe::egui::Rect::from_min_size(Pos2::new(20., 40.), Vec2::new(900., 600.)),
            camera,
            Arc::new(DisplayMesh::new(&document)),
            false,
        )
    }
    #[test]
    fn perspective_picking_recovers_world_barycentrics_across_angles_and_zoom() {
        for perspective in [false, true] {
            for zoom in [0.2, 1., 8.] {
                for fov in [20., 45., 75.] {
                    let camera = Camera {
                        perspective,
                        field_of_view: fov,
                        zoom,
                        yaw: 0.6,
                        pitch: 2.3,
                        pan: Vec2::new(31., -27.),
                        ..Default::default()
                    };
                    let (right, up, view) = camera.basis();
                    let vertices = vec![
                        right * -0.8 - up * 0.6,
                        right * 0.9 - up * 0.5 + view * 0.6,
                        up * 0.8 - view * 0.4,
                    ];
                    let expected = vertices[0] * 0.2 + vertices[1] * 0.3 + vertices[2] * 0.5;
                    let scene = scene(&camera, vertices);
                    let pointer = scene.project(expected);
                    let (picked, normal) = scene.pick(pointer).unwrap();
                    assert!((picked - expected).len() < 2e-5);
                    assert!((normal.len() - 1.).abs() < 1e-5);
                    assert!((scene.at_depth(pointer, expected) - expected).len() < 2e-5);
                    let offset = scene.project(expected + right * (45. / scene.scale_at(expected)));
                    assert!((offset.distance(pointer) - 45.).abs() < 0.001);
                }
            }
        }
    }
    #[test]
    fn perspective_preserves_orbit_plane_framing_and_picks_near_clipped_faces() {
        let mut camera = Camera::default();
        let vertices = vec![Vec3(-1., -1., 0.), Vec3(1., -1., 0.), Vec3(0., 1., 0.)];
        let ortho = scene(&camera, vertices.clone());
        camera.perspective = true;
        let perspective = scene(&camera, vertices);
        assert_eq!(
            ortho.project(Vec3(0.3, -0.2, 0.)),
            perspective.project(Vec3(0.3, -0.2, 0.))
        );
        assert!(perspective.scale_at(Vec3(0., 0., 1.)) > perspective.scale);
        assert!(perspective.scale_at(Vec3(0., 0., -1.)) < perspective.scale);
        let distance = camera.eye_distance();
        let crossing = scene(
            &camera,
            vec![
                Vec3(-1., -1., 0.),
                Vec3(1., -1., 0.),
                Vec3(0., 1., distance + 0.5),
            ],
        );
        let expected = Vec3(0., -0.5, (distance + 0.5) * 0.25);
        assert!((crossing.pick(crossing.project(expected)).unwrap().0 - expected).len() < 1e-5);
        assert!(!crossing.visible_depth(Vec3(0., 0., distance)));
        let clipped = crossing
            .clip_segment([Vec3(0.2, 0., 0.), Vec3(0., 0., distance + 1.)])
            .unwrap();
        assert!(clipped.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        assert!(
            crossing
                .clip_segment([Vec3(0., 0., distance + 1.), Vec3(0., 1., distance + 1.)])
                .is_none()
        );
    }
}

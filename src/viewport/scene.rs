//! Orthographic rendering and surface picking. No document edits or input handling.
use crate::{
    camera::Camera,
    sculpt::{Mesh, Vec3},
};
use eframe::egui::{self, Color32, Pos2};
pub(super) struct Scene {
    pub scale: f32,
    positions: Vec<Pos2>,
    vertices: Vec<Vec3>,
    triangles: Vec<[usize; 3]>,
    pub(super) faces: Vec<(usize, f32)>,
    normals: Vec<Vec3>,
    view: Vec3,
    pub(super) center: Pos2,
    pub(super) right: Vec3,
    pub(super) up: Vec3,
}
impl Scene {
    pub fn paint(
        painter: &egui::Painter,
        rect: egui::Rect,
        mesh: &Mesh,
        camera: &Camera,
        wire: bool,
    ) -> Self {
        let center = rect.center() + camera.pan;
        let scale = rect.width().min(rect.height()) * 0.35 * camera.zoom;
        let (right, up, view) = camera.basis();
        let project = |v: Vec3| {
            Pos2::new(
                center.x + v.dot(right) * scale,
                center.y - v.dot(up) * scale,
            )
        };
        let normals = mesh.normals();
        let positions: Vec<_> = mesh.vertices.iter().copied().map(project).collect();
        let mut faces: Vec<_> = mesh
            .triangles
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                let a = mesh.vertices[t[0]];
                let b = mesh.vertices[t[1]];
                let c = mesh.vertices[t[2]];
                (b - a).cross(c - a).dot(view) > 0.
            })
            .map(|(i, t)| {
                (
                    i,
                    (mesh.vertices[t[0]] + mesh.vertices[t[1]] + mesh.vertices[t[2]]).dot(view),
                )
            })
            .collect();
        faces.sort_by(|a, b| a.1.total_cmp(&b.1));
        painter.rect_filled(rect, 0., Color32::from_rgb(23, 26, 31));
        let light = (view + up * 0.7 + right * (-0.5)).norm();
        let mut rendered = egui::Mesh::default();
        for (i, v) in mesh.vertices.iter().enumerate() {
            let illumination = 0.25 + 0.75 * normals[i].dot(light).max(0.);
            let mask = 1. - mesh.mask[i] * 0.75;
            rendered.colored_vertex(
                project(*v),
                Color32::from_rgb(
                    (190. * illumination * mask) as u8,
                    (155. * illumination * mask) as u8,
                    (125. * illumination * mask) as u8,
                ),
            );
        }
        for &(i, _) in &faces {
            for &v in &mesh.triangles[i] {
                rendered.indices.push(v as u32);
            }
        }
        painter.add(egui::Shape::mesh(rendered));
        if wire {
            for &(i, _) in &faces {
                let t = mesh.triangles[i];
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    painter.line_segment(
                        [positions[a], positions[b]],
                        egui::Stroke::new(0.3, Color32::from_black_alpha(55)),
                    );
                }
            }
        }
        Self {
            scale,
            positions,
            vertices: mesh.vertices.clone(),
            triangles: mesh.triangles.clone(),
            faces,
            normals,
            view,
            center,
            right,
            up,
        }
    }
    pub fn pick(&self, point: Pos2) -> Option<(Vec3, Vec3)> {
        pick_surface(
            &self.vertices,
            &self.triangles,
            &self.positions,
            &self.faces,
            &self.normals,
            self.view,
            point,
        )
    }
}
fn pick_surface(
    vertices: &[Vec3],
    triangles: &[[usize; 3]],
    positions: &[Pos2],
    faces: &[(usize, f32)],
    normals: &[Vec3],
    view: Vec3,
    point: Pos2,
) -> Option<(Vec3, Vec3)> {
    let mut hit = None;
    let mut depth = f32::NEG_INFINITY;
    for &(fi, _) in faces {
        let t = triangles[fi];
        let a = positions[t[0]];
        let v0 = positions[t[1]] - a;
        let v1 = positions[t[2]] - a;
        let v2 = point - a;
        let det = v0.x * v1.y - v0.y * v1.x;
        if det.abs() < 1e-7 {
            continue;
        }
        let u = (v2.x * v1.y - v2.y * v1.x) / det;
        let v = (v0.x * v2.y - v0.y * v2.x) / det;
        if u >= 0. && v >= 0. && u + v <= 1. {
            let p = vertices[t[0]] * (1. - u - v) + vertices[t[1]] * u + vertices[t[2]] * v;
            let z = p.dot(view);
            if z > depth {
                depth = z;
                hit = Some((
                    p,
                    (normals[t[0]] * (1. - u - v) + normals[t[1]] * u + normals[t[2]] * v).norm(),
                ));
            }
        }
    }
    hit
}

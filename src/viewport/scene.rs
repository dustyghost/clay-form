//! Orthographic rendering and surface picking. No document edits or input handling.
use crate::{
    camera::Camera,
    document::Document,
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
    all_faces: Vec<(usize, f32)>,
    owners: Vec<usize>,
    active: usize,
    pub(super) center: Pos2,
    pub(super) right: Vec3,
    pub(super) up: Vec3,
}
impl Scene {
    pub fn paint(
        painter: &egui::Painter,
        rect: egui::Rect,
        document: &Document,
        camera: &Camera,
        wire: bool,
    ) -> Self {
        let active = document.active_model_index();
        let mut mesh = Mesh {
            vertices: Vec::new(),
            triangles: Vec::new(),
            mask: Vec::new(),
        };
        let mut owners = Vec::new();
        let mut active_vertices = 0;
        for index in
            std::iter::once(active).chain((0..document.model_count()).filter(|i| *i != active))
        {
            let model = document.model(index).unwrap();
            if !model.metadata.visible {
                continue;
            }
            let surface = model.display_mesh(index == active);
            let offset = mesh.vertices.len();
            mesh.vertices.extend_from_slice(&surface.vertices);
            mesh.mask.extend_from_slice(&surface.mask);
            mesh.triangles
                .extend(surface.triangles.iter().map(|t| t.map(|i| i + offset)));
            owners.extend(std::iter::repeat_n(index, surface.triangles.len()));
            if index == active {
                active_vertices = surface.vertices.len();
            }
        }
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
            let mask = if i < active_vertices {
                1. - mesh.mask[i] * 0.75
            } else {
                1.
            };
            let base = if i < active_vertices {
                [190., 155., 125.]
            } else {
                [130., 143., 155.]
            };
            rendered.colored_vertex(
                project(*v),
                Color32::from_rgb(
                    (base[0] * illumination * mask) as u8,
                    (base[1] * illumination * mask) as u8,
                    (base[2] * illumination * mask) as u8,
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
            faces: faces
                .iter()
                .copied()
                .filter(|(i, _)| owners[*i] == active)
                .collect(),
            all_faces: faces,
            owners,
            active,
            normals,
            view,
            center,
            right,
            up,
        }
    }
    pub fn project(&self, point: Vec3) -> Pos2 {
        Pos2::new(
            self.center.x + point.dot(self.right) * self.scale,
            self.center.y - point.dot(self.up) * self.scale,
        )
    }
    pub fn pick_model(&self, point: Pos2) -> Option<(usize, Vec3, Vec3)> {
        let (p, normal, face) = pick_surface(
            &self.vertices,
            &self.triangles,
            &self.positions,
            &self.all_faces,
            &self.normals,
            self.view,
            point,
        )?;
        Some((self.owners[face], p, normal))
    }
    pub fn pick(&self, point: Pos2) -> Option<(Vec3, Vec3)> {
        self.pick_model(point)
            .filter(|(owner, _, _)| *owner == self.active)
            .map(|(_, p, n)| (p, n))
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
) -> Option<(Vec3, Vec3, usize)> {
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
                    fi,
                ));
            }
        }
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Transform, Vec3};
    fn scene(document: &Document) -> Scene {
        let context = egui::Context::default();
        let mut scene = None;
        let _ = context.run(egui::RawInput::default(), |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            scene = Some(Scene::paint(
                &painter,
                egui::Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(800.)),
                document,
                &Camera::default(),
                false,
            ));
        });
        scene.unwrap()
    }
    #[test]
    fn inactive_foreground_occludes_active_picks_and_hidden_models_disappear() {
        let mut document = Document::default();
        document.duplicate_model(false).unwrap();
        document
            .apply_transform(
                Transform {
                    translation: Vec3(0., 0., 2.),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        document.end_stroke();
        document.select_submodel(1).unwrap();
        let view = scene(&document);
        assert_eq!(view.pick_model(view.center).unwrap().0, 0);
        assert!(view.pick(view.center).is_none());
        let model = document.model(0).unwrap();
        let name = model.name.to_string();
        let mut metadata = model.metadata.clone();
        metadata.visible = false;
        document.set_model_metadata(0, name, metadata).unwrap();
        let view = scene(&document);
        assert_eq!(view.owners.len(), document.mesh().triangles.len());
        assert!(view.pick(view.center).is_some());
    }
    #[test]
    fn inactive_display_uses_lowest_level_without_changing_stored_detail() {
        let mut document = Document::default();
        let coarse_count = document.mesh().triangles.len();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document.replace_geometry(geometry).unwrap();
        document.duplicate_model(false).unwrap();
        let original = document.geometry_snapshot();
        let view = scene(&document);
        assert_eq!(view.owners.len(), coarse_count * 5);
        assert_eq!(
            view.faces.len(),
            view.all_faces
                .iter()
                .filter(|(i, _)| view.owners[*i] == 0)
                .count()
        );
        assert_eq!(document.geometry_snapshot(), original);
    }
}

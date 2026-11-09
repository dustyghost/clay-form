//! A shared orthographic thumbnail of the active model, with each route overlaid.
use super::*;
use crate::sculpt::{Mesh, Vec3, paths::SurfacePath};
use std::sync::Arc;

const WIDTH: usize = 80;
const HEIGHT: usize = 56;

pub(super) struct Preview {
    revision: u64,
    model: u64,
    center: Vec2,
    scale: f32,
    depth: Vec<f32>,
    texture: egui::TextureHandle,
}

fn view(p: Vec3) -> Vec3 {
    p.rotated(Vec3(0., 1., 0.), -0.46)
        .rotated(Vec3(1., 0., 0.), 0.18)
}

pub(super) fn active_model(ui: &egui::Ui, document: &Document) -> Arc<Preview> {
    let key = egui::Id::new("active_path_thumbnail");
    let revision = document.display_revision();
    let model = document.active_model_id();
    if let Some(preview) = ui.ctx().data(|data| data.get_temp::<Arc<Preview>>(key))
        && preview.revision == revision
        && preview.model == model
    {
        return preview;
    }
    let (image, depth, center, scale) = raster(document.mesh());
    let texture =
        ui.ctx()
            .load_texture("path thumbnail model", image, egui::TextureOptions::LINEAR);
    let preview = Arc::new(Preview {
        revision,
        model,
        center,
        scale,
        depth,
        texture,
    });
    ui.ctx()
        .data_mut(|data| data.insert_temp(key, preview.clone()));
    preview
}

fn raster(mesh: &Mesh) -> (egui::ColorImage, Vec<f32>, Vec2, f32) {
    let points: Vec<_> = mesh.vertices.iter().map(|&p| view(p)).collect();
    let mut bounds = egui::Rect::NOTHING;
    for &Vec3(x, y, _) in &points {
        bounds.extend_with(egui::pos2(x, -y));
    }
    let scale = if bounds.is_finite() {
        ((WIDTH - 8) as f32 / bounds.width().max(1e-6))
            .min((HEIGHT - 8) as f32 / bounds.height().max(1e-6))
    } else {
        1.
    };
    let center = if bounds.is_finite() {
        egui::vec2(WIDTH as f32 / 2., HEIGHT as f32 / 2.) - bounds.center().to_vec2() * scale
    } else {
        egui::vec2(WIDTH as f32 / 2., HEIGHT as f32 / 2.)
    };
    let mut image = egui::ColorImage::filled([WIDTH, HEIGHT], Color32::from_rgb(26, 30, 36));
    let mut depth = vec![f32::NEG_INFINITY; WIDTH * HEIGHT];
    for (face, &[a, b, c]) in mesh.triangles.iter().enumerate() {
        if !mesh.face_visible(face) {
            continue;
        }
        let [a, b, c] = [points[a], points[b], points[c]];
        let normal = (b - a).cross(c - a).norm();
        let [pa, pb, pc] = [a, b, c].map(|Vec3(x, y, _)| center + egui::vec2(x, -y) * scale);
        let edge = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
        let area = edge(pb - pa, pc - pa);
        if area.abs() < 1e-5 {
            continue;
        }
        let lo = pa.min(pb).min(pc).floor().max(Vec2::ZERO);
        let hi = pa
            .max(pb)
            .max(pc)
            .ceil()
            .min(egui::vec2((WIDTH - 1) as f32, (HEIGHT - 1) as f32));
        let shade = (0.42 + 0.53 * normal.dot(Vec3(-0.28, 0.45, 0.85)).abs()).min(1.);
        let color = Color32::from_rgb(
            (126. * shade) as u8,
            (148. * shade) as u8,
            (159. * shade) as u8,
        );
        for y in lo.y as usize..=hi.y as usize {
            for x in lo.x as usize..=hi.x as usize {
                let p = egui::vec2(x as f32 + 0.5, y as f32 + 0.5);
                let u = edge(p - pa, pc - pa) / area;
                let v = edge(pb - pa, p - pa) / area;
                if u < 0. || v < 0. || u + v > 1. {
                    continue;
                }
                let z = a.2 * (1. - u - v) + b.2 * u + c.2 * v;
                let index = y * WIDTH + x;
                if z > depth[index] {
                    depth[index] = z;
                    image.pixels[index] = color;
                }
            }
        }
    }
    (image, depth, center, scale)
}

impl Preview {
    pub(super) fn show(&self, ui: &mut egui::Ui, path: &SurfacePath) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(WIDTH as f32, HEIGHT as f32),
            egui::Sense::click(),
        );
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let painter = ui.painter().with_clip_rect(rect);
        painter.image(
            self.texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
        let curve = path.dense_curve();
        if curve.len() < 2 {
            return response;
        }
        let stride = curve.len().div_ceil(512).max(1);
        let project = |p| {
            let Vec3(x, y, z) = view(p);
            (self.center + egui::vec2(x, -y) * self.scale, z)
        };
        let mut last = project(curve[0].position);
        for i in (stride..curve.len() + stride - 1).step_by(stride) {
            let next = project(curve[i.min(curve.len() - 1)].position);
            let mid = (last.0 + next.0) * 0.5;
            let hidden = (0. ..WIDTH as f32).contains(&mid.x)
                && (0. ..HEIGHT as f32).contains(&mid.y)
                && self.depth[mid.y as usize * WIDTH + mid.x as usize]
                    > (last.1 + next.1) * 0.5 + 1.5 / self.scale;
            let color = if hidden {
                Color32::from_rgb(124, 165, 157)
            } else {
                Color32::from_rgb(123, 255, 210)
            };
            painter.line_segment(
                [rect.min + last.0, rect.min + next.0],
                egui::Stroke::new(1.6_f32, color),
            );
            last = next;
        }
        response.on_hover_text(&path.name)
    }
}

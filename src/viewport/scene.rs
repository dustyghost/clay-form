//! Camera projection, rendering and surface picking. No document edits or input handling.
use super::gpu::{DisplayMesh, Renderer};
use crate::{camera::Camera, document::Document, sculpt::Vec3};
use eframe::egui::{self, Color32, Pos2};
use std::sync::Arc;
pub(super) struct Scene {
    pub scale: f32,
    pub(super) eye_distance: f32,
    pub(super) rect: egui::Rect,
    pub(super) active_bounds: egui::Rect,
    positions: Vec<Pos2>,
    pub(super) geometry: Arc<DisplayMesh>,
    pub(super) faces: Vec<(usize, f32)>,
    pub(super) view: Vec3,
    all_faces: Vec<(usize, f32)>,
    active: usize,
    pub(super) center: Pos2,
    pub(super) right: Vec3,
    pub(super) up: Vec3,
}
impl Scene {
    #[cfg(test)]
    pub fn paint(
        painter: &egui::Painter,
        rect: egui::Rect,
        document: &Document,
        camera: &Camera,
        wire: bool,
        mode: super::WireMode,
        show_triangles: bool,
    ) -> Self {
        Self::paint_with_renderer(
            painter,
            rect,
            document,
            camera,
            wire,
            mode,
            show_triangles,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_with_renderer(
        painter: &egui::Painter,
        rect: egui::Rect,
        document: &Document,
        camera: &Camera,
        wire: bool,
        mode: super::WireMode,
        show_triangles: bool,
        mut renderer: Option<&mut Renderer>,
    ) -> Self {
        let geometry = if let Some(renderer) = renderer.as_mut() {
            renderer.geometry(document)
        } else {
            Arc::new(DisplayMesh::new(document))
        };
        let scene = Self::from_geometry(rect, camera, geometry, renderer.is_none());
        scene.paint_geometry(
            painter,
            renderer.as_deref(),
            wire,
            mode,
            show_triangles,
            None,
        );
        scene
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_with_stroke(
        painter: &egui::Painter,
        rect: egui::Rect,
        document: &Document,
        camera: &Camera,
        wire: bool,
        mode: super::WireMode,
        show_triangles: bool,
        mut renderer: Option<&mut Renderer>,
        stroke: Option<&super::stroke::PendingStroke>,
        model_preview: Option<&crate::sculpt::Mesh>,
    ) -> Self {
        if let Some(mesh) = model_preview {
            let geometry = if let Some(renderer) = renderer.as_mut() {
                renderer.geometry(document)
            } else {
                Arc::new(DisplayMesh::new(document))
            };
            let scene = Self::from_geometry(rect, camera, geometry, renderer.is_none());
            let display = Arc::new(DisplayMesh::with_active_mesh(document, mesh));
            let preview = Self::from_geometry(rect, camera, display.clone(), renderer.is_none());
            preview.paint_geometry(
                painter,
                renderer.as_deref(),
                wire,
                mode,
                show_triangles,
                Some(display),
            );
            return scene;
        }
        let Some(stroke) = stroke.filter(|s| s.is_stamp()) else {
            return Self::paint_with_renderer(
                painter,
                rect,
                document,
                camera,
                wire,
                mode,
                show_triangles,
                renderer,
            );
        };
        let geometry = if let Some(renderer) = renderer.as_mut() {
            renderer.geometry(document)
        } else {
            Arc::new(DisplayMesh::new(document))
        };
        // Pick against the original surface so moving/resizing never builds up a stamp.
        let scene = Self::from_geometry(rect, camera, geometry, renderer.is_none());
        let display = stroke
            .preview
            .display(stroke, document, &scene, painter.ctx());
        let preview = Self::from_geometry(rect, camera, display.clone(), renderer.is_none());
        preview.paint_geometry(
            painter,
            renderer.as_deref(),
            wire,
            mode,
            show_triangles,
            Some(display),
        );
        scene
    }
    pub(super) fn from_geometry(
        rect: egui::Rect,
        camera: &Camera,
        geometry: Arc<DisplayMesh>,
        sort_faces: bool,
    ) -> Self {
        let mesh = &geometry.mesh;
        let active = geometry.active;
        let active_vertices = geometry.active_vertices;
        let owners = &geometry.owners;
        let center = rect.center() + camera.pan;
        let scale = rect.width().min(rect.height()) * 0.35 * camera.zoom;
        let (right, up, view) = camera.basis();
        let eye_distance = camera.eye_distance();
        let project = |v: Vec3| {
            let scale = if eye_distance == 0. {
                scale
            } else {
                scale * eye_distance / (eye_distance - v.dot(view)).max(eye_distance * 0.001)
            };
            Pos2::new(
                center.x + v.dot(right) * scale,
                center.y - v.dot(up) * scale,
            )
        };
        let positions: Vec<_> = mesh.vertices.iter().copied().map(project).collect();
        let mut faces: Vec<_> = mesh
            .triangles
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                let a = mesh.vertices[t[0]];
                let b = mesh.vertices[t[1]];
                let c = mesh.vertices[t[2]];
                (b - a).cross(c - a).dot(if eye_distance == 0. {
                    view
                } else {
                    view * eye_distance - a
                }) > 0.
            })
            .map(|(i, t)| {
                (
                    i,
                    (mesh.vertices[t[0]] + mesh.vertices[t[1]] + mesh.vertices[t[2]]).dot(view),
                )
            })
            .collect();
        if sort_faces {
            faces.sort_by(|a, b| a.1.total_cmp(&b.1));
        }
        let active_bounds = positions
            .iter()
            .take(active_vertices)
            .fold(egui::Rect::NOTHING, |bounds, &p| {
                bounds.union(egui::Rect::from_min_max(p, p))
            });
        Self {
            rect,
            active_bounds,
            scale,
            eye_distance,
            positions,
            geometry: geometry.clone(),
            faces: faces
                .iter()
                .copied()
                .filter(|(i, _)| owners[*i] == active)
                .collect(),
            all_faces: faces,
            active,
            view,
            center,
            right,
            up,
        }
    }
    fn paint_geometry(
        &self,
        painter: &egui::Painter,
        renderer: Option<&Renderer>,
        wire: bool,
        mode: super::WireMode,
        show_triangles: bool,
        render_geometry: Option<Arc<DisplayMesh>>,
    ) {
        let rect = self.rect;
        let scale = self.scale;
        let right = self.right;
        let up = self.up;
        let view = self.view;
        let geometry = &self.geometry;
        let mesh = &geometry.mesh;
        let outlines = &geometry.outlines;
        let active_vertices = geometry.active_vertices;
        let positions = &self.positions;
        let faces: Vec<_> = self
            .all_faces
            .iter()
            .copied()
            .filter(|(i, _)| {
                mesh.triangles[*i]
                    .iter()
                    .all(|&v| self.visible_depth(mesh.vertices[v]))
            })
            .collect();
        let faces = &faces;
        let project = |v| self.project(v);
        painter.rect_filled(
            rect,
            0.,
            crate::preferences::viewport_background(painter.ctx()),
        );
        if renderer.is_none() {
            let lighting = crate::lighting::Lighting::current(painter.ctx());
            let mut rendered = egui::Mesh::default();
            for &(i, normal) in geometry.shading.vertices.iter() {
                let v = mesh.vertices[i];
                let mask = if i < active_vertices {
                    1. - mesh.mask[i] * 0.75
                } else {
                    1.
                };
                let base = geometry
                    .material(i)
                    .lit_color_many(
                        normal,
                        (view
                            - v * (if self.eye_distance > 0. {
                                1. / self.eye_distance
                            } else {
                                0.
                            }))
                        .norm(),
                        lighting.samples(v, right, up, view),
                        lighting.fill,
                    )
                    .map(|c| c * 255.);
                rendered.colored_vertex(
                    project(v),
                    Color32::from_rgb(
                        (base[0] * mask) as u8,
                        (base[1] * mask) as u8,
                        (base[2] * mask) as u8,
                    ),
                );
            }
            use super::WireMode;
            let edge_key = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
            let mut hidden = std::collections::HashSet::new();
            if !show_triangles {
                for &[a, b, c, d] in &mesh.quads {
                    hidden.insert(edge_key(a, c));
                    hidden.insert(edge_key(b, d));
                }
            }
            // A viewport-sized depth buffer also handles intersecting submodels and
            // partial edge occlusion, independently of the surface painter order.
            let width = rect.width().ceil().max(1.) as usize;
            let height = rect.height().ceil().max(1.) as usize;
            let mut depth = Vec::new();
            if wire && mode == WireMode::Visible {
                depth.resize(width * height, f32::NEG_INFINITY);
                for &(i, _) in faces {
                    let t = mesh.triangles[i];
                    let p = t.map(|v| positions[v] - rect.min);
                    let z = t.map(|v| self.screen_depth(mesh.vertices[v]));
                    let det = (p[1].x - p[0].x) * (p[2].y - p[0].y)
                        - (p[1].y - p[0].y) * (p[2].x - p[0].x);
                    if det.abs() < 1e-7 {
                        continue;
                    }
                    let min_x = p
                        .iter()
                        .map(|p| p.x)
                        .fold(f32::INFINITY, f32::min)
                        .floor()
                        .max(0.) as usize;
                    let max_x = (p
                        .iter()
                        .map(|p| p.x)
                        .fold(f32::NEG_INFINITY, f32::max)
                        .ceil()
                        .max(0.) as usize)
                        .min(width - 1);
                    let min_y = p
                        .iter()
                        .map(|p| p.y)
                        .fold(f32::INFINITY, f32::min)
                        .floor()
                        .max(0.) as usize;
                    let max_y = (p
                        .iter()
                        .map(|p| p.y)
                        .fold(f32::NEG_INFINITY, f32::max)
                        .ceil()
                        .max(0.) as usize)
                        .min(height - 1);
                    for y in min_y..=max_y {
                        for x in min_x..=max_x {
                            let dx = x as f32 + 0.5 - p[0].x;
                            let dy = y as f32 + 0.5 - p[0].y;
                            let u = (dx * (p[2].y - p[0].y) - dy * (p[2].x - p[0].x)) / det;
                            let v = ((p[1].x - p[0].x) * dy - (p[1].y - p[0].y) * dx) / det;
                            if u >= 0. && v >= 0. && u + v <= 1. {
                                let d = z[0] * (1. - u - v) + z[1] * u + z[2] * v;
                                depth[y * width + x] = depth[y * width + x].max(d);
                            }
                        }
                    }
                }
            }
            let mut seen_edges = std::collections::HashSet::new();
            let mut draw_edges = |i: usize| {
                let t = mesh.triangles[i];
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    if !hidden.contains(&edge_key(a, b)) && seen_edges.insert(edge_key(a, b)) {
                        let stroke = egui::Stroke::new(
                            0.7_f32,
                            if let Some(&color) = geometry.edge_colors.get(&edge_key(a, b)) {
                                color
                            } else if mode == WireMode::Transparent {
                                Color32::from_rgb(170, 185, 200)
                            } else {
                                Color32::from_black_alpha(110)
                            },
                        );
                        if depth.is_empty() {
                            painter.line_segment([positions[a], positions[b]], stroke);
                        } else {
                            let pa = positions[a];
                            let pb = positions[b];
                            let za = self.screen_depth(mesh.vertices[a]);
                            let zb = self.screen_depth(mesh.vertices[b]);
                            let steps = (pb - pa).length().ceil().max(1.) as usize;
                            let mut start = None;
                            for step in 0..=steps {
                                let t = step as f32 / steps as f32;
                                let p = pa + (pb - pa) * t;
                                let x = (p.x - rect.min.x).floor() as isize;
                                let y = (p.y - rect.min.y).floor() as isize;
                                let visible = x >= 0
                                    && y >= 0
                                    && (x as usize) < width
                                    && (y as usize) < height
                                    && za * (1. - t)
                                        + zb * t
                                        + 2. / scale / self.eye_distance.max(1.).powi(2)
                                        >= depth[y as usize * width + x as usize];
                                if visible {
                                    if start.is_none() {
                                        start = Some(p);
                                    }
                                } else if let Some(from) = start.take() {
                                    painter.line_segment([from, p], stroke);
                                }
                                if step == steps
                                    && let Some(from) = start.take()
                                {
                                    painter.line_segment([from, p], stroke);
                                }
                            }
                        }
                    }
                }
            };
            if !wire || mode != WireMode::Transparent {
                for &(i, _) in faces {
                    rendered
                        .indices
                        .extend(geometry.shading.triangles[i].map(|v| v as u32));
                }
                painter.add(egui::Shape::mesh(rendered));
            }
            if wire {
                if mode == WireMode::Transparent {
                    for i in 0..mesh.triangles.len() {
                        draw_edges(i);
                    }
                } else {
                    for &(i, _) in faces {
                        draw_edges(i);
                    }
                }
            }
        }
        if let Some(renderer) = renderer {
            renderer.paint(
                painter,
                rect,
                self,
                wire,
                mode,
                show_triangles,
                render_geometry,
            );
        }
        for edge in outlines.iter() {
            if let Some(line) = self.clip_segment(edge.map(|i| mesh.vertices[i])) {
                painter.line_segment(
                    line,
                    egui::Stroke::new(2_f32, Color32::from_rgb(190, 155, 125)),
                );
            }
        }
    }
    pub(super) fn brush_cursor(
        &self,
        painter: &egui::Painter,
        pos: Pos2,
        radius: f32,
        brush: &crate::sculpt::Brush,
        color: Color32,
    ) -> bool {
        self.brush_cursor_with_footprint(painter, pos, radius, brush, color, true)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn brush_cursor_with_footprint(
        &self,
        painter: &egui::Painter,
        pos: Pos2,
        radius: f32,
        brush: &crate::sculpt::Brush,
        color: Color32,
        footprint: bool,
    ) -> bool {
        let Some((center, normal)) = self.pick(pos) else {
            return false;
        };
        let world_radius = radius / self.scale_at(center);
        // Sample visible surface points, using the same camera-plane alpha and
        // depth/normal limits as sculpting. Missing samples break the overlay.
        if footprint {
            const N: usize = 17;
            let bounds = egui::Rect::from_center_size(pos, egui::Vec2::splat(radius * 2.));
            let candidates: Vec<_> = self
                .all_faces
                .iter()
                .copied()
                .filter(|(fi, _)| {
                    let [a, b, c] = self.geometry.mesh.triangles[*fi].map(|i| self.positions[i]);
                    let face_bounds = egui::Rect::from_min_max(
                        Pos2::new(a.x.min(b.x).min(c.x), a.y.min(b.y).min(c.y)),
                        Pos2::new(a.x.max(b.x).max(c.x), a.y.max(b.y).max(c.y)),
                    );
                    bounds.intersects(face_bounds)
                })
                .collect();
            let mut samples = vec![None; N * N];
            for y in 0..N {
                for x in 0..N {
                    let u = x as f32 / (N - 1) as f32 * 2. - 1.;
                    let v = 1. - y as f32 / (N - 1) as f32 * 2.;
                    let point = pos + egui::vec2(u, -v) * radius;
                    if let Some((p, n, face)) = pick_surface(self, &candidates, point)
                        && self.geometry.owners[face] == self.active
                        && (p - center).len() <= world_radius * 1.5
                        && n.dot(normal) >= 0.1
                    {
                        let offset = p - center;
                        samples[y * N + x] = Some((
                            point,
                            brush.weight(
                                offset.dot(self.right) / world_radius,
                                offset.dot(self.up) / world_radius,
                            ),
                        ));
                    }
                }
            }
            let mut overlay = egui::Mesh::default();
            for y in 0..N - 1 {
                for x in 0..N - 1 {
                    for ids in [
                        [y * N + x, y * N + x + 1, (y + 1) * N + x],
                        [y * N + x + 1, (y + 1) * N + x + 1, (y + 1) * N + x],
                    ] {
                        if let [Some(a), Some(b), Some(c)] = ids.map(|i| samples[i]) {
                            let start = overlay.vertices.len() as u32;
                            for (point, weight) in [a, b, c] {
                                let opacity =
                                    if matches!(brush.alpha, crate::sculpt::Alpha::Image { .. }) {
                                        weight.clamp(0., 1.).sqrt() * 0.75
                                    } else {
                                        weight * (90. / 255.)
                                    };
                                overlay.colored_vertex(
                                    point,
                                    Color32::from_rgba_unmultiplied(
                                        color.r(),
                                        color.g(),
                                        color.b(),
                                        (opacity * 255.) as u8,
                                    ),
                                );
                            }
                            overlay.add_triangle(start, start + 1, start + 2);
                        }
                    }
                }
            }
            painter.add(egui::Shape::mesh(overlay));
        }
        // A tangent ring and normal make surface inclination legible.
        let tangent = (self.right - normal * self.right.dot(normal)).norm();
        let tangent = if tangent.len() < 0.01 {
            self.up.cross(normal).norm()
        } else {
            tangent
        };
        let bitangent = normal.cross(tangent).norm();
        for fraction in [1., brush.inner_radius] {
            if fraction <= 0. {
                continue;
            }
            let points = (0..=64)
                .map(|i| {
                    let a = i as f32 / 64. * std::f32::consts::TAU;
                    self.project(
                        center
                            + (tangent * a.cos() + bitangent * a.sin()) * (world_radius * fraction),
                    )
                })
                .collect();
            painter.add(egui::Shape::line(points, egui::Stroke::new(1.2_f32, color)));
        }
        let end = self.project(center + normal * (world_radius * 0.6));
        painter.line_segment([pos, end], egui::Stroke::new(2_f32, color));
        painter.circle_filled(end, 3., color);
        true
    }
    pub fn pick_model(&self, point: Pos2) -> Option<(usize, Vec3, Vec3)> {
        let (p, normal, face) = pick_surface(self, &self.all_faces, point)?;
        Some((self.geometry.owners[face], p, normal))
    }
    /// Sample the underlying surface colour, independent of lighting and group overlays.
    #[cfg(test)]
    pub(super) fn pick_colour(&self, point: Pos2) -> Option<[f32; 3]> {
        Some(self.sample_material(point)?.base_color)
    }
    /// Sample stored colour and scalar maps, before lights, exposure or overlays.
    pub(super) fn sample_material(&self, point: Pos2) -> Option<crate::sculpt::Material> {
        let (p, _, face) = pick_surface(self, &self.all_faces, point)?;
        let mesh = &self.geometry.mesh;
        let ids = mesh.triangles[face];
        let [a, b, c] = ids.map(|i| mesh.vertices[i]);
        let (u, v, w) = (b - a, c - a, p - a);
        let (uu, uv, vv, wu, wv) = (u.dot(u), u.dot(v), v.dot(v), w.dot(u), w.dot(v));
        let denominator = uu * vv - uv * uv;
        if denominator <= 1e-20 {
            return Some(mesh.material(ids[0]));
        }
        let y = ((vv * wu - uv * wv) / denominator).clamp(0., 1.);
        let z = ((uu * wv - uv * wu) / denominator).clamp(0., 1.);
        let x = (1. - y - z).max(0.);
        let total = x + y + z;
        let weights = [x / total, y / total, z / total];
        let mut material = crate::sculpt::Material::blend(
            ids.into_iter()
                .zip(weights)
                .map(|(i, weight)| (mesh.material(i), weight)),
        );
        let corners = self.geometry.uv_corners[face];
        if corners[0][2] > 0.5 {
            let uv =
                std::array::from_fn(|c| corners.iter().zip(weights).map(|(p, w)| p[c] * w).sum());
            let maps = corners.map(|corner| self.geometry.sample_texture(corner[3] as usize, uv));
            for c in 0..3 {
                material.base_color[c] =
                    material.base_color[c] * (1. - maps[0][3]) + maps[0][c] * maps[0][3];
            }
            material.roughness =
                (material.roughness * (1. - maps[1][3]) + maps[1][0] * maps[1][3]).clamp(0., 1.);
            material.metallic =
                (material.metallic * (1. - maps[2][3]) + maps[2][0] * maps[2][3]).clamp(0., 1.);
        }
        Some(material)
    }
    pub(super) fn pick_face(&self, point: Pos2) -> Option<usize> {
        let (_, _, face) = pick_surface(self, &self.all_faces, point)?;
        (self.geometry.owners[face] == self.active).then_some(self.geometry.source_faces[face])
    }
    pub fn pick_polygroup(&self, point: Pos2, document: &Document) -> Option<usize> {
        let (_, _, face) = pick_surface(self, &self.all_faces, point)?;
        if self.geometry.owners[face] != self.active {
            return None;
        }
        Some(
            document
                .mesh()
                .polygroups
                .get(self.geometry.source_faces[face])
                .copied()
                .unwrap_or(0),
        )
    }
    pub fn pick(&self, point: Pos2) -> Option<(Vec3, Vec3)> {
        self.pick_model(point)
            .filter(|(owner, _, _)| *owner == self.active)
            .map(|(_, p, n)| (p, n))
    }
}
fn pick_surface(scene: &Scene, faces: &[(usize, f32)], point: Pos2) -> Option<(Vec3, Vec3, usize)> {
    let geometry = &scene.geometry;
    let positions = &scene.positions;
    let view = scene.view;
    let ray = scene.ray(point);
    let vertices = &geometry.mesh.vertices;
    let triangles = &geometry.mesh.triangles;
    let mut hit = None;
    let mut depth = f32::NEG_INFINITY;
    for &(fi, _) in faces {
        let t = triangles[fi];
        if scene.eye_distance > 0. {
            let [a, b, c] = t.map(|i| vertices[i]);
            let e1 = b - a;
            let e2 = c - a;
            let h = ray.1.cross(e2);
            let det = e1.dot(h);
            if det <= 1e-10 {
                continue;
            }
            let offset = ray.0 - a;
            let u = offset.dot(h) / det;
            let q = offset.cross(e1);
            let v = ray.1.dot(q) / det;
            if u < 0. || v < 0. || u + v > 1. {
                continue;
            }
            let p = a * (1. - u - v) + b * u + c * v;
            if !scene.visible_depth(p) {
                continue;
            }
            let z = p.dot(view);
            if z > depth {
                depth = z;
                let normals =
                    geometry.shading.triangles[fi].map(|i| geometry.shading.vertices[i].1);
                hit = Some((
                    p,
                    (normals[0] * (1. - u - v) + normals[1] * u + normals[2] * v).norm(),
                    fi,
                ));
            }
            continue;
        }
        let a = positions[t[0]];
        let b = positions[t[1]];
        let c = positions[t[2]];
        if point.x < a.x.min(b.x).min(c.x)
            || point.x > a.x.max(b.x).max(c.x)
            || point.y < a.y.min(b.y).min(c.y)
            || point.y > a.y.max(b.y).max(c.y)
        {
            continue;
        }
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
                let normals =
                    geometry.shading.triangles[fi].map(|i| geometry.shading.vertices[i].1);
                hit = Some((
                    p,
                    (normals[0] * (1. - u - v) + normals[1] * u + normals[2] * v).norm(),
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
    fn wire_shapes(
        document: &Document,
        mode: super::super::WireMode,
        triangles: bool,
    ) -> Vec<egui::epaint::ClippedShape> {
        let context = egui::Context::default();
        context
            .run(egui::RawInput::default(), |ctx| {
                Scene::paint(
                    &ctx.layer_painter(egui::LayerId::background()),
                    egui::Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(800.)),
                    document,
                    &Camera::default(),
                    true,
                    mode,
                    triangles,
                );
            })
            .shapes
    }
    fn line_count(shapes: &[egui::epaint::ClippedShape]) -> usize {
        shapes
            .iter()
            .filter(|s| matches!(s.shape, egui::Shape::LineSegment { .. }))
            .count()
    }
    #[test]
    fn wire_modes_hide_occluded_edges_and_transparency_removes_surfaces() {
        use super::super::WireMode;
        use crate::sculpt::{Geometry, Primitive, PrimitiveKind};
        let mut p = Primitive::for_kind(PrimitiveKind::Plane);
        p.divisions = [1; 3];
        let mut document = Document::default();
        document
            .replace_geometry(Geometry::new(p.mesh().unwrap()))
            .unwrap();
        assert_eq!(
            line_count(&wire_shapes(&document, WireMode::Front, false)),
            4
        );
        assert_eq!(
            line_count(&wire_shapes(&document, WireMode::Front, true)),
            5
        );
        document.duplicate_model(false).unwrap();
        document
            .apply_transform(
                Transform {
                    translation: Vec3(0., 0., 2.),
                    scale: Vec3(1.2, 1.2, 1.2),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        document.end_stroke();
        let front = wire_shapes(&document, WireMode::Front, false);
        let visible = wire_shapes(&document, WireMode::Visible, false);
        assert_eq!(line_count(&front), 8);
        assert_eq!(line_count(&visible), 4);
        let transparent = wire_shapes(&document, WireMode::Transparent, false);
        assert_eq!(line_count(&transparent), 8);
        assert!(
            !transparent
                .iter()
                .any(|s| matches!(s.shape, egui::Shape::Mesh(_)))
        );
        let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
        cube.divisions = [1; 3];
        document
            .replace_geometry(Geometry::new(cube.mesh().unwrap()))
            .unwrap();
        assert!(
            line_count(&wire_shapes(&document, WireMode::Transparent, false))
                > line_count(&wire_shapes(&document, WireMode::Front, false))
        );
    }
    #[test]
    fn hiding_a_polygroup_removes_picking_and_wire_edges_without_changing_mesh() {
        let mut d = Document::default();
        let original = d.mesh().clone();
        assert!(scene(&d).pick(egui::pos2(400., 400.)).is_some());
        d.set_vertex_group_visible(0, false).unwrap();
        let hidden = scene(&d);
        assert!(hidden.geometry.mesh.triangles.is_empty());
        assert!(hidden.pick(egui::pos2(400., 400.)).is_none());
        assert!(hidden.pick_model(egui::pos2(400., 400.)).is_none());
        assert_eq!(
            line_count(&wire_shapes(&d, super::super::WireMode::Transparent, true)),
            0
        );
        assert_eq!(d.mesh().vertices, original.vertices);
        assert_eq!(d.mesh().triangles, original.triangles);
        d.set_vertex_group_visible(0, true).unwrap();
        assert!(scene(&d).pick(egui::pos2(400., 400.)).is_some());
    }
    #[test]
    fn rectangle_visibility_filters_faces_picking_and_quad_wire_edges() {
        let mut plane = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
        plane.divisions = [2, 1, 1];
        let mut d = Document::default();
        d.replace_geometry(crate::sculpt::Geometry::new(plane.mesh().unwrap()))
            .unwrap();
        d.select_face_region(|p| p.0 > 0., true).unwrap();
        let view = scene(&d);
        assert_eq!(view.geometry.mesh.triangles.len(), 2);
        assert_eq!(view.geometry.mesh.quads.len(), 1);
        let left = view.project(Vec3(-0.25, 0., 0.));
        let right = view.project(Vec3(0.25, 0., 0.));
        assert!(view.pick(left).is_some());
        assert!(view.pick(right).is_none());
        assert_eq!(view.pick_polygroup(left, &d), Some(0));
        assert_eq!(
            line_count(&wire_shapes(&d, super::super::WireMode::Transparent, false)),
            4
        );
        d.show_all_faces().unwrap();
        assert!(scene(&d).pick(right).is_some());
    }
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
                super::super::WireMode::Visible,
                false,
            ));
        });
        scene.unwrap()
    }
    #[test]
    fn material_sampler_interpolates_surface_colours_without_display_shading() {
        use crate::sculpt::{Geometry, Material, Primitive, PrimitiveKind};
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [2, 2, 1];
        let mut mesh = plane.mesh().unwrap();
        mesh.materials = mesh
            .vertices
            .iter()
            .map(|p| Material {
                base_color: [(p.0 + 1.) * 0.5, 0.2, 0.4],
                ..Default::default()
            })
            .collect();
        mesh.mask.fill(1.);
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(mesh)).unwrap();
        let view = scene(&d);
        let p = Vec3(0.25, 0., 0.);
        let sampled = view.pick_colour(view.project(p)).unwrap();
        for (a, b) in sampled.into_iter().zip([0.625, 0.2, 0.4]) {
            assert!((a - b).abs() < 1e-5);
        }
        d.select_face_region(|_| true, true).unwrap();
        assert!(scene(&d).pick_colour(view.project(p)).is_none());
    }
    #[test]
    fn sampler_filters_hdr_colour_by_coverage_and_reads_scalar_maps() {
        use crate::sculpt::{
            Geometry, Material, Primitive, PrimitiveKind,
            texture::{Channel, Precision, Texture},
        };
        let material = Material {
            base_color: [0.2, 0.3, 0.4],
            roughness: 0.8,
            metallic: 0.1,
            ..Default::default()
        };
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        mesh.materials = vec![material.clone(); mesh.vertices.len()];
        mesh.mask.fill(1.);
        mesh.uv = mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
        mesh.uv.corners.fill([[0.5, 0.5]; 3]);
        for channel in Channel::ALL {
            let mut texture =
                Texture::new_with_precision(channel, 16, &material, Precision::Float32).unwrap();
            for i in 0..256 {
                let value = match channel {
                    Channel::Color if i % 16 < 8 => [2., 0., 0., 1.],
                    Channel::Color => [0., 0., 10., 0.],
                    Channel::Roughness => [0.25, 0.25, 0.25, 0.5],
                    Channel::Metallic => [0.75, 0.75, 0.75, 1.],
                };
                texture.pixels.set_pixel(i, value);
            }
            mesh.uv.textures.push(texture);
        }
        let mut document = Document::default();
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        let view = scene(&document);
        let sample = view
            .sample_material(view.project(Vec3(0.1, 0.1, 0.)))
            .unwrap();
        for (actual, expected) in sample.base_color.into_iter().zip([1.1, 0.15, 0.2]) {
            assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
        }
        assert!((sample.roughness - 0.525).abs() < 1e-5);
        assert!((sample.metallic - 0.75).abs() < 1e-5);
    }
    #[test]
    fn surface_cursor_renders_on_clay_and_falls_back_off_surface() {
        let document = Document::default();
        let view = scene(&document);
        let context = egui::Context::default();
        let mut brush = crate::sculpt::Brush::new(crate::sculpt::Kind::Standard);
        brush.inner_radius = 0.5;
        let output = context.run(egui::RawInput::default(), |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            assert!(view.brush_cursor(&painter, view.center, 60., &brush, Color32::WHITE));
            assert!(!view.brush_cursor(
                &painter,
                Pos2::new(-1000., -1000.),
                60.,
                &brush,
                Color32::WHITE
            ));
        });
        assert!(
            output
                .shapes
                .iter()
                .any(|s| matches!(s.shape, egui::Shape::Mesh(_)))
        );
        assert!(
            output
                .shapes
                .iter()
                .any(|s| matches!(s.shape, egui::Shape::Path(_)))
        );
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
        assert_eq!(view.geometry.owners.len(), document.mesh().triangles.len());
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
        assert_eq!(view.geometry.owners.len(), coarse_count * 5);
        assert_eq!(
            view.faces.len(),
            view.all_faces
                .iter()
                .filter(|(i, _)| view.geometry.owners[*i] == 0)
                .count()
        );
        assert_eq!(document.geometry_snapshot(), original);
    }
}

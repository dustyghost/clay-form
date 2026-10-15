//! Cached display geometry and camera-independent wire topology.
use crate::{
    document::Document,
    sculpt::{Mesh, SurfaceShading, Vec3},
};
use std::collections::HashMap;

pub(crate) struct DisplayMesh {
    pub mesh: Mesh,
    pub shading: SurfaceShading,
    pub owners: Vec<usize>,
    pub source_faces: Vec<usize>,
    pub poly_colors: Vec<[u8; 4]>,
    pub edge_colors: HashMap<(usize, usize), eframe::egui::Color32>,
    pub outlines: Vec<[usize; 2]>,
    pub active_vertices: usize,
    pub active: usize,
    pub(super) depth_extent: f32,
}
impl DisplayMesh {
    pub fn new(document: &Document) -> Self {
        Self::build(document, false, None)
    }
    pub(super) fn preview(document: &Document) -> Self {
        Self::build(document, true, None)
    }
    pub fn with_active_mesh(document: &Document, mesh: &Mesh) -> Self {
        Self::build(document, true, Some(mesh))
    }
    fn build(document: &Document, preview: bool, active_mesh: Option<&Mesh>) -> Self {
        let active = document.active_model_index();
        let mut mesh = Mesh {
            creases: Vec::new(),
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
            quads: Vec::new(),
            mask: Vec::new(),
        };
        let mut edge_colors = HashMap::new();
        let mut shading = SurfaceShading::default();
        let mut owners = Vec::new();
        let mut source_faces = Vec::new();
        let mut poly_colors = Vec::new();
        let mut outlines = Vec::new();
        let mut active_vertices = 0;
        for index in
            std::iter::once(active).chain((0..document.model_count()).filter(|i| *i != active))
        {
            let model = document.model(index).unwrap();
            if !model.metadata.visible {
                continue;
            }
            let original = if index == active {
                active_mesh.unwrap_or_else(|| model.display_mesh(true))
            } else {
                model.display_mesh(false)
            };
            let shell = if preview && model.metadata.temporary_thickness > 0. {
                original.solidify_with_metadata(model.metadata).ok()
            } else {
                None
            };
            let surface = shell.as_ref().unwrap_or(original);
            let offset = mesh.vertices.len();
            if surface.triangles.is_empty() && surface.vertices.len() > 1 {
                for i in 0..surface.vertices.len() {
                    outlines.push([offset + i, offset + (i + 1) % surface.vertices.len()]);
                }
            }
            let key = |mut t: [usize; 3]| {
                t.sort_unstable();
                t
            };
            let has_hidden = surface.has_hidden_faces();
            let visible_faces: std::collections::HashSet<_> = if has_hidden {
                surface
                    .triangles
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| surface.face_visible(*i))
                    .map(|(_, &t)| key(t))
                    .collect()
            } else {
                std::collections::HashSet::new()
            };
            mesh.quads.extend(
                surface
                    .quads
                    .iter()
                    .filter(|&&[a, b, c, d]| {
                        !has_hidden
                            || (visible_faces.contains(&key([a, b, c]))
                                && visible_faces.contains(&key([a, c, d])))
                            || (visible_faces.contains(&key([a, b, d]))
                                && visible_faces.contains(&key([b, c, d])))
                    })
                    .map(|q| q.map(|i| i + offset)),
            );
            for &[a, b] in &surface.creases {
                edge_colors.insert(
                    (a + offset, b + offset),
                    eframe::egui::Color32::from_rgb(255, 170, 65),
                );
            }
            mesh.vertices.extend_from_slice(&surface.vertices);
            mesh.mask.extend_from_slice(&surface.mask);
            let surface_shading = surface.surface_shading();
            let shading_offset = shading.vertices.len();
            shading.vertices.extend(
                surface_shading
                    .vertices
                    .iter()
                    .map(|&(i, n)| (i + offset, n)),
            );
            for (face, &triangle) in surface.triangles.iter().enumerate() {
                if !surface.face_visible(face) {
                    continue;
                }
                mesh.triangles.push(triangle.map(|i| i + offset));
                shading
                    .triangles
                    .push(surface_shading.triangles[face].map(|i| i + shading_offset));
                if index == active
                    && let Some(primitive) = &model.settings.primitive
                {
                    let [a, b, c] = triangle;
                    for (a, b) in [(a, b), (b, c), (c, a)] {
                        if let Some(axis) =
                            span_edge_axis(primitive, surface.vertices[a], surface.vertices[b])
                        {
                            let key = if a < b {
                                (a + offset, b + offset)
                            } else {
                                (b + offset, a + offset)
                            };
                            edge_colors.insert(key, super::super::controls::AXES[axis]);
                        }
                    }
                }
                owners.push(index);
                source_faces.push(face);
                let id = surface.polygroups.get(face).copied().unwrap_or(0);
                poly_colors.push(if id == 0 {
                    [0; 4]
                } else {
                    let c = Mesh::group_color(id - 1).map(|v| (v * 255.) as u8);
                    [c[0], c[1], c[2], 255]
                });
            }
            if index == active {
                active_vertices = surface.vertices.len();
            }
        }
        let depth_extent = mesh.vertices.iter().map(|p| p.len()).fold(1_f32, f32::max) + 1.;
        Self {
            mesh,
            shading,
            owners,
            source_faces,
            poly_colors,
            edge_colors,
            outlines,
            active_vertices,
            active,
            depth_extent,
        }
    }
    pub(super) fn vertices(&self) -> Vec<f32> {
        self.shading
            .vertices
            .iter()
            .flat_map(|&(i, n)| {
                let p = self.mesh.vertices[i];
                let color = if i < self.active_vertices {
                    [190., 155., 125.]
                } else {
                    [130., 143., 155.]
                };
                let mask = if i < self.active_vertices {
                    self.mesh.mask[i]
                } else {
                    -1.
                };
                [
                    p.0,
                    p.1,
                    p.2,
                    n.0,
                    n.1,
                    n.2,
                    color[0] / 255.,
                    color[1] / 255.,
                    color[2] / 255.,
                    mask,
                ]
            })
            .collect()
    }
    /// Adjacent face normals let the shader reject back-facing edges without
    /// rebuilding edge buffers on every camera movement.
    pub(super) fn edges(&self, triangles: bool) -> Vec<f32> {
        let key = |a, b| if a < b { (a, b) } else { (b, a) };
        let mut hidden = std::collections::HashSet::new();
        if !triangles {
            for &[a, b, c, d] in &self.mesh.quads {
                hidden.insert(key(a, c));
                hidden.insert(key(b, d));
            }
        }
        let mut edges: HashMap<(usize, usize), (Vec3, Vec3)> = HashMap::new();
        for &[a, b, c] in &self.mesh.triangles {
            let normal = (self.mesh.vertices[b] - self.mesh.vertices[a])
                .cross(self.mesh.vertices[c] - self.mesh.vertices[a])
                .norm();
            for (i, j) in [(a, b), (b, c), (c, a)] {
                if hidden.contains(&key(i, j)) {
                    continue;
                }
                edges
                    .entry(key(i, j))
                    .and_modify(|normals| normals.1 = normal)
                    .or_insert((normal, normal));
            }
        }
        edges
            .into_iter()
            .flat_map(|((a, b), (n, m))| {
                let color = self
                    .edge_colors
                    .get(&(a, b))
                    .map(|c| {
                        [
                            c.r() as f32 / 255.,
                            c.g() as f32 / 255.,
                            c.b() as f32 / 255.,
                        ]
                    })
                    .unwrap_or([-1.; 3]);
                [a, b].into_iter().flat_map(move |i| {
                    let p = self.mesh.vertices[i];
                    [
                        p.0, p.1, p.2, n.0, n.1, n.2, m.0, m.1, m.2, color[0], color[1], color[2],
                    ]
                })
            })
            .collect()
    }
}

/// Cut lines follow the axis whose span count places them, in primitive-local space.
fn span_edge_axis(p: &crate::sculpt::Primitive, a: Vec3, b: Vec3) -> Option<usize> {
    use crate::sculpt::PrimitiveKind as Kind;
    if !matches!(
        p.kind,
        Kind::Cube | Kind::Plane | Kind::Grid | Kind::Cylinder | Kind::Cone
    ) {
        return None;
    }
    let det = p.basis[0].dot(p.basis[1].cross(p.basis[2]));
    let local = |v: Vec3| {
        let v = v - p.center;
        [
            v.dot(p.basis[1].cross(p.basis[2])) / det * 2. / p.size.0,
            v.dot(p.basis[2].cross(p.basis[0])) / det * 2. / p.size.1,
            v.dot(p.basis[0].cross(p.basis[1])) / det * 2. / p.size.2,
        ]
    };
    let a = local(a);
    let b = local(b);
    if matches!(p.kind, Kind::Cylinder | Kind::Cone) {
        return ((a[1] - b[1]).abs() < 1e-5 && a[1].abs() < 1. - 1e-5).then_some(1);
    }
    if !matches!(p.kind, Kind::Cube | Kind::Plane | Kind::Grid) {
        return None;
    }
    let axes = if p.kind == Kind::Cube { 3 } else { 2 };
    for axis in 0..axes {
        if (a[axis] - b[axis]).abs() < 1e-5 && a[axis].abs() < 1. - 1e-5 {
            return Some(axis);
        }
    }
    // The outer edges are subdivided along their own axis.
    let changing: Vec<_> = (0..axes)
        .filter(|&axis| (a[axis] - b[axis]).abs() > 1e-5)
        .collect();
    (changing.len() == 1).then(|| changing[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Geometry, Primitive, PrimitiveKind};
    #[test]
    fn span_cut_colors_follow_local_axes_after_rotation_and_scaling() {
        let mut p = Primitive::for_kind(PrimitiveKind::Cube);
        p.divisions = [2; 3];
        p.center = Vec3(2., -1., 0.3);
        p.basis = [Vec3(0., 2., 0.), Vec3(-0.7, 0., 0.), Vec3(0., 0., 1.4)];
        for axis in 0..3 {
            let other = (axis + 1) % 3;
            let face = (axis + 2) % 3;
            let mut a = [0.; 3];
            let mut b = [0.; 3];
            a[face] = 1.;
            b[face] = 1.;
            a[other] = -1.;
            b[other] = 1.;
            let point = |v: [f32; 3]| p.control_point(Vec3(v[0], v[1], v[2]));
            assert_eq!(span_edge_axis(&p, point(a), point(b)), Some(axis));
        }
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [2, 2, 1];
        let mut geometry = Geometry::new(plane.mesh().unwrap());
        geometry.settings.primitive = Some(plane);
        let mut document = Document::default();
        document.replace_geometry(geometry).unwrap();
        let display = DisplayMesh::new(&document);
        for (&(a, b), &color) in &display.edge_colors {
            let a = display.mesh.vertices[a];
            let b = display.mesh.vertices[b];
            if a.0.abs() < 1e-6 && b.0.abs() < 1e-6 {
                assert_eq!(color, super::super::super::controls::AXES[0]);
            }
            if a.1.abs() < 1e-6 && b.1.abs() < 1e-6 {
                assert_eq!(color, super::super::super::controls::AXES[1]);
            }
        }
        assert!(
            display
                .edge_colors
                .values()
                .any(|&c| c == super::super::super::controls::AXES[0])
        );
        assert!(
            display
                .edge_colors
                .values()
                .any(|&c| c == super::super::super::controls::AXES[1])
        );
    }
}

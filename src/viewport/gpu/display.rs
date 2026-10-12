//! Cached display geometry and camera-independent wire topology.
use crate::{
    document::Document,
    sculpt::{Mesh, Vec3},
};
use std::collections::HashMap;

pub(crate) struct DisplayMesh {
    pub mesh: Mesh,
    pub normals: Vec<Vec3>,
    pub owners: Vec<usize>,
    pub outlines: Vec<[usize; 2]>,
    pub active_vertices: usize,
    pub active: usize,
    pub(super) depth_extent: f32,
}
impl DisplayMesh {
    pub fn new(document: &Document) -> Self {
        Self::build(document, false)
    }
    pub(super) fn preview(document: &Document) -> Self {
        Self::build(document, true)
    }
    fn build(document: &Document, preview: bool) -> Self {
        let active = document.active_model_index();
        let mut mesh = Mesh {
            vertices: Vec::new(),
            triangles: Vec::new(),
            quads: Vec::new(),
            mask: Vec::new(),
        };
        let mut owners = Vec::new();
        let mut outlines = Vec::new();
        let mut active_vertices = 0;
        for index in
            std::iter::once(active).chain((0..document.model_count()).filter(|i| *i != active))
        {
            let model = document.model(index).unwrap();
            if !model.metadata.visible {
                continue;
            }
            let original = model.display_mesh(index == active);
            let shell = if preview && model.metadata.temporary_thickness > 0. {
                original.solidify(model.metadata.temporary_thickness).ok()
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
            mesh.quads
                .extend(surface.quads.iter().map(|q| q.map(|i| i + offset)));
            mesh.vertices.extend_from_slice(&surface.vertices);
            mesh.mask.extend_from_slice(&surface.mask);
            mesh.triangles
                .extend(surface.triangles.iter().map(|t| t.map(|i| i + offset)));
            owners.extend(std::iter::repeat_n(index, surface.triangles.len()));
            if index == active {
                active_vertices = surface.vertices.len();
            }
        }
        let normals = mesh.normals();
        let depth_extent = mesh.vertices.iter().map(|p| p.len()).fold(1_f32, f32::max) + 1.;
        Self {
            mesh,
            normals,
            owners,
            outlines,
            active_vertices,
            active,
            depth_extent,
        }
    }
    pub(super) fn vertices(&self) -> Vec<f32> {
        self.mesh
            .vertices
            .iter()
            .enumerate()
            .flat_map(|(i, p)| {
                let n = self.normals[i];
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
                [a, b].into_iter().flat_map(move |i| {
                    let p = self.mesh.vertices[i];
                    [p.0, p.1, p.2, n.0, n.1, n.2, m.0, m.1, m.2]
                })
            })
            .collect()
    }
}

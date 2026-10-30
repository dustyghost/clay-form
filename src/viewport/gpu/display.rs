//! Cached display geometry and camera-independent wire topology.
use super::atlas::Atlas;
use crate::{
    document::Document,
    sculpt::{Mesh, SurfaceShading, SurfaceShadingTopology, Vec3},
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub(crate) struct DisplayMesh {
    pub(super) geometry: Arc<DisplayGeometry>,
    pub(super) texture_pixels: Atlas,
    pub texture_info: Vec<[u32; 4]>,
    texture_sources: Vec<TextureSource>,
}
#[derive(Clone)]
struct TextureSource {
    id: u64,
    storage: usize,
    revision: u64,
}

/// Original topology behind a filtered display. Shared by successive stroke
/// frames so hidden faces still contribute to the original smoothing fans.
struct HiddenStrokeTopology {
    source: Arc<HiddenSurfaceTopology>,
    fans: Vec<[usize; 3]>,
}
struct HiddenSurfaceTopology {
    model_id: u64,
    triangles: Vec<[usize; 3]>,
    quads: Vec<[usize; 4]>,
    hidden_faces: Vec<bool>,
    polygroups: Vec<usize>,
    group_visibility: Vec<bool>,
    shading: SurfaceShadingTopology,
}
impl HiddenSurfaceTopology {
    fn matches(&self, mesh: &Mesh, model_id: u64) -> bool {
        self.model_id == model_id
            && self.triangles == mesh.triangles
            && self.quads == mesh.quads
            && self.hidden_faces == mesh.hidden_faces
            && self.polygroups == mesh.polygroups
            && self
                .group_visibility
                .iter()
                .copied()
                .eq(mesh.vertex_groups.iter().map(|g| g.visible))
    }
}
#[derive(Clone)]
pub(crate) struct DisplayGeometry {
    pub mesh: Mesh,
    pub uv_corners: Vec<[[f32; 4]; 3]>,
    pub shading: SurfaceShading,
    pub owners: Vec<usize>,
    muted_vertices: Vec<bool>,
    pub source_faces: Vec<usize>,
    pub poly_colors: Vec<[u8; 4]>,
    pub edge_colors: HashMap<(usize, usize), eframe::egui::Color32>,
    pub outlines: Vec<[usize; 2]>,
    pub active_vertices: usize,
    pub active: usize,
    pub(super) depth_extent: f32,
    hidden_stroke_topology: Option<Arc<HiddenStrokeTopology>>,
    temporary_normals: bool,
}
impl std::ops::Deref for DisplayMesh {
    type Target = DisplayGeometry;
    fn deref(&self) -> &Self::Target {
        &self.geometry
    }
}
impl DisplayMesh {
    pub(super) fn shares_geometry(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.geometry, &other.geometry)
    }
    /// Retain geometry/shading and copy only atlas chunks touched since the last frame.
    pub(super) fn with_texture_updates(&self, document: &Document) -> Option<Self> {
        let mut next = self.clone();
        let mut sources = HashMap::new();
        let active = document.active_model_index();
        let models: Vec<_> = std::iter::once(active)
            .chain((0..document.model_count()).filter(|i| *i != active))
            .map(|i| document.model(i).map(|model| (i, model)))
            .collect::<Option<_>>()?;
        for (i, model) in &models {
            for texture in &model.display_mesh(*i == active).uv.textures {
                sources.entry(texture.id).or_insert(texture);
            }
        }
        for (i, previous) in next.texture_sources.iter_mut().enumerate() {
            let texture = *sources.get(&previous.id)?;
            let info = next.texture_info[i + 1];
            let encoding = if matches!(texture.pixels, crate::sculpt::texture::Pixels::Float32(_)) {
                2
            } else {
                u32::from(texture.channel == crate::sculpt::texture::Channel::Color)
            };
            if info[0] != texture.width || info[1] != texture.height || info[3] != encoding {
                return None;
            }
            let revision = texture.upload.revision();
            let storage = texture.pixels.storage_id();
            if revision != previous.revision || storage != previous.storage {
                let dirty = if revision == previous.revision {
                    None
                } else {
                    texture.upload.changes_since(previous.revision)
                }
                .unwrap_or(0..texture.width as usize * texture.height as usize);
                next.texture_pixels.update(texture, info[2] as usize, dirty);
                previous.revision = revision;
                previous.storage = storage;
            }
        }
        Some(next)
    }

    /// Reuse the smoothing fans while dragging a dense, fixed-topology mesh.
    /// Paint retains exact normals; deformation uses cheap temporary normals.
    /// The renderer restores full angle-weighted shading when the stroke ends.
    pub(super) fn during_stroke(&self, document: &Document) -> Option<Self> {
        let source = document.mesh();
        if !document.stroke_active()
            || document.model_count() != 1
            || !document.active_visible()
            || source.vertices.len() < 20_000
            || document.model(0)?.metadata.temporary_thickness != 0.
            || document.primitive().is_some()
            || !source.uv.textures.is_empty()
            || !source.creases.is_empty()
            || self.mesh.vertices.len() != source.vertices.len()
        {
            return None;
        }
        let fans = if let Some(topology) = &self.hidden_stroke_topology {
            if !source.uv.is_empty()
                || !topology
                    .source
                    .matches(source, document.model(0)?.metadata.model_id)
            {
                return None;
            }
            &topology.fans
        } else {
            if source.has_hidden_faces()
                || self.mesh.triangles != source.triangles
                || self.mesh.quads != source.quads
            {
                return None;
            }
            &self.shading.triangles
        };
        let mesh = if self.hidden_stroke_topology.is_some() {
            // Preserve filtered faces and source-face IDs. Copy current vertex
            // attributes without putting hidden faces back into picking/rendering.
            Mesh {
                vertices: source.vertices.clone(),
                mask: source.mask.clone(),
                materials: source.materials.clone(),
                triangles: self.mesh.triangles.clone(),
                quads: self.mesh.quads.clone(),
                ..Default::default()
            }
        } else {
            source.clone()
        };
        let mut geometry = DisplayGeometry {
            mesh,
            uv_corners: self.uv_corners.clone(),
            shading: self.shading.clone(),
            owners: self.owners.clone(),
            muted_vertices: self.muted_vertices.clone(),
            source_faces: self.source_faces.clone(),
            poly_colors: self.poly_colors.clone(),
            edge_colors: self.edge_colors.clone(),
            outlines: self.outlines.clone(),
            active_vertices: self.active_vertices,
            active: self.active,
            depth_extent: self.depth_extent,
            hidden_stroke_topology: self.hidden_stroke_topology.clone(),
            temporary_normals: self.temporary_normals,
        };
        if self.mesh.vertices != source.vertices {
            geometry.temporary_normals = true;
            let mut normals = vec![Vec3::default(); geometry.shading.vertices.len()];
            for (ids, fans) in source.triangles.iter().zip(fans) {
                let [a, b, c] = ids.map(|i| source.vertices[i]);
                let face = (b - a).cross(c - a);
                for &i in fans {
                    normals[i] = normals[i] + face;
                }
            }
            for ((_, normal), sum) in geometry.shading.vertices.iter_mut().zip(normals) {
                *normal = sum.norm();
            }
            geometry.depth_extent = source
                .vertices
                .iter()
                .map(|p| p.len())
                .fold(1_f32, f32::max)
                + 1.;
        }
        Some(Self {
            geometry: Arc::new(geometry),
            texture_pixels: self.texture_pixels.clone(),
            texture_info: self.texture_info.clone(),
            texture_sources: self.texture_sources.clone(),
        })
    }

    /// Finish a cached hidden-surface stroke without rebuilding visibility.
    /// All source faces still contribute to canonical, angle-weighted shading.
    pub(super) fn after_stroke(self: &Arc<Self>, document: &Document) -> Option<Arc<Self>> {
        let cached = self.hidden_stroke_topology.as_ref()?;
        let source = document.mesh();
        if document.stroke_active()
            || document.model_count() != 1
            || !document.active_visible()
            || document.model(0)?.metadata.temporary_thickness != 0.
            || document.primitive().is_some()
            || !source.uv.is_empty()
            || !source.creases.is_empty()
            || !cached.source.matches(source, document.model(0)?.metadata.model_id)
            // A final dab or another edit may not have been displayed yet.
            || self.mesh.vertices != source.vertices
            || self.mesh.mask != source.mask
            || self.mesh.materials != source.materials
        {
            return None;
        }
        if !self.temporary_normals {
            return Some(self.clone());
        }
        let shading = cached.source.shading.shade(source);
        let mut geometry = (*self.geometry).clone();
        geometry.shading = SurfaceShading {
            vertices: shading.vertices,
            triangles: self
                .source_faces
                .iter()
                .map(|&f| shading.triangles[f])
                .collect(),
        };
        geometry.hidden_stroke_topology = Some(Arc::new(HiddenStrokeTopology {
            source: cached.source.clone(),
            fans: shading.triangles,
        }));
        geometry.temporary_normals = false;
        Some(Arc::new(Self {
            geometry: Arc::new(geometry),
            texture_pixels: self.texture_pixels.clone(),
            texture_info: self.texture_info.clone(),
            texture_sources: self.texture_sources.clone(),
        }))
    }
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
            uv: Default::default(),
            creases: Vec::new(),
            materials: Vec::new(),
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
            quads: Vec::new(),
            mask: Vec::new(),
        };
        let mut uv_corners = Vec::new();
        let mut texture_pixels = Atlas::default();
        let mut texture_sources = Vec::new();
        let mut texture_info = vec![[0; 4]];
        let mut texture_ids = HashMap::new();
        let mut edge_colors = HashMap::new();
        let mut shading = SurfaceShading::default();
        let mut owners = Vec::new();
        let mut muted_vertices = Vec::new();
        let mut source_faces = Vec::new();
        let mut poly_colors = Vec::new();
        let mut outlines = Vec::new();
        let mut active_vertices = 0;
        let mut hidden_stroke_topology = None;
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
            if surface.uv.corners.is_empty() || surface.uv.current(surface) {
                for &[a, b] in &surface.uv.seams {
                    edge_colors.insert(
                        (a + offset, b + offset),
                        eframe::egui::Color32::from_rgb(195, 115, 235),
                    );
                }
            }
            for &[a, b] in &surface.creases {
                edge_colors.insert(
                    (a + offset, b + offset),
                    eframe::egui::Color32::from_rgb(255, 170, 65),
                );
            }
            mesh.vertices.extend_from_slice(&surface.vertices);
            mesh.mask.extend_from_slice(&surface.mask);
            muted_vertices.extend(std::iter::repeat_n(
                index != active && surface.materials.is_empty(),
                surface.vertices.len(),
            ));
            if !mesh.materials.is_empty() || !surface.materials.is_empty() {
                mesh.materials
                    .resize(offset, crate::sculpt::Material::default());
                mesh.materials
                    .extend((0..surface.vertices.len()).map(|i| surface.material(i)));
            }
            let uv_current = surface.uv.current(surface);
            let mut texture_channels = [0.; 3];
            if uv_current {
                for texture in &surface.uv.textures {
                    let id = *texture_ids.entry(texture.id).or_insert_with(|| {
                        let id = texture_info.len();
                        texture_info.push([
                            texture.width,
                            texture.height,
                            texture_pixels.len() as u32,
                            (if texture.pixels.precision()
                                == crate::sculpt::texture::Precision::Float32
                            {
                                2
                            } else {
                                u32::from(texture.channel == crate::sculpt::texture::Channel::Color)
                            }),
                        ]);
                        texture_pixels.append(texture);
                        texture_sources.push(TextureSource {
                            id: texture.id,
                            storage: texture.pixels.storage_id(),
                            revision: texture.upload.revision(),
                        });
                        id
                    });
                    texture_channels[texture.channel.index()] = id as f32;
                }
            }
            let cache_hidden = has_hidden
                && document.model_count() == 1
                && index == active
                && active_mesh.is_none()
                && surface.vertices.len() >= 20_000
                && model.metadata.temporary_thickness == 0.
                && model.settings.primitive.is_none()
                && surface.creases.is_empty()
                && surface.uv.is_empty();
            let topology = cache_hidden.then(|| SurfaceShadingTopology::new(surface));
            let surface_shading = topology.as_ref().map_or_else(
                || surface.surface_shading(),
                |topology| topology.shade(surface),
            );
            if let Some(topology) = topology {
                hidden_stroke_topology = Some(Arc::new(HiddenStrokeTopology {
                    source: Arc::new(HiddenSurfaceTopology {
                        model_id: model.metadata.model_id,
                        triangles: surface.triangles.clone(),
                        quads: surface.quads.clone(),
                        hidden_faces: surface.hidden_faces.clone(),
                        polygroups: surface.polygroups.clone(),
                        group_visibility: surface.vertex_groups.iter().map(|g| g.visible).collect(),
                        shading: topology,
                    }),
                    fans: surface_shading.triangles.clone(),
                }));
            }
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
                uv_corners.push(if uv_current && surface.uv.mapped(face) {
                    std::array::from_fn(|c| {
                        let q = surface.uv.corners[face][c];
                        [q[0], q[1], 1., texture_channels[c]]
                    })
                } else {
                    [[0.; 4]; 3]
                });
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
            texture_pixels,
            texture_info,
            texture_sources,
            geometry: Arc::new(DisplayGeometry {
                mesh,
                uv_corners,
                shading,
                owners,
                muted_vertices,
                source_faces,
                poly_colors,
                edge_colors,
                outlines,
                active_vertices,
                active,
                depth_extent,
                hidden_stroke_topology,
                temporary_normals: false,
            }),
        }
    }
    /// Bilinear, coverage-aware linear sample matching the fragment shader.
    pub(crate) fn sample_texture(&self, id: usize, uv: [f32; 2]) -> [f32; 4] {
        if id == 0 {
            return [0.; 4];
        }
        let [width, height, first, encoding] = self.texture_info[id];
        let xy = [
            uv[0].clamp(0., 1.) * width as f32 - 0.5,
            (1. - uv[1]).clamp(0., 1.) * height as f32 - 0.5,
        ];
        let size = [width, height];
        let lo = std::array::from_fn::<_, 2, _>(|i| {
            xy[i].floor().clamp(0., size[i] as f32 - 1.) as usize
        });
        let hi = std::array::from_fn::<_, 2, _>(|i| {
            (xy[i].floor() + 1.).clamp(0., size[i] as f32 - 1.) as usize
        });
        let t = xy.map(|v| v - v.floor());
        let pixel = |x: usize, y: usize| {
            let index =
                first as usize + (y * width as usize + x) * if encoding == 2 { 2 } else { 1 };
            let a = self.texture_pixels.get(index);
            let mut rgba = if encoding == 2 {
                let b = self.texture_pixels.get(index + 1);
                [a[0], a[1], b[0], b[1]].map(f32::from_bits)
            } else {
                [a[0] & 65535, a[0] >> 16, a[1] & 65535, a[1] >> 16].map(|v| v as f32 / 65535.)
            };
            for c in 0..3 {
                if encoding == 1 {
                    rgba[c] = crate::sculpt::material::srgb_to_linear(rgba[c]);
                }
                rgba[c] *= rgba[3];
            }
            rgba
        };
        let samples = [
            pixel(lo[0], lo[1]),
            pixel(hi[0], lo[1]),
            pixel(lo[0], hi[1]),
            pixel(hi[0], hi[1]),
        ];
        let weights = [
            (1. - t[0]) * (1. - t[1]),
            t[0] * (1. - t[1]),
            (1. - t[0]) * t[1],
            t[0] * t[1],
        ];
        let mut result: [f32; 4] =
            std::array::from_fn(|c| samples.iter().zip(weights).map(|(p, w)| p[c] * w).sum());
        for c in 0..3 {
            result[c] /= result[3].max(0.000001);
        }
        result
    }
    pub fn material(&self, i: usize) -> crate::sculpt::Material {
        if self.muted_vertices[i] {
            crate::sculpt::Material::from_srgb([130., 143., 155.].map(|v| v / 255.), 0.85, 0.)
        } else {
            self.mesh.material(i)
        }
    }
    pub(super) fn vertex_stride(&self) -> usize {
        if self
            .mesh
            .materials
            .iter()
            .any(|m| m.needs_extended_shading())
        {
            crate::sculpt::material::layers::VERTEX_STRIDE
        } else {
            12
        }
    }
    pub(super) fn vertices(&self, stride: usize) -> Vec<f32> {
        let mut vertices = Vec::with_capacity(self.shading.vertices.len() * stride);
        for &(i, n) in &self.shading.vertices {
            let p = self.mesh.vertices[i];
            let material = self.material(i);
            let color = material.base_color;
            let mask = if i < self.active_vertices {
                self.mesh.mask[i]
            } else {
                -1.
            };
            vertices.extend_from_slice(&[
                p.0,
                p.1,
                p.2,
                n.0,
                n.1,
                n.2,
                color[0],
                color[1],
                color[2],
                mask,
                material.roughness,
                material.metallic,
            ]);
            if stride > 12 {
                vertices.extend_from_slice(&material.values()[5..]);
            }
        }
        vertices
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
    fn texture_update_reuses_geometry_and_unmodified_atlas_chunks() {
        use crate::sculpt::{
            Material,
            texture::{Channel, Texture},
        };
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        mesh.uv = mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
        let texture = Texture::new(Channel::Color, 256, &Material::default()).unwrap();
        mesh.uv.textures.push(texture);
        let mut document = Document::default();
        document
            .replace_geometry(Geometry::new(mesh.clone()))
            .unwrap();
        let old = DisplayMesh::new(&document);
        mesh.uv.textures[0]
            .pixels
            .set_pixel(20_000, [1., 0., 0., 1.]);
        mesh.uv.textures[0].upload.record(20_000..20_001);
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        let next = old.with_texture_updates(&document).unwrap();
        assert!(old.shares_geometry(&next));
        for i in 0..old.texture_pixels.chunks.len() {
            assert_eq!(
                Arc::ptr_eq(
                    &old.texture_pixels.chunks[i],
                    &next.texture_pixels.chunks[i]
                ),
                i != 20_000 / super::super::atlas::CHUNK
            );
        }
        assert_ne!(
            old.texture_pixels.get(20_000),
            next.texture_pixels.get(20_000)
        );
    }
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

#[cfg(test)]
mod stroke_tests;

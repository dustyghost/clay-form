//! Cached display geometry and camera-independent wire topology.
use super::atlas::Atlas;
use crate::{
    document::Document,
    sculpt::{Material, Mesh, SurfaceShading, SurfaceShadingTopology, Vec3},
};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

mod live_normals;
mod surface;
use surface::DisplaySurface;

#[derive(Clone)]
pub(crate) struct DisplayMesh {
    pub(super) geometry: Arc<DisplayGeometry>,
    pub(super) texture_pixels: Atlas,
    pub texture_info: Vec<[u32; 4]>,
    texture_sources: Vec<TextureSource>,
}
pub(super) struct VertexUpdate {
    pub(super) ranges: Vec<std::ops::Range<usize>>,
    #[cfg(test)]
    pub(super) changed_vertices: usize,
}
#[derive(Clone)]
struct TextureSource {
    id: u64,
    storage: usize,
    revision: u64,
}

/// Source topology shared by successive fixed-topology stroke frames.
/// Hidden faces also contribute to the original smoothing fans.
struct StrokeTopology {
    source: Arc<StrokeSurfaceTopology>,
    fans: Arc<Vec<[usize; 3]>>,
}
struct StrokeSurfaceTopology {
    model_id: u64,
    filtered: bool,
    triangles: Vec<[usize; 3]>,
    quads: Vec<[usize; 4]>,
    hidden_faces: Vec<bool>,
    polygroups: Vec<usize>,
    group_visibility: Vec<bool>,
    shading: SurfaceShadingTopology,
    normal_corners: OnceLock<Option<live_normals::VertexCorners>>,
}
impl StrokeSurfaceTopology {
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
/// Display snapshots share immutable fan indices and normals. Deformation
/// replaces only the normal payload; Paint keeps both allocations.
#[derive(Clone, Default)]
pub(crate) struct DisplayShading {
    pub vertices: Arc<Vec<(usize, Vec3)>>,
    pub triangles: Arc<Vec<[usize; 3]>>,
}
impl From<SurfaceShading> for DisplayShading {
    fn from(shading: SurfaceShading) -> Self {
        Self {
            vertices: Arc::new(shading.vertices),
            triangles: Arc::new(shading.triangles),
        }
    }
}

#[derive(Clone)]
pub(crate) struct DisplayGeometry {
    pub mesh: DisplaySurface,
    // Immutable face payloads survive live snapshots and canonical release.
    // Keep the Vec allocation when wrapping freshly built data in an Arc.
    pub uv_corners: Arc<Vec<[[f32; 4]; 3]>>,
    pub shading: DisplayShading,
    pub owners: Arc<Vec<usize>>,
    muted_vertices: Arc<Vec<bool>>,
    pub source_faces: Arc<Vec<usize>>,
    pub poly_colors: Arc<Vec<[u8; 4]>>,
    pub edge_colors: Arc<HashMap<(usize, usize), eframe::egui::Color32>>,
    pub outlines: Arc<Vec<[usize; 2]>>,
    pub active_vertices: usize,
    pub active: usize,
    pub(super) depth_extent: f32,
    stroke_topology: Option<Arc<StrokeTopology>>,
    temporary_normals: bool,
}
impl DisplayGeometry {
    /// Copy retained display data for release, without copying shading that the
    /// caller immediately replaces with canonical normals and filtered fans.
    fn clone_without_shading(&self) -> Self {
        Self {
            mesh: self.mesh.clone(),
            uv_corners: self.uv_corners.clone(),
            shading: DisplayShading::default(),
            owners: self.owners.clone(),
            muted_vertices: self.muted_vertices.clone(),
            source_faces: self.source_faces.clone(),
            poly_colors: self.poly_colors.clone(),
            edge_colors: self.edge_colors.clone(),
            outlines: self.outlines.clone(),
            active_vertices: self.active_vertices,
            active: self.active,
            depth_extent: self.depth_extent,
            stroke_topology: self.stroke_topology.clone(),
            temporary_normals: self.temporary_normals,
        }
    }
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
        let fans = if let Some(topology) = &self.stroke_topology {
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
                || *self.mesh.triangles != source.triangles
                || *self.mesh.quads != source.quads
            {
                return None;
            }
            &self.shading.triangles
        };
        #[cfg(test)]
        stroke_profile::mark("live.validate");
        let positions_unchanged = self.mesh.same_positions(source);
        let mesh = self.mesh.update(source, positions_unchanged);
        #[cfg(test)]
        stroke_profile::mark("live.mesh_copy");
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
            stroke_topology: self.stroke_topology.clone(),
            temporary_normals: self.temporary_normals,
        };
        #[cfg(test)]
        stroke_profile::mark("live.display_copy");
        if !positions_unchanged {
            geometry.temporary_normals = true;
            geometry.shading.vertices = Arc::new(self.update_live_normals(source, fans).vertices);
            geometry.depth_extent = source
                .vertices
                .iter()
                .map(|p| p.len())
                .fold(1_f32, f32::max)
                + 1.;
        }
        #[cfg(test)]
        stroke_profile::mark("live.normals_and_bounds");
        Some(Self {
            geometry: Arc::new(geometry),
            texture_pixels: self.texture_pixels.clone(),
            texture_info: self.texture_info.clone(),
            texture_sources: self.texture_sources.clone(),
        })
    }

    /// Restore exact canonical shading using validated source topology.
    /// All source faces still contribute, including faces hidden from display.
    pub(super) fn after_stroke(self: &Arc<Self>, document: &Document) -> Option<Arc<Self>> {
        let cached = self.stroke_topology.as_ref()?;
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
            || !self.mesh.same_positions(source)
            || !self.mesh.same_mask(source)
            || !Material::slices_bitwise_equal(&self.mesh.materials, &source.materials)
        {
            return None;
        }
        #[cfg(test)]
        stroke_profile::mark("release.validate");
        if !self.temporary_normals {
            return Some(self.clone());
        }
        let shading = cached.source.shading.shade(source);
        #[cfg(test)]
        stroke_profile::mark("release.shade");
        let mut geometry = self.geometry.clone_without_shading();
        #[cfg(test)]
        stroke_profile::mark("release.copy");
        let fans = Arc::new(shading.triangles);
        geometry.shading = DisplayShading {
            vertices: Arc::new(shading.vertices),
            triangles: if cached.source.filtered {
                Arc::new(self.source_faces.iter().map(|&f| fans[f]).collect())
            } else {
                fans.clone()
            },
        };
        geometry.stroke_topology = Some(Arc::new(StrokeTopology {
            source: cached.source.clone(),
            fans,
        }));
        geometry.temporary_normals = false;
        #[cfg(test)]
        stroke_profile::mark("release.fans");
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
        let mut stroke_topology = None;
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
            let cache_stroke = document.model_count() == 1
                && index == active
                && active_mesh.is_none()
                && surface.vertices.len() >= 20_000
                && model.metadata.temporary_thickness == 0.
                && model.settings.primitive.is_none()
                && surface.creases.is_empty()
                && surface.uv.is_empty();
            let topology = cache_stroke.then(|| SurfaceShadingTopology::new(surface));
            let surface_shading = topology.as_ref().map_or_else(
                || surface.surface_shading(),
                |topology| topology.shade(surface),
            );
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
            if let Some(topology) = topology {
                stroke_topology = Some(Arc::new(StrokeTopology {
                    source: Arc::new(StrokeSurfaceTopology {
                        model_id: model.metadata.model_id,
                        filtered: has_hidden,
                        triangles: surface.triangles.clone(),
                        quads: surface.quads.clone(),
                        hidden_faces: surface.hidden_faces.clone(),
                        polygroups: surface.polygroups.clone(),
                        group_visibility: surface.vertex_groups.iter().map(|g| g.visible).collect(),
                        shading: topology,
                        normal_corners: OnceLock::new(),
                    }),
                    fans: if has_hidden {
                        Arc::new(surface_shading.triangles)
                    } else {
                        Arc::default()
                    },
                }));
            }
            if index == active {
                active_vertices = surface.vertices.len();
            }
        }
        // Shared snapshots retain the builder allocation, so do not keep its
        // geometric growth slack alive throughout the stroke.
        uv_corners.shrink_to_fit();
        poly_colors.shrink_to_fit();
        owners.shrink_to_fit();
        source_faces.shrink_to_fit();
        muted_vertices.shrink_to_fit();
        outlines.shrink_to_fit();
        edge_colors.shrink_to_fit();
        shading.vertices.shrink_to_fit();
        shading.triangles.shrink_to_fit();
        let depth_extent = mesh.vertices.iter().map(|p| p.len()).fold(1_f32, f32::max) + 1.;
        let shading = DisplayShading::from(shading);
        if let Some(cached) = &mut stroke_topology {
            // Only one model can take the cached path, so these indices are identical.
            if !cached.source.filtered {
                Arc::get_mut(cached).unwrap().fans = shading.triangles.clone();
            }
        }
        Self {
            texture_pixels,
            texture_info,
            texture_sources,
            geometry: Arc::new(DisplayGeometry {
                mesh: mesh.into(),
                uv_corners: Arc::new(uv_corners),
                shading,
                owners: Arc::new(owners),
                muted_vertices: Arc::new(muted_vertices),
                source_faces: Arc::new(source_faces),
                poly_colors: Arc::new(poly_colors),
                edge_colors: Arc::new(edge_colors),
                outlines: Arc::new(outlines),
                active_vertices,
                active,
                depth_extent,
                stroke_topology,
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
        let default = crate::sculpt::Material::default();
        let muted =
            crate::sculpt::Material::from_srgb([130., 143., 155.].map(|v| v / 255.), 0.85, 0.);
        for &(i, n) in self.shading.vertices.iter() {
            let p = self.mesh.vertices[i];
            let material = if self.muted_vertices[i] {
                &muted
            } else {
                self.mesh.materials.get(i).unwrap_or(&default)
            };
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
    // Frozen owned-material packer for byte-exact regression checks and diagnosis.
    #[cfg(test)]
    pub(super) fn reference_vertices(&self, stride: usize) -> Vec<f32> {
        let mut vertices = Vec::with_capacity(self.shading.vertices.len() * stride);
        for &(i, n) in self.shading.vertices.iter() {
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
    /// Refresh only records whose effective GPU values changed. This scans
    /// inputs but avoids owned material copies, a second packed array and a
    /// separate byte-difference pass. Fan mappings may change at the same size.
    pub(super) fn update_vertices(&self, stride: usize, packed: &mut [f32]) -> VertexUpdate {
        debug_assert!(stride == 12 || stride == crate::sculpt::material::layers::VERTEX_STRIDE);
        assert_eq!(packed.len(), self.shading.vertices.len() * stride);
        let default = crate::sculpt::Material::default();
        let muted =
            crate::sculpt::Material::from_srgb([130., 143., 155.].map(|v| v / 255.), 0.85, 0.);
        let mut dirty = super::upload::DirtyRanges::new(packed.len() * 4);
        #[cfg(test)]
        let mut changed_vertices = 0;
        for (fan, (record, &(i, n))) in packed
            .chunks_exact_mut(stride)
            .zip(self.shading.vertices.iter())
            .enumerate()
        {
            let p = self.mesh.vertices[i];
            let material = if self.muted_vertices[i] {
                &muted
            } else {
                self.mesh.materials.get(i).unwrap_or(&default)
            };
            let mask = if i < self.active_vertices {
                self.mesh.mask[i]
            } else {
                -1.
            };
            let compact = [
                p.0,
                p.1,
                p.2,
                n.0,
                n.1,
                n.2,
                material.base_color[0],
                material.base_color[1],
                material.base_color[2],
                mask,
                material.roughness,
                material.metallic,
            ];
            let extended = (stride > 12).then(|| material.values());
            let same_compact = bytemuck::cast_slice::<_, u8>(&record[..12])
                == bytemuck::cast_slice::<_, u8>(&compact);
            let same_extended = extended.as_ref().is_none_or(|values| {
                bytemuck::cast_slice::<_, u8>(&record[12..])
                    == bytemuck::cast_slice::<_, u8>(&values[5..])
            });
            if same_compact && same_extended {
                continue;
            }
            record[..12].copy_from_slice(&compact);
            if let Some(values) = extended {
                record[12..].copy_from_slice(&values[5..]);
            }
            dirty.mark(fan * stride * 4, (fan + 1) * stride * 4);
            #[cfg(test)]
            {
                changed_vertices += 1;
            }
        }
        VertexUpdate {
            ranges: dirty.finish(),
            #[cfg(test)]
            changed_vertices,
        }
    }
    /// Adjacent face normals let the shader reject back-facing edges without
    /// rebuilding edge buffers on every camera movement.
    pub(super) fn edges(&self, triangles: bool) -> Vec<f32> {
        let key = |a, b| if a < b { (a, b) } else { (b, a) };
        let mut hidden = std::collections::HashSet::new();
        if !triangles {
            for &[a, b, c, d] in self.mesh.quads.iter() {
                hidden.insert(key(a, c));
                hidden.insert(key(b, d));
            }
        }
        let mut edges: HashMap<(usize, usize), (Vec3, Vec3)> = HashMap::new();
        for &[a, b, c] in self.mesh.triangles.iter() {
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
        for (&(a, b), &color) in display.edge_colors.iter() {
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

#[cfg(test)]
mod packing_tests;

#[cfg(test)]
impl DisplayMesh {
    pub(super) fn profile_after_stroke(
        &self,
        document: &Document,
        use_angles: bool,
    ) -> Vec<(&'static str, f64)> {
        fn timed<T>(
            out: &mut Vec<(&'static str, f64)>,
            name: &'static str,
            f: impl FnOnce() -> T,
        ) -> T {
            let start = std::time::Instant::now();
            let result = f();
            out.push((name, start.elapsed().as_secs_f64() * 1000.));
            result
        }
        let mut times = Vec::new();
        let cached = self.stroke_topology.as_ref().unwrap();
        let source = document.mesh();
        assert!(!document.stroke_active());
        assert!(timed(&mut times, "validate.topology", || cached
            .source
            .matches(source, document.model(0).unwrap().metadata.model_id)));
        assert!(timed(&mut times, "validate.positions_mask", || self
            .mesh
            .same_positions(source)
            && self.mesh.same_mask(source)));
        assert!(timed(&mut times, "validate.materials", || {
            Material::slices_bitwise_equal(&self.mesh.materials, &source.materials)
        }));
        if !self.temporary_normals {
            return times;
        }
        let (shading, shading_times) = cached.source.shading.profile_shade(source, use_angles);
        times.extend(shading_times);
        let mut geometry = timed(&mut times, "clone_geometry", || {
            self.geometry.clone_without_shading()
        });
        geometry.shading = timed(&mut times, "filter_shading", || DisplayShading {
            vertices: Arc::new(shading.vertices),
            triangles: Arc::new(
                self.source_faces
                    .iter()
                    .map(|&f| shading.triangles[f])
                    .collect(),
            ),
        });
        // Check the diagnostic's staged shading against the production result outside timers.
        let expected = cached.source.shading.shade(source);
        assert_eq!(*geometry.shading.vertices, expected.vertices);
        assert_eq!(
            *geometry.shading.triangles,
            self.source_faces
                .iter()
                .map(|&f| expected.triangles[f])
                .collect::<Vec<_>>()
        );
        times
    }
    pub(super) fn release_cache_stats(&self, document: &Document) -> (usize, usize) {
        self.stroke_topology
            .as_ref()
            .unwrap()
            .source
            .shading
            .angle_cache_stats(document.mesh())
    }
}

#[cfg(test)]
pub(in crate::viewport::gpu) mod stroke_profile;

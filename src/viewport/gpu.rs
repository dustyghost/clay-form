//! Persistent wgpu scene buffers. Document geometry stays on the CPU.
use super::{WireMode, scene::Scene};
use crate::{
    document::Document,
    sculpt::{Alpha, Brush, Symmetry, Vec3},
};
use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
};
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;

pub(super) struct Renderer {
    revision: Option<u64>,
    live_display: bool,
    geometry: Option<Arc<DisplayMesh>>,
    render_geometry: Option<Arc<DisplayMesh>>,
    gpu: Arc<Mutex<Gpu>>,
    preview: Arc<Mutex<Preview>>,
}
impl Renderer {
    pub fn new(state: &egui_wgpu::RenderState) -> Result<Self, String> {
        Ok(Self {
            preview: Arc::new(Mutex::new(Preview::default())),
            revision: None,
            live_display: false,
            geometry: None,
            render_geometry: None,
            gpu: Arc::new(Mutex::new(Gpu::new(
                &state.device,
                &state.queue,
                state.target_format,
            ))),
        })
    }
    pub fn geometry(&mut self, document: &Document) -> Arc<DisplayMesh> {
        if self.revision != Some(document.display_revision())
            || (self.live_display && !document.stroke_active())
        {
            let preview = self
                .geometry
                .as_ref()
                .and_then(|display| display.during_stroke(document));
            self.live_display = preview.is_some();
            self.geometry = Some(Arc::new(
                preview.unwrap_or_else(|| DisplayMesh::new(document)),
            ));
            self.render_geometry = Some(
                if (0..document.model_count())
                    .any(|i| document.model(i).unwrap().metadata.temporary_thickness > 0.)
                {
                    Arc::new(DisplayMesh::preview(document))
                } else {
                    self.geometry.as_ref().unwrap().clone()
                },
            );
            self.revision = Some(document.display_revision());
        }
        self.geometry.as_ref().unwrap().clone()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        scene: &Scene,
        wire: bool,
        mode: WireMode,
        triangles: bool,
        geometry: Option<Arc<DisplayMesh>>,
    ) {
        let gpu = self.gpu.clone();
        let preview = self.preview.clone();
        let geometry = geometry.unwrap_or_else(|| self.render_geometry.as_ref().unwrap().clone());
        let mut params = Params::new(
            rect,
            scene.right,
            scene.up,
            scene.view,
            scene.scale,
            scene.center,
            geometry.depth_extent,
        );
        params.set_perspective(scene.eye_distance, geometry.depth_extent);
        params.lighting = crate::lighting::Lighting::current(painter.ctx());
        params.background = crate::preferences::viewport_background(painter.ctx()).to_array()[..3]
            .try_into()
            .unwrap();
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewCallback {
                gpu,
                preview,
                geometry,
                params,
                rect,
                wire,
                mode,
                triangles,
            },
        ));
    }

    pub fn clear_preview(&self) {
        *self.preview.lock().unwrap() = Preview::default();
    }
    pub fn set_backfaces(&self, opacity: f32) {
        self.preview.lock().unwrap().backface_opacity = opacity.clamp(0., 1.);
    }
    pub fn set_polygroups(&self, enabled: bool) {
        self.preview.lock().unwrap().show_polygroups = enabled;
    }
    pub fn set_guides(&self, symmetry: &Symmetry, opacity: f32) {
        let mut preview = self.preview.lock().unwrap();
        preview.symmetry = Some((symmetry.clone(), opacity));
    }
    pub fn set_brush(
        &self,
        scene: &Scene,
        pos: egui::Pos2,
        radius: f32,
        brush: &Brush,
        color: egui::Color32,
    ) {
        if let Some((center, normal)) = scene.pick(pos) {
            self.preview.lock().unwrap().brush = Some(BrushPreview {
                center,
                normal,
                radius: radius / scene.scale_at(center),
                brush: brush.clone(),
                color: [
                    color.r() as f32 / 255.,
                    color.g() as f32 / 255.,
                    color.b() as f32 / 255.,
                ],
            });
        }
    }
    pub fn destroy(&self) {
        // wgpu resources are reference counted; release cached allocations now.
        self.gpu.lock().unwrap().clear();
    }
}

mod state;
use state::{BrushPreview, Params, Preview};

struct ViewCallback {
    gpu: Arc<Mutex<Gpu>>,
    preview: Arc<Mutex<Preview>>,
    geometry: Arc<DisplayMesh>,
    params: Params,
    rect: egui::Rect,
    wire: bool,
    mode: WireMode,
    triangles: bool,
}
impl egui_wgpu::CallbackTrait for ViewCallback {
    fn prepare(
        &self,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        _: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        // Match egui's endpoint rounding, including fractional DPI and clipping.
        let viewport = egui::PaintCallbackInfo {
            viewport: self.rect,
            clip_rect: self.rect,
            pixels_per_point: screen.pixels_per_point,
            screen_size_px: screen.size_in_pixels,
        }
        .viewport_in_pixels();
        let size = [
            viewport.width_px.max(1) as u32,
            viewport.height_px.max(1) as u32,
        ];
        let mut gpu = self.gpu.lock().unwrap();
        gpu.upload(&self.geometry);
        gpu.encode(
            encoder,
            &self.params,
            size,
            self.wire,
            self.mode,
            self.triangles,
            &self.preview.lock().unwrap(),
        );
        Vec::new()
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _: &egui_wgpu::CallbackResources,
    ) {
        let gpu = self.gpu.lock().unwrap();
        if let Some(target) = &gpu.target {
            pass.set_pipeline(&gpu.composite);
            pass.set_bind_group(0, &target.composite, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
struct Target {
    #[cfg_attr(not(test), allow(dead_code))]
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    composite: wgpu::BindGroup,
    size: [u32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    right: [f32; 4],
    up: [f32; 4],
    view: [f32; 4],
    projection: [f32; 4],
    perspective: [f32; 4],
    brush_center: [f32; 4],
    brush_normal: [f32; 4],
    brush_color: [f32; 4],
    brush_shape: [f32; 4],
    guide_center: [f32; 4],
    flags: [f32; 4],
    light_position: [[f32; 4]; crate::lighting::MAX_LIGHTS],
    light_colour: [[f32; 4]; crate::lighting::MAX_LIGHTS],
    light_info: [f32; 4],
    material_info: [f32; 4],
    guide_normals: [[f32; 4]; 32],
    guide_sides: [[f32; 4]; 32],
    guide_colors: [[f32; 4]; 32],
}
struct MeshBuffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    colors: wgpu::Buffer,
    count: u32,
}
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::RenderPipeline,
    offset_surface: wgpu::RenderPipeline,
    backs: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    xray_lines: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    // Backfaces use a separate buffer: queue writes are visible to the entire submission.
    back_uniform: wgpu::Buffer,
    meshes: Vec<MeshBuffers>,
    vertex_stride: usize,
    edges: Option<Vec<(wgpu::Buffer, u32)>>,
    triangle_edges: Option<Vec<(wgpu::Buffer, u32)>>,
    uploaded: Option<Arc<DisplayMesh>>,
    target: Option<Target>,
    alpha: wgpu::Texture,
    alpha_source: Option<Alpha>,
    sampler: wgpu::Sampler,
}
fn buffer(
    device: &wgpu::Device,
    label: &str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: if contents.is_empty() {
            &[0; 16]
        } else {
            contents
        },
        usage,
    })
}
impl Gpu {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let mut entries = vec![
            uniform_entry,
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ];
        for binding in 2..=4 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX
                    | if binding == 2 {
                        wgpu::ShaderStages::FRAGMENT
                    } else {
                        wgpu::ShaderStages::empty()
                    },
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport inputs"),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewport"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport WGSL"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu/viewport.wgsl").into()),
        });
        let make_pipeline = |line: bool, xray: bool, back: bool, offset: bool| {
            let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3];
            let buffers = [wgpu::VertexBufferLayout {
                array_stride: 48,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if line { "wireframe" } else { "clay surface" }),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(if line {
                        "line_vertex"
                    } else {
                        "surface_vertex"
                    }),
                    compilation_options: Default::default(),
                    buffers: if line { &buffers } else { &[] },
                },
                primitive: wgpu::PrimitiveState {
                    topology: if line {
                        wgpu::PrimitiveTopology::LineList
                    } else {
                        wgpu::PrimitiveTopology::TriangleList
                    },
                    cull_mode: if line {
                        None
                    } else {
                        Some(if back {
                            wgpu::Face::Front
                        } else {
                            wgpu::Face::Back
                        })
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: !line && !back,
                    depth_compare: if xray {
                        wgpu::CompareFunction::Always
                    } else if line || back {
                        wgpu::CompareFunction::LessEqual
                    } else {
                        wgpu::CompareFunction::Less
                    },
                    stencil: Default::default(),
                    bias: if offset {
                        wgpu::DepthBiasState {
                            constant: 1,
                            slope_scale: 1.,
                            clamp: 0.,
                        }
                    } else {
                        Default::default()
                    },
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if line {
                        "line_fragment"
                    } else {
                        "surface_fragment"
                    }),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR,
                        blend: if line || back {
                            Some(wgpu::BlendState::ALPHA_BLENDING)
                        } else {
                            None
                        },
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let surface = make_pipeline(false, false, false, false);
        let offset_surface = make_pipeline(false, false, false, true);
        let backs = make_pipeline(false, false, true, false);
        let lines = make_pipeline(true, false, false, false);
        let xray_lines = make_pipeline(true, true, false, false);
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport composite"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport composite WGSL"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu/composite.wgsl").into()),
        });
        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewport composite"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&composite_layout],
                    push_constant_ranges: &[],
                }),
            ),
            vertex: wgpu::VertexState {
                module: &composite_shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &composite_shader,
                entry_point: Some(if format.is_srgb() {
                    "fragment_srgb"
                } else {
                    "fragment"
                }),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview: None,
            cache: None,
        });
        let uniform = || {
            buffer(
                device,
                "viewport uniforms",
                bytemuck::bytes_of(&<Uniforms as bytemuck::Zeroable>::zeroed()),
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            )
        };
        let alpha = Self::texture(
            device,
            [1, 1],
            wgpu::TextureFormat::R32Float,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        queue.write_texture(
            alpha.as_image_copy(),
            bytemuck::cast_slice(&[1f32]),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            alpha.size(),
        );
        Self {
            device: device.clone(),
            queue: queue.clone(),
            surface,
            offset_surface,
            backs,
            lines,
            xray_lines,
            composite,
            layout,
            composite_layout,
            uniform: uniform(),
            back_uniform: uniform(),
            meshes: vec![Self::mesh_buffers(device, &[], &[], &[])],
            vertex_stride: 12,
            edges: None,
            triangle_edges: None,
            uploaded: None,
            target: None,
            alpha,
            alpha_source: None,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor::default()),
        }
    }
    fn clear(&mut self) {
        self.target = None;
        self.uploaded = None;
        self.edges = None;
        self.triangle_edges = None;
        self.meshes = vec![Self::mesh_buffers(&self.device, &[], &[], &[])];
    }
    fn mesh_buffers(
        device: &wgpu::Device,
        vertices: &[f32],
        indices: &[u32],
        colors: &[[f32; 4]],
    ) -> MeshBuffers {
        MeshBuffers {
            vertices: buffer(
                device,
                "mesh vertices",
                bytemuck::cast_slice(vertices),
                wgpu::BufferUsages::STORAGE,
            ),
            indices: buffer(
                device,
                "mesh indices",
                bytemuck::cast_slice(indices),
                wgpu::BufferUsages::STORAGE,
            ),
            colors: buffer(
                device,
                "face colors",
                bytemuck::cast_slice(colors),
                wgpu::BufferUsages::STORAGE,
            ),
            count: indices.len() as u32,
        }
    }

    fn texture(
        device: &wgpu::Device,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport target"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }
    fn upload(&mut self, data: &Arc<DisplayMesh>) {
        if self
            .uploaded
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, data))
        {
            return;
        }
        self.upload_meshes(
            data,
            self.device.limits().max_storage_buffer_binding_size as usize,
        );
        self.edges = None;
        self.triangle_edges = None;
        self.uploaded = Some(data.clone());
    }
    // A document may contain several maximum-density models. Chunk the display
    // data at the device binding limit instead of imposing a new geometry limit.
    fn upload_meshes(&mut self, data: &DisplayMesh, limit: usize) {
        self.vertex_stride = data.vertex_stride();
        let stride = self.vertex_stride;
        let vertices = data.vertices(stride);
        let colors: Vec<[f32; 4]> = data
            .poly_colors
            .iter()
            .map(|c| c.map(|n| n as f32 / 255.))
            .collect();
        if vertices.len() * 4 <= limit && data.shading.triangles.len() * 16 <= limit {
            let indices: Vec<u32> = data
                .shading
                .triangles
                .iter()
                .flatten()
                .map(|&i| i as u32)
                .collect();
            self.meshes = vec![Self::mesh_buffers(
                &self.device,
                &vertices,
                &indices,
                &colors,
            )];
            return;
        }
        self.meshes.clear();
        let per_batch = (limit / (3 * stride * 4)).max(1);
        for (batch, triangles) in data.shading.triangles.chunks(per_batch).enumerate() {
            let mut mapping = std::collections::HashMap::new();
            let mut packed = Vec::new();
            let mut indices = Vec::with_capacity(triangles.len() * 3);
            for &source in triangles.iter().flatten() {
                let index = *mapping.entry(source).or_insert_with(|| {
                    let index = (packed.len() / stride) as u32;
                    packed.extend_from_slice(&vertices[source * stride..(source + 1) * stride]);
                    index
                });
                indices.push(index);
            }
            let start = batch * per_batch;
            self.meshes.push(Self::mesh_buffers(
                &self.device,
                &packed,
                &indices,
                &colors[start..start + triangles.len()],
            ));
        }
        if self.meshes.is_empty() {
            self.meshes
                .push(Self::mesh_buffers(&self.device, &[], &[], &[]));
        }
    }
    fn upload_edges(&mut self) {
        if self.edges.is_some() {
            return;
        }
        let data = self.uploaded.as_ref().unwrap();
        for triangles in [false, true] {
            let edges = data.edges(triangles);
            // Split only on whole line segments, including triangle diagonals.
            let per_batch = (self.device.limits().max_buffer_size as usize / 96).max(1) * 24;
            let buffers = edges
                .chunks(per_batch)
                .map(|edge| {
                    (
                        buffer(
                            &self.device,
                            "wire edges",
                            bytemuck::cast_slice(edge),
                            wgpu::BufferUsages::VERTEX,
                        ),
                        (edge.len() / 12) as u32,
                    )
                })
                .collect();
            if triangles {
                self.triangle_edges = Some(buffers);
            } else {
                self.edges = Some(buffers);
            }
        }
    }
    fn uniforms(&mut self, p: &Params, preview: &Preview, mode: WireMode) -> Uniforms {
        let vector = |v: Vec3, w| [v.0, v.1, v.2, w];
        let mut u: Uniforms = bytemuck::Zeroable::zeroed();
        u.material_info = [self.vertex_stride as f32, 0., 0., 0.];
        u.right = vector(p.right, 0.);
        u.up = vector(p.up, 0.);
        u.view = vector(p.view, 0.);
        u.projection = p.projection;
        u.perspective = [
            p.perspective[0],
            p.perspective[1],
            p.perspective[2],
            p.depth,
        ];
        u.light_info = [p.lighting.lights.len() as f32, p.lighting.fill, 0., 0.];
        for (i, light) in p.lighting.lights.iter().enumerate() {
            u.light_position[i] = vector(
                light.vector(p.right, p.up, p.view),
                f32::from(light.kind == crate::lighting::LightKind::Point),
            );
            u.light_colour[i] = [
                light.colour[0] * light.intensity,
                light.colour[1] * light.intensity,
                light.colour[2] * light.intensity,
                0.,
            ];
        }
        u.flags = [
            f32::from(preview.show_polygroups),
            f32::from(mode == WireMode::Transparent),
            0.,
            1.,
        ];
        if let Some(b) = &preview.brush {
            u.brush_center = vector(b.center, b.radius);
            u.brush_normal = vector(b.normal, 1.);
            u.brush_color = [
                b.color[0],
                b.color[1],
                b.color[2],
                match b.brush.alpha {
                    Alpha::Soft => 0.,
                    Alpha::Hard => 1.,
                    Alpha::Square => 2.,
                    Alpha::Image { .. } => 3.,
                },
            ];
            let (s, c) = b.brush.rotation.to_radians().sin_cos();
            u.brush_shape = [b.brush.inner_radius, b.brush.falloff, s, c];
            if let Alpha::Image {
                width,
                height,
                pixels,
            } = &b.brush.alpha
            {
                let fresh = !matches!(&self.alpha_source, Some(Alpha::Image { width:w, height:h, pixels:p }) if w == width && h == height && Arc::ptr_eq(p,pixels));
                if fresh {
                    self.alpha = Self::texture(
                        &self.device,
                        [*width as u32, *height as u32],
                        wgpu::TextureFormat::R32Float,
                        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    );
                    self.queue.write_texture(
                        self.alpha.as_image_copy(),
                        bytemuck::cast_slice(pixels),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(*width as u32 * 4),
                            rows_per_image: None,
                        },
                        self.alpha.size(),
                    );
                    self.alpha_source = Some(b.brush.alpha.clone());
                }
            }
        }
        if let Some((symmetry, opacity)) = &preview.symmetry {
            u.guide_center = vector(symmetry.center, *opacity);
            for (i, guide) in symmetry.guides().into_iter().take(32).enumerate() {
                u.right[3] = (i + 1) as f32; // Uniform guide count for derivative-safe loops.
                u.guide_normals[i] = vector(guide.normal, 1.);
                u.guide_sides[i] = vector(
                    guide.side.unwrap_or_default(),
                    f32::from(guide.side.is_some()),
                );
                let n = guide.normal;
                let rgb = if guide.side.is_some() {
                    [255., 215., 115.]
                } else if n.0.abs() > 0.99999 {
                    [255., 120., 140.]
                } else if n.1.abs() > 0.99999 {
                    [135., 245., 160.]
                } else if n.2.abs() > 0.99999 {
                    [130., 190., 255.]
                } else {
                    [255., 215., 115.]
                };
                u.guide_colors[i] = [rgb[0] / 255., rgb[1] / 255., rgb[2] / 255., 0.];
            }
        }
        u
    }
    fn bindings(&self, uniform: &wgpu::Buffer, mesh: &MeshBuffers) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport bindings"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &self.alpha.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: mesh.vertices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: mesh.indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: mesh.colors.as_entire_binding(),
                },
            ],
        })
    }
    fn target(&mut self, size: [u32; 2]) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let color = Self::texture(
            &self.device,
            size,
            COLOR,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = Self::texture(
            &self.device,
            size,
            DEPTH,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let composite = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport composite"),
            layout: &self.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.target = Some(Target {
            color,
            color_view,
            _depth: depth,
            depth_view,
            composite,
            size,
        });
    }
    #[allow(clippy::too_many_arguments)]
    fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        p: &Params,
        size: [u32; 2],
        wire: bool,
        mode: WireMode,
        triangles: bool,
        preview: &Preview,
    ) {
        if size.contains(&0) {
            return;
        }
        self.target(size);
        if wire {
            self.upload_edges();
        }
        let u = self.uniforms(p, preview, mode);
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let mut back_u = u;
        back_u.flags[2] = 1.;
        back_u.flags[3] = preview.backface_opacity;
        self.queue
            .write_buffer(&self.back_uniform, 0, bytemuck::bytes_of(&back_u));
        let bindings: Vec<_> = self
            .meshes
            .iter()
            .map(|m| self.bindings(&self.uniform, m))
            .collect();
        let back_bindings: Vec<_> = self
            .meshes
            .iter()
            .map(|m| self.bindings(&self.back_uniform, m))
            .collect();
        let target = self.target.as_ref().unwrap();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clay viewport"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: p.background[0] as f64 / 255.,
                        g: p.background[1] as f64 / 255.,
                        b: p.background[2] as f64 / 255.,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &bindings[0], &[]);
        if !wire || mode != WireMode::Transparent {
            pass.set_pipeline(if wire && mode == WireMode::Visible {
                &self.offset_surface
            } else {
                &self.surface
            });
            for (mesh, bind) in self.meshes.iter().zip(&bindings) {
                pass.set_bind_group(0, bind, &[]);
                pass.draw(0..mesh.count, 0..1);
            }
            if preview.backface_opacity > 0. {
                pass.set_pipeline(&self.backs);
                for (mesh, bind) in self.meshes.iter().zip(&back_bindings) {
                    pass.set_bind_group(0, bind, &[]);
                    pass.draw(0..mesh.count, 0..1);
                }
                pass.set_bind_group(0, &bindings[0], &[]);
            }
        }
        if wire {
            pass.set_pipeline(if mode == WireMode::Visible {
                &self.lines
            } else {
                &self.xray_lines
            });
            let edges = if triangles {
                self.triangle_edges.as_ref().unwrap()
            } else {
                self.edges.as_ref().unwrap()
            };
            for (buffer, count) in edges {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
        }
    }
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &mut self,
        p: &Params,
        size: [u32; 2],
        wire: bool,
        mode: WireMode,
        triangles: bool,
        preview: &Preview,
    ) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode(&mut encoder, p, size, wire, mode, triangles, preview);
        self.queue.submit([encoder.finish()]);
    }
}

mod display;
#[cfg(all(test, target_os = "linux"))]
mod profile;
#[cfg(test)]
mod tests;
pub(super) use display::DisplayMesh;
#[cfg(all(test, target_os = "linux"))]
mod backfaces_test;
#[cfg(all(test, target_os = "linux"))]
mod custom_ui_test;
#[cfg(all(test, target_os = "linux"))]
mod lasso_test;
#[cfg(all(test, target_os = "linux"))]
mod perspective_test;
#[cfg(all(test, target_os = "linux"))]
mod rotation_test;
#[cfg(all(test, target_os = "linux"))]
mod slice_test;
#[cfg(all(test, target_os = "linux"))]
pub(super) mod ui_capture;
#[cfg(all(test, target_os = "linux"))]
mod visibility_test;

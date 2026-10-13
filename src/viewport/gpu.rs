//! Persistent OpenGL scene buffers. Document geometry stays on the CPU.
use super::{WireMode, scene::Scene};
use crate::{
    document::Document,
    sculpt::{Alpha, Brush, Symmetry, Vec3},
};
use eframe::{
    egui,
    glow::{self, HasContext},
};
use std::sync::{Arc, Mutex};

pub(super) struct Renderer {
    revision: Option<u64>,
    geometry: Option<Arc<DisplayMesh>>,
    render_geometry: Option<Arc<DisplayMesh>>,
    gpu: Arc<Mutex<Gpu>>,
    preview: Arc<Mutex<Preview>>,
}
impl Renderer {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        Ok(Self {
            preview: Arc::new(Mutex::new(Preview::default())),
            revision: None,
            geometry: None,
            render_geometry: None,
            gpu: Arc::new(Mutex::new(Gpu::new(gl)?)),
        })
    }
    pub fn geometry(&mut self, document: &Document) -> Arc<DisplayMesh> {
        if self.revision != Some(document.display_revision()) {
            self.geometry = Some(Arc::new(DisplayMesh::new(document)));
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
        let params = Params::new(
            rect,
            scene.right,
            scene.up,
            scene.view,
            scene.scale,
            scene.center,
            geometry.depth_extent,
        );
        painter.add(egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, painter| {
                let viewport = info.viewport_in_pixels();
                let gl = painter.gl();
                let mut gpu = gpu.lock().unwrap();
                // The callback executes on the context-owning render thread.
                unsafe {
                    gpu.upload(gl, &geometry);
                    gpu.paint(
                        gl,
                        &params,
                        [viewport.width_px, viewport.height_px],
                        wire,
                        mode,
                        triangles,
                        &preview.lock().unwrap(),
                    );
                }
            })),
        });
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
                radius: radius / scene.scale,
                brush: brush.clone(),
                color: [
                    color.r() as f32 / 255.,
                    color.g() as f32 / 255.,
                    color.b() as f32 / 255.,
                ],
            });
        }
    }
    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            self.gpu.lock().unwrap().destroy(gl);
        }
    }
}

#[derive(Default)]
struct Preview {
    show_polygroups: bool,
    backface_opacity: f32,
    brush: Option<BrushPreview>,
    symmetry: Option<(Symmetry, f32)>,
}
struct BrushPreview {
    center: Vec3,
    normal: Vec3,
    radius: f32,
    brush: Brush,
    color: [f32; 3],
}

struct Params {
    right: Vec3,
    up: Vec3,
    view: Vec3,
    projection: [f32; 4],
    depth: f32,
}
impl Params {
    fn new(
        rect: egui::Rect,
        right: Vec3,
        up: Vec3,
        view: Vec3,
        scale: f32,
        center: egui::Pos2,
        extent: f32,
    ) -> Self {
        Self {
            right,
            up,
            view,
            projection: [
                scale * 2. / rect.width(),
                scale * 2. / rect.height(),
                (center.x - rect.center().x) * 2. / rect.width(),
                -(center.y - rect.center().y) * 2. / rect.height(),
            ],
            depth: 1. / extent,
        }
    }
}
struct Target {
    framebuffer: glow::Framebuffer,
    color: glow::Texture,
    depth: glow::Renderbuffer,
    size: [i32; 2],
}
struct Gpu {
    surface: glow::Program,
    lines: glow::Program,
    surface_vao: glow::VertexArray,
    vertices: glow::Buffer,
    indices: glow::Buffer,
    edge_vao: glow::VertexArray,
    edges: glow::Buffer,
    triangle_edge_vao: glow::VertexArray,
    triangle_edges: glow::Buffer,
    index_count: i32,
    edge_count: i32,
    triangle_edge_count: i32,
    uploaded: Option<Arc<DisplayMesh>>,
    edges_uploaded: bool,
    target: Option<Target>,
    destroyed: bool,
    poly_texture: glow::Texture,
    poly_available: bool,
    alpha_texture: Option<glow::Texture>,
    alpha_source: Option<Alpha>,
}
fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}
fn program(gl: &glow::Context, vertex: &str, fragment: &str) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        let result = (|| {
            for (kind, source) in [
                (glow::VERTEX_SHADER, vertex),
                (glow::FRAGMENT_SHADER, fragment),
            ] {
                let shader = gl.create_shader(kind)?;
                shaders.push(shader);
                gl.attach_shader(program, shader);
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    return Err(gl.get_shader_info_log(shader));
                }
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }
            Ok(program)
        })();
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        if result.is_err() {
            gl.delete_program(program);
        }
        result
    }
}
impl Gpu {
    fn new(gl: &glow::Context) -> Result<Self, String> {
        let version = gl.version();
        if version.is_embedded || version.major < 3 || (version.major == 3 && version.minor < 3) {
            return Err("Clay Form requires desktop OpenGL 3.3 or newer".into());
        }
        unsafe {
            let surface = program(gl, SURFACE_VERTEX, SURFACE_FRAGMENT)?;
            let lines = match program(gl, LINE_VERTEX, LINE_FRAGMENT) {
                Ok(lines) => lines,
                Err(e) => {
                    gl.delete_program(surface);
                    return Err(e);
                }
            };
            // All objects are created and deleted while this GL context is current.
            let surface_vao = gl.create_vertex_array()?;
            let vertices = gl.create_buffer()?;
            let indices = gl.create_buffer()?;
            let edge_vao = gl.create_vertex_array()?;
            let edges = gl.create_buffer()?;
            let triangle_edge_vao = gl.create_vertex_array()?;
            let triangle_edges = gl.create_buffer()?;
            Ok(Self {
                surface,
                lines,
                surface_vao,
                vertices,
                indices,
                edge_vao,
                edges,
                triangle_edge_vao,
                triangle_edges,
                index_count: 0,
                edge_count: 0,
                triangle_edge_count: 0,
                uploaded: None,
                edges_uploaded: false,
                target: None,
                destroyed: false,
                poly_texture: gl.create_texture()?,
                poly_available: false,
                alpha_texture: None,
                alpha_source: None,
            })
        }
    }
    unsafe fn upload(&mut self, gl: &glow::Context, data: &Arc<DisplayMesh>) {
        if self.destroyed {
            return;
        }
        if self
            .uploaded
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, data))
        {
            return;
        }
        unsafe {
            gl.bind_vertex_array(Some(self.surface_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vertices));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &bytes(&data.vertices()),
                glow::DYNAMIC_DRAW,
            );
            for (attribute, size, offset) in [(0, 3, 0), (1, 3, 12), (2, 3, 24), (3, 1, 36)] {
                gl.enable_vertex_attrib_array(attribute);
                gl.vertex_attrib_pointer_f32(attribute, size, glow::FLOAT, false, 40, offset);
            }
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.indices));
            let indices: Vec<u8> = data
                .shading
                .triangles
                .iter()
                .flatten()
                .flat_map(|&i| (i as u32).to_ne_bytes())
                .collect();
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, &indices, glow::DYNAMIC_DRAW);
            self.index_count = (data.mesh.triangles.len() * 3) as i32;
            let limit = gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE) as usize;
            let mut width = 4096.min(limit);
            let mut height = data.poly_colors.len().div_ceil(width).max(1);
            self.poly_available = height <= limit;
            let mut colors: Vec<u8> = if self.poly_available {
                data.poly_colors.iter().flatten().copied().collect()
            } else {
                width = 1;
                height = 1;
                vec![0; 4]
            };
            colors.resize(width * height * 4, 0);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.poly_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&colors)),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        self.edges_uploaded = false;
        self.uploaded = Some(data.clone());
    }
    unsafe fn upload_edges(&mut self, gl: &glow::Context) {
        if self.edges_uploaded {
            return;
        }
        let data = self.uploaded.as_ref().unwrap();
        unsafe {
            for (triangles, vao, buffer) in [
                (false, self.edge_vao, self.edges),
                (true, self.triangle_edge_vao, self.triangle_edges),
            ] {
                let edges = data.edges(triangles);
                if triangles {
                    self.triangle_edge_count = (edges.len() / 12) as i32;
                } else {
                    self.edge_count = (edges.len() / 12) as i32;
                }
                gl.bind_vertex_array(Some(vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes(&edges), glow::DYNAMIC_DRAW);
                for (attribute, offset) in [(0, 0), (1, 12), (2, 24), (3, 36)] {
                    gl.enable_vertex_attrib_array(attribute);
                    gl.vertex_attrib_pointer_f32(attribute, 3, glow::FLOAT, false, 48, offset);
                }
            }
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        self.edges_uploaded = true;
    }
    unsafe fn uniforms(gl: &glow::Context, program: glow::Program, p: &Params) {
        unsafe {
            gl.use_program(Some(program));
            for (name, v) in [("u_right", p.right), ("u_up", p.up), ("u_view", p.view)] {
                gl.uniform_3_f32(
                    gl.get_uniform_location(program, name).as_ref(),
                    v.0,
                    v.1,
                    v.2,
                );
            }
            gl.uniform_4_f32_slice(
                gl.get_uniform_location(program, "u_projection").as_ref(),
                &p.projection,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(program, "u_depth").as_ref(),
                p.depth,
            );
        }
    }
    unsafe fn preview_uniforms(&mut self, gl: &glow::Context, preview: &Preview) {
        unsafe {
            let program = self.surface;
            let location = |name: &str| gl.get_uniform_location(program, name);
            gl.uniform_1_i32(
                location("u_show_polygroups").as_ref(),
                i32::from(preview.show_polygroups && self.poly_available),
            );
            gl.uniform_1_i32(location("u_poly_colors").as_ref(), 1);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.poly_texture));
            gl.active_texture(glow::TEXTURE0);
            let vec3 =
                |name: &str, v: Vec3| gl.uniform_3_f32(location(name).as_ref(), v.0, v.1, v.2);
            gl.uniform_1_i32(
                location("u_brush_enabled").as_ref(),
                i32::from(preview.brush.is_some()),
            );
            if let Some(b) = &preview.brush {
                vec3("u_brush_center", b.center);
                vec3("u_brush_normal", b.normal);
                gl.uniform_3_f32_slice(location("u_brush_color").as_ref(), &b.color);
                gl.uniform_1_f32(location("u_brush_radius").as_ref(), b.radius);
                let (s, c) = b.brush.rotation.to_radians().sin_cos();
                gl.uniform_4_f32(
                    location("u_brush_shape").as_ref(),
                    b.brush.inner_radius,
                    b.brush.falloff,
                    s,
                    c,
                );
                let kind = match &b.brush.alpha {
                    Alpha::Soft => 0,
                    Alpha::Hard => 1,
                    Alpha::Square => 2,
                    Alpha::Image { .. } => 3,
                };
                gl.uniform_1_i32(location("u_alpha_kind").as_ref(), kind);
                if let Alpha::Image {
                    width,
                    height,
                    pixels,
                } = &b.brush.alpha
                {
                    let fresh = !matches!(&self.alpha_source, Some(Alpha::Image { width:w,height:h,pixels:p })
                        if w==width && h==height && Arc::ptr_eq(p,pixels));
                    let texture = *self.alpha_texture.get_or_insert_with(|| {
                        gl.create_texture().expect("create brush alpha texture")
                    });
                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                    if fresh {
                        let data = bytes(pixels);
                        gl.tex_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glow::R32F as i32,
                            *width as i32,
                            *height as i32,
                            0,
                            glow::RED,
                            glow::FLOAT,
                            glow::PixelUnpackData::Slice(Some(&data)),
                        );
                        for (name, value) in [
                            (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                            (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
                        ] {
                            gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
                        }
                        self.alpha_source = Some(b.brush.alpha.clone());
                    }
                    gl.uniform_1_i32(location("u_alpha_image").as_ref(), 0);
                }
            }
            let mut normals = Vec::new();
            let mut sides = Vec::new();
            let mut colors = Vec::new();
            if let Some((symmetry, opacity)) = &preview.symmetry {
                vec3("u_guide_center", symmetry.center);
                gl.uniform_1_f32(location("u_guide_opacity").as_ref(), *opacity);
                for guide in symmetry.guides().into_iter().take(32) {
                    let n = guide.normal;
                    normals.extend([n.0, n.1, n.2]);
                    let side = guide.side.unwrap_or_default();
                    sides.extend([
                        side.0,
                        side.1,
                        side.2,
                        if guide.side.is_some() { 1. } else { 0. },
                    ]);
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
                    colors.extend(rgb.map(|v| v / 255.));
                }
            }
            gl.uniform_1_i32(
                location("u_guide_count").as_ref(),
                (normals.len() / 3) as i32,
            );
            if !normals.is_empty() {
                gl.uniform_3_f32_slice(location("u_guide_normals[0]").as_ref(), &normals);
                gl.uniform_4_f32_slice(location("u_guide_sides[0]").as_ref(), &sides);
                gl.uniform_3_f32_slice(location("u_guide_colors[0]").as_ref(), &colors);
            }
        }
    }
    unsafe fn target(&mut self, gl: &glow::Context, size: [i32; 2]) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        unsafe {
            if let Some(target) = self.target.take() {
                target.destroy(gl);
            }
            let framebuffer = gl
                .create_framebuffer()
                .expect("create viewport framebuffer");
            let color = gl.create_texture().expect("create viewport texture");
            let depth = gl
                .create_renderbuffer()
                .expect("create viewport depth buffer");
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.bind_texture(glow::TEXTURE_2D, Some(color));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                size[0],
                size[1],
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(color),
                0,
            );
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
            gl.renderbuffer_storage(
                glow::RENDERBUFFER,
                glow::DEPTH_COMPONENT24,
                size[0],
                size[1],
            );
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                glow::DEPTH_ATTACHMENT,
                glow::RENDERBUFFER,
                Some(depth),
            );
            assert_eq!(
                gl.check_framebuffer_status(glow::FRAMEBUFFER),
                glow::FRAMEBUFFER_COMPLETE
            );
            self.target = Some(Target {
                framebuffer,
                color,
                depth,
                size,
            });
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn paint(
        &mut self,
        gl: &glow::Context,
        params: &Params,
        size: [i32; 2],
        wire: bool,
        mode: WireMode,
        triangles: bool,
        preview: &Preview,
    ) {
        if size[0] <= 0 || size[1] <= 0 || self.destroyed {
            return;
        }
        unsafe {
            let draw = gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING);
            let read = gl.get_parameter_framebuffer(glow::READ_FRAMEBUFFER_BINDING);
            let viewport = {
                let mut value = [0; 4];
                gl.get_parameter_i32_slice(glow::VIEWPORT, &mut value);
                value
            };
            let scissor = gl.is_enabled(glow::SCISSOR_TEST);
            let srgb = gl.is_enabled(glow::FRAMEBUFFER_SRGB);
            self.target(gl, size);
            gl.bind_framebuffer(
                glow::FRAMEBUFFER,
                Some(self.target.as_ref().unwrap().framebuffer),
            );
            gl.viewport(0, 0, size[0], size[1]);
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::FRAMEBUFFER_SRGB);
            gl.disable(glow::BLEND);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_mask(true);
            gl.depth_func(glow::LESS);
            gl.clear_depth_f32(1.);
            gl.clear_color(23. / 255., 26. / 255., 31. / 255., 1.);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            if wire && mode == WireMode::Visible {
                // Offset filled faces by their depth slope so coplanar lines
                // survive pixel sampling on tilted surfaces without showing backs.
                gl.enable(glow::POLYGON_OFFSET_FILL);
                gl.polygon_offset(1., 1.);
            }
            if !wire || mode != WireMode::Transparent {
                gl.enable(glow::CULL_FACE);
                gl.cull_face(glow::BACK);
                gl.front_face(glow::CCW);
                Self::uniforms(gl, self.surface, params);
                self.preview_uniforms(gl, preview);
                gl.bind_vertex_array(Some(self.surface_vao));
                gl.uniform_1_i32(
                    gl.get_uniform_location(self.surface, "u_backface").as_ref(),
                    0,
                );
                gl.uniform_1_f32(
                    gl.get_uniform_location(self.surface, "u_surface_opacity")
                        .as_ref(),
                    1.,
                );
                gl.draw_elements(glow::TRIANGLES, self.index_count, glow::UNSIGNED_INT, 0);
                if preview.backface_opacity > 0. {
                    // Ghost backs over visible front surfaces behind them, but
                    // never let a transparent back occlude those surfaces.
                    gl.cull_face(glow::FRONT);
                    gl.depth_func(glow::LEQUAL);
                    gl.depth_mask(false);
                    gl.enable(glow::BLEND);
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func_separate(
                        glow::SRC_ALPHA,
                        glow::ONE_MINUS_SRC_ALPHA,
                        glow::ONE,
                        glow::ONE_MINUS_SRC_ALPHA,
                    );
                    gl.uniform_1_i32(
                        gl.get_uniform_location(self.surface, "u_backface").as_ref(),
                        1,
                    );
                    gl.uniform_1_f32(
                        gl.get_uniform_location(self.surface, "u_surface_opacity")
                            .as_ref(),
                        preview.backface_opacity,
                    );
                    gl.draw_elements(glow::TRIANGLES, self.index_count, glow::UNSIGNED_INT, 0);
                    gl.depth_mask(true);
                    gl.disable(glow::BLEND);
                    gl.cull_face(glow::BACK);
                }
            }
            gl.disable(glow::POLYGON_OFFSET_FILL);
            if wire {
                self.upload_edges(gl);
                gl.depth_func(glow::LEQUAL);
                gl.disable(glow::CULL_FACE);
                if mode != WireMode::Visible {
                    gl.disable(glow::DEPTH_TEST);
                }
                gl.depth_mask(false);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                Self::uniforms(gl, self.lines, params);
                gl.uniform_1_i32(
                    gl.get_uniform_location(self.lines, "u_transparent")
                        .as_ref(),
                    i32::from(mode == WireMode::Transparent),
                );
                gl.bind_vertex_array(Some(if triangles {
                    self.triangle_edge_vao
                } else {
                    self.edge_vao
                }));
                gl.line_width(1.);
                gl.draw_arrays(
                    glow::LINES,
                    0,
                    if triangles {
                        self.triangle_edge_count
                    } else {
                        self.edge_count
                    },
                );
            }
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.depth_mask(true);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, draw);
            // Restore the clip before compositing; egui may clip the viewport.
            if scissor {
                gl.enable(glow::SCISSOR_TEST);
            }
            gl.blit_framebuffer(
                0,
                0,
                size[0],
                size[1],
                viewport[0],
                viewport[1],
                viewport[0] + viewport[2],
                viewport[1] + viewport[3],
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, read);
            gl.viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_renderbuffer(glow::RENDERBUFFER, None);
            if srgb {
                gl.enable(glow::FRAMEBUFFER_SRGB);
            }
            // egui restores its shader, VAO, blending and texture bindings.
        }
    }
    unsafe fn destroy(&mut self, gl: &glow::Context) {
        if self.destroyed {
            return;
        }
        unsafe {
            if let Some(target) = self.target.take() {
                target.destroy(gl);
            }
            gl.delete_texture(self.poly_texture);
            if let Some(texture) = self.alpha_texture.take() {
                gl.delete_texture(texture);
            }
            gl.delete_program(self.surface);
            gl.delete_program(self.lines);
            for vao in [self.surface_vao, self.edge_vao, self.triangle_edge_vao] {
                gl.delete_vertex_array(vao);
            }
            for buffer in [self.vertices, self.indices, self.edges, self.triangle_edges] {
                gl.delete_buffer(buffer);
            }
        }
        self.destroyed = true;
        self.uploaded = None;
    }
}
impl Target {
    unsafe fn destroy(self, gl: &glow::Context) {
        unsafe {
            gl.delete_framebuffer(self.framebuffer);
            gl.delete_texture(self.color);
            gl.delete_renderbuffer(self.depth);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, target_os = "linux"))]
mod profile;

mod display;
mod shaders;
pub(super) use display::DisplayMesh;
use shaders::{LINE_FRAGMENT, LINE_VERTEX, SURFACE_FRAGMENT, SURFACE_VERTEX};

#[cfg(all(test, target_os = "linux"))]
mod backfaces_test;

#[cfg(all(test, target_os = "linux"))]
mod slice_test;

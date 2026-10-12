use super::*;
use crate::{
    camera::Camera,
    sculpt::{Geometry, Primitive, PrimitiveKind, Transform},
};

#[test]
fn display_cache_inputs_keep_quad_edges_and_model_ownership() {
    let mut document = Document::default();
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [1; 3];
    document
        .replace_geometry(Geometry::new(cube.mesh().unwrap()))
        .unwrap();
    let display = DisplayMesh::new(&document);
    assert_eq!(display.edges(false).len() / 18, 12);
    assert_eq!(display.edges(true).len() / 18, 18);
    assert_eq!(display.active_vertices, 8);
    document.duplicate_model(false).unwrap();
    let display = DisplayMesh::new(&document);
    assert_eq!(display.mesh.vertices.len(), 16);
    assert!(display.owners.contains(&0) && display.owners.contains(&1));
}

/// Real shader compilation and framebuffer readback, without opening a window.
/// Uses the installed EGL implementation; run explicitly on Linux graphics hosts.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_offscreen_depth_wireframe_and_cache() {
    unsafe {
        let egl = Egl::new();
        let gl = glow::Context::from_loader_function(|name| {
            let name = std::ffi::CString::new(name).unwrap();
            (egl.get_proc)(name.as_ptr())
        });
        eprintln!(
            "OpenGL: {} / {}",
            gl.get_parameter_string(glow::VERSION),
            gl.get_parameter_string(glow::RENDERER)
        );
        let mut renderer = Renderer::new(&gl).unwrap();
        let mut document = Document::default();
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [1; 3];
        document
            .replace_geometry(Geometry::new(plane.mesh().unwrap()))
            .unwrap();
        let first = renderer.geometry(&document);
        assert!(Arc::ptr_eq(&first, &renderer.geometry(&document)));
        let mut brush = document.brushes()[0].clone();
        brush.rotation = 45.;
        document.update_brush(0, brush).unwrap();
        assert!(
            Arc::ptr_eq(&first, &renderer.geometry(&document)),
            "brush edits must not upload mesh buffers"
        );
        document.duplicate_model(false).unwrap();
        document
            .apply_transform(
                Transform {
                    translation: Vec3(0., 0., 0.4),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        document.end_stroke();
        document.select_submodel(1).unwrap();
        let display = renderer.geometry(&document);
        assert!(!Arc::ptr_eq(&first, &display));
        let camera = Camera::default();
        let (right, up, view) = camera.basis();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.));
        let params = Params::new(
            rect,
            right,
            up,
            view,
            89.6,
            rect.center(),
            display.depth_extent,
        );
        let handle = renderer.gpu.clone();
        let mut gpu = handle.lock().unwrap();
        gpu.upload(&gl, &display);
        gl.viewport(0, 0, 256, 256);
        let read = |gpu: &Gpu| {
            let mut rgba = vec![0; 256 * 256 * 4];
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(gpu.target.as_ref().unwrap().framebuffer),
            );
            gl.read_pixels(
                0,
                0,
                256,
                256,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut rgba)),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            rgba
        };
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let solid = read(&gpu);
        let center = (128 * 256 + 128) * 4;
        assert!(
            solid[center + 2] > solid[center],
            "inactive foreground must occlude clay behind it"
        );
        assert_eq!(&solid[..4], &[23, 26, 31, 255]);
        let uploaded = gpu.uploaded.clone().unwrap();
        gpu.upload(&gl, &display);
        assert!(Arc::ptr_eq(&uploaded, gpu.uploaded.as_ref().unwrap()));
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            true,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let visible = read(&gpu);
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            true,
            WireMode::Front,
            false,
            &Preview::default(),
        );
        let front = read(&gpu);
        assert!(
            visible != front,
            "visible mode rejects the occluded rear edges; front mode draws them"
        );
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            true,
            WireMode::Transparent,
            false,
            &Preview::default(),
        );
        let transparent = read(&gpu);
        assert_eq!(
            &transparent[center..center + 4],
            &[23, 26, 31, 255],
            "transparent wireframe has no surface fill"
        );
        assert!(transparent.chunks_exact(4).any(|p| p != [23, 26, 31, 255]));
        let make_preview = |alpha: Alpha| Preview {
            backface_opacity: 0.,
            brush: Some(BrushPreview {
                center: Vec3(0., 0., 0.),
                normal: Vec3(0., 0., 1.),
                radius: 0.5,
                brush: {
                    let mut b = Brush::new(crate::sculpt::Kind::Standard);
                    b.alpha = alpha;
                    b
                },
                color: [0.2, 0.8, 1.],
            }),
            symmetry: None,
        };
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &make_preview(Alpha::Hard),
        );
        assert!(
            read(&gpu) == solid,
            "alpha must not paint inactive foreground models"
        );
        document.select_submodel(0).unwrap();
        let active = renderer.geometry(&document);
        gpu.upload(&gl, &active);
        let mut preview = make_preview(Alpha::Hard);
        preview.brush.as_mut().unwrap().center.2 = 0.4;
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let active_solid = read(&gpu);
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &preview,
        );
        assert!(
            read(&gpu)[center + 2] > active_solid[center + 2],
            "hard alpha changes active surface shading"
        );
        preview.brush.as_mut().unwrap().brush.alpha = Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![0.; 4].into(),
        };
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &preview,
        );
        assert!(
            read(&gpu) == active_solid,
            "black image alpha must preserve shading"
        );
        preview.brush.as_mut().unwrap().brush.alpha = Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![1.; 4].into(),
        };
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &preview,
        );
        assert!(
            read(&gpu)[center + 2] > active_solid[center + 2],
            "replacement alpha texture must upload"
        );
        // Compare shader alpha against CPU sculpt weights at an actual pixel.
        for alpha in [
            Alpha::Soft,
            Alpha::Hard,
            Alpha::Square,
            Alpha::Image {
                width: 2,
                height: 2,
                pixels: vec![1., 0., 0., 0.].into(),
            },
        ] {
            let mut preview = make_preview(alpha);
            let b = preview.brush.as_mut().unwrap();
            b.center.2 = 0.4;
            b.brush.inner_radius = 0.35;
            b.brush.rotation = 35.;
            let q = 0.5 / 89.6 / b.radius;
            let weight = b.brush.weight(q, q);
            let tint = b.color;
            gpu.paint(
                &gl,
                &params,
                [256, 256],
                false,
                WireMode::Visible,
                false,
                &preview,
            );
            let rgba = read(&gpu);
            for channel in 0..3 {
                let expected = active_solid[center + channel] as f32 * (1. - weight * 90. / 255.)
                    + tint[channel] * weight * 90.;
                assert!(
                    (rgba[center + channel] as f32 - expected).abs() < 2.,
                    "GPU alpha must match sculpting alpha"
                );
            }
        }
        let symmetry = Symmetry {
            enabled: true,
            ..Default::default()
        };
        let preview = Preview {
            backface_opacity: 0.,
            brush: None,
            symmetry: Some((symmetry, 1.)),
        };
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &preview,
        );
        assert!(
            read(&gpu)[center] > active_solid[center],
            "mirror guide appears on the surface"
        );
        gpu.paint(
            &gl,
            &params,
            [128, 128],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        assert_eq!(gpu.target.as_ref().unwrap().size, [128, 128]);
        assert_eq!(gl.get_error(), glow::NO_ERROR);
        gpu.destroy(&gl);
        gpu.destroy(&gl); // shutdown is safe even when a callback retains the handle
        drop(gpu);
        drop(renderer);
        let gl = Arc::new(gl);
        let mut painter = eframe::egui_glow::Painter::new(gl.clone(), "", None, false).unwrap();
        let mut renderer = Renderer::new(&gl).unwrap();
        let document = Document::default();
        let mut camera = Camera::default();
        camera.orbit(egui::vec2(50., 20.));
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                let model_rect = rect.shrink(24.);
                let scene = Scene::paint_with_renderer(
                    &painter,
                    model_rect,
                    &document,
                    &camera,
                    false,
                    WireMode::Visible,
                    false,
                    Some(&mut renderer),
                );
                let brush = Brush::new(crate::sculpt::Kind::Standard);
                renderer.set_brush(
                    &scene,
                    egui::pos2(155., 128.),
                    45.,
                    &brush,
                    egui::Color32::from_rgb(125, 210, 245),
                );
                renderer.set_guides(
                    &Symmetry {
                        enabled: true,
                        ..Default::default()
                    },
                    1.,
                );
                painter.rect_filled(
                    egui::Rect::from_min_max(egui::pos2(30., 30.), egui::pos2(42., 42.)),
                    0.,
                    egui::Color32::RED,
                );
                painter.text(
                    egui::pos2(30., 216.),
                    egui::Align2::LEFT_TOP,
                    "GPU viewport",
                    egui::FontId::proportional(14.),
                    egui::Color32::WHITE,
                );
            },
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.clear_color(23. / 255., 26. / 255., 31. / 255., 1.);
        gl.clear(glow::COLOR_BUFFER_BIT);
        let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
        painter.paint_and_update_textures(
            [256, 256],
            output.pixels_per_point,
            &shapes,
            &output.textures_delta,
        );
        let mut rgba = vec![0; 256 * 256 * 4];
        gl.read_pixels(
            0,
            0,
            256,
            256,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut rgba)),
        );
        let overlay = ((256 - 36) * 256 + 36) * 4;
        assert_eq!(
            &rgba[overlay..overlay + 4],
            &[255, 0, 0, 255],
            "egui overlays must survive the callback"
        );
        assert!(rgba[center] > 40, "model callback must render inside egui");
        assert_eq!(gl.get_error(), glow::NO_ERROR);
        if let Ok(path) = std::env::var("CLAY_GPU_PREVIEW") {
            let mut flipped = Vec::with_capacity(rgba.len());
            for row in rgba.chunks_exact(256 * 4).rev() {
                flipped.extend_from_slice(row);
            }
            image::save_buffer(path, &flipped, 256, 256, image::ColorType::Rgba8).unwrap();
        }
        // Face-less primitive outlines are egui overlays and must be painted
        // after the GPU callback's background, not erased by it.
        let mut circle = Primitive::for_kind(PrimitiveKind::Circle);
        circle.filled = false;
        let mut document = Document::default();
        document
            .replace_geometry(Geometry::new(circle.mesh().unwrap()))
            .unwrap();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                Scene::paint_with_renderer(
                    &painter,
                    rect,
                    &document,
                    &Camera::default(),
                    false,
                    WireMode::Visible,
                    false,
                    Some(&mut renderer),
                );
            },
        );
        gl.clear(glow::COLOR_BUFFER_BIT);
        let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
        painter.paint_and_update_textures(
            [256, 256],
            output.pixels_per_point,
            &shapes,
            &output.textures_delta,
        );
        gl.read_pixels(
            0,
            0,
            256,
            256,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut rgba)),
        );
        assert!(
            rgba.chunks_exact(4).filter(|p| p[0] > 80).count() > 100,
            "unfilled circle outline must remain visible"
        );
        assert_eq!(gl.get_error(), glow::NO_ERROR);
        let cache = renderer.geometry(&document);
        document.new_sphere();
        assert!(
            !Arc::ptr_eq(&cache, &renderer.geometry(&document)),
            "New must invalidate cached geometry"
        );
        let cache = renderer.geometry(&document);
        document.initialize_primitive().unwrap();
        document.undo();
        assert!(
            !Arc::ptr_eq(&cache, &renderer.geometry(&document)),
            "undo must invalidate cached geometry"
        );
        assert_eq!(
            renderer.geometry(&document).mesh.vertices,
            document.mesh().vertices
        );
        document.invert_mask().unwrap();
        document.delete_masked().unwrap();
        let empty = renderer.geometry(&document);
        assert!(empty.mesh.vertices.is_empty());
        {
            let mut gpu = renderer.gpu.lock().unwrap();
            gpu.upload(&gl, &empty);
            gl.viewport(0, 0, 256, 256);
            for (wire, mode) in [
                (false, WireMode::Visible),
                (true, WireMode::Visible),
                (true, WireMode::Transparent),
            ] {
                gpu.paint(
                    &gl,
                    &params,
                    [256, 256],
                    wire,
                    mode,
                    false,
                    &Preview::default(),
                );
            }
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(gpu.target.as_ref().unwrap().framebuffer),
            );
            gl.read_pixels(
                0,
                0,
                256,
                256,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut rgba)),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            assert!(
                rgba.chunks_exact(4).all(|p| p == [23, 26, 31, 255]),
                "an empty model must render a clean background"
            );
            assert_eq!(gl.get_error(), glow::NO_ERROR);
        }
        let handle = renderer.gpu.clone();
        let mut viewport = super::super::Viewport {
            renderer: Some(renderer),
            ..Default::default()
        };
        viewport.cancel(&mut document);
        assert!(
            Arc::ptr_eq(&handle, &viewport.renderer.as_ref().unwrap().gpu),
            "gesture cancellation must keep GL resources"
        );
        let renderer = viewport.renderer.take().unwrap();
        renderer.destroy(&gl);
        painter.destroy();
        drop(painter);
        drop(renderer);
        drop(gl);
    }
}

#[cfg(target_os = "linux")]
static EGL_LOCK: Mutex<()> = Mutex::new(());
#[cfg(target_os = "linux")]
pub(super) struct Egl {
    _lock: std::sync::MutexGuard<'static, ()>,
    library: *mut libc::c_void,
    display: *mut libc::c_void,
    context: *mut libc::c_void,
    surface: *mut libc::c_void,
    pub(super) get_proc: unsafe extern "C" fn(*const libc::c_char) -> *const libc::c_void,
    make_current: unsafe extern "C" fn(
        *mut libc::c_void,
        *mut libc::c_void,
        *mut libc::c_void,
        *mut libc::c_void,
    ) -> u32,
    destroy_context: unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> u32,
    destroy_surface: unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> u32,
    terminate: unsafe extern "C" fn(*mut libc::c_void) -> u32,
}
#[cfg(target_os = "linux")]
impl Egl {
    pub(super) unsafe fn new() -> Self {
        let lock = EGL_LOCK.lock().unwrap();
        unsafe {
            let library = libc::dlopen(c"libEGL.so.1".as_ptr(), libc::RTLD_NOW);
            assert!(!library.is_null(), "libEGL.so.1 is required");
            macro_rules! symbol {
                ($name:literal,$ty:ty) => {{
                    let pointer = libc::dlsym(library, concat!($name, "\0").as_ptr().cast());
                    assert!(!pointer.is_null(), "missing EGL entry point");
                    std::mem::transmute::<*mut libc::c_void, $ty>(pointer)
                }};
            }
            type Ptr = *mut libc::c_void;
            let get_display = symbol!(
                "eglGetPlatformDisplay",
                unsafe extern "C" fn(u32, Ptr, *const isize) -> Ptr
            );
            let initialize = symbol!(
                "eglInitialize",
                unsafe extern "C" fn(Ptr, *mut i32, *mut i32) -> u32
            );
            let choose = symbol!(
                "eglChooseConfig",
                unsafe extern "C" fn(Ptr, *const i32, *mut Ptr, i32, *mut i32) -> u32
            );
            let bind = symbol!("eglBindAPI", unsafe extern "C" fn(u32) -> u32);
            let create_context = symbol!(
                "eglCreateContext",
                unsafe extern "C" fn(Ptr, Ptr, Ptr, *const i32) -> Ptr
            );
            let create_surface = symbol!(
                "eglCreatePbufferSurface",
                unsafe extern "C" fn(Ptr, Ptr, *const i32) -> Ptr
            );
            let make_current = symbol!(
                "eglMakeCurrent",
                unsafe extern "C" fn(Ptr, Ptr, Ptr, Ptr) -> u32
            );
            let display = get_display(0x31DD, std::ptr::null_mut(), std::ptr::null()); // EGL_PLATFORM_SURFACELESS_MESA
            assert!(!display.is_null());
            assert_eq!(
                initialize(display, std::ptr::null_mut(), std::ptr::null_mut()),
                1
            );
            assert_eq!(bind(0x30A2), 1); // EGL_OPENGL_API
            let attributes = [
                0x3033, 1, 0x3040, 8, 0x3024, 8, 0x3023, 8, 0x3022, 8, 0x3021, 8, 0x3038,
            ];
            let mut config = std::ptr::null_mut();
            let mut count = 0;
            assert_eq!(
                choose(display, attributes.as_ptr(), &mut config, 1, &mut count),
                1
            );
            assert!(count > 0);
            let context_attributes = [0x3098, 3, 0x30FB, 3, 0x30FD, 1, 0x3038]; // OpenGL 3.3 core
            let context = create_context(
                display,
                config,
                std::ptr::null_mut(),
                context_attributes.as_ptr(),
            );
            assert!(!context.is_null());
            let surface =
                create_surface(display, config, [0x3057, 256, 0x3056, 256, 0x3038].as_ptr());
            assert!(!surface.is_null());
            assert_eq!(make_current(display, surface, surface, context), 1);
            Self {
                _lock: lock,
                library,
                display,
                context,
                surface,
                make_current,
                get_proc: symbol!(
                    "eglGetProcAddress",
                    unsafe extern "C" fn(*const libc::c_char) -> *const libc::c_void
                ),
                destroy_context: symbol!(
                    "eglDestroyContext",
                    unsafe extern "C" fn(Ptr, Ptr) -> u32
                ),
                destroy_surface: symbol!(
                    "eglDestroySurface",
                    unsafe extern "C" fn(Ptr, Ptr) -> u32
                ),
                terminate: symbol!("eglTerminate", unsafe extern "C" fn(Ptr) -> u32),
            }
        }
    }
}
#[cfg(target_os = "linux")]
impl Drop for Egl {
    fn drop(&mut self) {
        unsafe {
            (self.make_current)(
                self.display,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            (self.destroy_surface)(self.display, self.surface);
            (self.destroy_context)(self.display, self.context);
            (self.terminate)(self.display);
            libc::dlclose(self.library);
        }
    }
}

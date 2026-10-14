use super::{tests::Egl, *};
use crate::{
    camera::Camera,
    sculpt::{Geometry, Primitive, PrimitiveKind},
};

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_backfaces_are_faint_depth_tested_and_do_not_modify_geometry() {
    unsafe {
        let egl = Egl::new();
        let gl = glow::Context::from_loader_function(|name| {
            let name = std::ffi::CString::new(name).unwrap();
            (egl.get_proc)(name.as_ptr())
        });
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [1; 3];
        let mut mesh = plane.mesh().unwrap();
        for t in &mut mesh.triangles {
            t.swap(1, 2);
        }
        for q in &mut mesh.quads {
            q.swap(1, 3);
        }
        let mut document = Document::default();
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        let mut renderer = Renderer::new(&gl).unwrap();
        let display = renderer.geometry(&document);
        let original = document.geometry_snapshot();
        let (right, up, view) = Camera::default().basis();
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
        let sample = |gpu: &Gpu| {
            let mut color = [0; 4];
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(gpu.target.as_ref().unwrap().framebuffer),
            );
            gl.read_pixels(
                128,
                128,
                1,
                1,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut color)),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            color
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
        assert_eq!(
            sample(&gpu),
            [23, 26, 31, 255],
            "back faces are hidden by default"
        );
        let faint = Preview {
            backface_opacity: 0.15,
            ..Default::default()
        };
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &faint,
        );
        let low = sample(&gpu);
        assert!(
            low[0] > 23 && low[0] < 65,
            "back surface is present but faint"
        );
        assert_eq!(
            low[3], 255,
            "ghosts must not make the composited window transparent"
        );
        let brighter = Preview {
            backface_opacity: 0.4,
            ..Default::default()
        };
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &brighter,
        );
        assert!(
            sample(&gpu)[0] > low[0],
            "opacity slider changes the faint surface"
        );
        assert_eq!(document.geometry_snapshot(), original);
        assert!(
            Arc::ptr_eq(&display, &renderer.geometry(&document)),
            "view settings must not rebuild geometry"
        );

        // Put an opaque front-facing copy behind the translucent back.
        document.duplicate_model(false).unwrap();
        let mut geometry = document.geometry_snapshot();
        for p in &mut geometry.mesh.vertices {
            p.0 *= 0.45;
            p.1 *= 0.45;
            p.2 = -0.4;
        }
        for t in &mut geometry.mesh.triangles {
            t.swap(1, 2);
        }
        for q in &mut geometry.mesh.quads {
            q.swap(1, 3);
        }
        document.replace_geometry(geometry).unwrap();
        let display = renderer.geometry(&document);
        gpu.upload(&gl, &display);
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let behind = sample(&gpu);
        assert!(
            behind[0] > 100,
            "front surface behind the back stays visible"
        );
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &faint,
        );
        let combined = sample(&gpu);
        assert!(
            combined[0] > 100 && combined != behind,
            "faint back blends over the visible surface behind it"
        );
        if let Ok(path) = std::env::var("CLAY_BACKFACE_PREVIEW") {
            let mut pixels = vec![0; 256 * 256 * 4];
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
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            let mut flipped = Vec::with_capacity(pixels.len());
            for row in pixels.as_chunks::<{ 256 * 4 }>().0.iter().rev() {
                flipped.extend_from_slice(row);
            }
            image::save_buffer(path, &flipped, 256, 256, image::ColorType::Rgba8).unwrap();
        }
        // Move that opaque surface ahead: it must hide the back completely.
        let mut geometry = document.geometry_snapshot();
        for p in &mut geometry.mesh.vertices {
            p.2 = 0.4;
        }
        document.replace_geometry(geometry).unwrap();
        let display = renderer.geometry(&document);
        gpu.upload(&gl, &display);
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let front = sample(&gpu);
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &faint,
        );
        assert_eq!(
            sample(&gpu),
            front,
            "opaque foreground hides backs behind it"
        );
        document.new_sphere();
        let display = renderer.geometry(&document);
        gpu.upload(&gl, &display);
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &Preview::default(),
        );
        let sphere = sample(&gpu);
        gpu.paint(
            &gl,
            &params,
            [256, 256],
            false,
            WireMode::Visible,
            false,
            &faint,
        );
        assert_eq!(sample(&gpu), sphere, "a closed sphere stays solid");
        assert_eq!(gl.get_error(), glow::NO_ERROR);
        gpu.destroy(&gl);
    }
}

use super::{tests::Egl, *};
use crate::{
    camera::Camera,
    sculpt::{Geometry, SliceMode, SliceOptions, SlicePlane},
};

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_slice_cap_shading_preview() {
    unsafe {
        let egl = Egl::new();
        let gl = glow::Context::from_loader_function(|name| {
            let name = std::ffi::CString::new(name).unwrap();
            (egl.get_proc)(name.as_ptr())
        });
        for separate in [false, true] {
            let mut geometry = Geometry::new(crate::sculpt::Mesh::starting_sphere());
            let options = SliceOptions {
                mode: SliceMode::Trim,
                ..Default::default()
            };
            if separate {
                geometry
                    .slice(
                        SlicePlane {
                            origin: Vec3(0., 0.05, 0.),
                            normal: Vec3(0.6, 1., 0.1),
                        },
                        SliceOptions {
                            mode: SliceMode::Separate,
                            ..Default::default()
                        },
                        &|| false,
                    )
                    .unwrap();
            } else {
                for (height, normal) in [(0.25, Vec3(0., -1., 0.)), (-0.3, Vec3(0., 1., 0.))] {
                    geometry
                        .slice(
                            SlicePlane {
                                origin: Vec3(0., height, 0.),
                                normal,
                            },
                            options,
                            &|| false,
                        )
                        .unwrap();
                }
            }
            let mut document = Document::default();
            document.replace_geometry(geometry).unwrap();
            let before = document.geometry_snapshot();
            let mut renderer = Renderer::new(&gl).unwrap();
            let display = renderer.geometry(&document);
            let camera = Camera {
                yaw: 0.35,
                pitch: -0.32,
                ..Default::default()
            };
            let (right, up, view) = camera.basis();
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 600.));
            let params = Params::new(
                rect,
                right,
                up,
                view,
                650.,
                rect.center(),
                display.depth_extent,
            );
            let mut gpu = renderer.gpu.lock().unwrap();
            gpu.upload(&gl, &display);
            gpu.paint(
                &gl,
                &params,
                [1000, 600],
                false,
                WireMode::Visible,
                false,
                &Preview {
                    show_polygroups: !separate,
                    ..Default::default()
                },
            );
            let mut pixels = vec![0; 1000 * 600 * 4];
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(gpu.target.as_ref().unwrap().framebuffer),
            );
            gl.read_pixels(
                0,
                0,
                1000,
                600,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            assert_eq!(gl.get_error(), glow::NO_ERROR);
            assert_eq!(document.geometry_snapshot(), before);
            if !separate {
                // These rows lie just below the visible cap seam. Old area-weighted
                // normals produce triangular spikes (second differences up to 26).
                // Smooth shading stays within rounding error in this uninterrupted wall.
                for y in [350, 360, 370] {
                    let red = |x: usize| pixels[((599 - y) * 1000 + x) * 4] as i32;
                    for x in 201..799 {
                        assert!(
                            (red(x - 1) - 2 * red(x) + red(x + 1)).abs() <= 3,
                            "shading spike at {x},{y}"
                        );
                    }
                }
            }
            if let Ok(path) = std::env::var(if separate {
                "CLAY_SLICE_SEPARATE_PREVIEW"
            } else {
                "CLAY_SLICE_PREVIEW"
            }) {
                let flipped: Vec<_> = pixels
                    .as_chunks::<{ 1000 * 4 }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image::save_buffer(path, &flipped, 1000, 600, image::ColorType::Rgba8).unwrap();
            }
            gpu.destroy(&gl);
        }
    }
}

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_sliced_hemisphere_thickness_preview_matches_applied_shell() {
    unsafe {
        let egl = Egl::new();
        let gl = glow::Context::from_loader_function(|name| {
            let name = std::ffi::CString::new(name).unwrap();
            (egl.get_proc)(name.as_ptr())
        });
        let mut geometry = Geometry::new(crate::sculpt::Mesh::starting_sphere());
        geometry
            .slice(
                SlicePlane {
                    origin: Vec3(0., 0.017, 0.),
                    normal: Vec3(-0.37, -1., -0.21),
                },
                SliceOptions {
                    mode: SliceMode::Trim,
                    cap: false,
                    flip: false,
                },
                &|| false,
            )
            .unwrap();
        geometry.scene.active_metadata.temporary_thickness = 0.18;
        geometry.scene.active_metadata.thickness_groups = true;
        let mut document = Document::default();
        document.replace_geometry(geometry).unwrap();
        let source = document.geometry_snapshot();
        let mut renderer = Renderer::new(&gl).unwrap();
        let base = renderer.geometry(&document);
        assert_eq!(base.mesh.vertices, source.mesh.vertices);
        let display = renderer.render_geometry.as_ref().unwrap().clone();
        display.mesh.validate_closed().unwrap();
        let camera = Camera {
            yaw: 0.35,
            pitch: -0.45,
            ..Default::default()
        };
        let (right, up, view) = camera.basis();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 600.));
        let params = Params::new(
            rect,
            right,
            up,
            view,
            600.,
            rect.center(),
            display.depth_extent,
        );
        {
            let mut gpu = renderer.gpu.lock().unwrap();
            gpu.upload(&gl, &display);
            gpu.paint(
                &gl,
                &params,
                [1000, 600],
                false,
                WireMode::Visible,
                false,
                &Preview {
                    show_polygroups: true,
                    ..Default::default()
                },
            );
            let mut pixels = vec![0; 1000 * 600 * 4];
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(gpu.target.as_ref().unwrap().framebuffer),
            );
            gl.read_pixels(
                0,
                0,
                1000,
                600,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            assert_eq!(gl.get_error(), glow::NO_ERROR);
            if let Ok(path) = std::env::var("CLAY_SLICE_THICKNESS_PREVIEW") {
                let flipped: Vec<_> = pixels
                    .as_chunks::<{ 1000 * 4 }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image::save_buffer(path, &flipped, 1000, 600, image::ColorType::Rgba8).unwrap();
            }
        }
        assert_eq!(document.geometry_snapshot(), source);
        document.apply_thickness().unwrap();
        let applied = renderer.geometry(&document);
        assert_eq!(applied.mesh, display.mesh);
        assert_eq!(applied.shading.triangles, display.shading.triangles);
        assert_eq!(applied.shading.vertices, display.shading.vertices);
        renderer.destroy(&gl);
    }
}

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn gpu_rounded_slice_path_preview() {
    unsafe {
        let egl = Egl::new();
        let gl = glow::Context::from_loader_function(|name| {
            let name = std::ffi::CString::new(name).unwrap();
            (egl.get_proc)(name.as_ptr())
        });
        let mut geometry = Geometry::new(crate::sculpt::Mesh::starting_sphere());
        let a = Vec3(-0.5, 0.2, 0.);
        let b = Vec3(0., -0.25, 0.);
        let c = Vec3(0.5, 0.2, 0.);
        let mut points = vec![a + (a - b) * 3.];
        for i in 0..=8 {
            let t = i as f32 / 8.;
            points.push(a * (1. - t).powi(2) + b * (2. * t * (1. - t)) + c * t * t);
        }
        points.push(c + (c - b) * 3.);
        geometry
            .slice_path(
                &crate::sculpt::SlicePath {
                    points,
                    depth: Vec3(0., 0., 1.),
                    eye: None,
                },
                SliceOptions {
                    mode: SliceMode::Trim,
                    ..Default::default()
                },
                &|| false,
            )
            .unwrap();
        geometry.mesh.validate_closed().unwrap();
        let mut document = Document::default();
        document.replace_geometry(geometry).unwrap();
        let mut renderer = Renderer::new(&gl).unwrap();
        let display = renderer.geometry(&document);
        let camera = Camera {
            yaw: 0.2,
            pitch: -0.65,
            ..Default::default()
        };
        let (right, up, view) = camera.basis();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 600.));
        let params = Params::new(
            rect,
            right,
            up,
            view,
            280.,
            rect.center(),
            display.depth_extent,
        );
        let mut gpu = renderer.gpu.lock().unwrap();
        gpu.upload(&gl, &display);
        gpu.paint(
            &gl,
            &params,
            [1000, 600],
            false,
            WireMode::Visible,
            false,
            &Preview {
                show_polygroups: true,
                ..Default::default()
            },
        );
        let mut pixels = vec![0; 1000 * 600 * 4];
        gl.bind_framebuffer(
            glow::READ_FRAMEBUFFER,
            Some(gpu.target.as_ref().unwrap().framebuffer),
        );
        gl.read_pixels(
            0,
            0,
            1000,
            600,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut pixels)),
        );
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        assert_eq!(gl.get_error(), glow::NO_ERROR);
        if let Ok(path) = std::env::var("CLAY_SLICE_PATH_PREVIEW") {
            let flipped: Vec<_> = pixels
                .as_chunks::<{ 1000 * 4 }>()
                .0
                .iter()
                .rev()
                .flatten()
                .copied()
                .collect();
            image::save_buffer(path, &flipped, 1000, 600, image::ColorType::Rgba8).unwrap();
        }
        gpu.destroy(&gl);
    }
}

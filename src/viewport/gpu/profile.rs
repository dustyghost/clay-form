//! Opt-in baseline, separated from correctness tests and CI timing assertions.
use super::{ui_capture::Headless, *};
use crate::{
    camera::Camera,
    sculpt::{Dab, Geometry, Kind, Mesh},
};
use std::time::Instant;

#[test]
#[ignore = "load profile requires Vulkan; run in release mode"]
fn gpu_load_profile() {
    {
        let gl = Headless::new();
        let driver = format!("{:?}", gl.state.adapter.get_info());
        eprintln!("{driver}");
        let mut csv = String::from(
            "levels,models,vertices,triangles,mesh_prepare_ms,upload_ms,cpu_view_ms,pick_ms,solid_p50_ms,solid_p95_ms,wire_p50_ms,wire_p95_ms,dab_ms\n",
        );
        let context = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.));
        for (levels, models) in [(0, 1), (1, 1), (2, 1), (3, 1), (0, 4), (2, 4)] {
            let mut geometry = Geometry::new(Mesh::sphere());
            for _ in 0..levels {
                geometry.subdivide().unwrap();
            }
            // Give all display models the requested density; inactive LODs
            // would otherwise hide the cost of a genuinely dense scene.
            geometry.settings.lower.clear();
            if models > 1 {
                for p in &mut geometry.mesh.vertices {
                    *p = *p * 0.45;
                }
            }
            let mut document = Document::default();
            document.replace_geometry(geometry).unwrap();
            if models > 1 {
                let centers = [
                    Vec3(-0.65, -0.65, 0.),
                    Vec3(0.65, -0.65, 0.),
                    Vec3(0.65, 0.65, 0.),
                    Vec3(-0.65, 0.65, 0.),
                ];
                let mut previous = Vec3::default();
                for (i, center) in centers.into_iter().enumerate() {
                    if i > 0 {
                        document.duplicate_model(false).unwrap();
                    }
                    document
                        .apply_transform(
                            crate::sculpt::Transform {
                                translation: center - previous,
                                ..Default::default()
                            },
                            false,
                        )
                        .unwrap();
                    document.end_stroke();
                    previous = center;
                }
            }
            let mut renderer = Renderer::new(&gl.state).unwrap();
            let start = Instant::now();
            let display = renderer.geometry(&document);
            let mesh_prepare = start.elapsed().as_secs_f64() * 1000.;
            let mut gpu = renderer.gpu.lock().unwrap();
            let start = Instant::now();
            gpu.upload(&display);
            gl.wait();
            let upload = start.elapsed().as_secs_f64() * 1000.;
            let mut solid = Vec::new();
            let mut wire = Vec::new();
            for (wireframe, times) in [(false, &mut solid), (true, &mut wire)] {
                for frame in 0..17 {
                    let camera = Camera {
                        yaw: frame as f32 * 0.025,
                        ..Default::default()
                    };
                    let (right, up, view) = camera.basis();
                    let params = Params::new(
                        rect,
                        right,
                        up,
                        view,
                        89.6,
                        rect.center(),
                        display.depth_extent,
                    );
                    let start = Instant::now();
                    gpu.paint(
                        &params,
                        [256, 256],
                        wireframe,
                        WireMode::Visible,
                        false,
                        &Preview::default(),
                    );
                    gl.wait();
                    // First draw includes framebuffer/edge buffer creation.
                    if frame > 0 {
                        times.push(start.elapsed().as_secs_f64() * 1000.);
                    }
                }
                times.sort_by(f64::total_cmp);
            }
            drop(gpu);
            let mut scene = None;
            let start = Instant::now();
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(rect),
                    ..Default::default()
                },
                |ctx| {
                    scene = Some(Scene::paint_with_renderer(
                        &ctx.layer_painter(egui::LayerId::background()),
                        rect,
                        &document,
                        &Camera::default(),
                        false,
                        WireMode::Visible,
                        false,
                        Some(&mut renderer),
                    ));
                },
            );
            let cpu_view = start.elapsed().as_secs_f64() * 1000.;
            let scene = scene.unwrap();
            let start = Instant::now();
            for i in 0..16 {
                std::hint::black_box(scene.pick(rect.center() + egui::vec2(i as f32 - 8., 4.)));
            }
            let pick = start.elapsed().as_secs_f64() * 1000. / 16.;
            let brush = Brush::new(Kind::Standard);
            let start = Instant::now();
            for _ in 0..3 {
                document.begin_sculpt_frame();
                document.apply_dab(
                    &brush,
                    Dab {
                        center: if models > 1 {
                            Vec3(-0.65, 0.65, 0.45)
                        } else {
                            Vec3(0., 0., 1.)
                        },
                        normal: Vec3(0., 0., 1.),
                        right: Vec3(1., 0., 0.),
                        up: Vec3(0., 1., 0.),
                        radius: 0.3,
                        pressure: 1.,
                        delta: Vec3::default(),
                        invert: false,
                    },
                );
            }
            document.end_stroke();
            let dab = start.elapsed().as_secs_f64() * 1000. / 3.;
            let line = format!(
                "{levels},{models},{},{},{mesh_prepare:.3},{upload:.3},{cpu_view:.3},{pick:.3},{:.3},{:.3},{:.3},{:.3},{dab:.3}\n",
                display.mesh.vertices.len(),
                display.mesh.triangles.len(),
                solid[7],
                solid[15],
                wire[7],
                wire[15]
            );
            eprint!("{line}");
            csv.push_str(&line);
            renderer.destroy();
        }
        if let Ok(path) = std::env::var("CLAY_GPU_PROFILE") {
            std::fs::write(path, format!("# {driver}\n{csv}")).unwrap();
        }
    }
}

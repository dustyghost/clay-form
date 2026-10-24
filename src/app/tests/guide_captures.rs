//! Purpose-built documentation scenes, rendered with the real app controls.
use super::*;
use crate::{
    preferences::{BuiltinMenu, Dock, Tool},
    sculpt::{AdaptiveSettings, Primitive, PrimitiveKind, Symmetry},
    viewport::UiCapture,
};

struct Guide {
    app: App,
    ctx: egui::Context,
    capture: UiCapture,
    size: [i32; 2],
    time: f64,
    toolbar: bool,
    viewport: bool,
    interactive: bool,
    directory: std::path::PathBuf,
    text_positions: Vec<(String, Pos2)>,
}

#[test]
#[ignore = "Flatten stroke visual review; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn flatten_stroke_offscreen_review() {
    use crate::sculpt::{Geometry, Kind};
    let directory = std::path::PathBuf::from(std::env::var("CLAY_GUIDE_PREVIEW").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1000, 900]);
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [40, 40, 2];
    let mut mesh = cube.mesh().unwrap();
    for p in &mut mesh.vertices {
        if p.2 > 0.9 && p.0.abs() < 0.95 && p.1.abs() < 0.95 {
            p.2 += 0.055 * (p.0 * 16.).cos() * (p.1 * 16.).cos();
        }
    }
    g.app
        .document
        .replace_geometry(Geometry::new(mesh))
        .unwrap();
    g.app.ui.selected = g
        .app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == Kind::Flatten)
        .unwrap();
    let mut brush = g.app.document.brushes()[g.app.ui.selected].clone();
    brush.radius = 170.;
    brush.strength = 0.4;
    brush.spacing = 0.12;
    g.app
        .document
        .update_brush(g.app.ui.selected, brush)
        .unwrap();
    g.panel(Tool::Brush, Dock::Right);
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = -0.25;
    g.save("flatten-before");
    let before = g.app.document.geometry_snapshot();
    let history = g.app.document.history_counts();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    let (rect, _) = g.frame(vec![]);
    let start = rect.center();
    g.frame(vec![
        egui::Event::PointerMoved(start),
        primary_event(start, true),
    ]);
    let mut end = start;
    for step in 1..=72 {
        end = start + Vec2::new(90. * (step as f32 * std::f32::consts::PI / 12.).sin(), 0.);
        g.frame(vec![egui::Event::PointerMoved(end)]);
    }
    g.frame(vec![primary_event(end, false), egui::Event::PointerGone]);
    let after = g.app.document.geometry_snapshot();
    assert_ne!(before, after);
    after.mesh.validate_closed().unwrap();
    assert_eq!(g.app.document.history_counts().0, history.0 + 1);
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = -0.25;
    g.save("flatten-after");
    g.app.document.undo();
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.app.document.redo();
    assert_eq!(g.app.document.geometry_snapshot(), after);
    g.app.viewport.destroy_renderer();
}

#[test]
#[ignore = "renders adaptive Reactor rings drag stamp; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn reactor_ring_stamp_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW"),
    );
    std::fs::create_dir_all(&directory).unwrap();
    for (name, offset, radius, rotation, subtract, surface_cursor) in [
        (
            "reactor-original",
            Vec2::new(-0.0591254, -0.07994466),
            298.28677,
            59.30068_f32,
            false,
            false,
        ),
        (
            "reactor-latest",
            Vec2::new(-0.18910993, -0.22439388),
            252.10057,
            55.801712_f32,
            false,
            false,
        ),
        (
            "reactor-subtract",
            Vec2::new(-0.18910993, -0.22439388),
            252.10057,
            55.801712_f32,
            true,
            false,
        ),
        (
            "reactor-surface",
            Vec2::new(-0.18910993, -0.22439388),
            252.10057,
            55.801712_f32,
            false,
            true,
        ),
    ] {
        let mut g = Guide::new(&directory, [1000, 1000]);
        g.toolbar = false;
        g.app.camera.zoom = 0.875;
        g.app
            .document
            .set_symmetry(Symmetry {
                enabled: false,
                ..Default::default()
            })
            .unwrap();
        g.app
            .document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        let mut brush = g.app.document.brushes()[0].clone();
        brush.stroke_mode = crate::sculpt::StrokeMode::DragRect;
        brush.subtract = subtract;
        brush.surface_cursor = surface_cursor;
        brush.alpha = crate::sculpt::alpha::builtins()
            .iter()
            .find(|a| a.name == "Reactor rings")
            .unwrap()
            .image
            .clone();
        g.app.document.update_brush(0, brush).unwrap();
        let before = g.app.document.geometry_snapshot();
        let (rect, _) = g.frame(vec![]);
        let scale = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom;
        let start = rect.center() + offset * scale;
        let angle = rotation.to_radians();
        let end = start + Vec2::new(angle.cos(), angle.sin()) * radius;
        g.frame(vec![
            egui::Event::PointerMoved(start),
            primary_event(start, true),
        ]);
        g.frame(vec![egui::Event::PointerMoved(end)]);
        g.save(&format!("{name}-moving"));
        if std::env::var_os("CLAY_GUIDE_MOVING_ONLY").is_some() {
            continue;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while g.app.viewport.stamp_preview_pending() {
            assert!(
                std::time::Instant::now() < deadline,
                "stamp preview did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
            g.frame(vec![]);
        }
        g.save(&format!("{name}-preview"));
        assert_eq!(g.app.document.geometry_snapshot(), before);
        g.frame(vec![primary_event(end, false), egui::Event::PointerGone]);
        assert!(g.app.document.mesh().vertices.len() > 10_000);
        assert_ne!(
            &g.app.document.mesh().vertices[..before.mesh.vertices.len()],
            &before.mesh.vertices,
            "release must apply relief, not only refine the surface"
        );
        g.app.document.mesh().validate_closed().unwrap();
        g.save(&format!("{name}-committed"));
        g.app.camera.pitch = -0.35;
        g.save(&format!("{name}-angled"));
        let after = g.app.document.geometry_snapshot();
        g.app.document.undo();
        assert_eq!(g.app.document.geometry_snapshot(), before);
        g.app.document.redo();
        assert_eq!(g.app.document.geometry_snapshot(), after);
    }
}

#[test]
#[ignore = "local project review; requires Vulkan, CLAY_REVIEW_PROJECT and CLAY_GUIDE_PREVIEW"]
fn project_file_offscreen_review() {
    let path = std::env::var("CLAY_REVIEW_PROJECT").expect("Set CLAY_REVIEW_PROJECT");
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW"),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let project = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let mut g = Guide::new(&directory, [1000, 1000]);
    g.toolbar = false;
    g.app.document.replace_project(project, &path).unwrap();
    if let Ok(manifest) = std::env::var("CLAY_REVIEW_MANIFEST") {
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
        let camera = &manifest["current_settings"]["view_input"]["camera"];
        let value = |key: &str| camera[key].as_f64().unwrap() as f32;
        g.app.camera.yaw = value("yaw");
        g.app.camera.pitch = value("pitch");
        g.app.camera.zoom = value("zoom");
        g.app.camera.pan = Vec2::new(
            camera["pan"][0].as_f64().unwrap() as f32,
            camera["pan"][1].as_f64().unwrap() as f32,
        );
        g.app.camera.perspective = camera["perspective"].as_bool().unwrap();
        g.app.camera.field_of_view = value("field_of_view");
        g.app.camera.perspective_radius = value("perspective_radius");
    }
    g.save("project-review");
}

#[test]
#[ignore = "renders Polish cleanup; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn polish_brush_offscreen_review() {
    use crate::sculpt::{Brush, Dab, Geometry, Kind, Vec3};
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1200, 900]);
    g.panel(Tool::Brush, Dock::Right);
    g.panel(Tool::Library, Dock::Left);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [40, 40, 2];
    let mut mesh = primitive.mesh().unwrap();
    for p in &mut mesh.vertices {
        if p.2 > 0.9 && p.0.abs() < 0.95 && p.1.abs() < 0.95 {
            p.2 += 0.035 * (p.0 * 20.).cos() * (p.1 * 20.).cos();
        }
    }
    g.app
        .document
        .replace_geometry(Geometry::new(mesh))
        .unwrap();
    g.app.ui.selected = g
        .app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == Kind::Polish)
        .unwrap();
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = -0.2;
    g.app.camera.zoom = 0.7;
    g.save("polish-before");
    let before = g.app.document.geometry_snapshot();
    for _ in 0..20 {
        g.app.document.apply_dab(
            &Brush::new(Kind::Polish),
            Dab {
                center: Vec3(0., 0., 1.),
                normal: Vec3(0., 0., 1.),
                right: Vec3(1., 0., 0.),
                up: Vec3(0., 1., 0.),
                radius: 1.8,
                pressure: 1.,
                delta: Vec3::default(),
                invert: false,
            },
        );
    }
    g.app.document.end_stroke();
    g.app.document.mesh().validate_closed().unwrap();
    assert_ne!(g.app.document.geometry_snapshot(), before);
    g.save("polish-after");
    g.app.document.undo();
    assert_eq!(g.app.document.geometry_snapshot(), before);
}

#[test]
#[ignore = "smooth/decimate review; requires Vulkan, CLAY_GUIDE_PREVIEW; optional CLAY_REVIEW_PROJECT"]
fn smooth_decimate_offscreen_review() {
    use crate::sculpt::{Brush, Dab, Geometry, Kind, Mesh, Vec3};
    let directory = std::path::PathBuf::from(std::env::var("CLAY_GUIDE_PREVIEW").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1000, 1000]);
    g.toolbar = false;
    let exported = std::env::var("CLAY_REVIEW_PROJECT").ok();
    if let Some(path) = &exported {
        g.app
            .document
            .replace_project(
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap(),
                path,
            )
            .unwrap();
        g.app.camera.yaw = 0.184723;
        g.app.camera.pitch = 2.6622775;
        if let Ok(manifest) = std::env::var("CLAY_REVIEW_MANIFEST") {
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
            let camera = &manifest["current_settings"]["view_input"]["camera"];
            g.app.camera.yaw = camera["yaw"].as_f64().unwrap() as f32;
            g.app.camera.pitch = camera["pitch"].as_f64().unwrap() as f32;
        }
    } else {
        let mut mesh = Mesh::sphere().subdivide().unwrap();
        let pull = Brush::new(Kind::Snake);
        let mut d = Dab {
            center: Vec3(0., 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.4,
            pressure: 1.,
            delta: Vec3(0., 0., 0.012),
            invert: false,
        };
        for _ in 0..12 {
            mesh.stamp(&pull, d);
            d.center = d.center + pull.snake_motion(d);
        }
        g.app
            .document
            .replace_geometry(Geometry::new(mesh))
            .unwrap();
        g.app.camera.yaw = 0.45;
        g.app.camera.pitch = -0.25;
    }
    let mut settings = g.app.document.mesh_settings().adaptive.clone();
    settings.enabled = true;
    settings.simplify = false; // Dedicated Decimate must work independently.
    g.app.document.set_adaptive(settings).unwrap();
    let before = g.app.document.geometry_snapshot();
    g.save("cleanup-before");
    let (right, up, view) = g.app.camera.basis();
    let sweep: [f32; 3] =
        std::env::var("CLAY_REVIEW_SWEEP").map_or([-0.52, -0.25, 0.35], |value| {
            value
                .split(',')
                .map(|v| v.parse().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .expect("CLAY_REVIEW_SWEEP needs x,y_start,y_end")
        });
    let dabs: Vec<_> = (0..36)
        .map(|i| {
            let (center, normal, radius) = if exported.is_some() {
                let x = sweep[0];
                let y = sweep[1] + (i % 12) as f32 / 11. * (sweep[2] - sweep[1]);
                let p = right * x + up * y + view * (1. - x * x - y * y).sqrt();
                (p, view, 0.24)
            } else {
                (Vec3(0., 0., 1.), Vec3(0., 0., 1.), 0.65)
            };
            Dab {
                center,
                normal,
                radius,
                right,
                up,
                pressure: 1.,
                delta: Vec3::default(),
                invert: false,
            }
        })
        .collect();
    let opposed = |mesh: &Mesh| {
        let mut edges = std::collections::HashMap::new();
        let mut count = 0;
        for &[a, b, c] in &mesh.triangles {
            let normal = (mesh.vertices[b] - mesh.vertices[a])
                .cross(mesh.vertices[c] - mesh.vertices[a])
                .norm();
            for (a, b) in [(a, b), (b, c), (c, a)] {
                if let Some(other) = edges.insert((a.min(b), a.max(b)), normal) {
                    count += usize::from(normal.dot(other) < -1e-5);
                }
            }
        }
        count
    };
    let mut previous_folds = opposed(g.app.document.mesh());
    let quality = |[a, b, c]: [Vec3; 3]| {
        let ab = b - a;
        let ac = c - a;
        let bc = c - b;
        2. * 3f32.sqrt() * ab.cross(ac).len() / (ab.dot(ab) + ac.dot(ac) + bc.dot(bc))
    };
    let before_quality: Vec<_> = before
        .mesh
        .triangles
        .iter()
        .map(|t| quality(t.map(|i| before.mesh.vertices[i])))
        .collect();
    for kind in [Kind::Smooth, Kind::Decimate] {
        let mut brush = Brush::new(kind);
        brush.radius = if exported.is_some() { 84. } else { 100. };
        if let Ok(strength) = std::env::var("CLAY_REVIEW_STRENGTH") {
            brush.strength = strength.parse().unwrap();
        }
        let start = std::time::Instant::now();
        for &dab in &dabs {
            g.app.document.begin_sculpt_frame();
            g.app.document.apply_dab(&brush, dab);
        }
        g.app.document.end_stroke();
        eprintln!(
            "{kind:?}: 36 dabs in {:?}, {} vertices",
            start.elapsed(),
            g.app.document.mesh().vertices.len()
        );
        let name = if kind == Kind::Smooth {
            "cleanup-smoothed"
        } else {
            "cleanup-decimated"
        };
        g.save(name);
        std::fs::write(
            directory.join(format!("{name}.clay")),
            serde_json::to_vec(&g.app.document.project()).unwrap(),
        )
        .unwrap();
        g.app.document.mesh().validate_closed().unwrap();
        let folds = opposed(g.app.document.mesh());
        eprintln!("{kind:?}: opposed neighbors {previous_folds} -> {folds}");
        assert!(
            folds <= previous_folds,
            "cleanup must not introduce extra opposing face pairs"
        );
        previous_folds = folds;
        if kind == Kind::Smooth {
            assert_eq!(g.app.document.mesh().triangles, before.mesh.triangles);
            assert_ne!(g.app.document.mesh().vertices, before.mesh.vertices);
            for (t, q) in g.app.document.mesh().triangles.iter().zip(&before_quality) {
                assert!(
                    quality(t.map(|i| g.app.document.mesh().vertices[i])) >= q.min(0.02) * 0.999,
                    "Smooth must respect the collapse floor and protect existing degeneracies"
                );
            }
        } else {
            assert!(g.app.document.mesh().vertices.len() < before.mesh.vertices.len());
        }
    }
    g.app.document.undo();
    g.app.document.undo();
    assert_eq!(g.app.document.geometry_snapshot(), before);
    // Inspect the actual controls, including a legacy project's missing preset.
    g.panel(Tool::Mesh, Dock::Right);
    g.panel(Tool::Brush, Dock::Left);
    g.interactive = true;
    g.save("cleanup-controls");
    click_guide_label(&mut g, "Decimate brush");
    assert_eq!(
        g.app.document.brushes()[g.app.ui.selected].kind,
        Kind::Decimate
    );
    g.save("cleanup-decimate-controls");
}
impl Guide {
    fn new(directory: &std::path::Path, size: [i32; 2]) -> Self {
        let mut capture = UiCapture::new(size);
        let mut app = App::default();
        app.viewport.initialize_renderer(&capture.state).unwrap();
        for panel in &mut app.ui.preferences.panels {
            panel.dock = Dock::Hidden;
        }
        app.ui.show_polygroups = false;
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        // Install the app's theme even for panel-only captures.
        let output = ctx.run(Default::default(), |ctx| {
            let _ = app.ui.toolbar(ctx, &app.document, &app.camera);
        });
        capture.render(&ctx, output);
        Self {
            app,
            ctx,
            capture,
            size,
            time: 0.,
            toolbar: true,
            viewport: true,
            interactive: false,
            directory: directory.into(),
            text_positions: Vec::new(),
        }
    }
    fn panel(&mut self, tool: Tool, dock: Dock) {
        self.app
            .ui
            .preferences
            .panels
            .iter_mut()
            .find(|p| p.tool == tool)
            .unwrap()
            .dock = dock;
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> (egui::Rect, Vec<u8>) {
        self.time += 0.1;
        let mut rect = egui::Rect::NOTHING;
        let mut commands = Vec::new();
        let output = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(self.size[0] as f32, self.size[1] as f32),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ctx| {
                if self.toolbar {
                    commands.extend(
                        self.app
                            .ui
                            .toolbar(ctx, &self.app.document, &self.app.camera),
                    );
                }
                commands.extend(self.app.ui.brush_panel(
                    ctx,
                    &self.app.document,
                    "Guide",
                    1.,
                    &self.app.camera,
                ));
                egui::CentralPanel::default().show(ctx, |ui| {
                    rect = ui.available_rect_before_wrap();
                    if self.viewport {
                        self.app.show_viewport(ui);
                    }
                });
                self.app.show_recovery(ctx);
                commands.extend(self.app.ui.project_file_dialog(ctx));
                commands.extend(self.app.ui.alpha_popup(ctx));
                commands.extend(self.app.ui.projection_window(ctx, &self.app.document));
            },
        );
        self.text_positions = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((
                    text.galley.text().to_owned(),
                    text.pos + text.galley.size() * 0.5,
                )),
                _ => None,
            })
            .collect();
        let rgba = self.capture.render(&self.ctx, output);
        if self.interactive {
            for command in commands {
                self.app.dispatch(command, &self.ctx);
            }
        }
        (rect, rgba)
    }
    fn save(&mut self, name: &str) {
        for _ in 0..2 {
            self.frame(vec![]);
        }
        let (_, rgba) = self.frame(vec![]);
        self.capture
            .save(self.directory.join(format!("{name}.png")), &rgba);
    }
    fn stroke(&mut self, start: Pos2, end: Pos2) {
        self.frame(vec![
            egui::Event::PointerMoved(start),
            primary_event(start, true),
        ]);
        for i in 1..=24 {
            self.frame(vec![egui::Event::PointerMoved(
                start.lerp(end, i as f32 / 24.),
            )]);
        }
        self.frame(vec![primary_event(end, false), egui::Event::PointerGone]);
    }
}
impl Drop for Guide {
    fn drop(&mut self) {
        self.app.viewport.destroy_renderer();
    }
}

#[test]
#[ignore = "renders documentation screenshots; requires Vulkan and CLAY_GUIDE_PREVIEW directory"]
fn guide_workflows_offscreen_preview() {
    let Ok(directory) = std::env::var("CLAY_GUIDE_PREVIEW") else {
        return;
    };
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();

    {
        let mut g = Guide::new(directory, [1000, 720]);
        g.app
            .ui
            .preferences
            .dock_menu(BuiltinMenu::Transform, Dock::Left);
        g.app
            .ui
            .preferences
            .pin_menu(BuiltinMenu::Mask, [660., 150.]);
        g.app.dispatch(
            Command::TransformMode(crate::viewport::TransformMode::World),
            &g.ctx,
        );
        g.save("menu-placement");
    }
    {
        let mut g = Guide::new(directory, [540, 470]);
        g.toolbar = false;
        g.viewport = false;
        let recovery_directory = crate::recovery::tests::Directory::new();
        g.app.initialize_recovery(recovery_directory.0.clone());
        g.app.document.initialize_primitive().unwrap();
        g.app.save_recovery(&g.ctx);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while g.app.recovery.as_ref().unwrap().busy {
            g.app.tick_recovery(&g.ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(
            crate::recovery::entries(&recovery_directory.0)
                .unwrap()
                .len(),
            1
        );
        g.app.open_recovery();
        g.save("recovery-dialog");
    }
    {
        let mut g = Guide::new(directory, [1000, 800]);
        g.panel(Tool::Submodels, Dock::Right);
        g.app.ui.obj_path = format!("{}/docs/examples/pyramid.obj", env!("CARGO_MANIFEST_DIR"));
        g.app.dispatch(Command::Action(Action::ImportObj), &g.ctx);
        g.app.dispatch(Command::AcceptProjectFile(false), &g.ctx);
        finish_mesh_job(&mut g.app);
        assert_eq!(g.app.document.model_count(), 2);
        g.app.ui.obj_path = "docs/examples/pyramid.obj".into();
        g.app
            .document
            .set_model_metadata(
                0,
                "Sphere".into(),
                crate::sculpt::ModelMetadata {
                    visible: false,
                    ..Default::default()
                },
            )
            .unwrap();
        g.app.camera.yaw = 0.5;
        g.app.camera.pitch = -0.25;
        g.app.ui.wire = true;
        g.save("obj-import");
    }

    // A compact panel pair makes the reset's actual changes readable.
    {
        let mut g = Guide::new(directory, [330, 740]);
        g.toolbar = false;
        g.viewport = false;
        g.panel(Tool::Brush, Dock::Right);
        g.app.dispatch(Command::DuplicateBrush(0), &g.ctx);
        let index = g.app.ui.selected;
        let mut brush = g.app.document.brushes()[index].clone();
        brush.name = "Fold study".into();
        brush.radius = 95.;
        brush.strength = 0.65;
        brush.spacing = 0.3;
        brush.stabilize = true;
        g.app.document.update_brush(index, brush).unwrap();
        g.save("brush-reset-before");
        g.app
            .dispatch(Command::Action(Action::ResetBrushSettings), &g.ctx);
        assert_eq!(g.app.document.brushes()[index].name, "Fold study");
        assert_eq!(g.app.document.brushes()[index].radius, 60.);
        g.save("brush-reset-after");
    }
    {
        let mut g = Guide::new(directory, [1000, 800]);
        g.panel(Tool::Library, Dock::Left);
        let mut brush = g.app.document.brushes()[0].clone();
        brush.strength = 0.5;
        g.app.document.update_brush(0, brush).unwrap();
        g.save("first-sculpt-before");
        let (rect, _) = g.frame(vec![]);
        let start = rect.center() + Vec2::new(-70., -90.);
        let before = g.app.document.geometry_snapshot();
        g.stroke(start, start + Vec2::new(20., 150.));
        assert_ne!(g.app.document.geometry_snapshot(), before);
        g.save("first-sculpt-after");
        g.app.dispatch(Command::Undo, &g.ctx);
        assert_eq!(g.app.document.geometry_snapshot(), before);
        g.panel(Tool::Library, Dock::Hidden);
        g.app
            .ui
            .preferences
            .dock_menu(BuiltinMenu::Symmetry, Dock::Left);
        g.app.dispatch(
            Command::SetSymmetry(Symmetry {
                enabled: true,
                ..Default::default()
            }),
            &g.ctx,
        );
        let (rect, _) = g.frame(vec![]);
        let start = rect.center() + Vec2::new(-90., -70.);
        g.stroke(start, start + Vec2::new(0., 140.));
        g.save("symmetry-stroke");
    }
    {
        let mut g = Guide::new(directory, [900, 700]);
        g.panel(Tool::Mesh, Dock::Right);
        g.app.dispatch(
            Command::SetAdaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            }),
            &g.ctx,
        );
        g.app.ui.wire = true;
        g.save("adaptive-settings");
    }
    {
        let mut g = Guide::new(directory, [950, 700]);
        g.app.document.initialize_primitive().unwrap();
        let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
        cube.divisions = [2; 3];
        cube.size = crate::sculpt::Vec3(1.4, 0.8, 1.);
        g.app.document.update_primitive(cube).unwrap();
        g.app.document.end_primitive_edit();
        g.app
            .ui
            .preferences
            .dock_menu(BuiltinMenu::Primitive, Dock::Right);
        g.app.camera.yaw = 0.5;
        g.app.camera.pitch = -0.3;
        g.app.ui.wire = true;
        g.save("primitive-cube");
    }
    {
        let mut g = Guide::new(directory, [1100, 700]);
        g.app
            .ui
            .preferences
            .dock_menu(BuiltinMenu::Slice, Dock::Left);
        g.app.ui.slice_options.mode = crate::sculpt::SliceMode::Trim;
        g.app.dispatch(Command::SliceTool(true), &g.ctx);
        let (rect, _) = g.frame(vec![]);
        let start = rect.center() + Vec2::new(25., -220.);
        let end = rect.center() + Vec2::new(25., 220.);
        g.frame(vec![
            egui::Event::PointerMoved(start),
            primary_event(start, true),
        ]);
        g.frame(vec![egui::Event::PointerMoved(end)]);
        g.save("slice-trim-preview");
        g.frame(vec![primary_event(end, false), egui::Event::PointerGone]);
        finish_mesh_job(&mut g.app);
        assert!(g.app.document.can_undo());
        g.app.document.mesh().validate_closed().unwrap();
        g.app.dispatch(Command::SliceTool(false), &g.ctx);
        g.app.camera.yaw = 0.6;
        g.app.camera.pitch = -0.2;
        g.app.ui.show_polygroups = true;
        g.save("slice-trim-result");
    }
    // A simple ellipsoid supplies clear, repeatable front/side studies.
    {
        let mut g = Guide::new(directory, [1100, 760]);
        let mut study = Primitive::for_kind(PrimitiveKind::UvSphere);
        study.size = crate::sculpt::Vec3(1.4, 2., 0.8);
        let study = study.mesh().unwrap();
        for (name, yaw, center) in [
            ("Front study", 0., [0.18, 0.3]),
            ("Side study", std::f32::consts::FRAC_PI_2, [0.18, 0.75]),
        ] {
            let icon = crate::brush_icon::capture(
                &study,
                &crate::camera::Camera {
                    yaw,
                    ..Default::default()
                },
            )
            .unwrap();
            let path = directory.join(format!("{}.png", name.replace(' ', "-").to_lowercase()));
            image::save_buffer(
                &path,
                &icon.rgba,
                icon.size as u32,
                icon.size as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            let mut reference = crate::reference::ReferenceImage::load(&path).unwrap();
            reference.name = name.into();
            reference.placement.center = center;
            reference.placement.width = 0.28;
            reference.placement.locked = name == "Side study";
            reference.placement.opacity = if name == "Side study" { 0.7 } else { 1. };
            let placement = reference.placement;
            g.app.document.add_reference(reference).unwrap();
            let index = g.app.document.references().len() - 1;
            g.app.document.place_reference(index, placement).unwrap();
        }
        g.app
            .document
            .replace_geometry(crate::sculpt::Geometry::new(study))
            .unwrap();
        g.app
            .ui
            .preferences
            .dock_menu(BuiltinMenu::References, Dock::Right);
        g.app.ui.selected_reference = 1;
        g.app.ui.arrange_references = true;
        g.app.camera.yaw = 0.4;
        g.app.camera.set_zoom(0.6);
        g.app.camera.pan = Vec2::new(110., 0.);
        g.save("reference-images");
    }
}

#[test]
#[ignore = "renders mirror documentation screenshots; requires Vulkan and CLAY_GUIDE_PREVIEW directory"]
fn mirror_guide_offscreen_preview() {
    let Ok(directory) = std::env::var("CLAY_GUIDE_PREVIEW") else {
        return;
    };
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1100, 850]);
    g.app
        .ui
        .preferences
        .dock_menu(BuiltinMenu::Symmetry, Dock::Left);
    let mut brush = g.app.document.brushes()[0].clone();
    brush.radius = 80.;
    brush.strength = 0.65;
    g.app.document.update_brush(0, brush).unwrap();
    let (rect, _) = g.frame(vec![]);
    let start = rect.center() + Vec2::new(-110., -110.);
    g.stroke(start, start + Vec2::new(15., 200.));
    g.save("mirror-before");
    let before = g.app.document.geometry_snapshot();
    g.app.dispatch(
        Command::Action(Action::Mirror(crate::sculpt::Axis::X, false)),
        &g.ctx,
    );
    finish_mesh_job(&mut g.app);
    assert_ne!(
        g.app.document.geometry_snapshot(),
        before,
        "{}",
        g.app.ui.status
    );
    g.app.document.mesh().validate_closed().unwrap();
    assert_eq!(g.app.document.undo_label(), Some("Mirror one side"));
    g.save("mirror-after");
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), before);
}

#[test]
#[ignore = "renders edge tool documentation; requires Vulkan and CLAY_GUIDE_PREVIEW directory"]
fn edge_tools_offscreen_preview() {
    use crate::sculpt::{BorderTarget, Geometry};
    let Ok(directory) = std::env::var("CLAY_GUIDE_PREVIEW") else {
        return;
    };
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    {
        let mut g = Guide::new(directory, [1200, 1000]);
        g.toolbar = false; // Keep the loop controls visible beside the hole.
        g.panel(Tool::Mesh, Dock::Right);
        g.app.ui.edges.selecting = true;
        g.app.ui.wire = true;
        let mut primitive = Primitive::for_kind(PrimitiveKind::Grid);
        primitive.divisions = [6, 6, 1];
        let mut mesh = primitive.mesh().unwrap();
        for (i, p) in mesh.vertices.iter().enumerate() {
            mesh.mask[i] = if p.0.abs() < 0.4 && p.1.abs() < 0.4 {
                1.
            } else {
                0.
            };
        }
        let mut mesh = mesh.delete_masked().unwrap().unwrap();
        mesh.mask.fill(0.);
        g.app
            .document
            .replace_geometry(Geometry::new(mesh))
            .unwrap();
        g.app.ui.edges.target = BorderTarget::Holes;
        g.app.ui.edges.loops = 2;
        g.app.ui.edges.width = 0.3;
        open_edge_controls(&mut g);
        click_guide_label(&mut g, "Support loops");
        g.save("edge-loops-before");
        g.app
            .dispatch(Command::Action(Action::AddSupportLoops), &g.ctx);
        finish_mesh_job(&mut g.app);
        assert_eq!(
            g.app.document.undo_label(),
            Some("Add support loops"),
            "{}",
            g.app.ui.status
        );
        g.save("edge-loops-after");
    }
    for crease in [false, true] {
        let mut g = Guide::new(directory, [1200, 1000]);
        g.panel(Tool::Mesh, Dock::Right);
        g.app.ui.edges.selecting = true;
        g.app.ui.wire = true;
        g.app.camera.zoom = 0.7;
        g.app.camera.yaw = 0.5;
        g.app.camera.pitch = -0.3;
        let mut p = Primitive::for_kind(PrimitiveKind::Cube);
        p.divisions = [1; 3];
        let mut mesh = p.mesh().unwrap();
        if crease {
            mesh.creases = mesh.polygon_edges();
        }
        g.app
            .document
            .replace_geometry(Geometry::new(mesh))
            .unwrap();
        for _ in 0..2 {
            g.app
                .dispatch(Command::Action(Action::SmoothSubdivide), &g.ctx);
            finish_mesh_job(&mut g.app);
            assert_eq!(
                g.app.document.undo_label(),
                Some("Smooth subdivide"),
                "{}",
                g.app.ui.status
            );
        }
        open_edge_controls(&mut g);
        g.save(if crease {
            "crease-smooth-after"
        } else {
            "crease-smooth-before"
        });
        if crease {
            g.app
                .dispatch(Command::Action(Action::SelectCreases), &g.ctx);
            assert!(!g.app.ui.edges.selected.is_empty());
            g.save("crease-selection");
            // Check real hover, keyboard-focus, and disabled help rendering.
            hover_guide_label(&mut g, "Remove crease");
            assert!(
                g.text_positions
                    .iter()
                    .any(|(text, _)| text.contains("Remove sharp-edge marks"))
            );
            g.save("edge-help");
            g.frame(vec![egui::Event::PointerGone]);
            let mut focused_help = false;
            for _ in 0..40 {
                g.frame(vec![
                    slice_key(egui::Key::Tab, true),
                    slice_key(egui::Key::Tab, false),
                ]);
                g.frame(vec![]);
                if g.text_positions
                    .iter()
                    .any(|(text, _)| text.contains("Turn edge picking on or off"))
                {
                    focused_help = true;
                    break;
                }
            }
            assert!(focused_help, "Keyboard focus exposes edge-selection help");
            g.save("edge-focus-help");
            if let Some(id) = g.ctx.memory(|m| m.focused()) {
                g.ctx.memory_mut(|m| m.surrender_focus(id));
            }
            g.app.ui.mesh_busy = true;
            hover_guide_label(&mut g, "Smooth subdivide");
            assert!(
                g.text_positions
                    .iter()
                    .any(|(text, _)| text.contains("Wait for the mesh operation"))
            );
            g.save("edge-disabled-help");
        }
    }
    {
        let mut g = Guide::new(directory, [1200, 1000]);
        g.panel(Tool::Submodels, Dock::Right);
        g.app.ui.wire = true;
        g.app.camera.yaw = 0.35;
        g.app.camera.pitch = -0.4;
        let mut geometry = Geometry::new(Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap());
        geometry.scene.active_metadata = crate::sculpt::ModelMetadata {
            temporary_thickness: 0.35,
            thickness_loops: 2,
            thickness_crease: true,
            ..Default::default()
        };
        g.app.document.replace_geometry(geometry).unwrap();
        g.frame(vec![]);
        g.frame(vec![
            egui::Event::PointerMoved(Pos2::new(1080., 800.)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0., -300.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        g.frame(vec![egui::Event::PointerGone]);
        g.save("thickness-modifier");
    }
}

fn open_edge_controls(g: &mut Guide) {
    click_guide_label(g, "Edges and creases");
}

fn click_guide_modeling_checkbox(g: &mut Guide) {
    g.frame(vec![]);
    // The Mesh panel is painted after the top-bar Modeling menu.
    let point = g
        .text_positions
        .iter()
        .rev()
        .find(|(text, _)| text == "Modeling")
        .expect("Mesh panel Modeling checkbox is visible")
        .1;
    g.frame(vec![
        egui::Event::PointerMoved(point),
        primary_event(point, true),
    ]);
    g.frame(vec![primary_event(point, false), egui::Event::PointerGone]);
}

fn click_guide_label(g: &mut Guide, label: &str) {
    g.frame(vec![]);
    let point = g
        .text_positions
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("{label} is visible"))
        .1;
    g.frame(vec![
        egui::Event::PointerMoved(point),
        primary_event(point, true),
    ]);
    g.frame(vec![primary_event(point, false), egui::Event::PointerGone]);
}

#[test]
#[ignore = "renders project file dialogs; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn project_file_dialog_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    let projects = directory.join("projects");
    std::fs::create_dir_all(projects.join("Studies")).unwrap();
    let file = projects.join("Sculpt study.clay");
    persistence::save_project(&file.to_string_lossy(), &Document::default().project()).unwrap();
    let mut g = Guide::new(directory, [1200, 900]);
    g.interactive = true;
    g.app.ui.path = file.to_string_lossy().into();
    let before = g.app.document.geometry_snapshot();
    click_guide_label(&mut g, "Open");
    assert!(g.app.ui.file_picker.is_some());
    g.save("project-open-dialog");
    click_guide_label(&mut g, "Cancel");
    assert!(g.app.ui.file_picker.is_none());
    click_guide_label(&mut g, "Save as…");
    assert!(g.app.ui.file_picker.as_ref().unwrap().save);
    g.save("project-save-dialog");
    click_guide_label(&mut g, "Save project");
    assert!(g.app.ui.file_picker.as_ref().unwrap().overwrite.is_some());
    g.save("project-overwrite-dialog");
    click_guide_label(&mut g, "Cancel");
    assert!(g.app.ui.file_picker.is_none());
    assert_eq!(g.app.document.geometry_snapshot(), before);
}

#[test]
#[ignore = "renders asset file pickers; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn asset_file_dialog_offscreen_review() {
    let directory = std::path::PathBuf::from(std::env::var("CLAY_GUIDE_PREVIEW").unwrap());
    let images = directory.join("Images");
    std::fs::create_dir_all(images.join("Studies")).unwrap();
    let path = images.join("Front study.png");
    image::RgbImage::from_fn(32, 32, |x, y| {
        image::Rgb([(x * 8) as u8, (y * 8) as u8, 120])
    })
    .save(&path)
    .unwrap();
    std::fs::write(images.join("notes.txt"), "Not an image").unwrap();
    for size in [[1200, 900], [760, 560]] {
        let mut g = Guide::new(&directory, size);
        g.interactive = true;
        g.app
            .ui
            .preferences
            .dock_menu(BuiltinMenu::References, Dock::Right);
        g.app.ui.reference_path = images.to_string_lossy().into();
        click_guide_label(&mut g, "Add image…");
        assert_eq!(
            g.app.ui.file_picker.as_ref().unwrap().asset,
            Some(crate::command::AssetKind::Reference)
        );
        g.save(&format!("reference-picker-{}", size[0]));
        assert!(!g.text_positions.iter().any(|(text, _)| text == "notes.txt"));
        // Exercise selection and acceptance with actual pointer events.
        click_guide_label(&mut g, "Front study.png");
        g.frame(vec![]);
        let point = g
            .text_positions
            .iter()
            .rev()
            .find(|(text, _)| text == "Add reference image")
            .unwrap()
            .1;
        g.frame(vec![
            egui::Event::PointerMoved(point),
            primary_event(point, true),
        ]);
        g.frame(vec![primary_event(point, false), egui::Event::PointerGone]);
        assert!(g.app.ui.file_picker.is_none());
        assert_eq!(g.app.document.references().len(), 1);
        assert!(g.app.ui.arrange_references);
        g.save(&format!("reference-imported-{}", size[0]));
    }
    let mut g = Guide::new(&directory, [1200, 900]);
    g.interactive = true;
    g.panel(Tool::Alpha, Dock::Right);
    g.app.ui.alpha_path = images.to_string_lossy().into();
    let before = g.app.document.project();
    click_guide_label(&mut g, "Import image");
    g.save("alpha-import-expanded");
    // Toolbar wrapping can leave the expanded importer below the sidebar fold.
    // Reach the button through the actual sidebar scroll instead of assuming it
    // remains visible at a fixed window height.
    for _ in 0..12 {
        g.frame(vec![]);
        if g.text_positions.iter().any(|(text, point)| {
            text == "Choose image…" && point.y > 0. && point.y < g.size[1] as f32 - 20.
        }) {
            break;
        }
        g.frame(vec![
            egui::Event::PointerMoved(Pos2::new(g.size[0] as f32 - 120., g.size[1] as f32 - 40.)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0., -100.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        g.frame(vec![egui::Event::PointerGone]);
    }
    g.save("alpha-import-controls");
    click_guide_label(&mut g, "Choose image…");
    g.save("alpha-picker");
    click_guide_label(&mut g, "Front study.png");
    g.frame(vec![]);
    let point = g
        .text_positions
        .iter()
        .rev()
        .find(|(text, _)| text == "Preview alpha image")
        .unwrap()
        .1;
    g.frame(vec![
        egui::Event::PointerMoved(point),
        primary_event(point, true),
    ]);
    g.frame(vec![primary_event(point, false), egui::Event::PointerGone]);
    assert!(g.app.ui.file_picker.is_none());
    assert!(g.app.ui.alphas.draft.is_some());
    assert!(g.app.document.project() == before);
    g.save("alpha-import-preview");
}

fn hover_guide_label(g: &mut Guide, label: &str) {
    g.frame(vec![]);
    let point = g
        .text_positions
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("{label} is visible"))
        .1;
    g.frame(vec![egui::Event::PointerMoved(point)]);
    for _ in 0..12 {
        g.frame(vec![]);
    }
}

#[test]
#[ignore = "interactive rendered walkthrough; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn edge_workflow_offscreen_review() {
    use crate::sculpt::{BorderTarget, Geometry, PrimitiveKind};
    let Ok(directory) = std::env::var("CLAY_GUIDE_PREVIEW") else {
        return;
    };
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1200, 1250]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    g.app
        .document
        .replace_geometry(Geometry::new(
            Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap(),
        ))
        .unwrap();
    open_edge_controls(&mut g);
    let before = g.app.document.geometry_snapshot();
    click_guide_label(&mut g, "Select target borders");
    assert!(g.app.ui.edges.selecting);
    assert_eq!(g.app.ui.edges.selected.len(), 4);
    // Navigate from empty space across the surface while keeping the selection.
    let (rect, _) = g.frame(vec![]);
    let start = rect.min + Vec2::splat(60.);
    g.frame(vec![
        egui::Event::PointerMoved(start),
        primary_event(start, true),
    ]);
    g.frame(vec![egui::Event::PointerMoved(rect.center())]);
    g.frame(vec![
        primary_event(rect.center(), false),
        egui::Event::PointerGone,
    ]);
    assert_ne!(g.app.camera.yaw, 0.);
    assert_eq!(g.app.ui.edges.selected.len(), 4);
    assert_eq!(g.app.document.geometry_snapshot(), before);
    click_guide_label(&mut g, "Add crease");
    finish_mesh_job(&mut g.app);
    assert_eq!(g.app.document.mesh().creases.len(), 4);
    click_guide_label(&mut g, "Edge selection");
    assert!(!g.app.ui.edges.selecting);
    click_guide_label(&mut g, "Select creases");
    assert!(
        g.app.ui.edges.selecting,
        "Selecting creases must switch from sculpting to picking"
    );
    click_guide_label(&mut g, "Expand loop");
    click_guide_label(&mut g, "Remove crease");
    finish_mesh_job(&mut g.app);
    assert!(g.app.document.mesh().creases.is_empty());
    click_guide_label(&mut g, "Undo");
    assert_eq!(g.app.document.mesh().creases.len(), 4);
    click_guide_label(&mut g, "Subdivide (+ level)");
    finish_mesh_job(&mut g.app);
    assert_eq!(g.app.document.mesh_settings().levels(), 2);
    let stack = g.app.document.geometry_snapshot();
    click_guide_label(&mut g, "Smooth subdivide");
    finish_mesh_job(&mut g.app);
    assert_eq!(g.app.document.mesh_settings().levels(), 1);
    click_guide_label(&mut g, "Undo");
    assert_eq!(g.app.document.geometry_snapshot(), stack);
    g.save("subdivision-review");

    // Select each scope through the actual dropdown and insert loops using its button.
    for target in [
        BorderTarget::Holes,
        BorderTarget::Masked,
        BorderTarget::VisibleGroups,
    ] {
        let mut p = Primitive::for_kind(PrimitiveKind::Grid);
        p.divisions = [6, 6, 1];
        let mut mesh = p.mesh().unwrap();
        for (i, point) in mesh.vertices.iter().enumerate() {
            mesh.mask[i] = if point.0.abs() < 0.4 && point.1.abs() < 0.4 {
                1.
            } else {
                0.
            };
        }
        if target == BorderTarget::Holes {
            mesh = mesh.delete_masked().unwrap().unwrap();
        }
        if target == BorderTarget::VisibleGroups {
            mesh.create_vertex_group("Center".into(), 0.5, false)
                .unwrap();
        }
        g.app
            .document
            .replace_geometry(Geometry::new(mesh))
            .unwrap();
        g.frame(vec![]);
        let current_target = g.app.ui.edges.target.label();
        click_guide_label(&mut g, current_target);
        click_guide_label(&mut g, target.label());
        if target == BorderTarget::Holes {
            click_guide_label(&mut g, "Support loops");
        }
        let before = g.app.document.geometry_snapshot();
        click_guide_label(&mut g, "Add support loops");
        finish_mesh_job(&mut g.app);
        assert!(
            g.app.document.mesh().vertices.len() > before.mesh.vertices.len(),
            "{}: {}",
            target.label(),
            g.app.ui.status
        );
        click_guide_label(&mut g, "Undo");
        assert_eq!(g.app.document.geometry_snapshot(), before);
    }

    g.app
        .document
        .replace_geometry(Geometry::new(
            Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap(),
        ))
        .unwrap();
    g.panel(Tool::Mesh, Dock::Hidden);
    g.panel(Tool::Submodels, Dock::Right);
    click_guide_label(&mut g, "Thickness modifier");
    click_guide_label(&mut g, "Crease rim edges");
    // Rim loop count is covered by the preview/apply equality regression too.
    let source = g.app.document.geometry_snapshot();
    let expected = source
        .mesh
        .solidify_with_metadata(&source.scene.active_metadata)
        .unwrap();
    assert!(!expected.creases.is_empty());
    click_guide_label(&mut g, "Apply Thickness modifier");
    assert_eq!(*g.app.document.mesh(), expected);
    click_guide_label(&mut g, "Undo");
    assert_eq!(g.app.document.geometry_snapshot(), source);
    g.save("thickness-review");
}

#[test]
#[ignore = "interactive modeling walkthrough; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_workflow_offscreen_review() {
    use crate::sculpt::Geometry;
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1100, 1100]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [3; 3];
    g.app
        .document
        .replace_geometry(Geometry::new(primitive.mesh().unwrap()))
        .unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    g.app.camera.zoom = 0.65;
    g.frame(vec![]);
    click_guide_modeling_checkbox(&mut g);
    assert!(g.app.ui.modeling.active);
    let (rect, _) = g.frame(vec![]);
    let center = rect.center();
    g.frame(vec![
        egui::Event::PointerMoved(center),
        primary_event(center, true),
    ]);
    g.frame(vec![primary_event(center, false)]);
    assert_eq!(g.app.ui.modeling.count, 1);
    click_guide_label(&mut g, "Selection to vertex group");
    click_guide_label(&mut g, "Create group from selection");
    assert_eq!(g.app.document.mesh().vertex_groups.len(), 1);
    g.save("modeling-selection");
    click_guide_label(&mut g, "Selection to vertex group");
    click_guide_label(&mut g, "Extrude");
    let before = g.app.document.geometry_snapshot();
    let history = g.app.document.history_counts();
    g.app.camera.yaw = 0.2;
    g.app.camera.pitch = -0.15;
    g.frame(vec![
        egui::Event::PointerMoved(center),
        primary_event(center, true),
    ]);
    g.frame(vec![egui::Event::PointerMoved(
        center + Vec2::new(0., -70.),
    )]);
    assert!(g.app.viewport.modeling.preview().is_some());
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.save("modeling-extrusion-preview");
    g.frame(vec![slice_key(egui::Key::Escape, true)]);
    g.frame(vec![
        primary_event(center, false),
        slice_key(egui::Key::Escape, false),
    ]);
    assert_eq!(g.app.document.history_counts(), history);
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    let distance = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom * 2.;
    g.stroke(center, center + Vec2::new(0., distance));
    assert_eq!(
        g.app.document.undo_label(),
        Some("Extrude tunnel"),
        "{}",
        g.app.ui.status
    );
    g.app.document.mesh().validate_closed().unwrap();
    assert_eq!(g.app.document.history_counts().0, history.0 + 1);
    // Small turn exposes the thickness of the tunnel walls.
    g.app.camera.yaw = 0.2;
    g.app.camera.pitch = -0.15;
    g.save("modeling-tunnel");
    click_guide_label(&mut g, "Undo");
    assert_eq!(g.app.document.geometry_snapshot(), before);
    assert!(g.app.ui.modeling.active);
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    click_guide_label(&mut g, "Edge");
    click_guide_label(&mut g, "Loop cut");
    let edge = center + Vec2::new(distance / 6., 0.);
    g.frame(vec![egui::Event::PointerMoved(edge)]);
    g.save("modeling-loop-preview");
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.frame(vec![primary_event(edge, true)]);
    g.frame(vec![primary_event(edge, false)]);
    assert!(
        g.app.document.mesh().vertices.len() > before.mesh.vertices.len(),
        "{}",
        g.app.ui.status
    );
    assert_eq!(g.app.document.undo_label(), Some("Insert edge loops"));
    let single_count = g.app.document.mesh().vertices.len() - before.mesh.vertices.len();
    click_guide_label(&mut g, "Undo");
    click_guide_label(&mut g, "+");
    click_guide_label(&mut g, "+");
    assert_eq!(g.app.ui.modeling.loop_count, 3);
    g.frame(vec![
        egui::Event::PointerMoved(edge),
        primary_event(edge, true),
    ]);
    let slid = edge + Vec2::new(0., 10.);
    g.frame(vec![egui::Event::PointerMoved(slid)]);
    assert!(g.app.viewport.modeling.dragging());
    g.save("modeling-loop-drag");
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.frame(vec![primary_event(slid, false)]);
    assert_eq!(
        g.app.document.mesh().vertices.len() - before.mesh.vertices.len(),
        single_count * 3
    );
    assert_eq!(g.app.document.history_counts().0, history.0 + 1);
    click_guide_label(&mut g, "Undo");
    assert_eq!(g.app.document.geometry_snapshot(), before);
}

#[test]
#[ignore = "boundary/adjacent tunnel walkthrough; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_boundary_join_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1100, 900]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [4; 3];
    let mut mesh = primitive.mesh().unwrap();
    for v in &mut mesh.vertices {
        v.2 *= 0.1;
    }
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(mesh))
        .unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    g.app.camera.zoom = 0.65;
    click_guide_modeling_checkbox(&mut g);
    click_guide_label(&mut g, "Extrude");
    let before = g.app.document.geometry_snapshot();
    let history = g.app.document.history_counts();
    let (rect, _) = g.frame(vec![]);
    let scale = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom;
    // Adjacent holes, followed by a notch reaching the outside bottom edge.
    for (x, y) in [(0.25, 0.25), (-0.25, 0.25), (0.25, -0.75)] {
        let start = rect.center() + Vec2::new(x * scale, -y * scale);
        g.stroke(start, start + Vec2::new(0., 0.21 * scale));
        assert_eq!(
            g.app.document.undo_label(),
            Some("Extrude tunnel"),
            "{}",
            g.app.ui.status
        );
        g.app.document.mesh().validate_closed().unwrap();
    }
    assert_eq!(g.app.document.history_counts().0, history.0 + 3);
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = -0.2;
    g.save("modeling-boundary-join");
    let after = g.app.document.geometry_snapshot();
    for _ in 0..3 {
        g.app.dispatch(Command::Undo, &g.ctx);
    }
    assert_eq!(g.app.document.geometry_snapshot(), before);
    for _ in 0..3 {
        g.app.dispatch(Command::Redo, &g.ctx);
    }
    assert_eq!(g.app.document.geometry_snapshot(), after);
    g.app.document.project().validate().unwrap();
}

#[test]
#[ignore = "selection painting and loops walkthrough; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_selection_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1100, 1000]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Grid);
    primitive.divisions = [5, 5, 1];
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(primitive.mesh().unwrap()))
        .unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    g.app.camera.zoom = 0.7;
    click_guide_modeling_checkbox(&mut g);
    let before = g.app.document.geometry_snapshot();
    let history = g.app.document.history_counts();
    let (rect, _) = g.frame(vec![]);
    let scale = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom;
    let p = |x: f32, y: f32| rect.center() + Vec2::new(x * scale, -y * scale);
    g.stroke(p(-0.8, 0.), p(0.8, 0.));
    assert_eq!(g.app.ui.modeling.count, 5);
    g.save("modeling-paint-selection");
    click_guide_label(&mut g, "Loop");
    g.frame(vec![egui::Event::PointerMoved(p(0., 0.18))]);
    g.save("modeling-face-loop");
    g.frame(vec![primary_event(p(0., 0.18), true)]);
    g.frame(vec![primary_event(p(0., 0.18), false)]);
    assert_eq!(g.app.ui.modeling.count, 5);
    click_guide_label(&mut g, "Edge");
    click_guide_label(&mut g, "Parallel edges");
    g.frame(vec![
        egui::Event::PointerMoved(p(0.2, 0.)),
        primary_event(p(0.2, 0.), true),
    ]);
    g.frame(vec![primary_event(p(0.2, 0.), false)]);
    assert_eq!(g.app.ui.modeling.count, 6);
    g.save("modeling-edge-ring");
    click_guide_label(&mut g, "Loop");
    g.frame(vec![
        egui::Event::PointerMoved(p(0.2, 0.)),
        primary_event(p(0.2, 0.), true),
    ]);
    g.frame(vec![primary_event(p(0.2, 0.), false)]);
    assert_eq!(g.app.ui.modeling.count, 5);
    g.save("modeling-edge-loop");
    assert_eq!(g.app.document.geometry_snapshot(), before);
    assert_eq!(g.app.document.history_counts(), history);
}

#[test]
#[ignore = "symmetric selection, extrusion and dissolve walkthrough; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_symmetry_dissolve_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1100, 1050]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Grid);
    primitive.divisions = [5, 5, 1];
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(primitive.mesh().unwrap()))
        .unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    g.app.camera.zoom = 0.65;
    click_guide_modeling_checkbox(&mut g);
    click_guide_label(&mut g, "Symmetry");
    assert!(g.app.document.symmetry().enabled);
    let (rect, _) = g.frame(vec![]);
    let scale = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom;
    let p = |x: f32, y: f32| rect.center() + Vec2::new(x * scale, -y * scale);
    g.frame(vec![
        egui::Event::PointerMoved(p(0.4, 0.)),
        primary_event(p(0.4, 0.), true),
    ]);
    g.frame(vec![primary_event(p(0.4, 0.), false)]);
    assert_eq!(g.app.ui.modeling.count, 2);
    g.save("modeling-symmetry-selection");
    click_guide_label(&mut g, "Extrude");
    let before = g.app.document.geometry_snapshot();
    let history = g.app.document.history_counts();
    g.frame(vec![
        egui::Event::PointerMoved(p(0.4, 0.)),
        primary_event(p(0.4, 0.), true),
    ]);
    let end = p(0.4, 0.) + Vec2::new(0., -50.);
    g.frame(vec![egui::Event::PointerMoved(end)]);
    let preview = g.app.viewport.modeling.preview().expect("mirrored preview");
    assert_eq!(preview.quads.len(), before.mesh.quads.len() + 8);
    crate::sculpt::ModelingSymmetry::new(preview, g.app.document.symmetry()).unwrap();
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.frame(vec![primary_event(end, false)]);
    assert_eq!(g.app.document.history_counts().0, history.0 + 1);
    g.app.camera.yaw = 0.25;
    g.app.camera.pitch = -0.2;
    g.save("modeling-symmetry-extrude");
    click_guide_label(&mut g, "Undo");
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    assert_eq!(g.app.document.geometry_snapshot(), before);
    click_guide_label(&mut g, "Edge");
    click_guide_label(&mut g, "Loop");
    g.frame(vec![
        egui::Event::PointerMoved(p(0.2, 0.)),
        primary_event(p(0.2, 0.), true),
    ]);
    g.frame(vec![primary_event(p(0.2, 0.), false)]);
    assert_eq!(g.app.ui.modeling.count, 10);
    g.frame(vec![egui::Event::PointerMoved(
        rect.left_top() + Vec2::new(5., 5.),
    )]);
    g.save("modeling-dissolve-before");
    click_guide_label(&mut g, "Dissolve selected loops");
    assert_eq!(
        (
            g.app.document.mesh().vertices.len(),
            g.app.document.mesh().quads.len()
        ),
        (24, 15),
        "{}",
        g.app.ui.status
    );
    assert_eq!(g.app.document.history_counts().0, history.0 + 1);
    g.save("modeling-dissolve-after");
    click_guide_label(&mut g, "Undo");
    assert_eq!(g.app.document.geometry_snapshot(), before);
}

#[test]
#[ignore = "integrated pen construction, presets and smooth preview; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_milestones_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1200, 1250]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [3; 3];
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(primitive.mesh().unwrap()))
        .unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    g.app.camera.zoom = 0.65;
    click_guide_modeling_checkbox(&mut g);
    click_guide_label(&mut g, "Symmetry");
    let (rect, _) = g.frame(vec![]);
    let scale = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom;
    let start = rect.center() + Vec2::new(scale * 2. / 3., 0.);
    g.frame(vec![
        egui::Event::PointerMoved(start),
        primary_event(start, true),
    ]);
    g.frame(vec![primary_event(start, false)]);
    assert_eq!(g.app.ui.modeling.count, 2);
    click_guide_label(&mut g, "Inset");
    g.stroke(start, start + Vec2::new(0., -scale * 0.08));
    assert_eq!(g.app.viewport.modeling.selected.len(), 2);
    assert!((g.app.ui.modeling.amount - 0.08).abs() < 1e-4);
    g.save("modeling-milestone-inset");
    let repeat_target = start + Vec2::new(0., -scale * 2. / 3.);
    let faces = g.app.document.mesh().quads.len();
    let history = g.app.document.history_counts().0;
    g.frame(vec![
        egui::Event::PointerMoved(repeat_target),
        primary_event(repeat_target, true),
    ]);
    g.frame(vec![primary_event(repeat_target, false)]);
    assert_eq!(g.app.document.mesh().quads.len(), faces + 8);
    assert_eq!(g.app.document.history_counts().0, history + 1);
    assert!((g.app.ui.modeling.amount - 0.08).abs() < 1e-4);
    g.save("modeling-tap-repeat");
    click_guide_label(&mut g, "Tool presets");
    click_guide_label(&mut g, "Preset name");
    g.frame(vec![egui::Event::Text("Measured inset".into())]);
    click_guide_label(&mut g, "Save tool preset");
    assert_eq!(g.app.ui.preferences.modeling_presets.len(), 1);
    click_guide_label(&mut g, "Tool presets");
    click_guide_label(&mut g, "Extrude");
    g.stroke(start, start + Vec2::new(0., -scale * 0.2));
    g.frame(vec![]);
    assert_eq!(g.app.viewport.modeling.selected.len(), 2);
    crate::sculpt::ModelingSymmetry::new(g.app.document.mesh(), g.app.document.symmetry()).unwrap();
    click_guide_label(&mut g, "Scale");
    g.stroke(start, start + Vec2::new(0., -15.));
    let cage = g.app.document.geometry_snapshot();
    let history = g.app.document.history_counts();
    click_guide_label(&mut g, "Smooth preview");
    g.frame(vec![]);
    assert_eq!(g.app.document.geometry_snapshot(), cage);
    assert_eq!(g.app.document.history_counts(), history);
    assert!(g.app.viewport.modeling.preview().unwrap().quads.len() > cage.mesh.quads.len());
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = -0.2;
    g.save("modeling-milestone-smooth");
    click_guide_label(&mut g, "Tool presets");
    click_guide_label(&mut g, "Use Measured inset");
    assert_eq!(g.app.ui.modeling.tool, crate::viewport::ModelTool::Inset);
    assert!((g.app.ui.modeling.amount - 0.08).abs() < 1e-4);
    assert_eq!(g.app.document.geometry_snapshot(), cage);
    g.save("modeling-milestone-preset");
}

#[test]
#[ignore = "renders exact face group assignment; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_face_group_gap_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1200, 1100]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [3, 5, 1];
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(primitive.mesh().unwrap()))
        .unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    g.app.camera.zoom = 0.65;
    click_guide_modeling_checkbox(&mut g);
    click_guide_label(&mut g, "Symmetry");
    let (rect, _) = g.frame(vec![]);
    let scale = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom;
    let start = rect.center() + Vec2::new(scale * 2. / 3., -scale * 0.8);
    let end = rect.center() + Vec2::new(scale * 2. / 3., scale * 0.8);
    g.stroke(start, end);
    let selected: std::collections::BTreeSet<_> = g
        .app
        .viewport
        .modeling
        .selected_faces()
        .into_iter()
        .collect();
    assert_eq!(selected.len(), 10);
    click_guide_label(&mut g, "Selection to vertex group");
    click_guide_label(&mut g, "Create group from selection");
    assert_eq!(g.app.document.mesh().vertex_groups.len(), 1);
    for (face, polygon) in g.app.document.mesh().polygons().iter().enumerate() {
        assert_eq!(polygon.group, usize::from(selected.contains(&face)));
    }
    g.app.ui.show_polygroups = true;
    g.app.camera.yaw = 0.2;
    g.app.camera.pitch = -0.1;
    g.save("modeling-face-group-gap");
}

#[test]
#[ignore = "renders edge, point and face construction; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_component_construction_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1200, 1200]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [4; 3];
    let mesh = primitive.mesh().unwrap();
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(mesh.clone()))
        .unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    g.app.camera.zoom = 0.65;
    click_guide_modeling_checkbox(&mut g);
    click_guide_label(&mut g, "Symmetry");
    click_guide_label(&mut g, "Vertex");
    click_guide_label(&mut g, "Split point");
    let (rect, _) = g.frame(vec![]);
    let scale = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom;
    let point = rect.center() + Vec2::new(scale * 0.5, -scale * 0.5);
    click_guide_label(&mut g, "Crease ring");
    g.stroke(point, point + Vec2::new(0., -scale * 0.1));
    g.app.document.mesh().validate_closed().unwrap();
    assert_ne!(g.app.document.mesh(), &mesh);
    crate::sculpt::ModelingSymmetry::new(g.app.document.mesh(), g.app.document.symmetry()).unwrap();
    g.app.camera.yaw = 0.2;
    g.app.camera.pitch = -0.1;
    g.save("modeling-point-split");
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    click_guide_label(&mut g, "Face");
    click_guide_label(&mut g, "Extrude");
    let split = g.app.document.mesh().clone();
    g.stroke(point, point + Vec2::new(0., -scale * 0.35));
    assert_ne!(g.app.document.mesh(), &split);
    g.app.document.mesh().validate_closed().unwrap();
    g.app.camera.yaw = 0.3;
    g.app.camera.pitch = -0.2;
    g.save("modeling-point-tube");
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(mesh.clone()))
        .unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    click_guide_label(&mut g, "Edge");
    click_guide_label(&mut g, "Extrude edge");
    let point = rect.center() + Vec2::new(scale * 0.75, -scale * 0.5 + 2.);
    g.frame(vec![
        egui::Event::PointerMoved(point),
        primary_event(point, true),
    ]);
    g.frame(vec![egui::Event::PointerMoved(
        point + Vec2::new(0., -scale * 0.15),
    )]);
    assert_eq!(g.app.document.mesh(), &mesh);
    assert_ne!(g.app.viewport.modeling.preview().unwrap(), &mesh);
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = -0.25;
    g.save("modeling-edge-construction-preview");
    g.frame(vec![primary_event(point, false)]);
    g.app.document.mesh().validate_closed().unwrap();
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(mesh.clone()))
        .unwrap();
    click_guide_label(&mut g, "Face");
    click_guide_label(&mut g, "Split face");
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    let point = rect.center() + Vec2::new(scale * 0.75, -scale * 0.75);
    g.frame(vec![
        egui::Event::PointerMoved(point),
        primary_event(point, true),
    ]);
    g.frame(vec![primary_event(point, false)]);
    assert_ne!(g.app.document.mesh(), &mesh);
    g.app.document.mesh().validate_closed().unwrap();
    g.save("modeling-face-split");
}

#[test]
#[ignore = "renders component gizmos and modeling initialization; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_transform_gizmo_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1200, 1150]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    let before = g.app.document.geometry_snapshot();
    click_guide_label(&mut g, "New mesh…");
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.save("modeling-new-mesh-warning");
    click_guide_label(&mut g, "Replace and start modeling");
    assert!(g.app.ui.modeling.active);
    g.app.document.require_modeling().unwrap();
    g.app.camera.yaw = 0.;
    g.app.camera.pitch = 0.;
    g.app.camera.zoom = 0.65;
    let (rect, _) = g.frame(vec![]);
    g.stroke(rect.center(), rect.center());
    assert_eq!(g.app.viewport.modeling.selected.len(), 1);
    click_guide_label(&mut g, "Move");
    click_guide_label(&mut g, "Gizmo");
    let before = g.app.document.geometry_snapshot();
    g.stroke(
        rect.center() + Vec2::new(85., 0.),
        rect.center() + Vec2::new(110., 0.),
    );
    assert_ne!(g.app.document.geometry_snapshot(), before);
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = -0.25;
    g.save("modeling-move-gizmo");
    click_guide_label(&mut g, "Scale");
    g.save("modeling-scale-gizmo");
    click_guide_label(&mut g, "Rotate");
    g.save("modeling-rotate-gizmo");
    click_guide_label(&mut g, "Free");
    assert_eq!(
        g.app.ui.modeling.interaction,
        crate::viewport::ModelInteraction::Free
    );
}

#[test]
#[ignore = "renders connected region fusion; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn modeling_region_fuse_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(std::path::Path::new(&directory), [1200, 1050]);
    g.interactive = true;
    g.panel(Tool::Mesh, Dock::Right);
    for (divisions, xs, name) in [
        ([4, 4, 1], [-0.25, 0.25], "modeling-region-fuse"),
        ([6, 4, 1], [-0.5, 0.5], "modeling-separate-fuse"),
    ] {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
        primitive.divisions = divisions;
        let mut mesh = primitive.mesh().unwrap();
        for p in &mut mesh.vertices {
            p.2 *= 0.15;
        }
        g.app
            .document
            .replace_geometry(crate::sculpt::Geometry::new(mesh))
            .unwrap();
        g.app.camera.yaw = 0.;
        g.app.camera.pitch = 0.;
        g.app.camera.zoom = 0.8;
        if !g.app.ui.modeling.active {
            click_guide_modeling_checkbox(&mut g);
        }
        click_guide_label(&mut g, "Select");
        g.app.ui.modeling.change = crate::viewport::SelectionChange::Replace;
        let (rect, _) = g.frame(vec![]);
        let scale = rect.width().min(rect.height()) * 0.35 * g.app.camera.zoom;
        for x in xs {
            let point = rect.center() + Vec2::new(x * scale, 0.25 * scale);
            g.frame(vec![
                egui::Event::PointerMoved(point),
                primary_event(point, true),
            ]);
            g.frame(vec![primary_event(point, false)]);
            g.app.ui.modeling.change = crate::viewport::SelectionChange::Add;
        }
        assert_eq!(g.app.viewport.modeling.selected.len(), 2);
        click_guide_label(&mut g, "Extrude and fuse");
        g.app.ui.modeling.join = true;
        g.app.ui.modeling.amount = -0.8;
        g.app.camera.yaw = 0.4;
        g.app.camera.pitch = -0.2;
        g.save(&format!("{name}-selection"));
        let before = g.app.document.geometry_snapshot();
        let history = g.app.document.history_counts();
        click_guide_label(&mut g, "Apply amount");
        g.app.document.mesh().validate_closed().unwrap();
        g.save(&format!("{name}-result"));
        assert!((g.app.ui.modeling.amount + 0.3).abs() < 1e-5);
        assert_eq!(g.app.document.history_counts().0, history.0 + 1);
        click_guide_label(&mut g, "Undo");
        assert_eq!(g.app.document.geometry_snapshot(), before);
    }
}

#[test]
#[ignore = "renders projection controls; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn detail_projection_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1200, 850]);
    g.interactive = true;
    let mut source = crate::sculpt::Mesh::sphere();
    for p in &mut source.vertices {
        *p = *p * (1. + 0.07 * (p.0 * 12.).cos() * (p.1 * 12.).cos());
    }
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(source))
        .unwrap();
    g.app.dispatch(Command::KeepProjectionReference, &g.ctx);
    g.app.dispatch(Command::SetDynamicMesh(true), &g.ctx);
    g.app
        .dispatch(Command::SetRemeshSettings(24, false), &g.ctx);
    g.app.dispatch(Command::Remesh, &g.ctx);
    finish_mesh_job(&mut g.app);
    g.app.camera.zoom = 0.85;
    g.app.camera.pan = Vec2::new(200., 0.);
    let before = g.app.document.geometry_snapshot();
    g.app.document.mark_saved("projection-review.clay");
    g.app.dispatch(Command::OpenProjection, &g.ctx);
    g.app.ui.projection.options.max_distance = 0.2;
    g.save("detail-projection-before");
    click_guide_label(&mut g, "Use model below");
    click_guide_label(&mut g, "Preview projection");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while g.app.ui.projection.busy {
        g.app.poll_projection();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(g.app.ui.projection.report.as_ref().unwrap().moved > 0);
    let preview = g.app.projection.draft.as_ref().unwrap().mesh.clone();
    assert_eq!(g.app.document.geometry_snapshot(), before);
    assert!(!g.app.document.is_dirty());
    g.save("detail-projection-preview");
    click_guide_label(&mut g, "Cancel");
    assert!(!g.app.ui.projection.open);
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.app.dispatch(Command::OpenProjection, &g.ctx);
    click_guide_label(&mut g, "Preview projection");
    while g.app.ui.projection.busy {
        g.app.poll_projection();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    click_guide_label(&mut g, "Apply projection");
    assert_eq!(g.app.document.mesh(), preview.as_ref());
    assert_eq!(g.app.document.undo_label(), Some("Project details"));
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.app.dispatch(Command::Redo, &g.ctx);
    assert_eq!(g.app.document.mesh(), preview.as_ref());
}

#[test]
#[ignore = "renders submodel deletion; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn delete_submodel_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1100, 850]);
    g.interactive = true;
    g.panel(Tool::Submodels, Dock::Left);
    g.app.dispatch(Command::KeepProjectionReference, &g.ctx);
    g.app.dispatch(Command::SelectSubmodel(1), &g.ctx);
    let before = g.app.document.geometry_snapshot();
    let counts = g.app.document.history_counts();
    g.save("submodel-delete");
    click_guide_label(&mut g, "Delete submodel");
    assert_eq!(g.app.document.model_count(), 1);
    assert_eq!(g.app.document.history_counts().0, counts.0 + 1);
    assert_eq!(g.app.document.undo_label(), Some("Delete submodel"));
    let after = g.app.document.geometry_snapshot();
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.app.dispatch(Command::Redo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), after);
    click_guide_label(&mut g, "Delete submodel");
    assert_eq!(g.app.document.model_count(), 1);
    assert!(g.app.document.mesh().vertices.is_empty());
    g.save("submodel-delete-empty");
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), after);
}

#[test]
#[ignore = "renders alpha library and stencil workflows; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn alpha_library_stencil_offscreen_review() {
    use crate::sculpt::{
        Alpha, Geometry,
        alpha::{Asset, Stencil},
    };
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(std::path::Path::new(&directory), [1300, 1000]);
    g.interactive = true;
    g.panel(Tool::Alpha, Dock::Right);
    let mut primitive = Primitive::for_kind(PrimitiveKind::Plane);
    primitive.divisions = [128, 128, 1];
    g.app
        .document
        .replace_geometry(Geometry::new(primitive.mesh().unwrap()))
        .unwrap();
    // Built-ins also exist in projects with no saved library assets.
    assert!(g.app.document.alphas().is_empty());
    for (name, alpha) in [
        ("Hard", Alpha::Hard),
        ("Square", Alpha::Square),
        ("Soft", Alpha::Soft),
    ] {
        g.app.document.mark_saved("alpha-builtins.clay");
        let previous = g.app.document.brushes()[0].alpha.clone();
        click_guide_label(&mut g, name);
        assert_eq!(g.app.document.brushes()[0].alpha, previous);
        assert!(!g.app.document.is_dirty());
        click_guide_label(&mut g, "Use on brush");
        assert_eq!(g.app.document.brushes()[0].alpha, alpha);
        click_guide_label(&mut g, "Place stencil");
        assert_eq!(g.app.document.stencil().unwrap().asset.image, alpha);
        // Allow the new floating Area to finish its initial sizing pass before
        // locating a label that also appears in the sidebar.
        g.frame(vec![]);
        g.frame(vec![]);
        g.frame(vec![]);
        click_guide_label(&mut g, "Remove stencil");
        assert!(g.app.document.stencil().is_none());
    }
    click_guide_label(&mut g, "Duplicate alpha");
    assert!(g.app.ui.alphas.draft.is_some());
    click_guide_label(&mut g, "Save alpha");
    assert_eq!(g.app.document.alphas()[0].name, "Soft copy");
    click_guide_label(&mut g, "Remove alpha");
    assert!(g.app.document.alphas().is_empty());
    let image = Alpha::Image {
        width: 128,
        height: 128,
        pixels: (0..128 * 128)
            .map(|i| {
                let x = (i % 128) as f32 / 127. * 2. - 1.;
                let y = (i / 128) as f32 / 127. * 2. - 1.;
                if x.abs() < 0.72 && y.abs() < 0.72 {
                    ((x * 12.).sin() * 2.).clamp(0., 1.)
                } else {
                    0.
                }
            })
            .collect::<Vec<_>>()
            .into(),
    };
    let asset = Asset {
        name: "Carved stripes".into(),
        category: "Relief".into(),
        image,
    };
    g.app.document.save_alpha(None, asset.clone()).unwrap();
    g.app.ui.alphas.selected = Some(crate::ui::alphas::Selection::Project(0));
    g.save("alpha-library");
    click_guide_label(&mut g, "Edit alpha");
    assert!(g.app.ui.alphas.draft.is_some());
    g.save("alpha-editor");
    click_guide_label(&mut g, "Cancel");
    assert!(g.app.ui.alphas.draft.is_none());
    click_guide_label(&mut g, "Use on brush");
    assert_eq!(g.app.document.brushes()[0].alpha, asset.image);
    // Use a soft brush to brush through the placed pattern, rather than stamp it twice.
    g.app.document.set_alpha(0, Alpha::Soft).unwrap();
    let mut stencil = Stencil::new(asset);
    stencil.rotation = 20.;
    g.app.dispatch(Command::SetStencil(Some(stencil)), &g.ctx);
    click_guide_label(&mut g, "Stencil placement");
    g.save("alpha-view-stencil");
    let (rect, _) = g.frame(vec![]);
    g.app.dispatch(Command::AttachStencil, &g.ctx);
    g.frame(vec![
        egui::Event::PointerMoved(rect.center()),
        primary_event(rect.center(), true),
    ]);
    g.frame(vec![primary_event(rect.center(), false)]);
    assert!(g.app.document.stencil().unwrap().surface.is_some());
    let before = g.app.document.geometry_snapshot();
    g.stroke(
        rect.center() + Vec2::new(-140., 0.),
        rect.center() + Vec2::new(140., 0.),
    );
    assert!(g.app.document.geometry_snapshot() != before);
    let mut stencil = g.app.document.stencil().unwrap().clone();
    stencil.opacity = 0.;
    g.app.dispatch(Command::SetStencil(Some(stencil)), &g.ctx);
    g.save("alpha-sculpt-through");
    let mut stencil = g.app.document.stencil().unwrap().clone();
    stencil.opacity = 0.3;
    g.app.dispatch(Command::SetStencil(Some(stencil)), &g.ctx);
    g.app.camera.yaw = 0.45;
    g.app.camera.pitch = -0.25;
    g.save("alpha-surface-stencil");
    let geometry = g.app.document.geometry_snapshot();
    let alphas = g.app.document.alphas().to_vec();
    let brush_alpha = g.app.document.brushes()[0].alpha.clone();
    click_guide_label(&mut g, "Remove stencil");
    assert!(g.app.document.stencil().is_none());
    assert_eq!(g.app.document.geometry_snapshot(), geometry);
    assert_eq!(g.app.document.alphas(), alphas);
    assert_eq!(g.app.document.brushes()[0].alpha, brush_alpha);
    // A coarse closed surface must gain real geometry beneath a built-in stencil.
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [10, 10, 10];
    g.app
        .document
        .replace_geometry(Geometry::new(cube.mesh().unwrap()))
        .unwrap();
    g.app
        .document
        .set_adaptive(AdaptiveSettings {
            enabled: true,
            detail: 4.,
            simplify: true,
            ..Default::default()
        })
        .unwrap();
    g.app.camera = Default::default();
    let mut brush = g.app.document.brushes()[0].clone();
    brush.stroke_mode = crate::sculpt::StrokeMode::DragRect;
    brush.alpha = Alpha::Soft;
    brush.inner_radius = 0.7;
    brush.strength = 0.15;
    g.app.document.update_brush(0, brush).unwrap();
    click_guide_label(&mut g, "Nature's curve");
    click_guide_label(&mut g, "Place stencil");
    g.app.ui.wire = true;
    g.app.ui.wire_triangles = true;
    let before = g.app.document.geometry_snapshot();
    let (rect, _) = g.frame(vec![]);
    g.save("alpha-adaptive-before");
    let start = rect.center();
    let end = start + Vec2::new(270., 0.);
    g.frame(vec![
        egui::Event::PointerMoved(start),
        primary_event(start, true),
    ]);
    g.frame(vec![egui::Event::PointerMoved(end)]);
    assert!(
        g.app.document.geometry_snapshot() == before,
        "stamp preview stays private"
    );
    g.frame(vec![primary_event(end, false), egui::Event::PointerGone]);
    assert!(g.app.document.mesh().vertices.len() > before.mesh.vertices.len());
    assert!(g.app.document.mesh().vertices.iter().any(|p| p.2 > 1.01));
    let mut stencil = g.app.document.stencil().unwrap().clone();
    stencil.opacity = 0.;
    g.app.dispatch(Command::SetStencil(Some(stencil)), &g.ctx);
    g.save("alpha-adaptive-wire");
    g.app.ui.wire = false;
    g.app.camera.yaw = 0.3;
    g.app.camera.pitch = -0.2;
    g.save("alpha-adaptive-spiral");
    g.app.document.undo();
    assert!(g.app.document.geometry_snapshot() == before);
    click_guide_label(&mut g, "Remove stencil");
    g.app.camera = Default::default();
    click_guide_label(&mut g, "Weathered bricks");
    click_guide_label(&mut g, "Place stencil");
    review_stencil_controls(&mut g);
}

#[test]
#[ignore = "renders on-canvas stencil controls; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn stencil_controls_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    std::fs::create_dir_all(&directory).unwrap();
    // A short sidebar overflows, reproducing the parent scrollbar obstruction.
    let mut g = Guide::new(std::path::Path::new(&directory), [1300, 860]);
    g.interactive = true;
    let cube = Primitive::for_kind(PrimitiveKind::Cube);
    g.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(cube.mesh().unwrap()))
        .unwrap();
    g.app
        .document
        .set_stencil(Some(crate::sculpt::alpha::Stencil::new(
            crate::sculpt::alpha::builtins()[7].clone(),
        )))
        .unwrap();
    review_stencil_controls(&mut g);
    g.panel(Tool::Alpha, Dock::Right);
    g.app.ui.alphas.selected = Some(crate::ui::alphas::Selection::Builtin(7));
    click_guide_label(&mut g, "Place stencil");
    g.save("alpha-library-expanded");
    let handle = g
        .text_positions
        .iter()
        .find(|(text, _)| text == "Drag to resize library")
        .unwrap()
        .1;
    let geometry = g.app.document.geometry_snapshot();
    // Wake the parent's floating scrollbar before reaching for the handle.
    g.frame(vec![egui::Event::PointerMoved(Pos2::new(1294., handle.y))]);
    g.frame(vec![
        egui::Event::PointerMoved(handle),
        primary_event(handle, true),
    ]);
    let end = handle + Vec2::new(0., 70.);
    g.frame(vec![egui::Event::PointerMoved(end)]);
    g.frame(vec![primary_event(end, false)]);
    g.frame(vec![]);
    let resized = g
        .text_positions
        .iter()
        .find(|(text, _)| text == "Drag to resize library")
        .unwrap()
        .1;
    assert!(
        (resized.y - handle.y - 70.).abs() < 2.,
        "the centered handle resizes despite the parent scrollbar"
    );
    assert!(g.app.document.geometry_snapshot() == geometry);
    g.save("alpha-library-resized");
}

#[test]
#[ignore = "renders a crowded sidebar resize; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn alpha_library_crowded_sidebar_offscreen_review() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(std::path::Path::new(&directory), [1300, 860]);
    g.panel(Tool::Brush, Dock::Right);
    g.panel(Tool::Alpha, Dock::Right);
    g.app.ui.alphas.selected = Some(crate::ui::alphas::Selection::Builtin(7));
    let handle = |g: &Guide| {
        g.text_positions
            .iter()
            .find_map(|(text, pos)| (text == "Drag to resize library").then_some(*pos))
    };
    // Scroll the preceding brush controls out of view, keeping room below the handle.
    for _ in 0..40 {
        g.frame(vec![]);
        if handle(&g).is_some_and(|pos| (200. ..720.).contains(&pos.y)) {
            break;
        }
        let sidebar_margin = 1300. - g.app.ui.preferences.right_width + 12.;
        g.frame(vec![
            egui::Event::PointerMoved(Pos2::new(sidebar_margin, 500.)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0., -60.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    g.save("alpha-library-crowded-before");
    let start = handle(&g).expect("visible library resize handle");
    assert!((200. ..720.).contains(&start.y));
    g.frame(vec![
        egui::Event::PointerMoved(start),
        primary_event(start, true),
    ]);
    let end = start + Vec2::new(0., 70.);
    for step in 1..=10 {
        g.frame(vec![egui::Event::PointerMoved(
            start.lerp(end, step as f32 / 10.),
        )]);
    }
    g.frame(vec![primary_event(end, false)]);
    g.save("alpha-library-crowded-after");
    let resized = handle(&g).unwrap();
    assert!(
        (resized.y - start.y - 70.).abs() < 2.,
        "the library must grow below the brush panel: {start:?} -> {resized:?}"
    );
}

fn review_stencil_controls(g: &mut Guide) {
    let before = g.app.document.geometry_snapshot();
    g.panel(Tool::Alpha, Dock::Hidden);
    let (rect, _) = g.frame(vec![]);
    g.frame(vec![]);
    let unit = rect.width().min(rect.height());
    let start = rect.center() - Vec2::new(0., unit * 0.25 + 19.);
    let end = start + Vec2::new(60., 30.);
    g.frame(vec![
        egui::Event::PointerMoved(start),
        primary_event(start, true),
    ]);
    g.frame(vec![egui::Event::PointerMoved(end)]);
    g.frame(vec![primary_event(end, false)]);
    assert!((g.app.document.stencil().unwrap().position[0] - 60. / unit).abs() < 1e-5);
    g.save("alpha-stencil-controls");
    click_guide_label(g, "Reset placement");
    assert_eq!(g.app.document.stencil().unwrap().position, [0.; 2]);
    click_guide_label(g, "Stencil enabled");
    assert!(!g.app.document.stencil().unwrap().enabled);
    click_guide_label(g, "Remove stencil");
    assert!(g.app.document.stencil().is_none());
    assert!(g.app.document.geometry_snapshot() == before);
}

#[test]
#[ignore = "renders viewport function menus; requires Vulkan and CLAY_GUIDE_PREVIEW"]
fn viewport_context_menu_offscreen_review() {
    let directory = std::path::PathBuf::from(std::env::var("CLAY_GUIDE_PREVIEW").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    for size in [[1100, 800], [640, 480]] {
        let mut g = Guide::new(&directory, size);
        g.interactive = true;
        g.app
            .document
            .replace_geometry(crate::sculpt::Geometry::new(
                Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap(),
            ))
            .unwrap();
        g.app.dispatch(Command::ModelingMode(true), &g.ctx);
        g.frame(vec![]);
        let history = g.app.document.history_counts();
        let before = g.app.document.geometry_snapshot();
        click_guide_label(&mut g, "Tool menu");
        assert!(g.app.ui.viewport_menu.context.is_some());
        g.app.ui.viewport_menu.position =
            Some(egui::pos2(size[0] as f32 - 10., size[1] as f32 - 10.));
        g.save(&format!("viewport-menu-{}", size[0]));
        click_guide_label(&mut g, "Edge");
        assert_eq!(g.app.ui.modeling.mode, crate::viewport::ComponentMode::Edge);
        assert!(g.app.ui.viewport_menu.context.is_some());
        click_guide_label(&mut g, "Parallel edges");
        assert_eq!(g.app.ui.modeling.span, crate::viewport::SelectionSpan::Ring);
        assert!(g.app.ui.viewport_menu.context.is_some());
        g.save(&format!("viewport-menu-edge-{}", size[0]));
        click_guide_label(&mut g, "Vertex");
        assert_eq!(
            g.app.ui.modeling.mode,
            crate::viewport::ComponentMode::Vertex
        );
        click_guide_label(&mut g, "Connected");
        assert_eq!(
            g.app.ui.modeling.span,
            crate::viewport::SelectionSpan::Connected
        );
        click_guide_label(&mut g, "Face");
        assert_eq!(g.app.ui.modeling.mode, crate::viewport::ComponentMode::Face);
        assert_eq!(g.app.ui.viewport_menu.selected, 0);
        // Up from the first selector wraps to the last function and reveals it,
        // including when the small-window menu needs to scroll.
        for pressed in [true, false] {
            g.frame(vec![egui::Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        g.save(&format!("viewport-menu-keyboard-{}", size[0]));
        let point = g
            .text_positions
            .iter()
            .find(|(text, _)| text == "Split face")
            .expect("last Modeling function remains available")
            .1;
        assert!(point.y > 0. && point.y < size[1] as f32 - 10.);
        click_guide_label(&mut g, "Split face");
        assert_eq!(
            g.app.ui.modeling.tool,
            crate::viewport::ModelTool::FaceSplit
        );
        assert!(g.app.ui.viewport_menu.context.is_none());
        click_guide_label(&mut g, "Modeling");
        g.save(&format!("modeling-bar-menu-{}", size[0]));
        let pin = g
            .ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("icon_button", "Pin menu open"))))
            .unwrap()
            .center();
        g.frame(vec![
            egui::Event::PointerMoved(pin),
            primary_event(pin, true),
        ]);
        g.frame(vec![primary_event(pin, false), egui::Event::PointerGone]);
        g.save(&format!("modeling-pinned-{}", size[0]));
        assert_eq!(
            g.app.ui.preferences.pinned_menus[0].menu,
            BuiltinMenu::Modeling
        );
        let unpin = g
            .ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("icon_button", "Unpin menu"))))
            .unwrap()
            .center();
        assert!(unpin.y >= 0. && unpin.y < size[1] as f32);
        g.frame(vec![
            egui::Event::PointerMoved(unpin),
            primary_event(unpin, true),
        ]);
        g.frame(vec![primary_event(unpin, false), egui::Event::PointerGone]);
        assert!(g.app.ui.preferences.pinned_menus.is_empty());
        for dock in [Dock::Left, Dock::Right] {
            g.app.dispatch(
                Command::ModelingComponent(crate::viewport::ComponentMode::Face),
                &g.ctx,
            );
            g.app.ui.preferences.dock_menu(BuiltinMenu::Modeling, dock);
            g.frame(vec![]);
            click_guide_label(&mut g, "Edge");
            g.save(&format!("modeling-docked-{dock:?}-top-{}", size[0]));
            for _ in 0..8 {
                if g.text_positions
                    .iter()
                    .any(|(text, point)| text == "Parallel edges" && point.y < size[1] as f32 - 20.)
                {
                    break;
                }
                let x = if dock == Dock::Left {
                    120.
                } else {
                    size[0] as f32 - 120.
                };
                g.frame(vec![
                    egui::Event::PointerMoved(Pos2::new(x, size[1] as f32 - 40.)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0., -60.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                g.frame(vec![egui::Event::PointerGone]);
            }
            click_guide_label(&mut g, "Parallel edges");
            assert_eq!(g.app.ui.modeling.span, crate::viewport::SelectionSpan::Ring);
            g.save(&format!("modeling-docked-{dock:?}-{}", size[0]));
        }
        assert_eq!(g.app.document.geometry_snapshot(), before);
        assert_eq!(g.app.document.history_counts(), history);
        g.app.viewport.destroy_renderer();
    }
}

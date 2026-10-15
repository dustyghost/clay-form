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
impl Guide {
    fn new(directory: &std::path::Path, size: [i32; 2]) -> Self {
        let mut capture = UiCapture::new(size);
        let mut app = App::default();
        app.viewport.initialize_renderer(&capture.gl).unwrap();
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
        self.app.viewport.destroy_renderer(&self.capture.gl);
    }
}

#[test]
#[ignore = "renders documentation screenshots; requires EGL and CLAY_GUIDE_PREVIEW directory"]
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
        g.app.dispatch(Command::ImportObj, &g.ctx);
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
#[ignore = "renders mirror documentation screenshots; requires EGL and CLAY_GUIDE_PREVIEW directory"]
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
#[ignore = "renders edge tool documentation; requires EGL and CLAY_GUIDE_PREVIEW directory"]
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
#[ignore = "interactive rendered walkthrough; requires EGL and CLAY_GUIDE_PREVIEW"]
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
#[ignore = "interactive modeling walkthrough; requires EGL and CLAY_GUIDE_PREVIEW"]
fn modeling_workflow_offscreen_review() {
    use crate::sculpt::Geometry;
    let directory = std::env::var("CLAY_GUIDE_PREVIEW").expect("Set CLAY_GUIDE_PREVIEW");
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut g = Guide::new(directory, [1100, 900]);
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
    click_guide_label(&mut g, "Pen modeling");
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
#[ignore = "boundary/adjacent tunnel walkthrough; requires EGL and CLAY_GUIDE_PREVIEW"]
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
    click_guide_label(&mut g, "Pen modeling");
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
#[ignore = "selection painting and loops walkthrough; requires EGL and CLAY_GUIDE_PREVIEW"]
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
    click_guide_label(&mut g, "Pen modeling");
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
#[ignore = "symmetric selection, extrusion and dissolve walkthrough; requires EGL and CLAY_GUIDE_PREVIEW"]
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
    click_guide_label(&mut g, "Pen modeling");
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

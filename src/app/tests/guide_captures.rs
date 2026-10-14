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
    directory: std::path::PathBuf,
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
            directory: directory.into(),
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
                    let _ = self
                        .app
                        .ui
                        .toolbar(ctx, &self.app.document, &self.app.camera);
                }
                let _ =
                    self.app
                        .ui
                        .brush_panel(ctx, &self.app.document, "Guide", 1., &self.app.camera);
                egui::CentralPanel::default().show(ctx, |ui| {
                    rect = ui.available_rect_before_wrap();
                    if self.viewport {
                        self.app.show_viewport(ui);
                    }
                });
                self.app.show_recovery(ctx);
            },
        );
        (rect, self.capture.render(&self.ctx, output))
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
    // A real base-head model supplies the front/side reference images.
    {
        let mut g = Guide::new(directory, [1100, 760]);
        let head = Primitive::for_kind(PrimitiveKind::HumanHead)
            .mesh()
            .unwrap();
        for (name, yaw, center) in [
            ("Front study", 0., [0.18, 0.3]),
            ("Side study", std::f32::consts::FRAC_PI_2, [0.18, 0.75]),
        ] {
            let icon = crate::brush_icon::capture(
                &head,
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
            .replace_geometry(crate::sculpt::Geometry::new(head))
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

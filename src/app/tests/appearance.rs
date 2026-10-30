use super::*;
use crate::{
    preferences::BuiltinMenu,
    sculpt::{
        Geometry, Kind, Material, Primitive, PrimitiveKind,
        texture::{Channel, Precision},
    },
};

struct AppearanceHarness {
    app: App,
    ctx: egui::Context,
    size: [i32; 2],
    output: egui::FullOutput,
    capture: Option<crate::viewport::UiCapture>,
    pixels: Vec<u8>,
    time: f64,
    alpha: bool,
}
impl AppearanceHarness {
    fn new(size: [i32; 2], capture: bool) -> Self {
        let mut app = App::default();
        app.document
            .replace_geometry(Geometry::new(
                Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap(),
            ))
            .unwrap();
        let ctx = egui::Context::default();
        ctx.style_mut(|s| s.animation_time = 0.);
        Self {
            app,
            ctx,
            size,
            output: Default::default(),
            capture: capture.then(|| crate::viewport::UiCapture::new(size)),
            pixels: vec![],
            time: 0.,
            alpha: false,
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.1;
        let mut commands = Vec::new();
        self.output = self.ctx.run(
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
                commands.extend(
                    self.app
                        .ui
                        .toolbar(ctx, &self.app.document, &self.app.camera),
                );
                if self.alpha {
                    commands.extend(self.app.ui.brush_panel(
                        ctx,
                        &self.app.document,
                        "Review",
                        1.,
                        &self.app.camera,
                    ));
                }
                commands.extend(self.app.ui.uv_window(ctx, &self.app.document, &[]));
                egui::CentralPanel::default().show(ctx, |_ui| {});
            },
        );
        if let Some(capture) = &mut self.capture {
            self.pixels = capture.render(&self.ctx, self.output.clone());
        }
        for command in commands {
            self.app.dispatch(command, &self.ctx);
        }
    }
    fn point(&self, label: &str) -> Option<Pos2> {
        self.output.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                let p = t.pos + t.galley.size() * 0.5;
                (s.clip_rect.shrink(2.).contains(p) && self.ctx.content_rect().contains(p))
                    .then_some(p)
            }
            _ => None,
        })
    }
    fn scroll(&mut self, delta: f32) {
        self.frame(vec![
            egui::Event::PointerMoved(Pos2::new(42., 350.)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0., delta),
                modifiers: Default::default(),
            },
        ]);
        self.frame(vec![]);
    }
    fn reach(&mut self, label: &str) -> Pos2 {
        self.frame(vec![]);
        if let Some(p) = self.point(label) {
            return p;
        }
        self.scroll(10000.);
        for _ in 0..80 {
            if let Some(p) = self.point(label) {
                return p;
            }
            self.scroll(-100.);
        }
        let labels: Vec<_> = self
            .output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if s.clip_rect.contains(t.pos) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        panic!("Unreachable control {label}: {labels:?}");
    }
    fn click(&mut self, label: &str) {
        let p = self.reach(label);
        self.frame(vec![egui::Event::PointerMoved(p), primary_event(p, true)]);
        self.frame(vec![primary_event(p, false)]);
        self.frame(vec![]);
        self.frame(vec![]);
    }
    fn open(&mut self, action: Action) {
        self.app.dispatch(Command::Action(action), &self.ctx);
        self.frame(vec![]);
        self.frame(vec![]);
    }
    fn save(&self, name: &str) {
        if let Some(capture) = &self.capture {
            let directory = std::path::PathBuf::from(
                std::env::var("CLAY_APPEARANCE_REVIEW")
                    .unwrap_or_else(|_| "/tmp/clay-appearance-review".into()),
            );
            std::fs::create_dir_all(&directory).unwrap();
            capture.save(
                directory.join(format!("{name}-{}x{}.png", self.size[0], self.size[1])),
                &self.pixels,
            );
        }
    }
}

fn texture_workflow(size: [i32; 2], capture: bool) {
    let mut h = AppearanceHarness::new(size, capture);
    h.open(Action::OpenTextures);
    assert_ne!(
        h.app.document.brushes()[h.app.ui.selected].kind,
        Kind::Paint
    );
    h.click("Create library image");
    let first = h.app.ui.uv.asset_selected.unwrap();
    assert!(h.app.document.mesh().uv.textures.is_empty());
    assert!(h.app.document.texture_asset(first).is_some());
    h.click("Create / import image"); // reclaim space after the first creation
    h.reach("Selected image: Colour image");
    h.save("texture-created");
    h.click("Generate UVs for this model");
    finish_mesh_job(&mut h.app);
    h.frame(vec![]);
    assert!(h.app.document.mesh().uv.current(h.app.document.mesh()));
    h.click("Assign selected to model");
    assert_eq!(h.app.document.mesh().uv.textures[0].id, first);
    h.click("Duplicate image");
    let second = h.app.ui.uv.asset_selected.unwrap();
    assert_ne!(first, second);
    assert_eq!(
        h.app.document.mesh().uv.textures[0].id,
        first,
        "duplicating does not reassign"
    );
    h.click("Assign selected to model");
    assert_eq!(h.app.document.mesh().uv.textures[0].id, second);
    h.click("Unassign selected image");
    assert!(h.app.document.mesh().uv.textures.is_empty());
    assert!(h.app.document.texture_asset(second).is_some());
    h.click("Delete unused image…");
    h.click("Delete unused image");
    assert!(h.app.document.texture_asset(second).is_none());
    h.app.dispatch(Command::Undo, &h.ctx);
    assert!(h.app.document.texture_asset(second).is_some());
    h.app.ui.uv.asset_selected = Some(first);
    h.frame(vec![]);
    h.click("Assign selected to model");
    h.app.document.duplicate_model(false).unwrap();
    h.frame(vec![]);
    assert_eq!(h.app.document.texture_users(first).len(), 2);
    h.reach("Replace image pixels…");
    h.click("Replace image pixels…");
    h.reach("Choose replacement file…");
    h.save("texture-shared-replace");
    h.click("Keep current pixels");
    let before = h.app.document.geometry_snapshot();
    h.click("Colour image copy");
    assert_eq!(
        h.app.document.geometry_snapshot(),
        before,
        "browsing never assigns"
    );
    assert_eq!(h.app.ui.uv.asset_selected, Some(second));
    h.click("Export selected image");
    let export = std::env::temp_dir().join(format!(
        "clay-library-export-{}.png",
        crate::sculpt::texture::Texture::new_id()
    ));
    h.click("texture.png");
    h.frame(vec![
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        },
        egui::Event::Text(export.to_string_lossy().into()),
    ]);
    h.click("Export selected pixels");
    let exported = crate::persistence::texture::load(export.to_str().unwrap(), Channel::Color)
        .unwrap_or_else(|error| panic!("Export selected image: {error}; {}", h.app.ui.status));
    assert_eq!(
        exported.pixels,
        h.app.document.texture_asset(second).unwrap().pixels
    );
    std::fs::remove_file(export).unwrap();
    assert_eq!(
        h.app.document.mesh().uv.textures[0].id,
        first,
        "export addresses the selected asset without reassigning it"
    );
    h.app.ui.uv.asset_selected = Some(first);
    h.frame(vec![]);
    h.click("Paint assigned image");
    assert_eq!(h.app.document.texture_paint, Some(Channel::Color));
    assert_eq!(
        h.app.document.brushes()[h.app.ui.selected].kind,
        Kind::Paint
    );
    assert!(!h.app.ui.uv.open);
}

#[test]
fn appearance_texture_controls_normal_and_compact() {
    for size in [[1200, 900], [720, 600]] {
        texture_workflow(size, false);
    }
}

#[test]
fn appearance_material_controls_without_selecting_paint() {
    let mut h = AppearanceHarness::new([900, 700], false);
    let sculpt = h.app.ui.selected;
    let index = h.app.ui.paint_input_index(&h.app.document).unwrap();
    h.open(Action::OpenMaterials);
    h.click("Copper");
    assert_eq!(h.app.ui.selected, sculpt);
    assert!(h.app.document.mesh().materials.is_empty());
    assert_eq!(
        h.app.document.brushes()[index].material.primary_asset(),
        Some(5)
    );
    h.click("Assign material to model");
    assert!(
        h.app
            .document
            .mesh()
            .materials
            .iter()
            .all(|m| *m == Material::examples()[4].1.with_link(5))
    );
    h.app.dispatch(Command::Undo, &h.ctx);
    assert!(h.app.document.mesh().materials.is_empty());
    h.app.ui.preferences.pinned_menus.clear();
    h.open(Action::OpenColours);
    h.click("+ Colour");
    h.click("Add colour");
    assert_eq!(h.app.ui.selected, sculpt);
    assert!(h.app.document.mesh().materials.is_empty());
    assert!(
        h.app.document.brushes()[index]
            .material
            .primary_asset()
            .is_none()
    );
    h.click("Paint with these inputs");
    assert_eq!(h.app.ui.selected, index);
    assert!(
        h.app
            .ui
            .preferences
            .pinned_menus
            .iter()
            .any(|p| p.menu == BuiltinMenu::Colours)
    );
}

#[test]
fn appearance_actions_preserve_docked_layout_and_preset_inputs() {
    let mut h = AppearanceHarness::new([900, 700], false);
    h.app
        .ui
        .preferences
        .dock_menu(BuiltinMenu::Materials, crate::preferences::Dock::Right);
    h.app
        .ui
        .preferences
        .menu_sections
        .iter_mut()
        .find(|s| s.menu == BuiltinMenu::Materials)
        .unwrap()
        .open = false;
    h.open(Action::OpenMaterials);
    assert!(h.app.ui.preferences.pinned_menus.is_empty());
    assert!(
        h.app
            .ui
            .preferences
            .menu_sections
            .iter()
            .any(|s| s.menu == BuiltinMenu::Materials && s.open)
    );
    let original = h.app.ui.paint_input_index(&h.app.document).unwrap();
    let mut custom = crate::sculpt::Brush::new(Kind::Paint);
    custom.name = "HDR detail".into();
    custom.texture_colour = Some([4., -1., 0.5]);
    let index = h.app.document.append_brush(custom.clone()).unwrap();
    h.app.ui.select_brush(index, &h.app.document);
    h.app.ui.select_brush(0, &h.app.document);
    assert_eq!(h.app.ui.paint_input_index(&h.app.document), Some(index));
    h.app
        .texture_command(crate::app::textures::TextureCommand::Paint(None))
        .unwrap();
    assert_eq!(h.app.ui.selected, index);
    assert_eq!(
        h.app.document.brushes()[index].texture_colour,
        custom.texture_colour
    );
    assert_eq!(h.app.document.brushes()[index].material, custom.material);
    assert_ne!(index, original);
}

#[test]
fn appearance_import_replace_float_and_export_bind_image_identity() {
    let mut h = AppearanceHarness::new([900, 700], false);
    let directory = std::env::temp_dir().join(format!(
        "clay-appearance-{}",
        crate::sculpt::texture::Texture::new_id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("source.exr");
    let mut image = crate::sculpt::texture::Texture::new_with_precision(
        Channel::Color,
        16,
        &Material::default(),
        Precision::Float32,
    )
    .unwrap();
    image.pixels.set_pixel(0, [4., -1., 0.25, 0.5]);
    crate::persistence::texture::save(&path, &image).unwrap();
    h.app
        .import_asset(
            crate::command::AssetKind::Texture,
            path.to_string_lossy().into(),
        )
        .unwrap();
    let id = h.app.ui.uv.asset_selected.unwrap();
    assert!(h.app.document.mesh().uv.textures.is_empty());
    let before = h.app.document.texture_asset(id).unwrap().clone();
    image.pixels.set_pixel(0, [8., -2., 0.5, 1.]);
    crate::persistence::texture::save(&path, &image).unwrap();
    h.app
        .import_asset(
            crate::command::AssetKind::ReplaceTexture(id),
            path.to_string_lossy().into(),
        )
        .unwrap();
    assert_eq!(
        h.app.document.texture_asset(id).unwrap().pixels,
        image.pixels
    );
    h.app.dispatch(Command::Undo, &h.ctx);
    assert_eq!(h.app.document.texture_asset(id), Some(&before));
    let project_path = directory.join("library.clay");
    crate::persistence::save_project(project_path.to_str().unwrap(), &h.app.document.project())
        .unwrap();
    let saved = crate::persistence::load_project(project_path.to_str().unwrap()).unwrap();
    assert_eq!(saved.scene.texture_library[0], before);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "manual appearance workflow captures with actual controls; requires Vulkan"]
fn appearance_workflow_offscreen_review() {
    for size in [[1200, 900], [720, 600]] {
        texture_workflow(size, true);
    }
    let mut comparison = AppearanceHarness::new([1200, 900], true);
    for panel in &mut comparison.app.ui.preferences.panels {
        panel.dock = if panel.tool == crate::preferences::Tool::Alpha {
            crate::preferences::Dock::Right
        } else {
            crate::preferences::Dock::Hidden
        };
    }
    comparison.alpha = true;
    comparison.open(Action::OpenTextures);
    comparison.click("Create library image");
    comparison.click("Create / import image");
    comparison.save("textures-beside-alphas");
    for size in [[1200, 900], [720, 600]] {
        let mut h = AppearanceHarness::new(size, true);
        h.open(Action::OpenMaterials);
        h.click("Copper");
        h.save("materials");
        h.click("Assign material to model");
        h.click("Edit shared project material");
        h.reach("Shared colour");
        h.save("shared-material");
        h.app.ui.preferences.pinned_menus.clear();
        h.open(Action::OpenColours);
        h.save("colours");
    }
}

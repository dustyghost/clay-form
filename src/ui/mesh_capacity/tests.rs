use super::*;
use egui::{Event, Pos2, Rect, Vec2};
struct Harness {
    state: UiState,
    document: Document,
    ctx: egui::Context,
    output: Option<egui::FullOutput>,
    capture: Option<crate::viewport::UiCapture>,
}
impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        ctx.style_mut(|s| s.animation_time = 0.);
        Self {
            state: UiState::default(),
            document: Document::default(),
            ctx,
            output: None,
            capture: None,
        }
    }
    fn frame(&mut self, events: Vec<Event>) {
        design::appearance(&self.ctx, &self.state.preferences.appearance);
        let out = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    if self.capture.is_some() {
                        Vec2::new(480., 330.)
                    } else {
                        Vec2::new(900., 600.)
                    },
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if !self.state.mesh_capacity_open {
                        self.state.builtin_menu_contents(
                            crate::preferences::BuiltinMenu::Settings,
                            ui,
                            &self.document,
                            &crate::camera::Camera::default(),
                            &mut Vec::new(),
                        );
                    }
                });
                self.state.settings(ctx, &self.document);
            },
        );
        if let Some(capture) = &mut self.capture {
            let pixels = capture.render(&self.ctx, out.clone());
            capture.save(
                std::env::var_os("CLAY_MESH_CAPACITY_PREVIEW")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::temp_dir().join("clay-mesh-capacity.png")),
                &pixels,
            );
        }
        self.output = Some(out);
    }
    fn point(&self, text: &str) -> Pos2 {
        self.output
            .as_ref()
            .unwrap()
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t)
                    if t.galley.text() == text
                        && s.clip_rect.contains(t.pos + t.galley.size() * 0.5) =>
                {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing visible control: {text}"))
    }
    fn click(&mut self, text: &str) {
        let pos = self.point(text);
        self.frame(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(vec![]);
        self.frame(vec![]);
    }
}
#[test]
fn mesh_capacity_settings_open_edit_and_reset_without_changing_active_limits() {
    let mut h = Harness::new();
    h.frame(vec![]);
    h.frame(vec![]);
    h.click("Mesh capacity…");
    assert!(h.state.mesh_capacity_open);
    h.click("500000");
    let key = |key, modifiers| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    h.frame(vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        Event::Text("2000000".into()),
        key(egui::Key::Enter, egui::Modifiers::NONE),
    ]);
    h.frame(vec![]);
    assert_eq!(h.state.preferences.mesh_capacity.vertices, 2_000_000);
    h.point("Restart required to apply these limits.");
    assert_eq!(capacity::current(), MeshCapacity::DEFAULT);
    h.click("Reset mesh limits");
    assert_eq!(h.state.preferences.mesh_capacity, MeshCapacity::DEFAULT);
}
#[test]
#[ignore = "manual GPU capture of the mesh capacity settings"]
fn gpu_mesh_capacity_settings_capture() {
    let mut h = Harness::new();
    h.capture = Some(crate::viewport::UiCapture::new([480, 330]));
    h.frame(vec![]);
    h.frame(vec![]);
    h.click("Mesh capacity…");
    h.state.preferences.mesh_capacity = MeshCapacity {
        vertices: 6_000_000,
        triangles: 12_000_000,
    };
    h.frame(vec![]);
    h.frame(vec![]);
    h.point("Restart required to apply these limits.");
    h.point("Reset mesh limits");
}

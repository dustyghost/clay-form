use super::*;
use crate::app::App;
use egui::{Event, Pos2, Rect};
struct Harness {
    app: App,
    ui: UiState,
    ctx: egui::Context,
    capture: crate::viewport::UiCapture,
    output: Option<egui::FullOutput>,
}
impl Harness {
    fn frame(&mut self, events: Vec<Event>) {
        if let Some(out) = self.output.take() {
            self.capture.render(&self.ctx, out);
        }
        self.output = Some(self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 900.))),
                events,
                ..Default::default()
            },
            |ctx| {
                let mut commands = self.ui.uv_window(ctx, &self.app.document, &[]);
                if self.ui.modeling.active {
                    egui::Window::new("Modeling tools")
                        .default_width(450.)
                        .show(ctx, |ui| {
                            self.ui
                                .modeling_controls(ui, &self.app.document, &mut commands)
                        });
                }
                for command in commands {
                    self.app.dispatch(command, ctx);
                }
            },
        ));
        if std::env::var_os("CLAY_UV_DEBUG_CAPTURE").is_some() {
            let pixels = self
                .capture
                .render(&self.ctx, self.output.as_ref().unwrap().clone());
            self.capture
                .save(std::env::temp_dir().join("clay-uv-last.png"), &pixels);
        }
    }
    fn label(&self, label: &str) -> Pos2 {
        self.output
            .as_ref()
            .unwrap()
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing visible control: {label}"))
    }
    fn click(&mut self, label: &str) {
        let p = self.label(label);
        self.tap(p);
        self.frame(vec![]);
        self.frame(vec![]);
    }
    fn tap(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p), button(p, true)]);
        self.frame(vec![button(p, false)]);
    }
    fn canvas(&self) -> Rect {
        self.output
            .as_ref()
            .unwrap()
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r)
                    if r.fill == Color32::from_gray(24) && r.rect.width() > 170. =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .expect("UV canvas")
    }
    fn drag(&mut self, start: Pos2, end: Pos2, cancel: bool) {
        self.frame(vec![Event::PointerMoved(start), button(start, true)]);
        self.frame(vec![Event::PointerMoved(end)]);
        if cancel {
            self.frame(vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }]);
        }
        self.frame(vec![button(end, false)]);
        self.frame(vec![]);
    }
}
fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
#[test]
#[ignore = "requires a Vulkan adapter for actual UI rendering"]
fn uv_island_controls_offscreen_review() {
    let mut h = Harness {
        app: App::default(),
        ui: UiState::default(),
        ctx: egui::Context::default(),
        capture: crate::viewport::UiCapture::new([1200, 900]),
        output: None,
    };
    h.ctx.style_mut(|s| s.animation_time = 0.);
    let mut p = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
    p.divisions = [4; 3];
    let mut mesh = p.mesh().unwrap();
    mesh.uv = mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
    // Leave room to move and rotate the island without leaving the canvas.
    let faces = (0..mesh.triangles.len()).collect::<Vec<_>>();
    mesh.uv.transform(&faces, [0.; 2], 0.55, 0.).unwrap();
    h.app
        .document
        .replace_geometry(crate::sculpt::Geometry::new(mesh))
        .unwrap();
    h.ui.uv.open = true;
    h.frame(vec![]);
    h.frame(vec![]);
    let small = h.canvas();
    h.click("Expand editor");
    let big = h.canvas();
    assert!(big.width() > small.width());
    assert!(h.label("Apply transform").y < 900.);
    h.tap(big.center());
    h.frame(vec![]);
    assert!(!h.ui.uv.selected.is_empty());
    let before = h.app.document.mesh().uv.clone();
    let history = h.app.document.history_counts().0;
    let start = interaction::pivot(&before, &h.ui.uv.selected, big).unwrap();
    h.drag(start, start + egui::vec2(24., -12.), false);
    assert_ne!(h.app.document.mesh().uv.corners, before.corners);
    assert_eq!(h.app.document.history_counts().0, history + 1);
    h.app.viewport.modeling.sync(&h.app.document);
    assert!(
        !h.app.viewport.modeling.selected.is_empty(),
        "UV edits retain the linked 3D selection"
    );
    h.app.dispatch(Command::Undo, &h.ctx);
    h.frame(vec![]);
    assert_eq!(h.app.document.mesh().uv, before);
    for (label, tool, delta) in [
        ("Rotate", interaction::Tool::Rotate, egui::vec2(-38., -38.)),
        ("Scale", interaction::Tool::Scale, egui::vec2(15., -15.)),
    ] {
        h.click(label);
        let center = interaction::pivot(&before, &h.ui.uv.selected, h.canvas()).unwrap();
        let start = interaction::handle(tool, center);
        h.drag(start, start + delta, false);
        assert_ne!(h.app.document.mesh().uv.corners, before.corners, "{label}");
        h.app.dispatch(Command::Undo, &h.ctx);
        h.frame(vec![]);
        assert_eq!(h.app.document.mesh().uv, before);
        h.drag(start, start + delta, true);
        assert_eq!(h.app.document.mesh().uv, before, "Escape cancels {label}");
    }
    h.click("Paint mask");
    let rect = h.canvas();
    let q = before.corners[0][0];
    let point = egui::pos2(
        rect.left() + q[0] * rect.width(),
        rect.bottom() - q[1] * rect.height(),
    );
    h.drag(point, point + egui::vec2(4., 0.), true);
    assert_eq!(
        h.app.document.mesh().uv,
        before,
        "Escape cancels a mask stroke"
    );
    h.drag(point, point + egui::vec2(4., 0.), false);
    assert_eq!(h.app.document.mesh().uv.mask(0, 0), 1.);
    let masked = h.app.document.mesh().uv.clone();
    h.click("Move");
    let start = interaction::pivot(&masked, &h.ui.uv.selected, h.canvas()).unwrap();
    h.drag(start, start + egui::vec2(25., 15.), false);
    assert_eq!(h.app.document.mesh().uv.corners[0][0], q);
    assert_ne!(h.app.document.mesh().uv.corners, masked.corners);
    let output = h.output.take().unwrap();
    let pixels = h.capture.render(&h.ctx, output);
    h.capture.save(
        std::env::temp_dir().join("clay-uv-expanded-controls.png"),
        &pixels,
    );
    h.frame(vec![]);
    h.click("Clear UV mask");
    assert!(!h.app.document.mesh().uv.has_mask());
    let before_remove = h.app.document.geometry_snapshot();
    h.click("Remove islands");
    assert!(h.app.document.mesh().uv.islands().is_empty());
    assert_eq!(
        h.app.document.mesh().triangles,
        before_remove.mesh.triangles
    );
    h.app.dispatch(Command::Undo, &h.ctx);
    h.frame(vec![]);
    assert_eq!(h.app.document.geometry_snapshot(), before_remove);
    h.click("Restore window");
    assert!(!h.ui.uv.expanded);
    assert!(h.canvas().width() < big.width());
    h.ui.uv.open = false;
    h.ui.modeling.active = true;
    h.ui.modeling.mode = crate::viewport::ComponentMode::Edge;
    h.ui.modeling.count = 1;
    let edge = h
        .app
        .document
        .mesh()
        .polygon_edges()
        .into_iter()
        .find(|e| !h.app.document.mesh().uv.seams.contains(e))
        .unwrap();
    h.app.viewport.modeling.selected =
        std::collections::BTreeSet::from([crate::viewport::Component::Edge(edge)]);
    h.frame(vec![]);
    h.frame(vec![]);
    h.click("Mark UV seams");
    assert!(h.app.document.mesh().uv.seams.contains(&edge));
    h.click("Clear UV seams");
    assert!(!h.app.document.mesh().uv.seams.contains(&edge));
    h.app.dispatch(Command::Undo, &h.ctx);
    assert!(h.app.document.mesh().uv.seams.contains(&edge));
    h.click("Paint away selection");
    assert!(h.ui.modeling.erase_radius > 0.);
}

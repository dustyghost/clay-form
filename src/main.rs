mod model;
mod tablet;
use eframe::egui::{self, Color32, Pos2, Sense, Vec2};
use model::*;
struct App {
    mesh: Mesh,
    brushes: Vec<Brush>,
    selected: usize,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: Vec2,
    path: String,
    current_path: String,
    pending_save_path: String,
    preset_path: String,
    alpha_path: String,
    status: String,
    undo: Vec<Mesh>,
    redo: Vec<Mesh>,
    stroke: bool,
    last: Option<Pos2>,
    tablet: tablet::Tablet,
    pressure: f32,
    dirty: bool,
    confirm: Option<Action>,
    wire: bool,
    symmetry: bool,
}
#[derive(Clone, Copy)]
enum Action {
    New,
    Open,
    Quit,
}
impl Default for App {
    fn default() -> Self {
        Self {
            mesh: Mesh::sphere(),
            brushes: Kind::ALL.into_iter().map(Brush::new).collect(),
            selected: 0,
            yaw: 0.,
            pitch: 0.,
            zoom: 1.,
            pan: Vec2::ZERO,
            path: "sculpt.clay".into(),
            current_path: "sculpt.clay".into(),
            pending_save_path: "sculpt.clay".into(),
            preset_path: "brush.brush.json".into(),
            alpha_path: String::new(),
            status: "Ready • Sculpt with left drag; Alt reverses; Shift smooths".into(),
            undo: vec![],
            redo: vec![],
            stroke: false,
            last: None,
            tablet: tablet::Tablet::new(),
            pressure: 1.,
            dirty: false,
            confirm: None,
            wire: false,
            symmetry: false,
        }
    }
}
fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 128 * 1024 * 1024 {
        return Err("File exceeds 128 MB limit".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn write_json<T: serde::Serialize>(path: &str, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let temp = format!("{path}.tmp");
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(temp, path).map_err(|e| e.to_string())
}
fn same_path(a: &str, b: &str) -> bool {
    fn resolved(path: &str) -> std::path::PathBuf {
        let p = std::path::Path::new(path);
        p.canonicalize().unwrap_or_else(|_| {
            let parent = p
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            parent
                .canonicalize()
                .unwrap_or_else(|_| parent.into())
                .join(p.file_name().unwrap_or_default())
        })
    }
    resolved(a) == resolved(b)
}
fn brush_key(
    b: &Brush,
) -> (
    String,
    Kind,
    bool,
    [f32; 6],
    [bool; 2],
    std::mem::Discriminant<Alpha>,
) {
    (
        b.name.clone(),
        b.kind,
        b.subtract,
        [
            b.radius,
            b.strength,
            b.spacing,
            b.falloff,
            b.rotation,
            b.pressure_curve,
        ],
        [b.pressure, b.pressure_radius],
        std::mem::discriminant(&b.alpha),
    )
}
impl App {
    fn snapshot(&mut self) {
        if self.undo.len() >= 24 {
            self.undo.remove(0);
        }
        self.undo.push(self.mesh.clone());
        self.redo.clear();
    }
    fn save(&mut self) -> bool {
        let path = self.path.clone();
        self.save_to(&path)
    }
    fn save_to(&mut self, path: &str) -> bool {
        let p = Project {
            version: 1,
            mesh: self.mesh.clone(),
            brushes: self.brushes.clone(),
        };
        match write_json(path, &p) {
            Ok(()) => {
                self.status = format!("Saved {path}");
                self.current_path = path.into();
                self.dirty = false;
                true
            }
            Err(e) => {
                self.status = e;
                false
            }
        }
    }
    fn request(&mut self, a: Action, ctx: &egui::Context) {
        if self.dirty {
            self.pending_save_path = self.current_path.clone();
            self.confirm = Some(a)
        } else {
            self.action(a, ctx)
        }
    }
    fn action(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::New => {
                self.mesh = Mesh::sphere();
                self.undo.clear();
                self.redo.clear();
                self.dirty = false;
                self.current_path = "untitled.clay".into();
                self.status = "New sphere".into();
            }
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::Open => {
                let result = (|| {
                    let p: Project = read_json(&self.path)?;
                    if p.version != 1 {
                        return Err("Unsupported project version".into());
                    }
                    p.mesh.validate()?;
                    if p.brushes.is_empty() || p.brushes.len() > 256 {
                        return Err("Invalid brush library".into());
                    }
                    for b in &p.brushes {
                        validate_brush(b)?;
                    }
                    Ok(p)
                })();
                match result {
                    Ok(p) => {
                        self.mesh = p.mesh;
                        self.brushes = p.brushes;
                        self.selected = 0;
                        self.undo.clear();
                        self.redo.clear();
                        self.dirty = false;
                        self.current_path = self.path.clone();
                        self.status = format!("Opened {}", self.path);
                    }
                    Err(e) => self.status = e,
                }
            }
        }
    }
    fn basis(&self) -> (V, V, V) {
        let (s, c) = self.yaw.sin_cos();
        let (p, q) = self.pitch.sin_cos();
        (V(c, 0., -s), V(s * p, q, c * p), V(s * q, -p, c * q))
    }
    fn export(&mut self) {
        let path = std::path::Path::new(&self.path).with_extension("obj");
        let mut text = String::from("# Clay Form mesh\n");
        for v in &self.mesh.vertices {
            text.push_str(&format!("v {} {} {}\n", v.0, v.1, v.2));
        }
        for &[a, b, c] in &self.mesh.triangles {
            text.push_str(&format!("f {} {} {}\n", a + 1, b + 1, c + 1));
        }
        self.status = match std::fs::write(&path, text) {
            Ok(()) => format!("Exported {}", path.display()),
            Err(e) => e.to_string(),
        };
    }
    fn brush_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Brush library");
        for (i, b) in self.brushes.iter().enumerate() {
            ui.selectable_value(&mut self.selected, i, &b.name);
        }
        if ui.button("Duplicate brush").clicked() {
            let mut b = self.brushes[self.selected].clone();
            b.name.push_str(" copy");
            self.brushes.push(b);
            self.selected = self.brushes.len() - 1;
            self.dirty = true;
        }
        ui.separator();
        let before = brush_key(&self.brushes[self.selected]);
        let b = &mut self.brushes[self.selected];
        ui.text_edit_singleline(&mut b.name);
        egui::ComboBox::from_label("Operation")
            .selected_text(format!("{:?}", b.kind))
            .show_ui(ui, |ui| {
                for k in Kind::ALL {
                    ui.selectable_value(&mut b.kind, k, format!("{k:?}"));
                }
            });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut b.subtract, false, "+ Add");
            ui.selectable_value(&mut b.subtract, true, "− Subtract");
        });
        ui.add(egui::Slider::new(&mut b.radius, 1. ..=500.).text("Radius (px)"));
        ui.add(egui::Slider::new(&mut b.strength, 0. ..=1.).text("Strength"));
        ui.add(egui::Slider::new(&mut b.spacing, 0.01..=1.).text("Spacing"));
        ui.add(egui::Slider::new(&mut b.falloff, 0.1..=8.).text("Falloff"));
        ui.add(egui::Slider::new(&mut b.rotation, 0. ..=360.).text("Alpha rotation"));
        ui.checkbox(&mut b.pressure, "Pressure → strength");
        ui.checkbox(&mut b.pressure_radius, "Pressure → radius");
        ui.add(egui::Slider::new(&mut b.pressure_curve, 0.1..=4.).text("Pressure curve"));
        ui.label("Alpha");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut b.alpha, Alpha::Soft, "Soft");
            ui.selectable_value(&mut b.alpha, Alpha::Hard, "Hard");
            ui.selectable_value(&mut b.alpha, Alpha::Square, "Square");
        });
        if matches!(b.alpha, Alpha::Image { .. }) {
            ui.label("Custom image alpha loaded");
        }
        if before != brush_key(b) {
            self.dirty = true;
        }
        let (r, p) = ui.allocate_painter(Vec2::splat(100.), Sense::hover());
        let b = &self.brushes[self.selected];
        for y in 0..32 {
            for x in 0..32 {
                let value = (255. * b.weight(x as f32 / 15.5 - 1., 1. - y as f32 / 15.5)) as u8;
                p.rect_filled(
                    egui::Rect::from_min_size(
                        r.rect.min + Vec2::new(x as f32, y as f32) * 3.125,
                        Vec2::splat(3.125),
                    ),
                    0.,
                    Color32::from_gray(value),
                );
            }
        }
        ui.label("Image path (PNG/JPEG)");
        ui.text_edit_singleline(&mut self.alpha_path);
        if ui.button("Load alpha").clicked() {
            let loaded = (|| {
                let reader =
                    image::ImageReader::open(&self.alpha_path).map_err(|e| e.to_string())?;
                let mut reader = reader.with_guessed_format().map_err(|e| e.to_string())?;
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(2048);
                limits.max_image_height = Some(2048);
                reader.limits(limits);
                let image = reader.decode().map_err(|e| e.to_string())?.to_rgba8();
                Ok::<_, String>(Alpha::Image {
                    width: image.width() as usize,
                    height: image.height() as usize,
                    pixels: image
                        .pixels()
                        .map(|p| {
                            ((p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722)
                                / 255.)
                                * (p[3] as f32 / 255.)
                        })
                        .collect(),
                })
            })();
            match loaded {
                Ok(a) => {
                    self.brushes[self.selected].alpha = a;
                    self.dirty = true;
                    self.status = "Alpha loaded (white applies; black protects)".into();
                }
                Err(e) => self.status = e,
            }
        }
        ui.separator();
        ui.label("Preset path");
        ui.text_edit_singleline(&mut self.preset_path);
        ui.horizontal(|ui| {
            if ui.button("Save preset").clicked() {
                self.status = match write_json(&self.preset_path, &self.brushes[self.selected]) {
                    Ok(()) => "Brush saved".into(),
                    Err(e) => e,
                };
            }
            if ui.button("Load preset").clicked() {
                match read_json::<Brush>(&self.preset_path).and_then(|b| {
                    validate_brush(&b)?;
                    Ok(b)
                }) {
                    Ok(b) => {
                        self.brushes.push(b);
                        self.selected = self.brushes.len() - 1;
                        self.dirty = true;
                        self.status = "Brush loaded".into();
                    }
                    Err(e) => self.status = e,
                }
            }
        });
        ui.separator();
        ui.label(self.tablet.status());
        ui.label(format!("Pressure: {:.0}%", self.pressure * 100.));
        ui.label("Left / pen: sculpt\nAlt: reverse brush\nShift: smooth\nRight drag: orbit\nMiddle drag: pan\nWheel: zoom");
    }
    fn viewport(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;
        let center = rect.center() + self.pan;
        let scale = rect.width().min(rect.height()) * 0.35 * self.zoom;
        let (right, up, view) = self.basis();
        let project = |v: V| {
            Pos2::new(
                center.x + v.dot(right) * scale,
                center.y - v.dot(up) * scale,
            )
        };
        let normals = self.mesh.normals();
        let positions: Vec<_> = self.mesh.vertices.iter().copied().map(project).collect();
        let mut faces: Vec<_> = self
            .mesh
            .triangles
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                let a = self.mesh.vertices[t[0]];
                let b = self.mesh.vertices[t[1]];
                let c = self.mesh.vertices[t[2]];
                (b - a).cross(c - a).dot(view) > 0.
            })
            .map(|(i, t)| {
                (
                    i,
                    (self.mesh.vertices[t[0]]
                        + self.mesh.vertices[t[1]]
                        + self.mesh.vertices[t[2]])
                        .dot(view),
                )
            })
            .collect();
        faces.sort_by(|a, b| a.1.total_cmp(&b.1));
        painter.rect_filled(rect, 0., Color32::from_rgb(23, 26, 31));
        let light = (view + up * 0.7 + right * (-0.5)).norm();
        let mut rendered = egui::Mesh::default();
        for (i, v) in self.mesh.vertices.iter().enumerate() {
            let illumination = 0.25 + 0.75 * normals[i].dot(light).max(0.);
            let mask = 1. - self.mesh.mask[i] * 0.75;
            rendered.colored_vertex(
                project(*v),
                Color32::from_rgb(
                    (190. * illumination * mask) as u8,
                    (155. * illumination * mask) as u8,
                    (125. * illumination * mask) as u8,
                ),
            );
        }
        for &(i, _) in &faces {
            for &v in &self.mesh.triangles[i] {
                rendered.indices.push(v as u32);
            }
        }
        painter.add(egui::Shape::mesh(rendered));
        if self.wire {
            for &(i, _) in &faces {
                let t = self.mesh.triangles[i];
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    painter.line_segment(
                        [positions[a], positions[b]],
                        egui::Stroke::new(0.3, Color32::from_black_alpha(55)),
                    );
                }
            }
        }
        let pointer = response
            .hover_pos()
            .or_else(|| response.interact_pointer_pos());
        let modifiers = ui.input(|i| i.modifiers);
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.2, 8.);
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            let d = ui.input(|i| i.pointer.delta());
            self.yaw += d.x * 0.008;
            self.pitch = (self.pitch + d.y * 0.008).clamp(-1.5, 1.5);
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        let drawing = response.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.primary_down())
            && !modifiers.ctrl
            && !modifiers.command
            && self.confirm.is_none();
        if drawing && !self.stroke {
            self.snapshot();
            self.stroke = true;
            self.last = None;
        }
        if !drawing {
            self.stroke = false;
            self.last = None;
        }
        if let Some(pos) = pointer {
            let b = self.brushes[self.selected].clone();
            let radius = b.radius
                * if b.pressure_radius {
                    self.pressure.powf(b.pressure_curve).max(0.05)
                } else {
                    1.
                };
            painter.circle_stroke(
                pos,
                radius,
                egui::Stroke::new(
                    1.5,
                    if b.subtract ^ modifiers.alt {
                        Color32::from_rgb(255, 140, 100)
                    } else {
                        Color32::from_rgb(125, 210, 245)
                    },
                ),
            );
            if drawing {
                let previous = self.last.unwrap_or(pos);
                let distance = pos.distance(previous);
                let spacing = (radius * b.spacing).max(1.);
                if self.last.is_none() || distance >= spacing || b.kind == Kind::Grab {
                    let steps = if b.kind == Kind::Grab {
                        1
                    } else {
                        (distance / spacing).ceil().clamp(1., 128.) as usize
                    };
                    for step in 1..=steps {
                        let point = previous + (pos - previous) * (step as f32 / steps as f32);
                        let mut hit = None;
                        let mut depth = f32::NEG_INFINITY;
                        for &(fi, _) in &faces {
                            let t = self.mesh.triangles[fi];
                            let a = positions[t[0]];
                            let v0 = positions[t[1]] - a;
                            let v1 = positions[t[2]] - a;
                            let v2 = point - a;
                            let det = v0.x * v1.y - v0.y * v1.x;
                            if det.abs() < 1e-7 {
                                continue;
                            }
                            let u = (v2.x * v1.y - v2.y * v1.x) / det;
                            let v = (v0.x * v2.y - v0.y * v2.x) / det;
                            if u >= 0. && v >= 0. && u + v <= 1. {
                                let p = self.mesh.vertices[t[0]] * (1. - u - v)
                                    + self.mesh.vertices[t[1]] * u
                                    + self.mesh.vertices[t[2]] * v;
                                let z = p.dot(view);
                                if z > depth {
                                    depth = z;
                                    hit = Some((
                                        p,
                                        (normals[t[0]] * (1. - u - v)
                                            + normals[t[1]] * u
                                            + normals[t[2]] * v)
                                            .norm(),
                                    ));
                                }
                            }
                        }
                        if let Some((p, n)) = hit {
                            let mut brush = b.clone();
                            if modifiers.shift {
                                brush.kind = Kind::Smooth;
                            }
                            let d = (right * ((pos.x - previous.x) / scale)
                                + up * ((previous.y - pos.y) / scale))
                                * (1. / steps as f32);
                            self.mesh.stamp(
                                &brush,
                                Dab {
                                    center: p,
                                    normal: n,
                                    right,
                                    up,
                                    radius: radius / scale,
                                    pressure: self.pressure,
                                    delta: d,
                                    invert: modifiers.alt,
                                },
                            );
                            if self.symmetry && p.0.abs() > radius / scale {
                                let mirror = |v: V| V(-v.0, v.1, v.2);
                                self.mesh.stamp(
                                    &brush,
                                    Dab {
                                        center: mirror(p),
                                        normal: mirror(n),
                                        right: mirror(right),
                                        up: mirror(up),
                                        radius: radius / scale,
                                        pressure: self.pressure,
                                        delta: mirror(d),
                                        invert: modifiers.alt,
                                    },
                                );
                            }
                            self.dirty = true;
                        }
                    }
                    self.last = Some(pos);
                }
            }
        }
        painter.text(
            rect.left_top() + Vec2::splat(16.),
            egui::Align2::LEFT_TOP,
            format!(
                "{} vertices • {} faces",
                self.mesh.vertices.len(),
                self.mesh.triangles.len()
            ),
            egui::FontId::monospace(12.),
            Color32::GRAY,
        );
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let mut native_pressure = None;
        ctx.input(|i| {
            for e in &i.events {
                if let egui::Event::Touch { force: Some(p), .. } = e {
                    native_pressure = Some(*p);
                }
            }
        });
        self.pressure = self.tablet.pressure().or(native_pressure).unwrap_or(1.);
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Action::Quit, ctx);
        }
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("CLAY FORM");
                if ui.button("New sphere").clicked() {
                    self.request(Action::New, ctx);
                }
                ui.label("Project path");
                ui.text_edit_singleline(&mut self.path);
                if ui.button("Open").clicked() {
                    self.request(Action::Open, ctx);
                }
                if ui.button("Save").clicked() {
                    self.save();
                }
                if ui.button("Export OBJ").clicked() {
                    self.export();
                }
                if self.dirty {
                    ui.label("• Modified");
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.undo.is_empty(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.redo.push(self.mesh.clone());
                    self.mesh = self.undo.pop().unwrap();
                    self.dirty = true;
                }
                if ui
                    .add_enabled(!self.redo.is_empty(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.undo.push(self.mesh.clone());
                    self.mesh = self.redo.pop().unwrap();
                    self.dirty = true;
                }
                ui.checkbox(&mut self.wire, "Wireframe");
                ui.checkbox(&mut self.symmetry, "Mirror X");
                if ui.button("Clear mask").clicked() {
                    self.snapshot();
                    self.mesh.mask.fill(0.);
                    self.dirty = true;
                }
                if ui.button("Reset view").clicked() {
                    self.yaw = 0.;
                    self.pitch = 0.;
                    self.zoom = 1.;
                    self.pan = Vec2::ZERO;
                }
            });
        });
        egui::SidePanel::left("brushes")
            .default_width(260.)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.brush_controls(ui));
            });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });
        egui::CentralPanel::default().show(ctx, |ui| self.viewport(ui));
        if let Some(a) = self.confirm {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Save your current project before continuing?");
                    ui.label("Save current project to:");
                    ui.text_edit_singleline(&mut self.pending_save_path);
                    ui.horizontal(|ui| {
                        if ui.button("Save and continue").clicked() {
                            let path = self.pending_save_path.clone();
                            if matches!(a, Action::Open) && same_path(&path, &self.path) {
                                self.status = "Choose a different save path to avoid overwriting the project you are opening".into();
                            } else if self.save_to(&path) {
                                self.confirm = None;
                                self.action(a, ctx);
                            }
                        }
                        if ui.button("Discard changes").clicked() {
                            self.confirm = None;
                            if matches!(a, Action::Quit) {
                                self.dirty = false;
                            }
                            self.action(a, ctx);
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm = None;
                        }
                    });
                });
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}
fn main() -> eframe::Result {
    eframe::run_native(
        "Clay Form",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1280., 850.]),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::<App>::default())),
    )
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn viewport_pen_drag_sculpts_and_records_undo() {
        let mut app = App::default();
        let original = app.mesh.vertices.clone();
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000., 800.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.viewport(ui));
            },
        );
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            events: vec![
                egui::Event::PointerMoved(Pos2::new(500., 400.)),
                egui::Event::PointerButton {
                    pos: Pos2::new(500., 400.),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.viewport(ui));
        });
        assert!(app.dirty);
        assert_eq!(app.undo.len(), 1);
        assert!(
            app.mesh
                .vertices
                .iter()
                .zip(&original)
                .any(|(a, b)| (a.0, a.1, a.2) != (b.0, b.1, b.2))
        );
        app.mesh = app.undo.pop().unwrap();
        assert!(
            app.mesh
                .vertices
                .iter()
                .zip(original)
                .all(|(a, b)| (a.0, a.1, a.2) == (b.0, b.1, b.2))
        );
    }

    #[test]
    fn project_and_custom_preset_survive_disk_save() {
        let dir = std::env::temp_dir().join(format!(
            "clay-form-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let mut brush = Brush::new(Kind::Clay);
        brush.name = "My textured clay".into();
        brush.subtract = true;
        brush.pressure_radius = true;
        brush.rotation = 37.;
        brush.alpha = Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![0., 0.25, 0.75, 1.],
        };
        let mut mesh = Mesh::sphere();
        mesh.mask[7] = 0.75;
        mesh.vertices[7].0 += 0.05;
        let project = Project {
            version: 1,
            mesh,
            brushes: vec![brush.clone()],
        };
        let path = dir.join("project.clay");
        write_json(path.to_str().unwrap(), &project).unwrap();
        let loaded: Project = read_json(path.to_str().unwrap()).unwrap();
        loaded.mesh.validate().unwrap();
        assert_eq!(loaded.mesh.vertices[7].0, project.mesh.vertices[7].0);
        assert_eq!(loaded.mesh.mask[7], 0.75);
        assert_eq!(brush_key(&loaded.brushes[0]), brush_key(&brush));
        assert_eq!(loaded.brushes[0].alpha, brush.alpha);
        let preset = dir.join("preset.brush.json");
        write_json(preset.to_str().unwrap(), &brush).unwrap();
        let restored: Brush = read_json(preset.to_str().unwrap()).unwrap();
        validate_brush(&restored).unwrap();
        assert_eq!(restored.alpha, brush.alpha);
        assert!(!dir.join("project.clay.tmp").exists());
        assert!(same_path(
            path.to_str().unwrap(),
            dir.join("./project.clay").to_str().unwrap()
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

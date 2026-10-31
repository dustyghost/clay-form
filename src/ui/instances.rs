use super::*;
use crate::{
    command::InstanceCommand as C,
    sculpt::{Vec3, instances::*},
};

pub(crate) struct Controls {
    pub editor: Option<Set>,
    pub invalid: bool,
    pub asset: Option<u64>,
    pub name: String,
    pub pivot: Vec3,
    pub forward: usize,
    pub up: usize,
    pub asset_path: String,
    pub brush: crate::viewport::objects::BrushOptions,
    pub stop_placing: bool,
    thumbnails: std::collections::HashMap<u64, Thumbnail>,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            editor: None,
            invalid: false,
            asset: None,
            name: "Object brush".into(),
            pivot: Vec3::default(),
            forward: 0,
            up: 2,
            asset_path: "object.object.json".into(),
            brush: Default::default(),
            stop_placing: false,
            thumbnails: Default::default(),
        }
    }
}
fn vector(ui: &mut egui::Ui, label: &str, v: &mut Vec3) -> bool {
    let mut changed = false;
    ui.label(label);
    ui.horizontal_wrapped(|ui| {
        for (name, value) in [("X", &mut v.0), ("Y", &mut v.1), ("Z", &mut v.2)] {
            changed |= ui
                .add(
                    egui::DragValue::new(value)
                        .speed(0.05)
                        .prefix(format!("{name} ")),
                )
                .changed();
        }
    });
    changed
}
fn axes(ui: &mut egui::Ui, forward: &mut usize, up: &mut usize) -> bool {
    let before = (*forward, *up);
    ui.horizontal_wrapped(|ui| {
        ui.label("Forward");
        for (i, name) in ["X", "Y", "Z"].iter().enumerate() {
            ui.selectable_value(forward, i, *name);
        }
        ui.label("Up");
        for (i, name) in ["X", "Y", "Z"].iter().enumerate() {
            ui.selectable_value(up, i, *name);
        }
    });
    before != (*forward, *up)
}
fn choose_piece(ui: &mut egui::Ui, label: &str, piece: &mut Piece, document: &Document) -> bool {
    let before = *piece;
    let selected = match piece.source {
        Source::Model(id) => (0..document.model_count())
            .find(|&i| document.instance_model_id(i) == id)
            .map(|i| document.model(i).unwrap().name.to_owned()),
        Source::Asset(id) => document
            .instances()
            .assets
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone()),
    }
    .unwrap_or_else(|| "Missing source".into());
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!("{label}: {selected}"))
        .show_ui(ui, |ui| {
            for i in 0..document.model_count() {
                let m = document.model(i).unwrap();
                if ui
                    .selectable_label(
                        piece.source == Source::Model(document.instance_model_id(i)),
                        m.name,
                    )
                    .clicked()
                {
                    let pivot = m
                        .mesh
                        .vertices
                        .iter()
                        .copied()
                        .fold(Vec3::default(), |a, b| a + b)
                        * (1. / m.mesh.vertices.len().max(1) as f32);
                    *piece = Piece {
                        source: Source::Model(document.instance_model_id(i)),
                        pivot,
                        forward: 0,
                        up: 2,
                    };
                }
            }
            for a in &document.instances().assets {
                if ui
                    .selectable_label(
                        piece.source == Source::Asset(a.id),
                        format!("Brush: {}", a.name),
                    )
                    .clicked()
                {
                    *piece = Piece {
                        source: Source::Asset(a.id),
                        pivot: a.pivot,
                        forward: a.forward,
                        up: a.up,
                    };
                }
            }
        });
    ui.push_id(label, |ui| {
        egui::CollapsingHeader::new("Contact point and axes").show(ui, |ui| {
            vector(ui, "Contact point", &mut piece.pivot);
            axes(ui, &mut piece.forward, &mut piece.up);
        });
    });
    before != *piece
}
impl UiState {
    pub(super) fn instance_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        self.instances
            .thumbnails
            .retain(|id, _| document.instances().assets.iter().any(|a| a.id == *id));

        egui::CollapsingHeader::new("Instances and object brushes")
            .default_open(true)
            .show(ui, |ui| {
                ui.add_enabled_ui(!self.mesh_busy, |ui| {
                    self.instance_contents(ui, document, commands);
                });
            });
    }
    fn instance_contents(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if let Err(error) = document.instance_placements() {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        if self.instances.editor.is_some() && !document.instance_preview_active() {
            self.instances.editor = None;
        }
        if let Some(set) = &mut self.instances.editor {
            let before = set.clone();
            ui.label("Preview · Apply saves one undo step");
            let (apply, cancel) = ui
                .horizontal(|ui| {
                    (
                        ui.add_enabled(
                            !self.instances.invalid,
                            egui::Button::new("Apply instances"),
                        )
                        .clicked(),
                        ui.button("Cancel preview").clicked(),
                    )
                })
                .inner;
            ui.text_edit_singleline(&mut set.name);
            ui.checkbox(&mut set.visible, "Show instances");
            match &mut set.layout {
                Layout::Array(a) => {
                    choose_piece(ui, "Source", &mut a.piece, document);
                    ui.horizontal_wrapped(|ui| {
                        for (kind, name) in [
                            (ArrayKind::Linear, "Linear"),
                            (ArrayKind::Radial, "Radial"),
                            (ArrayKind::Grid, "Grid"),
                        ] {
                            ui.selectable_value(&mut a.kind, kind, name);
                        }
                    });
                    ui.add(egui::Slider::new(&mut a.count, 1..=256).text("Copies"));
                    vector(ui, "Origin / radial centre", &mut a.center);
                    match a.kind {
                        ArrayKind::Linear => {
                            vector(ui, "Step between copies", &mut a.spacing);
                        }
                        ArrayKind::Grid => {
                            ui.add(egui::Slider::new(&mut a.columns, 1..=256).text("Columns"));
                            vector(ui, "Grid spacing", &mut a.spacing);
                        }
                        ArrayKind::Radial => {
                            ui.add(
                                egui::DragValue::new(&mut a.radius)
                                    .range(0.001..=1e5)
                                    .speed(0.05)
                                    .prefix("Radius "),
                            );
                            ui.add(egui::Slider::new(&mut a.angle, 0.01..=360.).text("Arc °"));
                            ui.horizontal(|ui| {
                                ui.label("Axis");
                                for (i, n) in ["X", "Y", "Z"].iter().enumerate() {
                                    ui.selectable_value(&mut a.radial_axis, i, *n);
                                }
                            });
                            ui.checkbox(&mut a.rotate_copies, "Rotate copies around centre");
                        }
                    }
                }
                Layout::Path(a) => {
                    egui::ComboBox::from_id_salt("assembly_path")
                        .selected_text(
                            document
                                .path(a.path)
                                .map_or("Missing path", |p| p.name.as_str()),
                        )
                        .show_ui(ui, |ui| {
                            for p in document.paths() {
                                ui.selectable_value(&mut a.path, p.id, &p.name);
                            }
                        });
                    choose_piece(ui, "Middle", &mut a.middle, document);
                    let closed = document.path(a.path).is_some_and(|p| p.closed);
                    if closed {
                        ui.weak("Closed loop: start/end caps are omitted.");
                    }
                    ui.add_enabled_ui(!closed, |ui| {
                        for (label, piece) in [("Start", &mut a.start), ("End", &mut a.end)] {
                            let mut enabled = piece.is_some();
                            if ui
                                .checkbox(&mut enabled, format!("Use {label} piece"))
                                .changed()
                            {
                                *piece = enabled.then_some(a.middle);
                            }
                            if let Some(piece) = piece {
                                choose_piece(ui, label, piece, document);
                            }
                        }
                    });
                    ui.checkbox(&mut a.fit_count, "Fit count to path length");
                    if a.fit_count {
                        ui.add(egui::Slider::new(&mut a.count, 1..=254).text("Middle pieces"));
                    } else {
                        ui.add(
                            egui::DragValue::new(&mut a.spacing)
                                .range(0.001..=1e5)
                                .speed(0.05)
                                .prefix("Spacing "),
                        );
                        ui.weak("Fixed spacing leaves any unused length at the end.");
                    }
                    ui.add(
                        egui::DragValue::new(&mut a.scale)
                            .range(0.001..=1000.)
                            .speed(0.01)
                            .prefix("Piece scale "),
                    );
                    ui.add(egui::Slider::new(&mut a.twist, -180.0..=180.).text("Twist °"));
                    ui.weak("Rigid pieces follow the path. Bake + fuse joins overlapping closed pieces using voxel remeshing.");
                }
                Layout::Manual(p) => {
                    ui.label(format!("{} placed objects", p.len()));
                }
            }
            if let Some((id, slot)) = document.selected_instance.filter(|(id, _)| *id == set.id) {
                ui.separator();
                ui.label(format!("Selected copy {}", slot + 1));
                let mut adjustment = set
                    .adjustments
                    .iter()
                    .find(|a| a.slot == slot)
                    .cloned()
                    .unwrap_or_else(|| Adjustment::new(slot));
                let old = adjustment.clone();
                let a = &mut adjustment;
                ui.checkbox(&mut a.visible, "Show this copy");
                vector(ui, "Move", &mut a.offset);
                vector(ui, "Rotate °", &mut a.rotation);
                ui.add(
                    egui::DragValue::new(&mut a.scale)
                        .range(0.001..=1000.)
                        .speed(0.01)
                        .prefix("Scale "),
                );
                ui.weak(format!("Set {id}: changes affect this copy only."));
                if old != adjustment {
                    if let Some(a) = set.adjustments.iter_mut().find(|a| a.slot == slot) {
                        *a = adjustment;
                    } else {
                        set.adjustments.push(adjustment);
                    }
                }
            }
            if before != *set {
                commands.push(Command::Instances(C::Preview(Box::new(set.clone()))));
            }
            if apply {
                commands.push(Command::Instances(C::Apply));
            }
            if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                commands.push(Command::Instances(C::Cancel));
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("New array…")
                    .customize(Action::CreateInstanceArray)
                    .clicked()
                {
                    commands.push(Command::Instances(C::NewArray));
                }
                if ui
                    .add_enabled(
                        !document.paths().is_empty(),
                        egui::Button::new("Along path…"),
                    )
                    .customize(Action::CreatePathAssembly)
                    .clicked()
                {
                    commands.push(Command::Instances(C::NewPath));
                }
            });
            for set in &document.instances().sets {
                ui.push_id(set.id, |ui| instance_set(ui, document, set, commands));
            }
            if let Some((id, slot)) = document.selected_instance {
                ui.label(format!(
                    "Selected copy {} · Edit its set to move, rotate or scale",
                    slot + 1
                ));
                if ui.button("Edit selected copy…").clicked() {
                    commands.push(Command::Instances(C::Edit(id)));
                }
            }
            ui.separator();
            egui::CollapsingHeader::new("Capture object brush").show(ui, |ui| {
                ui.text_edit_singleline(&mut self.instances.name);
                vector(
                    ui,
                    "Contact point (source coordinates)",
                    &mut self.instances.pivot,
                );
                axes(ui, &mut self.instances.forward, &mut self.instances.up);
                if ui
                    .button("Capture active model")
                    .customize(Action::CaptureObjectBrush)
                    .clicked()
                {
                    commands.push(Command::Instances(C::Capture(
                        self.instances.name.clone(),
                        self.instances.pivot,
                        self.instances.forward,
                        self.instances.up,
                    )));
                }
            });
            for asset in &document.instances().assets {
                ui.horizontal(|ui| {
                    thumbnail(ui, asset, &mut self.instances.thumbnails);
                    if ui
                        .selectable_value(&mut self.instances.asset, Some(asset.id), &asset.name)
                        .changed()
                    {
                        self.instances.brush = asset.options;
                    }
                    if ui
                        .small_button("×")
                        .on_hover_text("Remove unused object brush")
                        .clicked()
                    {
                        commands.push(Command::Instances(C::RemoveAsset(asset.id)));
                    }
                });
            }
            if let Some(id) = self.instances.asset {
                ui.add(
                    egui::DragValue::new(&mut self.instances.brush.scale)
                        .range(0.001..=1000.)
                        .speed(0.01)
                        .prefix("Size "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.instances.brush.spacing)
                        .range(0.001..=1000.)
                        .speed(0.01)
                        .prefix("Repeat spacing "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.instances.brush.offset)
                        .speed(0.01)
                        .prefix("Surface offset "),
                );
                ui.add(
                    egui::Slider::new(&mut self.instances.brush.rotation, -180.0..=180.)
                        .text("Rotation °"),
                );
                ui.checkbox(
                    &mut self.instances.brush.independent,
                    "Create independent geometry",
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Place / brush objects").clicked() {
                        commands.push(Command::Instances(C::StartBrush(id)));
                    }
                    if ui.button("Stop placing").clicked() {
                        commands.push(Command::Instances(C::StopBrush));
                    }
                });
            }
            egui::CollapsingHeader::new("Object brush files").show(ui, |ui| {
                ui.text_edit_singleline(&mut self.instances.asset_path);
                ui.horizontal(|ui| {
                    if ui.button("Import object").clicked() {
                        commands.push(Command::Instances(C::ImportAsset(
                            self.instances.asset_path.clone(),
                        )));
                    }
                    if let Some(id) = self.instances.asset
                        && ui.button("Export object").clicked()
                    {
                        commands.push(Command::Instances(C::ExportAsset(
                            id,
                            self.instances.asset_path.clone(),
                        )));
                    }
                });
            });
        }
    }
}

fn instance_set(ui: &mut egui::Ui, document: &Document, set: &Set, commands: &mut Vec<Command>) {
    ui.label(&set.name);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Edit").clicked() {
            commands.push(Command::Instances(C::Edit(set.id)));
        }
        if ui.button("Bake")
            .on_hover_text("Make independent submodels; retain source UVs, materials and masks")
            .clicked()
        {
            commands.push(Command::Instances(C::Bake(set.id, false)));
        }
        if ui.button("Bake + fuse")
            .on_hover_text("Union overlapping closed pieces with voxel remeshing. Rebuilds topology and removes UVs; keeps the reusable source.")
            .clicked()
        {
            commands.push(Command::Instances(C::Bake(set.id, true)));
        }
        if ui.small_button("Remove set").clicked() {
            commands.push(Command::Instances(C::Remove(set.id)));
        }
    });
    egui::CollapsingHeader::new("Copies").show(ui, |ui| {
        // Hidden copies must remain selectable so their visibility can be restored.
        let mut listed = set.clone();
        for adjustment in &mut listed.adjustments {
            adjustment.visible = true;
        }
        if let Ok(placements) = listed.placements(document.paths()) {
            egui::ScrollArea::vertical()
                .max_height(160.)
                .show(ui, |ui| {
                    for (slot, _) in placements {
                        let visible = set
                            .adjustments
                            .iter()
                            .find(|a| a.slot == slot)
                            .is_none_or(|a| a.visible);
                        ui.horizontal(|ui| {
                            let label = format!(
                                "Copy {}{}",
                                slot + 1,
                                if visible { "" } else { " (hidden)" }
                            );
                            if ui
                                .selectable_label(
                                    document.selected_instance == Some((set.id, slot)),
                                    label,
                                )
                                .clicked()
                            {
                                commands.push(Command::Instances(C::Select(set.id, slot)));
                            }
                            if ui
                                .add_enabled(visible, egui::Button::new("Independent").small())
                                .clicked()
                            {
                                commands.push(Command::Instances(C::Detach(set.id, slot)));
                            }
                        });
                    }
                });
        }
    });
}

struct Thumbnail {
    source: std::sync::Arc<crate::sculpt::Mesh>,
    lines: Vec<[egui::Vec2; 2]>,
}
fn thumbnail_lines(mesh: &crate::sculpt::Mesh) -> Vec<[egui::Vec2; 2]> {
    if mesh.vertices.is_empty() {
        return Vec::new();
    }
    let mut lo = egui::pos2(f32::INFINITY, f32::INFINITY);
    let mut hi = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in &mesh.vertices {
        let q = egui::pos2(p.0 + p.2 * 0.4, -p.1 - p.2 * 0.25);
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let scale = 34. / (hi - lo).x.max((hi - lo).y).max(1e-5);
    let center = lo + (hi - lo) * 0.5;
    let project = |p: Vec3| (egui::pos2(p.0 + p.2 * 0.4, -p.1 - p.2 * 0.25) - center) * scale;
    let mut lines = Vec::new();
    for tri in mesh
        .triangles
        .iter()
        .step_by((mesh.triangles.len() / 100).max(1))
    {
        let p = tri.map(|i| project(mesh.vertices[i]));
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            lines.push([p[a], p[b]]);
        }
    }
    lines
}
fn thumbnail(
    ui: &mut egui::Ui,
    asset: &Asset,
    cache: &mut std::collections::HashMap<u64, Thumbnail>,
) {
    let cached = cache.entry(asset.id).or_insert_with(|| Thumbnail {
        source: asset.mesh.clone(),
        lines: thumbnail_lines(&asset.mesh),
    });
    if !std::sync::Arc::ptr_eq(&cached.source, &asset.mesh) {
        cached.source = asset.mesh.clone();
        cached.lines = thumbnail_lines(&asset.mesh);
    }
    let (r, _) = ui.allocate_exact_size(egui::vec2(42., 42.), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(r, 3., ui.visuals().extreme_bg_color);
    for line in &cached.lines {
        painter.line_segment(
            line.map(|p| r.center() + p),
            egui::Stroke::new(0.6_f32, egui::Color32::from_rgb(100, 195, 185)),
        );
    }
}

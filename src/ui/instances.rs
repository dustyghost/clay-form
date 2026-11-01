use super::*;
use crate::{
    command::InstanceCommand as C,
    sculpt::{Vec3, instances::*},
};
mod arrays;
mod brushes;
mod thickness;

pub(crate) struct Controls {
    pub editor: Option<Set>,
    pub array_layer: usize,
    pub invalid: bool,
    pub asset: Option<u64>,
    pub group: Option<u64>,
    pub group_name: String,
    pub library_open: bool,
    pub capture_frame: Option<u64>,
    pub personal_brushes: Vec<String>,
    pub remove_saved: Option<String>,
    pub bundle_path: String,
    pub name: String,
    pub pivot: Vec3,
    pub forward: usize,
    pub up: usize,
    pub asset_path: String,
    pub brush: crate::viewport::objects::BrushOptions,
    pub stop_placing: bool,
    pub placing: bool,
    pub inline_thickness: bool,
    pub thickness_path_points: bool,
    pub thickness_sections: bool,
    thumbnails: std::collections::HashMap<u64, Thumbnail>,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            editor: None,
            array_layer: 0,
            invalid: false,
            asset: None,
            group: None,
            group_name: String::new(),
            library_open: false,
            capture_frame: None,
            personal_brushes: Vec::new(),
            remove_saved: None,
            bundle_path: "kit.objects.json".into(),
            name: String::new(),
            pivot: Vec3::default(),
            forward: 0,
            up: 2,
            asset_path: "object.object.json".into(),
            brush: Default::default(),
            stop_placing: false,
            placing: false,
            inline_thickness: true,
            thickness_path_points: true,
            thickness_sections: true,
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
        egui::CollapsingHeader::new("Instances")
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
                    arrays::editor(ui, a, &mut self.instances.array_layer, document)
                }
                Layout::Path(a) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Sections");
                        ui.selectable_value(&mut a.bend, false, "Rigid");
                        ui.selectable_value(&mut a.bend, true, "Bend along path");
                    });
                    if a.bend {
                        ui.checkbox(&mut a.fit_sections, "Fit section lengths to spacing");
                        ui.checkbox(
                            &mut self.instances.inline_thickness,
                            "Inline thickness handles",
                        );
                        let closed = document.path(a.path).is_some_and(|p| p.closed);
                        egui::CollapsingHeader::new("Thickness along path").show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.checkbox(
                                    &mut self.instances.thickness_path_points,
                                    "Path points",
                                );
                                ui.checkbox(
                                    &mut self.instances.thickness_sections,
                                    "Section centres",
                                );
                            });
                            ui.weak("Gold: ends · Teal: path points · Purple: sections");
                            thickness::controls(ui, &mut a.thickness, closed);
                        });
                        ui.weak("Bending uses source vertices; subdivide along its Forward axis for smooth curves.");
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Connections");
                        ui.selectable_value(&mut a.join, JoinMode::Separate, "Separate");
                        ui.selectable_value(&mut a.join, JoinMode::Weld, "Join neighbours");
                    });
                    if a.join == JoinMode::Weld {
                        ui.add(
                            egui::DragValue::new(&mut a.join_distance)
                                .range(0.00001..=1000.)
                                .speed(0.001)
                                .prefix("Join distance "),
                        );
                        ui.weak("Welds matching planar end loops and removes internal caps. UVs are removed; source models are preserved.");
                    }
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
                    ui.weak("Bake makes the preview editable. Bake + fuse remeshes overlapping closed sections.");
                }
                Layout::Manual(p) => {
                    ui.label(format!("{} placed objects", p.len()));
                }
            }
            if let Some((id, slot)) = document
                .selected_instance
                .filter(|(id, _)| *id == set.id && !set.joined())
            {
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
                if matches!(set.layout, Layout::Array(_)) {
                    vector(ui, "Scale on X / Y / Z", &mut a.axis_scale);
                }
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
                    .button("Linked copy…")
                    .on_hover_text("Place one copy that follows edits to the source model.")
                    .clicked()
                {
                    commands.push(Command::Instances(C::NewLinkedCopy));
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
                let joined = document
                    .instances()
                    .sets
                    .iter()
                    .any(|s| s.id == id && s.joined());
                if joined {
                    ui.label("Joined assembly selected · edit its path and connection settings");
                } else {
                    ui.label(format!(
                        "Selected copy {} · Edit its set to move, rotate or scale",
                        slot + 1
                    ));
                }
                if ui
                    .button(if joined {
                        "Edit selected assembly…"
                    } else {
                        "Edit selected copy…"
                    })
                    .clicked()
                {
                    commands.push(Command::Instances(C::Edit(id)));
                }
            }
        }
    }
    pub(super) fn object_settings(&mut self, ui: &mut egui::Ui) {
        ui.weak("Click the surface to place a part, drag to repeat, or drag empty space to orbit.");
        self.instances.size_control(ui);
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
            egui::Slider::new(&mut self.instances.brush.rotation, -180.0..=180.).text("Rotation °"),
        );
        ui.checkbox(
            &mut self.instances.brush.independent,
            "Create independent geometry",
        );
    }
}

fn instance_set(ui: &mut egui::Ui, document: &Document, set: &Set, commands: &mut Vec<Command>) {
    ui.label(&set.name);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Edit").clicked() {
            commands.push(Command::Instances(C::Edit(set.id)));
        }
        if ui.button(if set.joined() {"Bake joined model"} else {"Bake"})
            .on_hover_text("Make the current preview editable. Separate sections retain UVs; joined output has no UVs.")
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
    if let Layout::Array(array) = &set.layout {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Repeat this array…")
                .on_hover_text(
                    "Use the whole array as a live source for a separate instance or array.",
                )
                .clicked()
            {
                commands.push(Command::Instances(C::RepeatArray(set.id)));
            }
            if let Some(id) = array.source_array {
                if ui.small_button("Edit source array…").clicked() {
                    commands.push(Command::Instances(C::Edit(id)));
                }
            } else if let Source::Model(id) = array.piece.source
                && let Some(index) =
                    (0..document.model_count()).find(|&i| document.instance_model_id(i) == id)
                && index != document.active_model_index()
                && ui.small_button("Select source model").clicked()
            {
                commands.push(Command::SelectSubmodel(index));
            }
        });
    }
    if set.joined() {
        ui.weak("Joined assembly · Bake creates one editable model. Choose Separate to adjust individual copies.");
        return;
    }
    egui::CollapsingHeader::new("Copies").show(ui, |ui| {
        // Hidden copies must remain selectable so their visibility can be restored.
        let mut listed = set.clone();
        for adjustment in &mut listed.adjustments {
            adjustment.visible = true;
        }
        if let Ok(placements) =
            ArrayResolver::new(document.instances()).placements(&listed, document.paths())
        {
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
    texture: egui::TextureHandle,
}

impl Controls {
    pub(super) fn selected_cover<'a>(&self, library: &'a Library) -> Option<(&'a str, &'a Asset)> {
        if !self.placing {
            return None;
        }
        let group = library.brushes.iter().find(|b| Some(b.id) == self.group);
        let id = group
            .and_then(|b| b.objects.first().copied())
            .or(self.asset)?;
        let asset = library.assets.iter().find(|a| a.id == id)?;
        Some((
            group.map_or(asset.name.as_str(), |b| b.name.as_str()),
            asset,
        ))
    }

    pub(super) fn size_control(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(
            egui::DragValue::new(&mut self.brush.scale)
                .range(0.001..=1000.)
                .speed(0.01)
                .prefix("Size "),
        )
    }

    pub(super) fn object_icon(
        &mut self,
        ctx: &egui::Context,
        asset: &Asset,
    ) -> Option<egui::TextureId> {
        if let Some(cached) = self.thumbnails.get(&asset.id)
            && std::sync::Arc::ptr_eq(&cached.source, &asset.mesh)
        {
            return Some(cached.texture.id());
        }
        let [yaw, pitch, roll] = asset.capture_angles;
        let camera = crate::camera::Camera {
            yaw,
            pitch,
            roll,
            ..Default::default()
        };
        let icon = crate::brush_icon::capture_part(&asset.mesh, &camera).ok()?;
        let texture = super::icons::texture(ctx, format!("object-icon-{}", asset.id), &icon);
        let texture_id = texture.id();
        self.thumbnails.insert(
            asset.id,
            Thumbnail {
                source: asset.mesh.clone(),
                texture,
            },
        );
        Some(texture_id)
    }
}
pub(super) use brushes::{library_tiles, manager, part_picker};

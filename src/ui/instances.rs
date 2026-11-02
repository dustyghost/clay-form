use super::*;
use crate::viewport::instances::{COPY_COLOR, SET_COLOR};
use crate::{
    command::InstanceCommand as C,
    sculpt::{Vec3, instances::*},
};
mod arrays;
mod attached;
mod brushes;
mod path_editor;
mod thickness;

pub(crate) struct Controls {
    pub editor: Option<Set>,
    pub bake: Option<BakeRequest>,
    pub array_layer: usize,
    pub array_edit: bool,
    pub followed_array: Option<u64>,
    pub focus_array: bool,
    pub invalid: bool,
    pub asset: Option<u64>,
    pub group: Option<u64>,
    pub group_name: String,
    pub capture_frame: Option<u64>,
    pub personal_brushes: Vec<String>,
    pub saved_brushes_listed: bool,
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
    ship_greebles_thumbnail: Option<Thumbnail>,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            editor: None,
            bake: None,
            array_layer: 0,
            array_edit: false,
            followed_array: None,
            focus_array: false,
            invalid: false,
            asset: None,
            group: None,
            group_name: String::new(),
            capture_frame: None,
            personal_brushes: Vec::new(),
            saved_brushes_listed: false,
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
            ship_greebles_thumbnail: None,
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
fn source_name(document: &Document, source: Source) -> Option<&str> {
    match source {
        Source::Model(id) => (0..document.model_count())
            .find(|&i| document.instance_model_id(i) == id)
            .and_then(|i| document.model(i).map(|m| m.name)),
        Source::Asset(id) => document
            .instances()
            .assets
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.as_str()),
    }
}
fn choose_piece(ui: &mut egui::Ui, label: &str, piece: &mut Piece, document: &Document) -> bool {
    let before = *piece;
    let selected = source_name(document, piece.source).unwrap_or("Missing source");
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
    pub(super) fn array_parameter_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) -> egui::Response {
        ui.vertical(|ui| {
            if let Some(mut set) = document
                .attached_array(document.active_model_index())
                .cloned()
            {
                let before = set.clone();
                if let Layout::Array(a) = &mut set.layout {
                    self.instances.array_layer = self.instances.array_layer.min(a.layers.len());
                    arrays::editor(ui, a, &mut self.instances.array_layer, document);
                }
                if set != before {
                    commands.push(Command::Instances(C::Preview(Box::new(set))));
                }
            } else {
                ui.weak("Select a submodel with an array");
            }
        })
        .response
    }
    pub(super) fn instance_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let response = egui::CollapsingHeader::new(
            if document
                .attached_array(document.active_model_index())
                .is_some()
            {
                "Array"
            } else {
                "Instances"
            },
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.add_enabled_ui(!self.mesh_busy, |ui| {
                self.instance_contents(ui, document, commands);
            });
        });
        if std::mem::take(&mut self.instances.focus_array) {
            ui.scroll_to_rect_animation(
                response.header_response.rect,
                Some(egui::Align::Min),
                egui::style::ScrollAnimation::none(),
            );
        }
    }
    fn instance_contents(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if let Some(set) = document
            .attached_array(document.active_model_index())
            .cloned()
            .filter(|s| {
                !(document.instance_preview_active()
                    && self.instances.editor.as_ref().is_some_and(|e| e.id != s.id))
            })
        {
            if let Some(request) = &mut self.instances.bake {
                if bake_options(ui, document, request, commands) {
                    self.instances.bake = None;
                }
            } else {
                attached::controls(ui, &mut self.instances, document, set, commands);
            }
            return;
        }
        if let Err(error) = document.instance_placements() {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        if document.instance_preview_active() {
            self.instances.bake = None;
        } else {
            self.instances.editor = None;
        }
        if let Some(request) = &mut self.instances.bake {
            if bake_options(ui, document, request, commands) {
                self.instances.bake = None;
            }
            return;
        }
        if let Some(set) = &mut self.instances.editor {
            let before = set.clone();
            let copy = document
                .selected_instance
                .filter(|(id, _)| *id == set.id && !set.joined())
                .map(|(_, slot)| slot);
            let color = if copy.is_some() {
                COPY_COLOR
            } else {
                SET_COLOR
            };
            ui.colored_label(color, egui::RichText::new(set.target_name(copy)).strong());
            if copy.is_some() {
                ui.label("Drag a coloured axis to move the copy, or drag its centre handle to move in the view plane. Drag elsewhere to orbit.");
            } else {
                ui.label(match &set.layout {
                    Layout::Array(_) => "Edit whole array · Change the count, spacing or layout of the blue-framed copies.",
                    Layout::Path(_) => "Edit path assembly · Change the route, pieces or how they fit together.",
                    Layout::Manual(_) => "Edit whole stroke · Rename it or show and hide all its copies together.",
                });
            }
            if document.instances().array_is_repeated(set.id) {
                ui.weak("Arrays that repeat this set also inherit these changes.");
            }
            let (apply, cancel) = ui
                .horizontal(|ui| {
                    (
                        ui.add_enabled(
                            !self.instances.invalid,
                            egui::Button::new(if copy.is_some() {
                                "Apply placement"
                            } else {
                                "Apply instances"
                            }),
                        )
                        .clicked(),
                        ui.button("Cancel preview").clicked(),
                    )
                })
                .inner;
            if let Some(slot) = copy {
                ui.separator();
                let source = document.instance_placements().ok().and_then(|placements| {
                    placements
                        .iter()
                        .find(|(id, n, _)| (*id, *n) == (set.id, slot))
                        .map(|(_, _, p)| p.piece.source)
                });
                if let Some(name) = source.and_then(|source| source_name(document, source)) {
                    ui.label(format!("Linked shape · {name}"));
                } else {
                    ui.label("Shape stays linked to its source.");
                }
                let mut adjustment = set
                    .adjustments
                    .iter()
                    .find(|a| a.slot == slot)
                    .cloned()
                    .unwrap_or_else(|| Adjustment::new(slot));
                let old = adjustment.clone();
                let a = &mut adjustment;
                ui.checkbox(&mut a.visible, "Show this copy");
                if !set.visible {
                    ui.weak("This whole set is hidden. Enable Show instances in the set editor to see the copy.");
                } else if !a.visible {
                    ui.weak("Enable Show this copy to see its placement.");
                }
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
                ui.weak(
                    "Drag a value to preview it. Apply placement keeps the edit. To sculpt this copy separately, choose Make independent in the Copies list.",
                );
                if old != adjustment {
                    if let Some(a) = set.adjustments.iter_mut().find(|a| a.slot == slot) {
                        *a = adjustment;
                    } else {
                        set.adjustments.push(adjustment);
                    }
                }
            } else {
                ui.text_edit_singleline(&mut set.name);
                ui.checkbox(&mut set.visible, "Show instances");
                match &mut set.layout {
                    Layout::Array(a) => {
                        arrays::editor(ui, a, &mut self.instances.array_layer, document)
                    }
                    Layout::Path(a) => {
                        if path_editor::controls(
                            ui,
                            a,
                            document,
                            &mut self.instances.inline_thickness,
                            &mut self.instances.thickness_path_points,
                            &mut self.instances.thickness_sections,
                        ) {
                            set.adjustments.clear();
                        }
                    }
                    Layout::Manual(p) => {
                        ui.label(format!("{} placed objects", p.len()));
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
                        egui::Button::new("Create path object…"),
                    )
                    .customize(Action::CreatePathAssembly)
                    .clicked()
                {
                    commands.push(Command::Instances(C::NewPath));
                }
            });
            if let Some((id, slot)) = document.selected_instance
                && let Some(set) = document.instances().sets.iter().find(|s| s.id == id)
            {
                let joined = set.joined();
                ui.colored_label(
                    if joined { SET_COLOR } else { COPY_COLOR },
                    egui::RichText::new(set.target_name(Some(slot))).strong(),
                );
                if joined {
                    ui.label("Joined assembly selected · edit its path and connections.");
                } else {
                    ui.label("Edit this placement to move, rotate, size or hide the copy.");
                }
                if ui
                    .button(if joined {
                        "Edit selected assembly…"
                    } else {
                        "Edit selected copy…"
                    })
                    .clicked()
                {
                    commands.push(Command::Instances(if joined {
                        C::Edit(id)
                    } else {
                        C::EditCopy(id, slot)
                    }));
                }
            }

            if !document.instances().sets.is_empty() && ui.button("Bake all…").clicked() {
                self.instances.bake = Some(BakeRequest::new(BakeScope::All));
            }
            for set in &document.instances().sets {
                ui.push_id(set.id, |ui| {
                    instance_set(ui, document, set, &mut self.instances.bake, commands)
                });
            }
            ui.collapsing("Advanced linking", |ui| {
                if ui
                    .add_enabled(
                        !document.paths().is_empty(),
                        egui::Button::new("Separate pieces along path…"),
                    )
                    .clicked()
                {
                    commands.push(Command::Instances(C::NewPathParts));
                }
                if ui.button("Standalone array…").clicked() {
                    commands.push(Command::Instances(C::NewLegacyArray));
                }
            });
        }
    }
}

fn bake_options(
    ui: &mut egui::Ui,
    document: &Document,
    request: &mut BakeRequest,
    commands: &mut Vec<Command>,
) -> bool {
    let replaces = matches!(request.scope, BakeScope::Set(id) if document.array_owner(id) == Some(document.active_model_index()));
    let name = match request.scope {
        BakeScope::Set(id) => {
            let Some(set) = document.instances().sets.iter().find(|s| s.id == id) else {
                return true;
            };
            &set.name
        }
        BakeScope::All => "all visible instance sets",
    };
    ui.strong(format!("Bake {name}"));
    ui.label(
        "Visible copies become editable geometry. Each baked set is removed. Undo restores it.",
    );
    ui.radio_value(
        &mut request.destination,
        BakeDestination::PerCopy,
        if replaces {
            "Separate submodels (keep source)"
        } else {
            "One new submodel per copy"
        },
    );
    ui.radio_value(
        &mut request.destination,
        BakeDestination::New,
        if replaces {
            "One new submodel (keep source)"
        } else {
            "One new submodel"
        },
    );
    let target = document.model(document.active_model_index()).unwrap();
    ui.radio_value(
        &mut request.destination,
        BakeDestination::Current,
        if replaces {
            format!("Replace array on: {}", target.name)
        } else {
            format!("Merge into selected submodel: {}", target.name)
        },
    );
    if request.destination == BakeDestination::PerCopy {
        request.fuse = false;
    } else {
        ui.checkbox(&mut request.fuse, "Fuse overlapping surfaces");
        if request.fuse {
            ui.weak("Voxel remeshing joins overlapping closed surfaces. Pieces separated by a gap stay disconnected. Remeshing rebuilds topology and removes UVs.");
        } else {
            ui.weak("Combines surfaces in one mesh and keeps compatible UVs. Overlaps keep their interior faces.");
        }
        if request.destination == BakeDestination::Current && replaces {
            ui.weak("Replaces this submodel with exactly the array result and resets its subdivision levels. Undo restores the source and live array. Other linked uses of this source follow the converted geometry.");
        } else if request.destination == BakeDestination::Current {
            ui.weak("Includes the selected surface, applies its thickness and resets its primitive and subdivision levels. Other live copies of this submodel follow the merged geometry.");
        }
    }
    let can_bake = request.destination != BakeDestination::Current || target.metadata.visible;
    if !can_bake {
        ui.weak("Show the selected submodel before merging into it.");
    }
    ui.horizontal(|ui| {
        if ui
            .add_enabled(can_bake, egui::Button::new("Bake now"))
            .clicked()
        {
            commands.push(Command::Instances(C::Bake(*request)));
            return true;
        }
        ui.button("Cancel bake").clicked()
    })
    .inner
}

fn instance_set(
    ui: &mut egui::Ui,
    document: &Document,
    set: &Set,
    bake: &mut Option<BakeRequest>,
    commands: &mut Vec<Command>,
) {
    ui.separator();
    ui.strong(set.target_name(None));
    ui.horizontal_wrapped(|ui| {
        let edit_label = match set.layout {
            Layout::Array(_) => "Edit array…",
            Layout::Path(_) => "Edit path assembly…",
            Layout::Manual(_) => "Edit stroke…",
        };
        if ui
            .button(edit_label)
            .on_hover_text("Preview changes to the whole set. Apply to save them.")
            .clicked()
        {
            commands.push(Command::Instances(C::Edit(set.id)));
        }
        if ui.button("Bake set…").clicked() {
            *bake = Some(BakeRequest::new(BakeScope::Set(set.id)));
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
        ui.weak("Joined assembly · Baking treats the joined sections as one piece. Choose Separate to adjust individual copies.");
        return;
    }
    egui::CollapsingHeader::new("Copies").show(ui, |ui| {
        ui.weak("Select a copy to edit its transform or visibility. Make independent creates a submodel you can sculpt.");
        // Hidden copies must remain selectable so their visibility can be restored.
        let mut listed = set.clone();
        for adjustment in &mut listed.adjustments {
            adjustment.visible = true;
        }
        if let Ok(placements) =
            ArrayResolver::new(document.instances()).with_sources(|s| document.instance_source(s)).placements(&listed, document.paths())
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
                                .add_enabled(visible, egui::Button::new("Make independent").small())
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

impl Thumbnail {
    fn new(ctx: &egui::Context, asset: &Asset) -> Option<Self> {
        let [yaw, pitch, roll] = asset.capture_angles;
        let camera = crate::camera::Camera {
            yaw,
            pitch,
            roll,
            ..Default::default()
        };
        let icon = crate::brush_icon::capture_part(&asset.mesh, &camera).ok()?;
        Some(Self {
            source: asset.mesh.clone(),
            texture: super::icons::texture(ctx, format!("object-icon-{}", asset.id), &icon),
        })
    }
}
impl Controls {
    fn placement_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.brush.stamp, false, "Create instance");
            ui.selectable_value(&mut self.brush.stamp, true, "Stamp mesh");
        });
        if self.brush.stamp {
            ui.label("Raise the surface to the model shape. Hold Alt to press it inward. Click or drag to stamp.");
            ui.weak(
                "Stamp detail depends on the target mesh resolution. Drag empty space to orbit.",
            );
        } else {
            ui.weak(
                "Click the surface to place a part, drag to repeat, or drag empty space to orbit.",
            );
        }
        self.size_control(ui);
        ui.add(
            egui::DragValue::new(&mut self.brush.spacing)
                .range(0.001..=1000.)
                .speed(0.01)
                .prefix("Repeat spacing "),
        );
        ui.add(
            egui::DragValue::new(&mut self.brush.offset)
                .speed(0.01)
                .prefix("Surface offset "),
        );
        ui.add(egui::Slider::new(&mut self.brush.rotation, -180.0..=180.).text("Rotation °"));
        if !self.brush.stamp {
            ui.checkbox(&mut self.brush.independent, "Create independent geometry");
        }
    }
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

    fn ship_greebles_icon(&mut self, ctx: &egui::Context) -> Option<egui::TextureId> {
        if self.ship_greebles_thumbnail.is_none() {
            self.ship_greebles_thumbnail = Thumbnail::new(ctx, &ship_greebles().objects[0]);
        }
        self.ship_greebles_thumbnail
            .as_ref()
            .map(|t| t.texture.id())
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
        let thumbnail = Thumbnail::new(ctx, asset)?;
        let texture_id = thumbnail.texture.id();
        self.thumbnails.insert(asset.id, thumbnail);
        Some(texture_id)
    }
}
pub(super) use brushes::{library_tiles, part_picker, settings};

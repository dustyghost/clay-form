use super::*;

fn capture(ui: &mut egui::Ui, c: &mut Controls, d: &Document, commands: &mut Vec<Command>) {
    c.capture_frame = Some(ui.ctx().cumulative_frame_nr());
    let model = d.active_model_index();
    egui::ComboBox::from_id_salt("capture_source_model")
        .selected_text(d.model(model).map_or("Select source", |m| m.name))
        .show_ui(ui, |ui| {
            for i in 0..d.model_count() {
                if ui
                    .selectable_label(i == model, d.model(i).unwrap().name)
                    .clicked()
                {
                    commands.push(Command::SelectSubmodel(i));
                }
            }
        });
    ui.add(
        egui::TextEdit::singleline(&mut c.name).hint_text("Object name (defaults to model name)"),
    );
    axes(ui, &mut c.forward, &mut c.up);
    ui.weak("Up points away from the surface. Forward follows your stroke.");
    vector(ui, "Contact point on the source", &mut c.pivot);
    if ui
        .button("Use base centre")
        .on_hover_text("Use the centre of the object's bounds on its lowest face along Up.")
        .clicked()
        && !d.mesh().vertices.is_empty()
    {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in &d.mesh().vertices {
            for (i, x) in [p.0, p.1, p.2].into_iter().enumerate() {
                lo[i] = lo[i].min(x);
                hi[i] = hi[i].max(x);
            }
        }
        let mut xyz: [f32; 3] = std::array::from_fn(|i| (lo[i] + hi[i]) * 0.5);
        xyz[c.up] = lo[c.up];
        c.pivot = Vec3(xyz[0], xyz[1], xyz[2]);
    }
    if c.forward == c.up {
        ui.weak("Choose different Forward and Up axes.");
    }
    if ui
        .add_enabled(
            c.forward != c.up && !d.mesh().triangles.is_empty(),
            egui::Button::new("Capture active model"),
        )
        .customize(Action::CaptureObjectBrush)
        .clicked()
    {
        commands.push(Command::Instances(C::Capture(
            c.name.clone(),
            c.pivot,
            c.forward,
            c.up,
        )));
    }
}
pub(in crate::ui) fn library_tiles(
    ui: &mut egui::Ui,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
    width: f32,
) {
    let library = d.instances();
    c.thumbnails
        .retain(|id, _| library.assets.iter().any(|a| a.id == *id));
    for brush in &library.brushes {
        let cover = brush
            .objects
            .first()
            .and_then(|id| library.assets.iter().find(|a| a.id == *id));
        if design::brush_tile_with_icon(
            ui,
            &brush.name,
            c.placing && c.group == Some(brush.id),
            width,
            |ctx| cover.and_then(|a| c.object_icon(ctx, a)),
        )
        .clicked()
        {
            commands.push(Command::Instances(C::ChooseBrush(brush.id)));
        }
    }
    if !library.brushes.iter().any(|b| b.name == SHIP_GREEBLES_NAME)
        && design::brush_tile_with_icon(ui, SHIP_GREEBLES_NAME, false, width, |ctx| {
            c.ship_greebles_icon(ctx)
        })
        .on_hover_text(
            "Built-in object brush with a hull vent, access hatch and raised service panel.",
        )
        .clicked()
    {
        commands.push(Command::Instances(C::ChooseShipGreebles));
    }
}

pub(in crate::ui) fn part_picker(
    ui: &mut egui::Ui,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
) -> egui::Response {
    ui.vertical(|ui| {
        ui.add(egui::Label::new("Object brush models").sense(egui::Sense::click()))
            .customize(Action::ObjectBrushModels);
        let Some(group) = d
            .instances()
            .brushes
            .iter()
            .find(|b| c.placing && Some(b.id) == c.group)
        else {
            ui.weak("Select an object brush in the Brush library.");
            return;
        };
        if group.objects.is_empty() {
            ui.weak("Capture a model or include an existing part in Brush settings.");
            return;
        }
        let width = 74.;
        ui.horizontal_wrapped(|ui| {
            for id in &group.objects {
                if let Some(part) = d.instances().assets.iter().find(|a| a.id == *id)
                    && design::brush_tile_with_icon(
                        ui,
                        &part.name,
                        c.asset == Some(*id),
                        width,
                        |ctx| c.object_icon(ctx, part),
                    )
                    .clicked()
                {
                    commands.push(Command::Instances(C::ChooseObject(*id)));
                    if ui
                        .stack()
                        .iter()
                        .any(|stack| stack.kind() == Some(egui::UiKind::Menu))
                    {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                }
            }
        });
    })
    .response
}

pub(in crate::ui) fn settings(
    ui: &mut egui::Ui,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
) {
    let Some(brush) = d.instances().brushes.iter().find(|b| Some(b.id) == c.group) else {
        return;
    };
    ui.strong("Object brush");
    ui.horizontal_wrapped(|ui| {
        ui.label("Name");
        let response = ui.add(
            egui::TextEdit::singleline(&mut c.group_name)
                .id(egui::Id::new(("object_brush_name", brush.id)))
                .desired_width(150.),
        );
        if response.lost_focus() && c.group_name.trim() != brush.name {
            commands.push(Command::Instances(C::RenameBrush(
                brush.id,
                c.group_name.clone(),
            )));
        }
    });
    if let Some(part) = d
        .instances()
        .assets
        .iter()
        .find(|part| Some(part.id) == c.asset)
    {
        ui.label(format!("Selected model: {}", part.name));
        if ui.button("Remove model from brush")
            .on_hover_text("Remove this model from this brush. Its capture and placed copies stay in the project.")
            .clicked() {
            commands.push(Command::Instances(C::BrushMember(brush.id, part.id, false)));
        }
        c.placement_settings(ui);
    }
    egui::CollapsingHeader::new("Capture a model into this brush")
        .id_salt(("capture_part", brush.id))
        .default_open(brush.objects.is_empty())
        .show(ui, |ui| capture(ui, c, d, commands));
    if !d.instances().assets.is_empty() {
        ui.collapsing("Include existing parts", |ui| {
            egui::ScrollArea::vertical()
                .id_salt("object_members")
                .max_height(130.)
                .show(ui, |ui| {
                    for a in &d.instances().assets {
                        let mut included = brush.objects.contains(&a.id);
                        if ui
                            .push_id(a.id, |ui| {
                                ui.horizontal(|ui| {
                                    if let Some(icon) = c.object_icon(ui.ctx(), a) {
                                        ui.image((icon, egui::Vec2::splat(28.)));
                                    }
                                    ui.checkbox(&mut included, &a.name)
                                })
                                .inner
                            })
                            .inner
                            .changed()
                        {
                            commands
                                .push(Command::Instances(C::BrushMember(brush.id, a.id, included)));
                        }
                    }
                });
        });
    }
    ui.separator();
    ui.collapsing("Reuse in other projects", |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !brush.objects.is_empty(),
                    egui::Button::new(if c.personal_brushes.contains(&brush.name) {
                        "Update saved brush"
                    } else {
                        "Save brush to my library"
                    }),
                )
                .clicked()
            {
                commands.push(Command::Instances(C::SavePersonalBrush(brush.id)));
            }
            if ui.button("Refresh library").clicked() {
                commands.push(Command::Instances(C::ListPersonalBrushes));
            }
        });
        if c.personal_brushes.is_empty() {
            ui.weak("No personal brushes saved yet.");
        }
        egui::ScrollArea::vertical()
            .id_salt("personal_brushes")
            .max_height(140.)
            .show(ui, |ui| {
                for name in &c.personal_brushes {
                    ui.push_id(name, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(name);
                            if let Some(b) = d.instances().brushes.iter().find(|b| b.name == *name)
                            {
                                if ui.small_button("Select in project").clicked() {
                                    commands.push(Command::Instances(C::ChooseBrush(b.id)));
                                }
                            } else if ui.small_button("Add to project").clicked() {
                                commands
                                    .push(Command::Instances(C::LoadPersonalBrush(name.clone())));
                            }
                            if ui
                                .small_button("Remove…")
                                .on_hover_text("Remove this saved personal copy")
                                .clicked()
                            {
                                c.remove_saved = Some(name.clone());
                            }
                        })
                    });
                }
            });
        if let Some(name) = c.remove_saved.clone() {
            ui.label(format!(
                "Remove {name} from My library? Project copies are kept."
            ));
            ui.horizontal(|ui| {
                if ui.button("Remove saved copy").clicked() {
                    commands.push(Command::Instances(C::RemovePersonalBrush(name)));
                }
                if ui.button("Keep saved copy").clicked() {
                    c.remove_saved = None;
                }
            });
        }
    });
    egui::CollapsingHeader::new("Transfer a whole brush (.objects.json)").show(ui, |ui| {
        ui.text_edit_singleline(&mut c.bundle_path);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Import brush file").clicked() {
                commands.push(Command::Instances(C::ImportBrush(c.bundle_path.clone())));
            }
            if ui
                .add_enabled(
                    !brush.objects.is_empty(),
                    egui::Button::new("Export brush file"),
                )
                .clicked()
            {
                commands.push(Command::Instances(C::ExportBrush(
                    brush.id,
                    c.bundle_path.clone(),
                )));
            }
        });
    });
    egui::CollapsingHeader::new("Transfer a single part (.object.json)").show(ui, |ui| {
        ui.label("This file contains one captured model and its placement settings.");
        ui.text_edit_singleline(&mut c.asset_path);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Import part").clicked() {
                commands.push(Command::Instances(C::ImportAsset(c.asset_path.clone())));
            }
            if let Some(id) = c.asset
                && let Some(part) = d.instances().assets.iter().find(|a| a.id == id)
                && ui.button(format!("Export {}", part.name)).clicked()
            {
                commands.push(Command::Instances(C::ExportAsset(id, c.asset_path.clone())));
            }
        });
    });
    egui::CollapsingHeader::new("Captured parts in this project").show(ui, |ui| {
        for part in &d.instances().assets {
            ui.push_id(part.id, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&part.name);
                    if ui.small_button("Remove unused part").clicked() {
                        commands.push(Command::Instances(C::RemoveAsset(part.id)));
                    }
                })
            });
        }
    });
    if ui.button("Remove brush from project").clicked() {
        commands.push(Command::Instances(C::RemoveBrush(brush.id)));
    }
}

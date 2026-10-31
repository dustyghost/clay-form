use super::*;

fn group_picker(ui: &mut egui::Ui, c: &Controls, library: &Library, commands: &mut Vec<Command>) {
    let selected = library.brushes.iter().find(|b| Some(b.id) == c.group);
    egui::ComboBox::from_id_salt("object_brush_group")
        .selected_text(selected.map_or("All captured objects", |b| b.name.as_str()))
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(c.group.is_none(), "All captured objects")
                .clicked()
            {
                commands.push(Command::Instances(C::ChooseBrush(None)));
            }
            for brush in &library.brushes {
                if ui
                    .selectable_label(c.group == Some(brush.id), &brush.name)
                    .clicked()
                {
                    commands.push(Command::Instances(C::ChooseBrush(Some(brush.id))));
                }
            }
        });
}
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
pub(super) fn palette(
    ui: &mut egui::Ui,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
) {
    let library = d.instances();
    if c.group
        .is_some_and(|id| !library.brushes.iter().any(|b| b.id == id))
    {
        c.group = None;
    }
    if c.asset
        .is_some_and(|id| !library.assets.iter().any(|a| a.id == id))
    {
        c.asset = None;
    }
    ui.strong("Object brushes");
    ui.horizontal_wrapped(|ui| {
        if !library.brushes.is_empty() {
            group_picker(ui, c, library, commands);
        }
        if ui.button("Manage brushes…").clicked() {
            c.library_open = true;
            commands.push(Command::Instances(C::ListPersonalBrushes));
        }
    });
    egui::CollapsingHeader::new("Capture object brush").show(ui, |ui| capture(ui, c, d, commands));
    let group = library.brushes.iter().find(|b| Some(b.id) == c.group);
    let assets: Vec<_> = if let Some(group) = group {
        group
            .objects
            .iter()
            .filter_map(|id| library.assets.iter().find(|a| a.id == *id))
            .collect()
    } else {
        library.assets.iter().collect()
    };
    if assets.is_empty() {
        if group.is_some() {
            ui.weak("Capture an object or add an existing one in Manage brushes.");
        }
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("object_brush_palette")
        .max_height(144.)
        .show_rows(ui, 42., assets.len(), |ui, rows| {
            for i in rows {
                let asset = assets[i];
                ui.push_id(asset.id, |ui| {
                    ui.horizontal(|ui| {
                        let icon = thumbnail(ui, asset, &mut c.thumbnails)
                            .on_hover_text(format!("Choose {} for the next stroke", asset.name));
                        let label = ui.selectable_label(c.asset == Some(asset.id), &asset.name);
                        if icon.clicked() || label.clicked() {
                            commands.push(Command::Instances(C::ChooseObject(asset.id)));
                        }
                        if ui
                            .small_button("×")
                            .on_hover_text(if group.is_some() {
                                "Remove from this brush; keep the captured object"
                            } else {
                                "Remove unused captured object"
                            })
                            .clicked()
                        {
                            let action = group.map_or(C::RemoveAsset(asset.id), |b| {
                                C::BrushMember(b.id, asset.id, false)
                            });
                            commands.push(Command::Instances(action));
                        }
                    })
                });
            }
        });
}

pub(super) fn manager(
    ctx: &egui::Context,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
) {
    let mut open = true;
    egui::Window::new("Object brush library")
        .open(&mut open)
        .default_pos(egui::pos2(18., 82.))
        .default_width(350.)
        .default_height(590.)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.add_enabled_ui(!d.instance_preview_active(), |ui| {
                management_contents(ui, c, d, commands)
            });
        });
    c.library_open = open;
}

fn management_contents(
    ui: &mut egui::Ui,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
) {
    ui.strong("Brushes in this project");
    group_picker(ui, c, d.instances(), commands);
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut c.group_name)
                .desired_width(165.)
                .hint_text("Brush name"),
        );
        if ui.button("New brush").clicked() {
            commands.push(Command::Instances(C::CreateBrush(c.group_name.clone())));
        }
        if let Some(id) = c.group
            && ui.button("Rename").clicked()
        {
            commands.push(Command::Instances(C::RenameBrush(id, c.group_name.clone())));
        }
    });
    let brush = d.instances().brushes.iter().find(|b| Some(b.id) == c.group);
    if let Some(brush) = brush {
        ui.label(format!("{} · {} objects", brush.name, brush.objects.len()));
        egui::CollapsingHeader::new("Capture a model into this brush")
            .show(ui, |ui| capture(ui, c, d, commands));
        if !d.instances().assets.is_empty() {
            ui.label("Include captured objects");
            egui::ScrollArea::vertical()
                .id_salt("object_members")
                .max_height(130.)
                .show(ui, |ui| {
                    for a in &d.instances().assets {
                        let mut included = brush.objects.contains(&a.id);
                        if ui
                            .push_id(a.id, |ui| ui.checkbox(&mut included, &a.name))
                            .inner
                            .changed()
                        {
                            commands
                                .push(Command::Instances(C::BrushMember(brush.id, a.id, included)));
                        }
                    }
                });
        }
        if ui
            .button("Remove brush from project")
            .on_hover_text("Keeps its captured objects and painted copies.")
            .clicked()
        {
            commands.push(Command::Instances(C::RemoveBrush(brush.id)));
        }
    } else {
        ui.weak(
            "Name a brush, then capture models into it or choose objects you already captured.",
        );
    }
    ui.separator();
    ui.strong("My library · available in every project");
    ui.horizontal_wrapped(|ui| {
        if let Some(b) = brush
            && ui
                .add_enabled(
                    !b.objects.is_empty(),
                    egui::Button::new(if c.personal_brushes.contains(&b.name) {
                        "Update saved brush"
                    } else {
                        "Save brush to my library"
                    }),
                )
                .clicked()
        {
            commands.push(Command::Instances(C::SavePersonalBrush(b.id)));
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
                        if let Some(b) = d.instances().brushes.iter().find(|b| b.name == *name) {
                            if ui.small_button("Select in project").clicked() {
                                commands.push(Command::Instances(C::ChooseBrush(Some(b.id))));
                            }
                        } else if ui.small_button("Add to project").clicked() {
                            commands.push(Command::Instances(C::LoadPersonalBrush(name.clone())));
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
    egui::CollapsingHeader::new("Import or export a whole brush").show(ui, |ui| {
        ui.text_edit_singleline(&mut c.bundle_path);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Import brush file").clicked() {
                commands.push(Command::Instances(C::ImportBrush(c.bundle_path.clone())));
            }
            if let Some(b) = brush
                && ui
                    .add_enabled(
                        !b.objects.is_empty(),
                        egui::Button::new("Export brush file"),
                    )
                    .clicked()
            {
                commands.push(Command::Instances(C::ExportBrush(
                    b.id,
                    c.bundle_path.clone(),
                )));
            }
        });
    });
}

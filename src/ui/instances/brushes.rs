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
/// Named brushes and standalone parts both work in projects created before the library tiles.
pub(in crate::ui) fn library_tiles(
    ui: &mut egui::Ui,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
    width: f32,
) {
    if ui.button("Manage object brushes…").clicked() {
        c.library_open = true;
        if c.placing {
            commands.push(Command::Instances(C::StopBrush));
        }
        commands.push(Command::Instances(C::ListPersonalBrushes));
    }
    let library = d.instances();
    c.thumbnails
        .retain(|id, _| library.assets.iter().any(|a| a.id == *id));
    let groups = library.brushes.iter().map(|b| {
        (
            Some(b.id),
            &b.name,
            b.objects
                .first()
                .and_then(|id| library.assets.iter().find(|a| a.id == *id)),
        )
    });
    let standalone = library
        .assets
        .iter()
        .filter(|a| !library.brushes.iter().any(|b| b.objects.contains(&a.id)))
        .map(|a| (None, &a.name, Some(a)));
    ui.horizontal_wrapped(|ui| {
        for (group, name, first) in groups.chain(standalone) {
            let selected = c.placing
                && c.group == group
                && (group.is_some() || c.asset == first.map(|a| a.id));
            if let Some(part) = first {
                if design::brush_tile_with_icon(ui, name, selected, width, |ctx| {
                    c.object_icon(ctx, part)
                })
                .clicked()
                {
                    commands.push(Command::Instances(C::UseObjectBrush(group, part.id)));
                }
            } else if ui
                .add_sized([width, 82.], egui::Button::new(name))
                .on_hover_text("Capture the first part")
                .clicked()
            {
                c.library_open = true;
                if c.placing {
                    commands.push(Command::Instances(C::StopBrush));
                }
                commands.push(Command::Instances(C::ChooseBrush(group)));
                commands.push(Command::Instances(C::ListPersonalBrushes));
            }
        }
    });
}

pub(in crate::ui) fn part_picker(
    ui: &mut egui::Ui,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
    width: f32,
) {
    let Some(group) = d.instances().brushes.iter().find(|b| Some(b.id) == c.group) else {
        return;
    };
    if group.objects.len() < 2 {
        return;
    }
    ui.label("Part to place");
    egui::ScrollArea::vertical()
        .id_salt("choose_object_part")
        .max_height(182.)
        .show(ui, |ui| {
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
                    }
                }
            });
        });
}

pub(in crate::ui) fn manager(
    ctx: &egui::Context,
    c: &mut Controls,
    d: &Document,
    commands: &mut Vec<Command>,
) {
    let mut open = true;
    egui::Window::new("Manage object brushes")
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
        if ui
            .add_enabled(
                !c.group_name.trim().is_empty(),
                egui::Button::new("New brush"),
            )
            .clicked()
        {
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
            .id_salt(("capture_part", brush.id))
            .default_open(true)
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
                                commands.push(Command::Instances(C::BrushMember(
                                    brush.id, a.id, included,
                                )));
                            }
                        }
                    });
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
        egui::CollapsingHeader::new("Capture a standalone part")
            .show(ui, |ui| capture(ui, c, d, commands));
    }
    ui.separator();
    ui.collapsing("Reuse in other projects", |ui| {
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
                            if let Some(b) = d.instances().brushes.iter().find(|b| b.name == *name)
                            {
                                if ui.small_button("Select in project").clicked() {
                                    commands.push(Command::Instances(C::ChooseBrush(Some(b.id))));
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
                    if ui
                        .selectable_label(c.asset == Some(part.id), &part.name)
                        .clicked()
                    {
                        commands.push(Command::Instances(C::ChooseBrush(None)));
                        commands.push(Command::Instances(C::ChooseObject(part.id)));
                    }
                    if ui.small_button("Remove unused part").clicked() {
                        commands.push(Command::Instances(C::RemoveAsset(part.id)));
                    }
                })
            });
        }
    });
}

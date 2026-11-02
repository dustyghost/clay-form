use super::*;

pub(super) fn controls(
    ui: &mut egui::Ui,
    state: &mut Controls,
    document: &Document,
    mut set: Set,
    commands: &mut Vec<Command>,
) {
    let before = set.clone();
    let source = document.editing_array_source();
    let copy = document
        .selected_instance
        .filter(|(id, _)| *id == set.id)
        .map(|(_, slot)| slot);
    let name = document.model(document.active_model_index()).unwrap().name;
    ui.colored_label(
        if copy.is_some() {
            COPY_COLOR
        } else {
            SET_COLOR
        },
        egui::RichText::new(format!("{name} · Array")).strong(),
    );
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(!source && copy.is_none() && state.array_edit, "Edit array")
            .clicked()
        {
            commands.push(Command::Instances(C::ArrayMode(false)));
        }
        if ui.selectable_label(source, "Edit source").clicked() {
            commands.push(Command::Instances(C::ArrayMode(true)));
        }
        if ui.selectable_label(copy.is_some(), "Edit copy…").clicked() {
            commands.push(Command::Instances(C::EditCopy(set.id, copy.unwrap_or(0))));
        }
    });
    ui.checkbox(&mut set.visible, "Enable array");
    if source {
        ui.strong(format!("Editing source: {name}"));
        ui.label("Sculpt the source surface; the grey linked copies update with it. Choose Edit array to change the pattern.");
    } else if let Some(slot) = copy {
        let count = match &set.layout {
            Layout::Array(a) => a
                .layers
                .iter()
                .fold(a.count, |n, l| n.saturating_mul(l.count)),
            _ => 0,
        };
        egui::ComboBox::from_id_salt(("owned_array_copy", set.id))
            .selected_text(format!("Copy {} of {count}", slot + 1))
            .show_ui(ui, |ui| {
                for i in 0..count {
                    if ui
                        .selectable_label(slot == i, format!("Copy {}", i + 1))
                        .clicked()
                    {
                        commands.push(Command::Instances(C::EditCopy(set.id, i)));
                    }
                }
            });
        let mut a = set
            .adjustments
            .iter()
            .find(|a| a.slot == slot)
            .cloned()
            .unwrap_or_else(|| Adjustment::new(slot));
        let old = a.clone();
        ui.checkbox(&mut a.visible, "Show this copy");
        vector(ui, "Move", &mut a.offset);
        vector(ui, "Rotate °", &mut a.rotation);
        vector(ui, "Scale X / Y / Z", &mut a.axis_scale);
        if a != old {
            if let Some(old) = set.adjustments.iter_mut().find(|a| a.slot == slot) {
                *old = a;
            } else {
                set.adjustments.push(a);
            }
        }
        ui.weak("Gold handles edit this placement. Its shape stays linked to the source.");
        if ui.button("Make this copy independent").clicked() {
            commands.push(Command::Instances(C::Detach(set.id, slot)));
        }
    } else if state.array_edit {
        if let Layout::Array(a) = &mut set.layout {
            arrays::editor(ui, a, &mut state.array_layer, document);
        }
    } else {
        ui.label("Live array · select Edit array for inline controls.");
    }
    if state.invalid {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            "The last valid array is shown. Reduce the count or correct the values.",
        );
    }
    if set != before {
        commands.push(Command::Instances(C::Preview(Box::new(set.clone()))));
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Done").clicked() {
            commands.push(Command::Instances(C::ArrayDone));
        }
        if ui
            .add_enabled(
                set.visible && document.active_visible(),
                egui::Button::new("Make editable…"),
            )
            .clicked()
        {
            commands.push(Command::Instances(C::MakeArrayEditable(set.id)));
        }
        if ui.button("Remove array").clicked() {
            commands.push(Command::Instances(C::Remove(set.id)));
        }
    });
    ui.weak("Count includes the source once. Done keeps the array live; Make editable creates mesh geometry.");
    egui::CollapsingHeader::new("Advanced instances").show(ui, |ui| {
        if ui.button("Repeat this array…").clicked() {
            commands.push(Command::Instances(C::RepeatArray(set.id)));
        }
        if ui.button("Bake all…").clicked() {
            state.bake = Some(BakeRequest::new(BakeScope::All));
        }
        for other in document.instances().sets.iter().filter(|s| s.id != set.id) {
            if let Some(owner) = document.array_owner(other.id) {
                if ui.button(format!("Select {}", other.name)).clicked() {
                    commands.push(Command::SelectSubmodel(owner));
                }
            } else {
                instance_set(ui, document, other, &mut state.bake, commands);
            }
        }
    });
    state.editor = Some(set);
}

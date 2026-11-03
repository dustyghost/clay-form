//! Primary one-model authoring, with the separate-piece workflow retained.
use super::*;

pub(super) fn controls(
    ui: &mut egui::Ui,
    a: &mut PathAssembly,
    document: &Document,
    inline_thickness: &mut bool,
    path_points: &mut bool,
    sections: &mut bool,
) -> bool {
    let grouped = a.regions.is_some();
    if grouped {
        ui.horizontal_wrapped(|ui| {
            let _ = ui.selectable_label(true, "Grouped model");
            if ui.button("Separate pieces (advanced)").clicked() {
                a.regions = None;
                a.bend = false;
                a.join = JoinMode::Separate;
            }
        });
    }
    if a.regions.is_some() {
        let source = a.middle.source;
        choose_piece(ui, "Source model", &mut a.middle, document, true);
        let mut groups_changed = source != a.middle.source;
        if groups_changed {
            a.regions = Some(
                document
                    .instance_source(a.middle.source)
                    .map_or_else(PathRegions::default, PathRegions::named),
            );
        }
        if let Some(mesh) = document.instance_source(a.middle.source) {
            let groups = a.regions.as_mut().unwrap();
            for (role, selected) in [
                ("Start", &mut groups.start),
                ("Middle", &mut groups.middle),
                ("End", &mut groups.end),
            ] {
                egui::ComboBox::from_id_salt(("path_region", role))
                    .selected_text(format!(
                        "{role}: {}",
                        if selected.is_empty() {
                            "Choose group…"
                        } else {
                            selected.as_str()
                        }
                    ))
                    .show_ui(ui, |ui| {
                        for (i, g) in mesh.vertex_groups.iter().enumerate() {
                            let color =
                                crate::sculpt::Mesh::group_color(i).map(|v| (v * 255.) as u8);
                            if ui
                                .selectable_label(
                                    *selected == g.name,
                                    egui::RichText::new(&g.name).color(egui::Color32::from_rgb(
                                        color[0], color[1], color[2],
                                    )),
                                )
                                .clicked()
                            {
                                groups_changed |= *selected != g.name;
                                *selected = g.name.clone();
                            }
                        }
                    });
            }
            if groups_changed {
                groups.orient_piece(mesh, &mut a.middle);
            }
            if mesh.vertex_groups.len() < 3 {
                ui.weak("Cancel the preview and use Selection to vertex group to mark three adjoining face regions on the source.");
            }
        }
        ui.weak("Forward follows Start to End. Up faces away from the path surface. Only Middle repeats.");
        ui.add(egui::Slider::new(&mut a.count, 1..=254).text("Middle repeats"));
        ui.weak("End lengths follow Piece scale. Middle sections fit the remaining path length. Closed paths repeat only Middle.");
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
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.label("Sections");
            ui.selectable_value(&mut a.bend, false, "Rigid");
            ui.selectable_value(&mut a.bend, true, "Bend along path");
        });
        thickness_controls(ui, a, document, inline_thickness, path_points, sections);
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
        choose_piece(ui, "Middle", &mut a.middle, document, false);
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
                    choose_piece(ui, label, piece, document, false);
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
    }
    ui.add(
        egui::DragValue::new(&mut a.scale)
            .range(0.001..=1000.)
            .speed(0.01)
            .prefix("Piece scale "),
    );
    ui.add(egui::Slider::new(&mut a.twist, -180.0..=180.).text("Twist °"));
    if a.regions.is_some() {
        thickness_controls(ui, a, document, inline_thickness, path_points, sections);
    }

    if a.regions.is_none() && ui.button("Use groups from one model…").clicked() {
        a.regions = Some(
            document
                .instance_source(a.middle.source)
                .map_or_else(PathRegions::default, PathRegions::named),
        );
        if let Some(mesh) = document.instance_source(a.middle.source) {
            a.regions
                .as_ref()
                .unwrap()
                .orient_piece(mesh, &mut a.middle);
        }
        a.start = None;
        a.end = None;
        a.bend = true;
        a.fit_sections = true;
        a.fit_count = true;
        a.join = JoinMode::Weld;
    }

    grouped != a.regions.is_some()
}

fn thickness_controls(
    ui: &mut egui::Ui,
    a: &mut PathAssembly,
    document: &Document,
    inline_thickness: &mut bool,
    path_points: &mut bool,
    sections: &mut bool,
) {
    if a.bend {
        if a.regions.is_none() {
            ui.checkbox(&mut a.fit_sections, "Fit section lengths to spacing");
        }
        ui.checkbox(inline_thickness, "Inline thickness handles");
        let closed = document.path(a.path).is_some_and(|p| p.closed);
        egui::CollapsingHeader::new("Thickness along path").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(path_points, "Path points");
                ui.checkbox(sections, "Section centres");
            });
            ui.weak("Gold: ends · Teal: path points · Purple: sections");
            thickness::controls(ui, &mut a.thickness, closed);
        });
        ui.weak(
            "Bending uses source vertices; subdivide along its Forward axis for smooth curves.",
        );
    }
}

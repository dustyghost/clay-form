use super::*;

pub(super) fn editor(ui: &mut egui::Ui, a: &mut Array, active: &mut usize, document: &Document) {
    if let Some(id) = a.source_array {
        let name = document
            .instances()
            .sets
            .iter()
            .find(|s| s.id == id)
            .map_or("Missing array", |s| s.name.as_str());
        ui.label(format!("Linked source: {name}"));
    } else if a.owner.is_none() {
        choose_piece(ui, "Source", &mut a.piece, document);
    }
    *active = (*active).min(a.layers.len());
    ui.horizontal_wrapped(|ui| {
        ui.label("Array layers");
        for i in 0..=a.layers.len() {
            ui.selectable_value(active, i, format!("Layer {}", i + 1));
        }
        if ui
            .add_enabled(
                a.layers.len() + 1 < MAX_ARRAY_LAYERS,
                egui::Button::new("Add layer"),
            )
            .on_hover_text("Repeat the entire result of the preceding layers.")
            .clicked()
        {
            let mut layer = ArrayLayer::repeat();
            if a.owner.is_some() {
                layer.spacing_mode = ArraySpacing::Relative;
                layer.spacing = Vec3(0., 1., 0.);
            }
            a.layers.push(layer);
            *active = a.layers.len();
        }
    });
    if *active > 0 {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Layer {} · repeats layer {}", *active + 1, *active));
            if ui.small_button("Remove layer").clicked() {
                a.layers.remove(*active - 1);
                *active -= 1;
            }
        });
    }
    let i = *active;
    let attached = a.owner.is_some();
    let layer = if i == 0 {
        &mut a.base
    } else {
        &mut a.layers[i - 1]
    };
    ui.push_id(i,|ui|{
        let old_kind = layer.kind;
        ui.horizontal_wrapped(|ui|{
            for (kind,name) in [(ArrayKind::Linear,"Linear"),(ArrayKind::Radial,"Radial"),(ArrayKind::Grid,"Grid")] {
                ui.selectable_value(&mut layer.kind,kind,name);
            }
        });
        if attached && layer.kind == ArrayKind::Grid && old_kind != layer.kind {
            layer.count = layer.count.div_ceil(layer.columns) * layer.columns;
            if layer.spacing_mode == ArraySpacing::Span { layer.spacing_mode = ArraySpacing::Distance; }
            if layer.spacing.1 == 0. { layer.spacing.1 = 1.; }
        }
        if layer.kind != ArrayKind::Grid || !attached {
            ui.add(egui::Slider::new(&mut layer.count,1..=256).text(if attached {"Count"} else {"Copies"}));
        }
        if attached && layer.kind != ArrayKind::Radial {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut layer.spacing_mode,ArraySpacing::Relative,"Relative");
                ui.selectable_value(&mut layer.spacing_mode,ArraySpacing::Distance,"Distance");
                if layer.kind == ArrayKind::Linear { ui.selectable_value(&mut layer.spacing_mode,ArraySpacing::Span,"Fit span"); }
            });
        }
        vector(ui,if i==0 {"Origin / radial centre"} else {"Layer origin offset"},&mut layer.center);
        match layer.kind {
            ArrayKind::Linear=>{vector(ui,"Step between copies",&mut layer.spacing);}
            ArrayKind::Grid=>{
                if attached {
                    let mut rows = layer.count.div_ceil(layer.columns);
                    let old = (rows,layer.columns);
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::DragValue::new(&mut layer.columns).range(1..=256).prefix("Columns "));
                        ui.add(egui::DragValue::new(&mut rows).range(1..=256).prefix("Rows "));
                    });
                    if old != (rows,layer.columns) { layer.count = rows * layer.columns; }
                    ui.label(format!("{} items in this layer",layer.count));
                } else {
                ui.add(egui::Slider::new(&mut layer.columns,1..=256).text("Columns"));
                }
                ui.label("Grid spacing");
                ui.horizontal_wrapped(|ui|{
                    ui.add(egui::DragValue::new(&mut layer.spacing.0).speed(0.05).prefix("X "));
                    ui.add(egui::DragValue::new(&mut layer.spacing.1).speed(0.05).prefix("Y "));
                });
            }
            ArrayKind::Radial=>{
                ui.add(egui::DragValue::new(&mut layer.radius).range(0.001..=1e5).speed(0.05).prefix("Radius "));
                ui.add(egui::Slider::new(&mut layer.angle,0.01..=360.).text("Arc °"));
                ui.horizontal(|ui|{
                    ui.label("Axis");
                    for (i,n) in ["X","Y","Z"].iter().enumerate(){ ui.selectable_value(&mut layer.radial_axis,i,*n); }
                });
                ui.checkbox(&mut layer.rotate_copies,"Rotate copies around centre");
            }
        }
        if attached { ui.weak(match layer.spacing_mode {
            ArraySpacing::Relative => "Spacing values multiply the input size on each axis.",
            ArraySpacing::Distance => "Spacing is the distance between placement origins.",
            ArraySpacing::Span => "Span is the distance from the first placement to the last.",
        }); }
        egui::CollapsingHeader::new("Whole layer transform").show(ui, |ui| {
            vector(ui,"Layer rotation (degrees)",&mut layer.rotation);
            vector(ui,"Layer size on X / Y / Z",&mut layer.size);
        });
        egui::CollapsingHeader::new("Per-repeat transforms").show(ui,|ui|{
            if layer.kind!=ArrayKind::Linear || layer.step_offset!=Vec3::default() {
                vector(ui,"Additional position step",&mut layer.step_offset);
            }
            vector(ui,"Rotation step (degrees)",&mut layer.step_rotation);
            vector(ui,"Scale factor per step",&mut layer.step_scale);
            ui.weak("The first repeat keeps its size and angle. Later repeats add rotation and multiply size on each axis.");
        });
    });
}

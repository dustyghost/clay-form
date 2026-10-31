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
    } else {
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
            a.layers.push(ArrayLayer::repeat());
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
    let layer = if i == 0 {
        &mut a.base
    } else {
        &mut a.layers[i - 1]
    };
    ui.push_id(i,|ui|{
        ui.horizontal_wrapped(|ui|{
            for (kind,name) in [(ArrayKind::Linear,"Linear"),(ArrayKind::Radial,"Radial"),(ArrayKind::Grid,"Grid")] {
                ui.selectable_value(&mut layer.kind,kind,name);
            }
        });
        ui.add(egui::Slider::new(&mut layer.count,1..=256).text("Copies"));
        vector(ui,if i==0 {"Origin / radial centre"} else {"Layer origin offset"},&mut layer.center);
        match layer.kind {
            ArrayKind::Linear=>{vector(ui,"Step between copies",&mut layer.spacing);}
            ArrayKind::Grid=>{
                ui.add(egui::Slider::new(&mut layer.columns,1..=256).text("Columns"));
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

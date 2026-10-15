//! Primitive replacement warning and inline submodel parameter controls.
use super::{Action, ActionResponse, UiState};
use crate::{
    command::Command,
    document::Document,
    sculpt::{Primitive, PrimitiveKind as Kind},
};
use eframe::egui;
fn axis_control(ui: &mut egui::Ui, axis: usize, add: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        let color = crate::viewport::AXIS_COLORS[axis];
        ui.visuals_mut().override_text_color = Some(color);
        let widgets = &mut ui.visuals_mut().widgets;
        for style in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
        ] {
            style.fg_stroke.color = color;
        }
        add(ui);
    });
}
fn resolution_control(ui: &mut egui::Ui, label: &str, value: &mut u32, min: u32, max: u32) {
    ui.push_id(label, |ui| {
        ui.horizontal(|ui| {
            ui.add_sized([105., 18.], egui::Label::new(label));
            if ui.small_button("-").clicked() {
                *value = value.saturating_sub(1).max(min);
            }
            ui.add(egui::DragValue::new(value).range(min..=max).speed(1.));
            if ui.small_button("+").clicked() {
                *value = (*value + 1).min(max);
            }
        });
    });
}
impl UiState {
    pub(crate) fn primitive_confirmation(
        &mut self,
        ctx: &egui::Context,
    ) -> (Vec<Command>, Option<egui::Rect>) {
        let mut commands = Vec::new();
        if let Some((_, name)) = &self.primitive_warning {
            let response=egui::Modal::new(egui::Id::new("replace_with_primitive")).show(ctx,|ui| {
                ui.set_max_width(380.);
                ui.heading("Replace submodel with a primitive?");
                ui.label(format!("The current sculpted geometry, masks and subdivision levels of '{name}' will be replaced."));
                ui.label("The new shape stays parametric until you convert it to a sculpting mesh. Undo can restore the previous model.");
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() { commands.push(Command::CancelPrimitive); }
                    if ui.button("Replace with primitive").clicked() { commands.push(Command::ConfirmPrimitive); }
                });
            });
            if response.should_close() && commands.is_empty() {
                commands.push(Command::CancelPrimitive);
            }
            return (commands, Some(response.response.rect));
        }
        (commands, None)
    }
    pub(crate) fn primitive_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let Some(current) = document.primitive() else {
            return;
        };
        let mut params = current.clone();
        let forced = self.primitive_controls_requested.then_some(true);
        let response = egui::CollapsingHeader::new(format!("Primitive · {}", current.kind.label()))
            .id_salt(("primitive_controls", document.active_model_index()))
            .default_open(true).open(forced).show(ui, |ui| {
                    ui.label("Drag shape handles in the viewport, or use these precise controls. Convert when ready to sculpt.");
                    let mut kind=params.kind;
                    egui::ComboBox::from_id_salt("primitive_shape").selected_text(kind.label()).show_ui(ui,|ui| {
                        for option in Kind::ALL {ui.selectable_value(&mut kind,option,option.label());}
                    });
                    if kind!=params.kind { let mut replacement=Primitive::for_kind(kind);replacement.center=params.center;replacement.basis=params.basis;params=replacement; }
                    ui.separator();
                    if matches!(kind,Kind::Cube|Kind::Plane|Kind::Grid|Kind::UvSphere|Kind::Icosphere) {
                        axis_control(ui, 0, |ui| { ui.add(egui::Slider::new(&mut params.size.0,0.05..=10.).text("Width")); });
                        axis_control(ui, 1, |ui| { ui.add(egui::Slider::new(&mut params.size.1,0.05..=10.).text("Height")); });
                        if !matches!(kind,Kind::Plane|Kind::Grid) {axis_control(ui, 2, |ui| { ui.add(egui::Slider::new(&mut params.size.2,0.05..=10.).text("Depth")); });}
                    }
                    match kind {
                        Kind::Cube=>{
                            for (axis,label) in ["X spans","Y spans","Z spans"].into_iter().enumerate() {axis_control(ui, axis, |ui| { resolution_control(ui,label,&mut params.divisions[axis],1,128); });}
                            ui.weak("Each face has spans + 1 vertices along its axes. Edges and corners are shared.");
                        }
                        Kind::Plane|Kind::Grid=>{
                            axis_control(ui, 0, |ui| { resolution_control(ui,"X spans",&mut params.divisions[0],1,128); });
                            axis_control(ui, 1, |ui| { resolution_control(ui,"Y spans",&mut params.divisions[1],1,128); });
                        }
                        Kind::UvSphere=>{
                            resolution_control(ui,"Segments",&mut params.segments,3,256);
                            resolution_control(ui,"Rings",&mut params.rings,2,128);
                        }
                        Kind::Icosphere=>{resolution_control(ui,"Subdivisions",&mut params.subdivisions,1,6);}
                        Kind::Circle=>{
                            ui.add(egui::Slider::new(&mut params.radius,0.05..=10.).text("Radius"));
                            resolution_control(ui,"Vertices",&mut params.segments,3,256);
                            ui.checkbox(&mut params.filled,"Fill circle");
                        }
                        Kind::Cylinder|Kind::Cone=>{
                            ui.add(egui::Slider::new(&mut params.radius,0.05..=10.).text(if kind==Kind::Cone {"Base radius"} else {"Radius"}));
                            if kind==Kind::Cone {ui.add(egui::Slider::new(&mut params.top_radius,0. ..=10.).text("Top radius"));}
                            axis_control(ui, 1, |ui| { ui.add(egui::Slider::new(&mut params.size.1,0.05..=10.).text("Height")); });
                            resolution_control(ui,"Sides",&mut params.segments,3,256);
                            axis_control(ui, 1, |ui| { resolution_control(ui,"Height spans",&mut params.divisions[1],1,128); });
                            ui.checkbox(&mut params.filled,"Cap ends");
                        }
                        Kind::Torus=>{
                            ui.add(egui::Slider::new(&mut params.radius,0.05..=10.).text("Major radius"));
                            params.tube_radius=params.tube_radius.min(params.radius-0.01);
                            ui.add(egui::Slider::new(&mut params.tube_radius,0.01..=(params.radius-0.01).min(5.)).text("Tube radius"));
                            resolution_control(ui,"Major segments",&mut params.segments,3,256);
                            resolution_control(ui,"Tube segments",&mut params.rings,3,128);
                        }
                    }

                    ui.separator();
                    ui.checkbox(&mut params.triangulated,"Triangle topology");
                    ui.checkbox(&mut self.wire,"Show wireframe");
                    ui.label(format!("{} vertices · {} triangles",document.mesh().vertices.len(),document.mesh().triangles.len()));
                    if params!=*current {commands.push(Command::UpdatePrimitive(params.clone()));}
                    if ui.add_enabled(!document.mesh().triangles.is_empty(),egui::Button::new("Convert to sculpting mesh")).customize(Action::ConvertPrimitive).clicked() {commands.push(Command::ConvertPrimitive);}
                    ui.weak("Conversion keeps this surface and removes its editable recipe. Undo restores the recipe.");
        });
        if forced.is_some() {
            response.header_response.scroll_to_me(None);
        }
        self.primitive_controls_requested = false;
        if !ui.input(|i| i.pointer.primary_down()) {
            commands.push(Command::EndPrimitiveEdit);
        }
    }
}

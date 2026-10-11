//! Primitive replacement warning and non-destructive parameter editor.
use super::UiState;
use crate::{
    command::Command,
    document::Document,
    sculpt::{Primitive, PrimitiveKind as Kind},
};
use eframe::egui;
impl UiState {
    pub(crate) fn primitive_resolution_toolbar(
        &mut self,
        ctx: &egui::Context,
        viewport: egui::Rect,
        document: &Document,
        enabled: bool,
    ) -> (Vec<Command>, Option<egui::Rect>) {
        let Some(current) = document.primitive() else {
            return (vec![], None);
        };
        let mut params = current.clone();
        let mut commands = vec![];
        let y = if self.transform_mode == crate::viewport::TransformMode::Sculpt {
            12.
        } else {
            150.
        };
        let response = egui::Area::new(egui::Id::new("primitive_resolution"))
            .order(egui::Order::Foreground)
            .default_size(egui::vec2(260., 210.))
            .fixed_pos(viewport.left_top() + egui::vec2(12., y))
            .constrain_to(viewport)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(240.);
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.strong(format!("Resolution · {}", params.kind.label()));
                        let mut row = |label: &str, value: &mut u32, min: u32, max: u32| {
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
                        };
                        match params.kind {
                            Kind::Cube => {
                                for (axis, label) in
                                    ["X spans", "Y spans", "Z spans"].into_iter().enumerate()
                                {
                                    row(label, &mut params.divisions[axis], 1, 128);
                                }
                            }
                            Kind::Plane | Kind::Grid => {
                                row("X spans", &mut params.divisions[0], 1, 128);
                                row("Y spans", &mut params.divisions[1], 1, 128);
                            }
                            Kind::UvSphere | Kind::HumanHead => {
                                row("Segments", &mut params.segments, 3, 256);
                                row("Rings", &mut params.rings, 2, 128);
                            }
                            Kind::Icosphere => row("Subdivisions", &mut params.subdivisions, 1, 6),
                            Kind::Circle => row("Vertices", &mut params.segments, 3, 256),
                            Kind::Cylinder | Kind::Cone => {
                                row("Sides", &mut params.segments, 3, 256);
                                row("Height spans", &mut params.divisions[1], 1, 128);
                            }
                            Kind::Torus => {
                                row("Major segments", &mut params.segments, 3, 256);
                                row("Tube segments", &mut params.rings, 3, 128);
                            }
                        }
                        ui.weak("Drag a number, type it, or use - / +.");
                        if params.kind == Kind::HumanHead {
                            ui.separator();
                            ui.strong("Face pose");
                            if params.head.anatomy_version < 2
                                && ui.button("Reset head proportions").clicked()
                            {
                                params.head = Default::default();
                                params.size = Primitive::for_kind(Kind::HumanHead).size;
                            }
                            let h = &mut params.head;
                            ui.add(
                                egui::Slider::new(&mut h.mouth_open, 0. ..=2.).text("Open mouth"),
                            );
                            ui.horizontal(|ui| {
                                if ui.small_button("Close").clicked() {
                                    h.mouth_open = 0.;
                                }
                                if ui.small_button("Open").clicked() {
                                    h.mouth_open = 1.;
                                }
                            });
                            if h.anatomy_version > 0 {
                                ui.add(
                                    egui::Slider::new(&mut h.upper_lid, 0. ..=1.).text("Upper lid"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut h.lower_lid, 0. ..=1.).text("Lower lid"),
                                );
                                ui.weak("Lids: 0 = open, 1 = closed.");
                            }
                        }
                        ui.checkbox(&mut params.triangulated, "Triangle topology");
                        ui.checkbox(&mut self.wire, "Wireframe");
                        ui.weak(format!(
                            "{} vertices · {} triangles",
                            document.mesh().vertices.len(),
                            document.mesh().triangles.len()
                        ));
                        if params != *current {
                            commands.push(Command::UpdatePrimitive(params.clone()));
                        }
                        ui.horizontal(|ui| {
                            if ui.small_button("Shape options").clicked() {
                                commands.push(Command::RequestPrimitive);
                            }
                            if ui
                                .add_enabled(
                                    !document.mesh().triangles.is_empty(),
                                    egui::Button::new("Convert to sculpt").small(),
                                )
                                .clicked()
                            {
                                commands.push(Command::ConvertPrimitive);
                            }
                        });
                    });
                });
            });
        if !ctx.input(|i| i.pointer.primary_down()) {
            commands.push(Command::EndPrimitiveEdit);
        }
        (commands, Some(response.response.rect))
    }
    pub(crate) fn primitive_popup(
        &mut self,
        ctx: &egui::Context,
        viewport: egui::Rect,
        document: &Document,
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
        let Some(current) = document.primitive() else {
            return (commands, None);
        };
        if !self.primitive_editor_open {
            return (commands, None);
        }
        let mut params = current.clone();
        let mut open = true;
        let response=egui::Window::new("Primitive shape")
            .id(egui::Id::new("primitive_editor"))
            .open(&mut open).default_width(330.)
            .default_pos(egui::pos2((viewport.right()-360.).max(viewport.left()+12.),viewport.top()+12.))
            .show(ctx,|ui| {
                egui::ScrollArea::vertical().max_height((viewport.height()-70.).max(200.)).show(ui,|ui| {
                    ui.label("Drag shape handles in the viewport, or use these precise controls. Convert when ready to sculpt.");
                    let mut kind=params.kind;
                    egui::ComboBox::from_id_salt("primitive_shape").selected_text(kind.label()).show_ui(ui,|ui| {
                        for option in Kind::ALL {ui.selectable_value(&mut kind,option,option.label());}
                    });
                    if kind!=params.kind { let mut replacement=Primitive::for_kind(kind);replacement.center=params.center;replacement.basis=params.basis;params=replacement; }
                    ui.separator();
                    if matches!(kind,Kind::Cube|Kind::Plane|Kind::Grid|Kind::UvSphere|Kind::Icosphere|Kind::HumanHead) {
                        ui.add(egui::Slider::new(&mut params.size.0,0.05..=10.).text("Width"));
                        ui.add(egui::Slider::new(&mut params.size.1,0.05..=10.).text("Height"));
                        if !matches!(kind,Kind::Plane|Kind::Grid) {ui.add(egui::Slider::new(&mut params.size.2,0.05..=10.).text("Depth"));}
                    }
                    match kind {
                        Kind::Cube=>{
                            for (axis,label) in ["X spans","Y spans","Z spans"].into_iter().enumerate() {ui.add(egui::Slider::new(&mut params.divisions[axis],1..=128).text(label));}
                            ui.weak("Each face has spans + 1 vertices along its axes. Edges and corners are shared.");
                        }
                        Kind::Plane|Kind::Grid=>{
                            ui.add(egui::Slider::new(&mut params.divisions[0],1..=128).text("X spans"));
                            ui.add(egui::Slider::new(&mut params.divisions[1],1..=128).text("Y spans"));
                        }
                        Kind::UvSphere|Kind::HumanHead=>{
                            ui.add(egui::Slider::new(&mut params.segments,3..=256).text("Segments"));
                            ui.add(egui::Slider::new(&mut params.rings,2..=128).text("Rings"));
                        }
                        Kind::Icosphere=>{ui.add(egui::Slider::new(&mut params.subdivisions,1..=6).text("Subdivisions"));}
                        Kind::Circle=>{
                            ui.add(egui::Slider::new(&mut params.radius,0.05..=10.).text("Radius"));
                            ui.add(egui::Slider::new(&mut params.segments,3..=256).text("Vertices"));
                            ui.checkbox(&mut params.filled,"Fill circle");
                        }
                        Kind::Cylinder|Kind::Cone=>{
                            ui.add(egui::Slider::new(&mut params.radius,0.05..=10.).text(if kind==Kind::Cone {"Base radius"} else {"Radius"}));
                            if kind==Kind::Cone {ui.add(egui::Slider::new(&mut params.top_radius,0. ..=10.).text("Top radius"));}
                            ui.add(egui::Slider::new(&mut params.size.1,0.05..=10.).text("Height"));
                            ui.add(egui::Slider::new(&mut params.segments,3..=256).text("Sides"));
                            ui.add(egui::Slider::new(&mut params.divisions[1],1..=128).text("Height spans"));
                            ui.checkbox(&mut params.filled,"Cap ends");
                        }
                        Kind::Torus=>{
                            ui.add(egui::Slider::new(&mut params.radius,0.05..=10.).text("Major radius"));
                            params.tube_radius=params.tube_radius.min(params.radius-0.01);
                            ui.add(egui::Slider::new(&mut params.tube_radius,0.01..=(params.radius-0.01).min(5.)).text("Tube radius"));
                            ui.add(egui::Slider::new(&mut params.segments,3..=256).text("Major segments"));
                            ui.add(egui::Slider::new(&mut params.rings,3..=128).text("Tube segments"));
                        }
                    }
                    if kind==Kind::HumanHead {
                        ui.separator();ui.strong("Face proportions");
                        if params.head.anatomy_version<2 { ui.weak("This saved head uses the earlier proportions."); }
                        if ui.button("Reset head proportions").on_hover_text("Use the revised anatomy, neutral facial proportions and default head dimensions; Undo restores your previous settings").clicked() { params.head=Default::default();params.size=Primitive::for_kind(Kind::HumanHead).size; }
                        let h=&mut params.head;
                        for (value,label) in [(&mut h.jaw_width,"Jaw width"),(&mut h.jaw_height,"Jaw length"),(&mut h.chin,"Chin projection"),(&mut h.cheeks,"Cheekbones"),(&mut h.nose_length,"Nose projection"),(&mut h.nose_width,"Nose width"),(&mut h.eye_spacing,"Eye spacing"),(&mut h.eye_depth,"Eye sockets"),(&mut h.brow,"Brow ridge"),(&mut h.ears,"Ear size"),(&mut h.eye_size,"Eye size"),(&mut h.eye_height,"Eye height"),(&mut h.mouth_width,"Mouth width"),(&mut h.lip_size,"Lip fullness")] {ui.add(egui::Slider::new(value,0. ..=2.).text(label));}
                        ui.add(egui::Slider::new(&mut h.mouth_open,0. ..=2.).text("Open mouth"));
                        ui.horizontal(|ui| {
                            if ui.small_button("Close mouth").clicked() { h.mouth_open=0.; }
                            if ui.small_button("Open mouth").clicked() { h.mouth_open=1.; }
                        });
                        if h.anatomy_version>0 {
                            ui.add(egui::Slider::new(&mut h.upper_lid,0. ..=1.).text("Upper eyelid closure"));
                            ui.add(egui::Slider::new(&mut h.lower_lid,0. ..=1.).text("Lower eyelid closure"));
                            ui.horizontal(|ui| {
                                if ui.small_button("Open eyes").clicked() {h.upper_lid=0.;h.lower_lid=0.;}
                                if ui.small_button("Close eyes").clicked() {h.upper_lid=1.;h.lower_lid=1.;}
                            });
                        }
                        if h.anatomy_version>=2 {
                            ui.separator();ui.strong("Neck");ui.checkbox(&mut h.neck,"Include neck");
                            ui.add_enabled_ui(h.neck,|ui| {
                                for (value,label) in [(&mut h.neck_length,"Neck length"),(&mut h.neck_width,"Neck width"),(&mut h.neck_muscle,"Neck muscle definition"),(&mut h.throat,"Throat prominence")] {ui.add(egui::Slider::new(value,0. ..=2.).text(label));}
                            });
                        }
                        ui.weak("Eyes, teeth and tongue are separate surfaces within this submodel. Convert to sculpt their anatomy.");
                    }
                    ui.separator();
                    ui.checkbox(&mut params.triangulated,"Triangle topology");
                    ui.checkbox(&mut self.wire,"Show wireframe");
                    ui.label(format!("{} vertices · {} triangles",document.mesh().vertices.len(),document.mesh().triangles.len()));
                    if params!=*current {commands.push(Command::UpdatePrimitive(params.clone()));}
                    if ui.add_enabled(!document.mesh().triangles.is_empty(),egui::Button::new("Convert to sculpting mesh")).clicked() {commands.push(Command::ConvertPrimitive);}
                    ui.weak("Conversion keeps this surface and removes its editable recipe. Undo restores the recipe.");
                });
            });
        self.primitive_editor_open = open;
        if !ctx.input(|i| i.pointer.primary_down()) || !open {
            commands.push(Command::EndPrimitiveEdit);
        }
        (commands, response.map(|r| r.response.rect))
    }
}

use crate::sculpt::instances::*;
use eframe::egui;

pub(super) fn controls(ui: &mut egui::Ui, thickness: &mut Thickness, closed: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut thickness.mode, ThicknessMode::Uniform, "Uniform");
        ui.selectable_value(
            &mut thickness.mode,
            ThicknessMode::Taper,
            "End-to-end taper",
        );
        ui.selectable_value(
            &mut thickness.mode,
            ThicknessMode::Profile,
            "Along-curve profile",
        );
    });
    ui.weak("Width/depth multiplier · 1× = unchanged")
        .on_hover_text("Scales the cross-section around its contact point, on top of Piece scale. Section length and spacing stay unchanged.");
    match thickness.mode {
        ThicknessMode::Uniform => {
            size(ui, "Thickness", &mut thickness.uniform);
        }
        ThicknessMode::Taper => {
            size(
                ui,
                if closed { "At seam" } else { "At start" },
                &mut thickness.start,
            );
            size(
                ui,
                if closed { "Halfway round" } else { "At end" },
                &mut thickness.end,
            );
            ui.checkbox(&mut thickness.smooth, "Smooth transitions");
            if closed {
                ui.weak("Tapers to halfway round, then returns to the seam size.");
            }
        }
        ThicknessMode::Profile => {
            profile(ui, thickness, closed);
            ui.checkbox(&mut thickness.smooth, "Smooth transitions");
            if closed {
                ui.weak("The 100% endpoint shares the seam's 0% size.");
            }
        }
    }
}

fn size(ui: &mut egui::Ui, label: &str, value: &mut f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        let response = ui.add(
            egui::DragValue::new(value)
                .range(MIN_THICKNESS..=MAX_THICKNESS)
                .speed(0.01)
                .suffix("×"),
        );
        let _response = response
            .on_hover_text("Drag to adjust; double-click to enter an exact size multiplier.");
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                egui::Id::new(("path_thickness_size", label)),
                _response.rect,
            )
        });
    });
}

fn profile(ui: &mut egui::Ui, thickness: &mut Thickness, closed: bool) {
    let selected_id = ui.make_persistent_id("thickness_selected_point");
    let mut selected = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(selected_id))
        .unwrap_or(1)
        .min(thickness.points.len() - 1);
    let width = ui.available_width().clamp(80., 360.);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, 118.), egui::Sense::click_and_drag());
    let plot = rect.shrink(10.);
    let scale_id = response.id.with("vertical_scale");
    let max = thickness
        .points
        .iter()
        .map(|p| p.factor)
        .fold(2_f32, f32::max)
        * 1.15;
    if response.drag_started() {
        ui.ctx().data_mut(|d| d.insert_temp(scale_id, max));
    }
    let max = if response.dragged() {
        ui.ctx()
            .data(|d| d.get_temp::<f32>(scale_id))
            .unwrap_or(max)
    } else {
        max
    };
    let point = |index: usize, value: &Thickness| {
        let p = value.points[index];
        let factor = if closed && index + 1 == value.points.len() {
            value.points[0].factor
        } else {
            p.factor
        };
        egui::pos2(
            plot.left() + p.position * plot.width(),
            plot.bottom() - factor / max * plot.height(),
        )
    };
    if (response.clicked() || response.drag_started())
        && let Some(cursor) = if response.drag_started() {
            ui.input(|i| i.pointer.press_origin())
                .or(response.interact_pointer_pos())
        } else {
            response.interact_pointer_pos()
        }
    {
        selected = (0..thickness.points.len())
            .min_by(|&a, &b| {
                point(a, thickness)
                    .distance_sq(cursor)
                    .total_cmp(&point(b, thickness).distance_sq(cursor))
            })
            .unwrap();
    }
    if response.dragged()
        && let Some(cursor) = response.interact_pointer_pos()
    {
        let factor =
            ((plot.bottom() - cursor.y) / plot.height() * max).clamp(MIN_THICKNESS, MAX_THICKNESS);
        let index = if closed && selected + 1 == thickness.points.len() {
            0
        } else {
            selected
        };
        thickness.points[index].factor = factor;
        if selected > 0 && selected + 1 < thickness.points.len() {
            let (lo, hi) = position_range(thickness, selected);
            thickness.points[selected].position =
                ((cursor.x - plot.left()) / plot.width()).clamp(lo, hi);
        }
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4., ui.visuals().extreme_bg_color);
    painter.line_segment(
        [
            egui::pos2(plot.left(), plot.bottom() - plot.height() / max),
            egui::pos2(plot.right(), plot.bottom() - plot.height() / max),
        ],
        egui::Stroke::new(1_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
    let line = (0..=100)
        .map(|i| {
            let t = i as f32 / 100.;
            egui::pos2(
                plot.left() + t * plot.width(),
                plot.bottom() - thickness.at(t, closed) / max * plot.height(),
            )
        })
        .collect();
    painter.add(egui::Shape::line(
        line,
        egui::Stroke::new(2_f32, ui.visuals().selection.stroke.color),
    ));
    for i in 0..thickness.points.len() {
        painter.circle_filled(
            point(i, thickness),
            if i == selected { 5. } else { 3.5 },
            if i == selected {
                ui.visuals().selection.stroke.color
            } else {
                ui.visuals().text_color()
            },
        );
    }
    #[cfg(test)]
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("path_thickness_plot"), (plot, max)));
    response.on_hover_text("Distance along the path: 0% on the left, 100% on the right. Click a point to select it; drag to change its position and thickness. The horizontal line marks 1×.");
    ui.weak("0% → 100% · drag points to shape thickness");
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                thickness.points.len() < MAX_THICKNESS_POINTS,
                egui::Button::new("Add point"),
            )
            .clicked()
        {
            let i = thickness
                .points
                .windows(2)
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    (a[1].position - a[0].position).total_cmp(&(b[1].position - b[0].position))
                })
                .unwrap()
                .0;
            let position = (thickness.points[i].position + thickness.points[i + 1].position) * 0.5;
            let factor = thickness.at(position, closed);
            thickness
                .points
                .insert(i + 1, ThicknessPoint { position, factor });
            selected = i + 1;
        }
        if ui
            .add_enabled(
                selected > 0 && selected + 1 < thickness.points.len(),
                egui::Button::new("Remove point"),
            )
            .clicked()
        {
            thickness.points.remove(selected);
            selected = selected.min(thickness.points.len() - 1);
        }
    });
    ui.label(format!(
        "Point {} of {}",
        selected + 1,
        thickness.points.len()
    ));
    ui.horizontal(|ui| {
        ui.label("Position");
        let mut percent = thickness.points[selected].position * 100.;
        let editable = selected > 0 && selected + 1 < thickness.points.len();
        let (lo, hi) = if editable {
            position_range(thickness, selected)
        } else {
            (0., 1.)
        };
        if ui
            .add_enabled(
                editable,
                egui::DragValue::new(&mut percent)
                    .range(lo * 100. ..=hi * 100.)
                    .speed(0.1)
                    .suffix("%"),
            )
            .changed()
        {
            thickness.points[selected].position = percent / 100.;
        }
    });
    let index = if closed && selected + 1 == thickness.points.len() {
        0
    } else {
        selected
    };
    size(ui, "Point thickness", &mut thickness.points[index].factor);
    ui.ctx().data_mut(|d| d.insert_temp(selected_id, selected));
}

fn position_range(thickness: &Thickness, index: usize) -> (f32, f32) {
    let (a, b) = (
        thickness.points[index - 1].position,
        thickness.points[index + 1].position,
    );
    let gap = ((b - a) * 0.25).min(0.001);
    (a + gap, b - gap)
}

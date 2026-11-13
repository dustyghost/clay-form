//! Pen controls drawn over the viewport without changing the active tool.
use super::*;
use egui::{Align2, CursorIcon, FontId, Rect};

impl PathNavigation {
    fn label(self) -> &'static str {
        match self {
            Self::Orbit => "Orbit",
            Self::Pan => "Pan",
            Self::Zoom => "Zoom",
        }
    }
}
impl PathBrush {
    fn label(self) -> &'static str {
        match self {
            Self::Surface => "Surface",
            Self::Free => "Free",
            Self::Ring => "Ring",
        }
    }
}

pub(super) fn camera_targets(gizmo: Rect) -> [(PathNavigation, Rect); 3] {
    [
        PathNavigation::Orbit,
        PathNavigation::Pan,
        PathNavigation::Zoom,
    ]
    .map(|mode| {
        let index = match mode {
            PathNavigation::Orbit => 0.,
            PathNavigation::Pan => 1.,
            PathNavigation::Zoom => 2.,
        };
        let origin = gizmo.left_bottom() + Vec2::new(index * 46., 5.);
        (mode, Rect::from_min_size(origin, Vec2::new(44., 30.)))
    })
}

pub(super) fn paint_camera_targets(
    painter: &egui::Painter,
    targets: &[(PathNavigation, Rect)],
    active: Option<PathNavigation>,
    pointer: Option<Pos2>,
) {
    for &(mode, rect) in targets {
        let highlighted = active == Some(mode) || pointer.is_some_and(|p| rect.contains(p));
        painter.rect_filled(
            rect,
            5.,
            Color32::from_gray(if highlighted { 74 } else { 38 }),
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            mode.label(),
            FontId::proportional(12.),
            Color32::WHITE,
        );
    }
}

pub(super) fn paint_camera_status(
    ctx: &egui::Context,
    painter: &egui::Painter,
    rect: Rect,
    mode: PathNavigation,
) {
    ctx.set_cursor_icon(CursorIcon::Grabbing);
    let at = rect.center_top() + Vec2::new(0., 58.);
    let background = Rect::from_center_size(at, Vec2::new(130., 28.));
    painter.rect_filled(background, 5., Color32::from_gray(35));
    painter.text(
        at,
        Align2::CENTER_CENTER,
        format!("Camera · {}", mode.label()),
        FontId::proportional(14.),
        Color32::WHITE,
    );
}

impl PathEditor {
    pub(super) fn brush_hud(&mut self, ctx: &egui::Context, viewport: Rect, document: &Document) {
        let Some(mut brush) = self.brush else { return };
        let drawing = self.editable_draft();
        let can_complete = drawing
            && self
                .draft
                .as_ref()
                .is_some_and(|p| p.points.len() >= if p.closed { 3 } else { 2 });
        let saved = self.draft.as_ref().is_some_and(|p| p.id != 0);
        let gesture = self.gesture_active();
        let enabled =
            !gesture && !self.frozen && self.confirm_delete.is_none() && self.camera_drag.is_none();
        let editing = drawing || self.suspended.is_some() || self.dirty(document);
        let enter = enabled
            && can_complete
            && !ctx.wants_keyboard_input()
            && ctx.input(|input| input.key_pressed(egui::Key::Enter));
        self.commit_gesture |= enter;

        let mut new_axis = None;
        egui::Area::new(egui::Id::new("path_brush_hud"))
            .order(egui::Order::Foreground)
            .fixed_pos(viewport.left_top() + Vec2::new(24., 12.))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.spacing_mut().interact_size.y = 28.;
                    ui.set_max_width((viewport.width() - 48.).clamp(160., 540.));
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(format!("Path Brush · {}", brush.label()));
                            egui::ComboBox::from_id_salt("path_brush_mode")
                                .selected_text("Mode")
                                .show_ui(ui, |ui| {
                                    ui.add_enabled_ui(!editing, |ui| {
                                        for mode in
                                            [PathBrush::Surface, PathBrush::Free, PathBrush::Ring]
                                        {
                                            ui.selectable_value(&mut brush, mode, mode.label());
                                        }
                                    });
                                });
                            if drawing
                                && ui
                                    .add_enabled(can_complete, egui::Button::new("Complete path"))
                                    .clicked()
                            {
                                self.commit_gesture = true;
                            }
                            if drawing && ui.button("Cancel").clicked() {
                                self.clear_selection();
                            }
                            if self.suspended.is_some() && ui.button("Resume draft").clicked() {
                                self.resume_draft();
                            }
                            if ui.button("Paths…").clicked() {
                                self.panel = !self.panel;
                            }
                            if ui
                                .add_enabled(!editing, egui::Button::new("Exit"))
                                .clicked()
                            {
                                self.exit_brush = true;
                            }
                        });
                        if brush == PathBrush::Ring {
                            egui::ComboBox::from_id_salt("new_ring_axis")
                                .selected_text(format!("New ring · {:?}", self.ring_axis))
                                .show_ui(ui, |ui| {
                                    for axis in
                                        [RingAxis::Auto, RingAxis::X, RingAxis::Y, RingAxis::Z]
                                    {
                                        ui.selectable_value(
                                            &mut self.ring_axis,
                                            axis,
                                            format!("{axis:?}"),
                                        );
                                    }
                                });
                        }
                        if !(drawing || saved) || self.draft.is_none() {
                            return;
                        }
                        ui.separator();
                        if let Some(plane) = self.ring_plane {
                            ui.horizontal_wrapped(|ui| {
                                if self.ring_move {
                                    ui.label("Tap surface to move ring");
                                    if ui.button("Cancel").clicked() {
                                        self.ring_move = false;
                                    }
                                } else if ui.button("Move ring").clicked() {
                                    self.ring_move = true;
                                }
                                egui::ComboBox::from_id_salt("ring_orientation")
                                    .selected_text(format!("{:?}", plane.orientation))
                                    .show_ui(ui, |ui| {
                                        for axis in
                                            [RingAxis::Auto, RingAxis::X, RingAxis::Y, RingAxis::Z]
                                        {
                                            if ui
                                                .selectable_label(
                                                    plane.orientation == axis,
                                                    format!("{axis:?}"),
                                                )
                                                .clicked()
                                            {
                                                new_axis = Some((plane.hit, axis));
                                            }
                                        }
                                    });
                            });
                        }
                        let Some(path) = self.draft.as_mut() else {
                            return;
                        };
                        ui.horizontal_wrapped(|ui| {
                            if saved {
                                ui.weak(&path.name);
                            } else {
                                ui.weak(format!("{} points", path.points.len()));
                            }
                            if path.ring_orientation.is_none() {
                                let smooth = path.smoothing == 1. && path.corner_rounding.is_empty();
                                if ui.selectable_label(smooth, "Smooth")
                                    .on_hover_text("Curve the whole path. Replaces individual corner rounding; Undo restores it.")
                                    .clicked()
                                {
                                    path.smoothing = if smooth { 0. } else { 1. };
                                    path.corner_rounding.clear();
                                    self.commit_gesture |= saved;
                                }
                                let label = if path.closed { "Open loop" } else { "Close loop" };
                                if ui.add_enabled(path.closed || path.points.len() >= 3, egui::Button::new(label))
                                    .on_disabled_hover_text("Place at least three points to close the loop")
                                    .clicked()
                                {
                                    match path.set_closed(!path.closed) {
                                        Ok(()) => {
                                            self.selected = None;
                                            self.commit_gesture |= saved;
                                        }
                                        Err(error) => self.notice = Some(error),
                                    }
                                }
                            }
                            if path.mode == PathMode::Surface {
                                let edit = ui.add(
                                    egui::DragValue::new(&mut path.offset)
                                        .speed(0.005)
                                        .range(-1000. ..=1000.)
                                        .prefix("Offset "),
                                );
                                self.commit_gesture |= saved && edit_finished(&edit);
                            }
                            if saved && ui.button("Delete…").clicked() {
                                self.confirm_delete = Some(path.id);
                            }
                            if path.ring_orientation.is_none() {
                                if saved
                                    && ui
                                        .selectable_label(
                                            self.insert_on_path,
                                            if self.insert_on_path {
                                                "Tap guide to insert"
                                            } else {
                                                "Insert point"
                                            },
                                        )
                                        .clicked()
                                {
                                    self.insert_on_path = !self.insert_on_path;
                                }
                                let minimum = if path.closed { 3 } else { 2 };
                                if let Some(index) = self.selected
                                    && ui
                                        .add_enabled(
                                            path.points.len() > minimum,
                                            egui::Button::new("Remove point"),
                                        )
                                        .clicked()
                                {
                                    match path.remove_point(index) {
                                        Ok(()) => {
                                            self.selected = None;
                                            self.commit_gesture |= saved;
                                        }
                                        Err(error) => self.notice = Some(error),
                                    }
                                }
                                if saved
                                    && path.closed
                                    && path.mode == PathMode::Surface
                                    && ui.button("Fit as ring").clicked()
                                {
                                    new_axis = Some((path.points[path.seam], self.ring_axis));
                                }
                            }
                        });
                        if path.mode == PathMode::Free
                            && let Some(point) = self.selected.and_then(|i| path.points.get_mut(i))
                        {
                            ui.horizontal(|ui| {
                                for (label, value) in [
                                    ("X ", &mut point.position.0),
                                    ("Y ", &mut point.position.1),
                                    ("Z ", &mut point.position.2),
                                ] {
                                    let edit = ui
                                        .add(egui::DragValue::new(value).speed(0.01).prefix(label));
                                    self.commit_gesture |= saved && edit_finished(&edit);
                                }
                            });
                        }
                    });
                });
            });

        if self.brush != Some(brush) && !editing {
            self.brush = Some(brush);
            self.ring_move = false;
            self.tool = PathTool::Add;
        }
        if let Some((hit, axis)) = new_axis {
            let previous = self.ring_plane;
            self.update_ring(document, hit, axis, None);
            self.commit_gesture |= saved && previous != self.ring_plane;
        }
    }
}

fn edit_finished(response: &egui::Response) -> bool {
    response.drag_stopped()
        || response.lost_focus()
        || (response.changed() && !response.dragged() && !response.has_focus())
}

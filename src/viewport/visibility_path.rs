//! Editable closed visibility regions. Drafts never alter mesh topology.
use super::visibility::{LassoRegion, paint_visibility_faces};
use super::{Gesture, SelectionTool, Viewport, scene::Scene};
use crate::{
    document::Document,
    preferences::{Action, VisibilitySettings, VisibilityShape},
    sculpt::Vec3,
};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

pub(super) struct VisibilityPath {
    points: Vec<Pos2>,
    selected: Option<usize>,
    dragging: bool,
    revision: u64,
    projection: (f32, Pos2, Vec3, Vec3, f32),
}

impl Viewport {
    pub(crate) fn visibility_path_action(&mut self, action: Action) {
        self.visibility_path_action = Some(action);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_visibility_path(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        document: &mut Document,
        gizmo: Rect,
        enabled: bool,
        select: Option<SelectionTool>,
        navigate: bool,
    ) -> bool {
        let Some(SelectionTool::Visibility(VisibilitySettings {
            shape: VisibilityShape::Path,
            hide,
            flip,
        })) = select
        else {
            return false;
        };
        let projection = (
            scene.scale,
            scene.center,
            scene.right,
            scene.up,
            scene.eye_distance,
        );
        let interrupted = !enabled
            || egui::Popup::is_any_open(ui.ctx())
            || ui.input(|i| {
                !i.focused
                    || i.key_pressed(egui::Key::Escape)
                    || i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::PointerGone))
            })
            || self.visibility_path.as_ref().is_some_and(|p| {
                p.revision != document.display_revision() || p.projection != projection
            });
        if interrupted {
            let had_path = self.visibility_path.take().is_some();
            self.visibility_path_action = None;
            if self.gesture == Some(Gesture::Select) {
                self.gesture = None;
            }
            if had_path {
                self.wait_for_release();
            }
            return had_path;
        }
        let mut action = self.visibility_path_action.take();
        let panel = egui::Area::new(egui::Id::new("visibility_path_editor"))
            .order(egui::Order::Foreground)
            .fixed_pos(scene.rect.left_top() + Vec2::new(12., 40.))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(240.);
                    ui.strong("Hide/Isolate · closed path");
                    ui.label("Click to add · drag points to edit");
                    ui.label("Click an edge to insert a point");
                    ui.label("Red faces will be hidden");
                    let count = self.visibility_path.as_ref().map_or(0, |p| p.points.len());
                    ui.label(format!("{count} / 64 points · closes last to first"));
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(count >= 3, egui::Button::new("Apply visibility"))
                            .clicked()
                        {
                            action = Some(Action::ApplyVisibilityPath);
                        }
                        if ui.button("Clear path").clicked() {
                            action = Some(Action::ClearVisibilityPath);
                        }
                    });
                    if ui
                        .add_enabled(
                            self.visibility_path
                                .as_ref()
                                .is_some_and(|p| p.selected.is_some()),
                            egui::Button::new("Delete point"),
                        )
                        .clicked()
                    {
                        action = Some(Action::DeleteVisibilityPoint);
                    }
                });
            })
            .response
            .rect;
        if matches!(action, Some(Action::ClearVisibilityPath)) {
            self.visibility_path = None;
            self.gesture = None;
            self.wait_for_release();
            return true;
        }
        let press = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some(*pos),
                _ => None,
            })
        });
        let editor_press = press.is_some_and(|p| panel.contains(p));
        let press = press.filter(|p| {
            response.rect.contains(*p)
                && !gizmo.contains(*p)
                && !panel.contains(*p)
                && ui.ctx().layer_id_at(*p) == Some(response.layer_id)
        });
        if let Some(pos) = press
            && !navigate
            && self.gesture.is_none()
            && document.active_visible()
            && document.primitive().is_none()
        {
            let path = self.visibility_path.get_or_insert_with(|| VisibilityPath {
                points: Vec::new(),
                selected: None,
                dragging: false,
                revision: document.display_revision(),
                projection,
            });
            path.dragging = true;
            self.gesture = Some(Gesture::Select);
            if let Some(i) = path.points.iter().position(|p| p.distance(pos) <= 12.) {
                path.selected = Some(i);
            } else if path.points.len() < 64 {
                let segment = (0..path.points.len()).find(|&i| {
                    let a = path.points[i];
                    let b = path.points[(i + 1) % path.points.len()];
                    let delta = b - a;
                    let t = ((pos - a).dot(delta) / delta.length_sq()).clamp(0., 1.);
                    pos.distance(a + delta * t) <= 8.
                });
                let i = segment.map_or(path.points.len(), |i| i + 1);
                path.points.insert(i, pos);
                path.selected = Some(i);
            } else {
                path.dragging = false;
                self.gesture = None;
                self.status = Some("Visibility paths are limited to 64 points".into());
            }
        }
        if let Some(path) = &mut self.visibility_path {
            if path.dragging
                && press.is_none()
                && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
            {
                if let Some(i) = path.selected {
                    path.points[i] = pos;
                }
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                path.dragging = false;
                if self.gesture == Some(Gesture::Select) {
                    self.gesture = None;
                }
            }
            if matches!(action, Some(Action::DeleteVisibilityPoint))
                && let Some(i) = path.selected.take()
            {
                path.points.remove(i);
                path.dragging = false;
            }
            let region = LassoRegion::new(path.points.clone());
            if let Some(region) = &region {
                paint_visibility_faces(
                    painter,
                    scene,
                    document,
                    |p| region.contains(p) != flip,
                    hide,
                );
            }
            let color = Color32::from_rgb(110, 220, 210);
            for pair in path.points.windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(2_f32, color));
            }
            if path.points.len() > 2 {
                painter.line_segment(
                    [*path.points.last().unwrap(), path.points[0]],
                    Stroke::new(2_f32, color),
                );
            }
            for (i, &pos) in path.points.iter().enumerate() {
                painter.circle_filled(pos, if path.selected == Some(i) { 6. } else { 4. }, color);
            }
            if matches!(action, Some(Action::ApplyVisibilityPath)) {
                if let Some(region) = region {
                    let result = document.select_face_region(
                        |p| scene.visible_depth(p) && (region.contains(scene.project(p)) != flip),
                        hide,
                    );
                    self.status = Some(
                        result
                            .err()
                            .unwrap_or_else(|| "Visibility path applied".into()),
                    );
                    self.visibility_path = None;
                    self.gesture = None;
                    self.wait_for_release();
                } else {
                    self.status = Some("Add at least three points enclosing an area".into());
                }
                return true;
            }
        }
        // Let camera navigation run between edits; the changed projection then
        // invalidates this view-space draft on the following frame.
        self.gesture == Some(Gesture::Select) || press.is_some() || editor_press
    }
}

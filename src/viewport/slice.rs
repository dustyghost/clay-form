//! A view-aligned plane gesture. Geometry is only committed by a mesh worker.
use super::{Gesture, Viewport, controls, scene::Scene};
use crate::{
    document::Document,
    sculpt::{SliceMode, SliceOptions, SlicePlane, Vec3, path_geometry},
};
use eframe::egui::{self, Color32, Pos2, Vec2};

pub(super) struct SliceDrag {
    last: Pos2,
    start: Pos2,
    end: Pos2,
    revision: u64,
    options: SliceOptions,
    projection: (Vec3, Vec3, Pos2, f32, f32),
}
impl SliceDrag {
    fn plane(&self, scene: &Scene) -> Option<SlicePlane> {
        let delta = self.end - self.start;
        if delta.length() < 6. {
            return None;
        }
        let origin = scene.at_depth(self.start, Vec3::default());
        let end = scene.at_depth(self.end, Vec3::default());
        let depth = if scene.eye_distance > 0. {
            scene.view * scene.eye_distance - origin
        } else {
            scene.view
        };
        Some(SlicePlane {
            origin,
            normal: (end - origin).cross(depth).norm(),
        })
    }
    fn paint(&self, painter: &egui::Painter, scene: &Scene) {
        let direction = (self.end - self.start).normalized();
        let color = controls::ACTION;
        let reach = scene.rect.width() + scene.rect.height();
        painter.line_segment(
            [self.start - direction * reach, self.end + direction * reach],
            egui::Stroke::new(1_f32, color.gamma_multiply(0.45)),
        );
        painter.line_segment([self.start, self.end], egui::Stroke::new(2_f32, color));
        for p in [self.start, self.end] {
            painter.circle_filled(p, 4., color);
        }
        if direction.length_sq() == 0. {
            return;
        }
        let center = self.start + (self.end - self.start) * 0.5;
        if self.options.mode == SliceMode::Trim {
            let keep =
                Vec2::new(-direction.y, direction.x) * if self.options.flip { -1. } else { 1. };
            painter.arrow(
                center,
                keep * 32.,
                egui::Stroke::new(2_f32, Color32::LIGHT_GREEN),
            );
            controls::label(painter, center + keep * 42., "Keep", Color32::LIGHT_GREEN);
            controls::label(painter, center - keep * 42., "Remove", Color32::LIGHT_RED);
        } else {
            controls::label(
                painter,
                center + Vec2::new(10., 10.),
                self.options.mode.label(),
                color,
            );
        }
    }
}
impl Viewport {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_slice(
        &mut self,
        ui: &egui::Ui,
        scene: &Scene,
        painter: &egui::Painter,
        document: &Document,
        pointer: Option<Pos2>,
        enabled: bool,
        options: SliceOptions,
        path_mode: bool,
        clicked: bool,
    ) {
        if path_mode {
            self.session.slice = None;
            self.update_slice_path(
                ui, scene, painter, document, pointer, enabled, options, clicked,
            );
            return;
        }
        self.session.slice_path = None;
        if !enabled
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
            || self.session.slice.as_ref().is_some_and(|s| {
                s.revision != document.display_revision()
                    || s.options != options
                    || s.projection
                        != (
                            scene.right,
                            scene.up,
                            scene.center,
                            scene.scale,
                            scene.eye_distance,
                        )
            })
        {
            self.session.slice = None;
            if self.session.gesture == Some(Gesture::Slice) {
                self.session.gesture = None;
                self.wait_for_release();
            }
            return;
        }
        if self.session.gesture == Some(Gesture::Slice)
            && let Some(point) = pointer
        {
            let drag = self.session.slice.get_or_insert(SliceDrag {
                last: point,
                start: point,
                end: point,
                revision: document.display_revision(),
                options,
                projection: (
                    scene.right,
                    scene.up,
                    scene.center,
                    scene.scale,
                    scene.eye_distance,
                ),
            });
            let delta = point - drag.last;
            if ui.input(|i| i.key_down(egui::Key::Space)) {
                drag.start += delta;
            }
            drag.end += delta;
            drag.last = point;
        }
        if let Some(drag) = &mut self.session.slice {
            if let Some(point) = ui.input(|i| i.pointer.interact_pos()) {
                let delta = point - drag.last;
                if ui.input(|i| i.key_down(egui::Key::Space)) {
                    drag.start += delta;
                }
                drag.end += delta;
                drag.last = point;
            }
            drag.paint(painter, scene);
            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                self.completed_slice = drag
                    .plane(scene)
                    .map(|plane| crate::mesh_job::Operation::Slice(plane, drag.options));
                self.session.slice = None;
            } else if !ui.input(|i| i.pointer.primary_down()) {
                self.session.slice = None;
            }
        }
        painter.text(
            scene.rect.left_bottom() + Vec2::new(16., -16.),
            egui::Align2::LEFT_BOTTOM,
            "Slice · drag across the model · Space-drag moves line · release to apply · Esc cancels and exits · right drag to orbit",
            egui::FontId::proportional(13.),
            controls::NEUTRAL,
        );
    }
}

#[derive(Clone, Copy)]
struct PathPoint {
    position: Pos2,
    rounding: f32,
}
#[derive(Clone, Copy)]
struct CornerHandle {
    point: usize,
    position: Pos2,
    axis: Vec2,
    scale: f32,
    diamond: bool,
}
pub(super) struct SlicePathDraft {
    points: Vec<PathPoint>,
    selected: Option<usize>,
    dragging: bool,
    corner_drag: Option<CornerHandle>,
    last: Pos2,
    revision: u64,
    projection: (Vec3, Vec3, Pos2, f32, f32),
}
impl SlicePathDraft {
    fn corner_limit(&self, i: usize) -> f32 {
        if i == 0 || i + 1 >= self.points.len() {
            return 0.;
        }
        let point = |index: usize| {
            let p = self.points[index].position;
            Vec3(p.x, p.y, 0.)
        };
        path_geometry::corner_limit(point(i - 1), point(i), point(i + 1))
    }
    fn corner_handles(&self) -> Vec<CornerHandle> {
        let Some(i) = self.selected else {
            return Vec::new();
        };
        let limit = self.corner_limit(i);
        if limit < 1. {
            return Vec::new();
        }
        let p = self.points[i].position;
        let incoming = (self.points[i - 1].position - p).normalized();
        let outgoing = (self.points[i + 1].position - p).normalized();
        let sum = incoming + outgoing;
        // At a straight point there is no bend to round.
        if sum.length() < 0.01 {
            return Vec::new();
        }
        let radius = self.points[i].rounding.min(limit);
        let axis = sum.normalized();
        let mut handles = vec![CornerHandle {
            point: i,
            position: p + axis * (22. + radius * 0.5),
            axis,
            scale: 0.5,
            diamond: true,
        }];
        if radius >= 18. {
            for axis in [incoming, outgoing] {
                handles.push(CornerHandle {
                    point: i,
                    position: p + axis * radius,
                    axis,
                    scale: 1.,
                    diamond: false,
                });
            }
        }
        handles
    }
    fn corner_at(&self, position: Pos2) -> Option<CornerHandle> {
        self.corner_handles()
            .into_iter()
            .filter(|h| h.position.distance(position) < 9.)
            .min_by(|a, b| {
                a.position
                    .distance(position)
                    .total_cmp(&b.position.distance(position))
            })
    }
    fn drag_corner(&mut self, handle: CornerHandle, delta: Vec2) {
        let limit = self.corner_limit(handle.point);
        let point = &mut self.points[handle.point];
        point.rounding =
            (point.rounding.min(limit) + delta.dot(handle.axis) / handle.scale).clamp(0., limit);
    }
    fn sampled(&self) -> Vec<Pos2> {
        let mut out = Vec::new();
        for (i, p) in self.points.iter().enumerate() {
            if i == 0 || i + 1 == self.points.len() || p.rounding < 0.5 {
                out.push(p.position);
                continue;
            }
            let distance = p.rounding.min(self.corner_limit(i));
            let steps = ((distance.sqrt() * 1.5).ceil() as usize).clamp(2, 8);
            let point = |position: Pos2| Vec3(position.x, position.y, 0.);
            let Some(samples) = path_geometry::rounded_corner(
                point(self.points[i - 1].position),
                point(p.position),
                point(self.points[i + 1].position),
                distance,
                steps,
            ) else {
                if out
                    .last()
                    .is_none_or(|last: &Pos2| last.distance(p.position) > 0.01)
                {
                    out.push(p.position);
                }
                continue;
            };
            for sample in samples {
                let v = Pos2::new(sample.position.0, sample.position.1);
                if out.last().is_none_or(|last: &Pos2| last.distance(v) > 0.01) {
                    out.push(v);
                }
            }
        }
        out
    }
    fn cut(&self, scene: &Scene) -> Option<crate::sculpt::SlicePath> {
        let sampled = self.sampled();
        if sampled.len() < 2 {
            return None;
        }
        Some(crate::sculpt::SlicePath {
            points: sampled
                .iter()
                .map(|p| {
                    scene.right * ((p.x - scene.center.x) / scene.scale)
                        + scene.up * ((scene.center.y - p.y) / scene.scale)
                })
                .collect(),
            depth: scene.right.cross(scene.up).norm(),
            eye: (scene.eye_distance > 0.).then_some(scene.view * scene.eye_distance),
        })
    }
    fn paint(&self, painter: &egui::Painter, scene: &Scene, options: SliceOptions) {
        let sampled = self.sampled();
        let stroke = egui::Stroke::new(2_f32, controls::ACTION);
        for pair in sampled.windows(2) {
            painter.line_segment([pair[0], pair[1]], stroke);
        }
        if sampled.len() > 1 {
            let n = sampled.len();
            let reach = scene.rect.width() + scene.rect.height();
            for (end, neighbor) in [(sampled[0], sampled[1]), (sampled[n - 1], sampled[n - 2])] {
                painter.line_segment(
                    [end, end + (end - neighbor).normalized() * reach],
                    egui::Stroke::new(1_f32, controls::ACTION.gamma_multiply(0.45)),
                );
            }
            if options.mode == SliceMode::Trim {
                let a = sampled[0];
                let b = sampled[1];
                let d = (b - a).normalized();
                let keep = Vec2::new(-d.y, d.x) * if options.flip { -1. } else { 1. };
                let center = a + (b - a) * 0.5;
                painter.arrow(
                    center,
                    keep * 28.,
                    egui::Stroke::new(2_f32, Color32::LIGHT_GREEN),
                );
                controls::label(painter, center + keep * 40., "Keep", Color32::LIGHT_GREEN);
                controls::label(painter, center - keep * 40., "Remove", Color32::LIGHT_RED);
            }
        }
        for (i, p) in self.points.iter().enumerate() {
            if p.rounding > 0. && i > 0 && i + 1 < self.points.len() {
                painter.line_segment(
                    [self.points[i - 1].position, p.position],
                    egui::Stroke::new(1_f32, controls::ACTION.gamma_multiply(0.25)),
                );
                painter.line_segment(
                    [p.position, self.points[i + 1].position],
                    egui::Stroke::new(1_f32, controls::ACTION.gamma_multiply(0.25)),
                );
            }
            painter.circle_filled(
                p.position,
                if self.selected == Some(i) { 6. } else { 4. },
                controls::ACTION,
            );
            if self.selected == Some(i) {
                painter.circle_stroke(p.position, 9., egui::Stroke::new(1_f32, Color32::WHITE));
            }
        }
        for handle in self.corner_handles() {
            let color = Color32::from_rgb(130, 215, 255);
            painter.line_segment(
                [self.points[handle.point].position, handle.position],
                egui::Stroke::new(1_f32, color.gamma_multiply(0.7)),
            );
            if handle.diamond {
                let p = handle.position;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        p + Vec2::new(0., -6.),
                        p + Vec2::new(6., 0.),
                        p + Vec2::new(0., 6.),
                        p + Vec2::new(-6., 0.),
                    ],
                    color,
                    egui::Stroke::new(1_f32, Color32::BLACK),
                ));
            } else {
                painter.circle_filled(handle.position, 5., Color32::from_rgb(25, 30, 36));
                painter.circle_stroke(handle.position, 5., egui::Stroke::new(2_f32, color));
            }
        }
    }
}
impl Viewport {
    pub(crate) fn pause_slice_path(&mut self, document: &mut Document) {
        let mut draft = self.session.slice_path.take();
        if let Some(path) = &mut draft {
            path.dragging = false;
            path.corner_drag = None;
        }
        self.cancel(document);
        self.session.slice_path = draft;
    }
    #[allow(clippy::too_many_arguments)]
    fn update_slice_path(
        &mut self,
        ui: &egui::Ui,
        scene: &Scene,
        painter: &egui::Painter,
        document: &Document,
        pointer: Option<Pos2>,
        enabled: bool,
        options: SliceOptions,
        clicked: bool,
    ) {
        if ui.input(|i| i.key_pressed(egui::Key::Escape))
            || self.session.slice_path.as_ref().is_some_and(|s| {
                s.revision != document.display_revision()
                    || s.projection
                        != (
                            scene.right,
                            scene.up,
                            scene.center,
                            scene.scale,
                            scene.eye_distance,
                        )
            })
        {
            self.session.slice_path = None;
            self.session.gesture = None;
            self.wait_for_release();
            return;
        }
        let mut apply = false;
        let mut clear = false;
        let mut remove = false;
        let editor = egui::Area::new(egui::Id::new("slice_path_editor"))
            .default_size(Vec2::new(260., 160.))
            .order(egui::Order::Foreground)
            .fixed_pos(scene.rect.left_top() + Vec2::splat(12.))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(260.);
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.strong("Slice path");
                        ui.label("Click to add · drag points to edit");
                        ui.label("Click an edge to insert a point");
                        ui.label("Space-drag moves the whole path");
                        ui.label("Shift-drag snaps to 15°");
                        ui.label("Select a corner to reveal its handles");
                        if let Some(path) = &mut self.session.slice_path {
                            ui.label(format!("{} / 24 points", path.points.len()));
                            if let Some(i) = path.selected {
                                ui.horizontal(|ui| {
                                    ui.label(format!("Point {}", i + 1));
                                    remove = ui.button("Delete point").clicked();
                                });
                                if i > 0 && i + 1 < path.points.len() {
                                    let limit = path.corner_limit(i);
                                    path.points[i].rounding = path.points[i].rounding.min(limit);
                                    ui.horizontal(|ui| {
                                        let sharp = path.points[i].rounding < 0.5;
                                        if ui.selectable_label(sharp, "Sharp").clicked() {
                                            path.points[i].rounding = 0.;
                                        }
                                        if ui.selectable_label(!sharp, "Rounded").clicked() && sharp
                                        {
                                            path.points[i].rounding = 32_f32.min(limit);
                                        }
                                    });
                                    ui.add(
                                        egui::Slider::new(
                                            &mut path.points[i].rounding,
                                            0. ..=limit,
                                        )
                                        .text("Round (px)"),
                                    );
                                    ui.label("Drag the diamond or either round handle");
                                    let a = path.points[i].position - path.points[i - 1].position;
                                    let b = path.points[i + 1].position - path.points[i].position;
                                    let mut angle =
                                        (a.x * b.y - a.y * b.x).atan2(a.dot(b)).to_degrees();
                                    ui.horizontal(|ui| {
                                        ui.label("Turn");
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut angle)
                                                    .speed(1.)
                                                    .range(-170. ..=170.)
                                                    .suffix("°"),
                                            )
                                            .changed()
                                        {
                                            let theta = a.angle() + angle.to_radians();
                                            path.points[i + 1].position = path.points[i].position
                                                + Vec2::angled(theta) * b.length();
                                        }
                                    });
                                }
                            }
                        }
                        ui.horizontal(|ui| {
                            apply = ui
                                .add_enabled(
                                    self.session
                                        .slice_path
                                        .as_ref()
                                        .is_some_and(|p| p.points.len() > 1),
                                    egui::Button::new("Apply cut"),
                                )
                                .clicked();
                            clear = ui.button("Clear").clicked();
                        });
                    });
                });
            })
            .response
            .rect;
        let typing = ui.ctx().wants_keyboard_input();
        if clear {
            self.session.slice_path = None;
            self.session.gesture = None;
            self.wait_for_release();
        }
        if !enabled {
            if let Some(path) = &mut self.session.slice_path {
                path.dragging = false;
                path.corner_drag = None;
            }
            return;
        }
        let press = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        if press
            && (self.session.gesture == Some(Gesture::Slice) || clicked)
            && let Some(point) = pointer.filter(|p| !editor.contains(*p))
        {
            let path = self
                .session
                .slice_path
                .get_or_insert_with(|| SlicePathDraft {
                    points: Vec::new(),
                    selected: None,
                    dragging: false,
                    corner_drag: None,
                    last: point,
                    revision: document.display_revision(),
                    projection: (
                        scene.right,
                        scene.up,
                        scene.center,
                        scene.scale,
                        scene.eye_distance,
                    ),
                });
            path.last = point;
            path.dragging = true;
            path.corner_drag = if space { None } else { path.corner_at(point) };
            if !space && path.corner_drag.is_none() {
                if let Some(i) = path
                    .points
                    .iter()
                    .position(|p| p.position.distance(point) < 12.)
                {
                    path.selected = Some(i);
                } else if path.points.len() < 24 {
                    let distance = |a: Pos2, b: Pos2| {
                        let d = b - a;
                        let t = ((point - a).dot(d) / d.length_sq()).clamp(0., 1.);
                        point.distance(a + d * t)
                    };
                    let on_curve = path.sampled().windows(2).any(|p| distance(p[0], p[1]) < 8.);
                    let segment = on_curve
                        .then(|| {
                            path.points
                                .windows(2)
                                .enumerate()
                                .min_by(|(_, a), (_, b)| {
                                    distance(a[0].position, a[1].position)
                                        .total_cmp(&distance(b[0].position, b[1].position))
                                })
                                .map(|(i, _)| i)
                        })
                        .flatten();
                    let i = segment.map_or(path.points.len(), |i| i + 1);
                    path.points.insert(
                        i,
                        PathPoint {
                            position: point,
                            rounding: 0.,
                        },
                    );
                    path.selected = Some(i);
                }
            }
        }
        if let Some(path) = &mut self.session.slice_path {
            if path.dragging
                && !press
                && let Some(point) = ui.input(|i| i.pointer.interact_pos())
            {
                let delta = point - path.last;
                if space {
                    for p in &mut path.points {
                        p.position += delta;
                    }
                } else if let Some(handle) = path.corner_drag {
                    path.drag_corner(handle, delta);
                } else if let Some(i) = path.selected {
                    path.points[i].position += delta;
                    if i > 0 && ui.input(|i| i.modifiers.shift) {
                        let anchor = path.points[i - 1].position;
                        let d = path.points[i].position - anchor;
                        let step = 15_f32.to_radians();
                        path.points[i].position =
                            anchor + Vec2::angled((d.angle() / step).round() * step) * d.length();
                    }
                }
                path.last = point;
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                path.dragging = false;
                path.corner_drag = None;
            }
            if (remove
                || (!typing
                    && ui.input(|i| {
                        i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                    })))
                && let Some(i) = path.selected.take()
            {
                path.points.remove(i);
                path.corner_drag = None;
                path.dragging = false;
                path.selected = (!path.points.is_empty()).then(|| i.min(path.points.len() - 1));
            }
            path.paint(painter, scene, options);
            if pointer
                .filter(|p| !editor.contains(*p))
                .is_some_and(|p| path.corner_at(p).is_some())
                || path.corner_drag.is_some()
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            if apply || (!typing && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                self.completed_slice = path
                    .cut(scene)
                    .map(|path| crate::mesh_job::Operation::SlicePath(path, options));
            }
        }
        painter.text(
            scene.rect.left_bottom() + Vec2::new(16., -16.),
            egui::Align2::LEFT_BOTTOM,
            "Slice path · Enter applies · Esc cancels and exits · ends extend through the model",
            egui::FontId::proportional(13.),
            controls::NEUTRAL,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_corner_keeps_slice_pixel_thresholds_tessellation_and_open_ends() {
        let mut draft = SlicePathDraft {
            points: [Pos2::new(-100., 0.), Pos2::ZERO, Pos2::new(0., 100.)]
                .into_iter()
                .map(|position| PathPoint {
                    position,
                    rounding: 32.,
                })
                .collect(),
            selected: Some(1),
            dragging: false,
            corner_drag: None,
            last: Pos2::ZERO,
            revision: 0,
            projection: (Vec3::default(), Vec3::default(), Pos2::ZERO, 1., 0.),
        };
        let points = draft.sampled();
        assert_eq!(points.len(), 11); // Open endpoints plus the existing eight-step curve.
        assert_eq!(points.first(), Some(&draft.points[0].position));
        assert_eq!(points.last(), Some(&draft.points[2].position));
        assert_eq!(draft.corner_limit(0), 0.);
        assert_eq!(draft.corner_limit(1), 45.);
        assert_eq!(draft.corner_limit(2), 0.);
        for j in 0..=8 {
            let t = j as f32 / 8.;
            let legacy = Vec2::new(-32., 0.) * (1. - t).powi(2) + Vec2::new(0., 32.) * t * t;
            assert!(points[j + 1].distance(legacy.to_pos2()) < 1e-4);
        }
        draft.points[1].rounding = 0.49;
        assert_eq!(
            draft.sampled(),
            draft.points.iter().map(|p| p.position).collect::<Vec<_>>()
        );
        draft.points[1].rounding = 0.5;
        assert_eq!(draft.sampled().len(), 5); // Two-step subpixel curve, still sharp open ends.
    }
    #[test]
    fn rounded_editor_path_matches_a_closed_cut_with_interpolated_weights() {
        let draft = SlicePathDraft {
            points: vec![
                PathPoint {
                    position: Pos2::new(-2., 0.2),
                    rounding: 0.,
                },
                PathPoint {
                    position: Pos2::new(0.1, -0.4),
                    rounding: 0.8,
                },
                PathPoint {
                    position: Pos2::new(2., 0.3),
                    rounding: 0.,
                },
            ],
            selected: Some(1),
            dragging: false,
            corner_drag: None,
            last: Pos2::ZERO,
            revision: 0,
            projection: (Vec3::default(), Vec3::default(), Pos2::ZERO, 1., 0.),
        };
        // The editor stores pixels; scale up for its subpixel rounding threshold.
        let mut draft = draft;
        for p in &mut draft.points {
            p.position = (p.position.to_vec2() * 100.).to_pos2();
            p.rounding *= 100.;
        }
        let points = draft.sampled();
        assert!(points.len() > 4);
        assert!(!points.contains(&draft.points[1].position));
        let path = crate::sculpt::SlicePath {
            points: points
                .iter()
                .map(|p| Vec3(p.x / 100., p.y / 100., 0.))
                .collect(),
            depth: Vec3(0., 0., 1.),
            eye: None,
        };
        let mut source = crate::sculpt::Mesh::starting_sphere();
        for (weight, p) in source.mask.iter_mut().zip(&source.vertices) {
            *weight = (p.0 + 1.) * 0.5;
        }
        for mode in SliceMode::ALL {
            let (mesh, other) = source
                .slice_path(
                    &path,
                    SliceOptions {
                        mode,
                        ..Default::default()
                    },
                    &|| false,
                )
                .unwrap();
            for mesh in std::iter::once(&mesh).chain(other.as_ref()) {
                mesh.validate_closed().unwrap();
                // Surface intersections preserve a linear source weight field.
                for (i, face) in mesh.triangles.iter().enumerate() {
                    let group = mesh.polygroups[i];
                    if group > 0 && mesh.vertex_groups[group - 1].name.starts_with("Cut") {
                        continue;
                    }
                    for &v in face {
                        assert!((mesh.mask[v] - (mesh.vertices[v].0 + 1.) * 0.5).abs() < 1e-5);
                    }
                }
            }
        }
    }
}

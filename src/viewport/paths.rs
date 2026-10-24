//! Session-only path drafts. Navigation never edits the draft or document.
use super::{ViewportOptions, controls, navigation, scene::Scene};
use crate::{
    camera::Camera,
    document::Document,
    sculpt::{
        Vec3,
        paths::{PathMode, PathPoint, SurfacePath},
    },
};
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};

#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum PathTool {
    #[default]
    Add,
    Move,
    Insert,
    Ring,
}
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum RingMode {
    #[default]
    Level,
    Shortest,
}
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum RingAxis {
    X,
    #[default]
    Y,
    Z,
}
impl RingAxis {
    fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3(1., 0., 0.),
            Self::Y => Vec3(0., 1., 0.),
            Self::Z => Vec3(0., 0., 1.),
        }
    }
}

struct GuideCache {
    draft: SurfacePath,
    revision: u64,
    points: Vec<PathPoint>,
    warning: Option<String>,
}

#[derive(Clone, Copy)]
struct CornerHandle {
    index: usize,
    position: Pos2,
    axis: Vec2,
    pixels_per_amount: f32,
    diamond: bool,
}
struct CornerDrag {
    handle: CornerHandle,
    before: SurfacePath,
    start: Pos2,
    amount: f32,
}

#[derive(Default)]
pub(crate) struct PathEditor {
    pub active: bool,
    pub frozen: bool,
    pub draft: Option<SurfacePath>,
    pub selected: Option<usize>,
    pub tool: PathTool,
    pub ring_mode: RingMode,
    pub ring_axis: RingAxis,
    pub notice: Option<String>,
    drag: Option<(usize, SurfacePath)>,
    corner_drag: Option<CornerDrag>,
    ring_drag: Option<SurfacePath>,
    ring_last: Option<Pos2>,
    drawing: bool,
    navigating: bool,
    last_draw: Option<Pos2>,
    guide: Option<GuideCache>,
}
impl PathEditor {
    pub(crate) fn select(&mut self, path: SurfacePath) {
        self.draft = Some(path);
        self.selected = None;
        self.drag = None;
        self.corner_drag = None;
        self.ring_drag = None;
        self.ring_last = None;
        self.drawing = false;
    }
    pub(crate) fn dirty(&self, document: &Document) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|p| document.path(p.id) != Some(p))
    }
    /// Geometry and draft edits invalidate the guide; camera navigation does not.
    fn refresh_guide(&mut self, document: &Document) {
        let Some(path) = &self.draft else {
            self.guide = None;
            return;
        };
        let revision = document.display_revision();
        if self
            .guide
            .as_ref()
            .is_some_and(|cache| cache.revision == revision && cache.draft == *path)
        {
            return;
        }
        let mut points = path.dense_curve();
        let mut warning = None;
        if path.mode == PathMode::Surface && path.points.len() >= 2 {
            if path.id != 0 && path.target != document.active_model_id() {
                warning = Some(
                    "Select the path's original target model to preview its surface route".into(),
                );
            } else if path.id != 0 && path.is_stale(document.mesh()) {
                warning=Some("Surface changed. Reproject the saved path; the orange guide shows its old world-space route".into());
            } else {
                let mut draft = path.clone();
                // New drafts are not persistent objects yet. Supply only the
                // identities needed by validation, without changing the draft.
                if draft.id == 0 {
                    draft.id = 1;
                    draft.target = document.active_model_id().max(1);
                    draft.surface_signature = crate::sculpt::paths::mesh_signature(document.mesh());
                }
                if draft.name.trim().is_empty() {
                    draft.name = "Draft".into();
                }
                let length: f32 = points
                    .windows(2)
                    .map(|p| (p[1].position - p[0].position).len())
                    .sum();
                let spacing = (length / 512.).max(1e-4);
                match draft.samples_on_mesh(document.mesh(), spacing) {
                    Ok(samples) => {
                        points = samples
                            .into_iter()
                            .map(|p| PathPoint {
                                position: p.position,
                                normal: p.normal,
                            })
                            .collect();
                    }
                    Err(error) => {
                        warning = Some(format!(
                            "Surface guide unavailable: {error}. The orange guide is unprojected; adjust its points or reproject before saving."
                        ))
                    }
                }
            }
        }
        if warning.is_some()
            && self.guide.as_ref().and_then(|cache| cache.warning.as_ref()) != warning.as_ref()
        {
            self.notice = warning.clone();
        }
        self.guide = Some(GuideCache {
            draft: path.clone(),
            revision,
            points,
            warning,
        });
    }
    fn point(&self, scene: &Scene, pos: Pos2) -> Option<PathPoint> {
        let draft = self.draft.as_ref()?;
        if draft.mode == PathMode::Surface {
            scene
                .pick(pos)
                .map(|(position, normal)| PathPoint { position, normal })
        } else {
            let anchor = self
                .selected
                .and_then(|i| draft.points.get(i))
                .or_else(|| draft.points.last())
                .map_or(Vec3::default(), |p| p.position);
            Some(PathPoint {
                position: scene.at_depth(pos, anchor),
                normal: scene.view,
            })
        }
    }
    fn nearest_point(&self, scene: &Scene, pos: Pos2) -> Option<usize> {
        let surface = self.draft.as_ref()?.mode == PathMode::Surface;
        self.draft
            .as_ref()?
            .points
            .iter()
            .enumerate()
            .filter(|(_, p)| scene.visible_depth(p.position))
            .filter(|(_, p)| !surface || faces_camera(scene, p))
            .map(|(i, p)| (i, scene.project(p.position).distance(pos)))
            .filter(|(_, d)| *d <= 12.)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
    fn insert_index(&self, scene: &Scene, pos: Pos2) -> Option<usize> {
        let path = self.draft.as_ref()?;
        let n = path.points.len();
        if n < 2 {
            return None;
        }
        (0..if path.closed { n } else { n - 1 })
            .map(|i| {
                let a = scene.project(path.points[i].position);
                let b = scene.project(path.points[(i + 1) % n].position);
                let edge = b - a;
                let t = ((pos - a).dot(edge) / edge.length_sq().max(1e-6)).clamp(0., 1.);
                (i + 1, (a + edge * t).distance(pos))
            })
            .filter(|(_, d)| *d < 24.)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
    fn add(&mut self, point: PathPoint, index: Option<usize>) {
        let Some(path) = self.draft.as_mut() else {
            return;
        };
        if path.points.len() >= crate::sculpt::paths::MAX_POINTS {
            self.notice = Some("A path supports up to 2048 control points".into());
            return;
        }
        let i = index.unwrap_or(path.points.len());
        match path.insert_point(i, point) {
            Ok(()) => self.selected = Some(i),
            Err(error) => self.notice = Some(error),
        }
    }
    fn corner_handles(&self, scene: &Scene) -> Vec<CornerHandle> {
        let mut handles = Vec::new();
        if self.tool != PathTool::Move {
            return handles;
        }
        let Some(path) = &self.draft else {
            return handles;
        };
        let Some(index) = self.selected.filter(|&i| path.corner_limit(i) > 1e-6) else {
            return handles;
        };
        let point = path.points[index];
        if !scene.visible_depth(point.position)
            || (path.mode == PathMode::Surface && !faces_camera(scene, &point))
        {
            return handles;
        }
        let n = path.points.len();
        let incoming = (path.points[(index + n - 1) % n].position - point.position).norm();
        let outgoing = (path.points[(index + 1) % n].position - point.position).norm();
        let bisector = (incoming + outgoing).norm();
        let amount = path.corner_rounding(index);
        let center = scene.project(point.position);
        let step = (path.corner_limit(index) * 0.01).clamp(1e-4, 0.01);
        for (axis, diamond) in [(bisector, true), (incoming, false), (outgoing, false)] {
            let projected = (scene.project(point.position + axis * step) - center) / step;
            let scale = projected.length();
            if scale < 1e-3 {
                continue;
            }
            let screen_axis = projected / scale;
            let position = if diamond {
                center + screen_axis * (22. + amount * scale * 0.5)
            } else {
                scene.project(point.position + axis * amount)
            };
            if !diamond && position.distance(center) < 18. {
                continue;
            }
            handles.push(CornerHandle {
                index,
                position,
                axis: screen_axis,
                pixels_per_amount: scale * if diamond { 0.5 } else { 1. },
                diamond,
            });
        }
        handles
    }
    fn corner_at(&self, scene: &Scene, pos: Pos2) -> Option<CornerHandle> {
        self.corner_handles(scene)
            .into_iter()
            .filter(|h| h.position.distance(pos) <= controls::HIT_RADIUS)
            .min_by(|a, b| {
                a.position
                    .distance(pos)
                    .total_cmp(&b.position.distance(pos))
            })
    }
    fn update_ring(&mut self, document: &Document, hit: PathPoint) {
        let Some(path) = &self.draft else {
            return;
        };
        if path.mode != PathMode::Surface {
            self.notice = Some("Start a new surface path before drawing a ring".into());
            return;
        }
        if path.id != 0
            && (path.target != document.active_model_id() || path.is_stale(document.mesh()))
        {
            self.notice = Some("Select and reproject this path's target before replacing its ring, or create a new ring".into());
            return;
        }
        let result = match self.ring_mode {
            RingMode::Level => crate::sculpt::paths::ring::section_ring(
                document.mesh(),
                hit,
                self.ring_axis.vector(),
            ),
            RingMode::Shortest => crate::sculpt::paths::ring::shortest_ring(
                document.mesh(),
                hit,
                self.ring_axis.vector(),
            ),
        };
        match result {
            Ok(points) => {
                let path = self.draft.as_mut().unwrap();
                path.points = points;
                path.corner_rounding.clear();
                path.attached_route = None;
                path.closed = true;
                path.seam = 0;
                path.smoothing = 0.;
                self.selected = None;
                self.notice = Some("Ring positioned. Release the pen, then Save & sculpt.".into());
            }
            Err(error) => self.notice = Some(format!("Ring unchanged: {error}")),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn show(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        camera: &mut Camera,
        document: &Document,
        options: ViewportOptions,
    ) {
        let pos = ui.input(|i| i.pointer.interact_pos());
        let primary = ui.input(|i| i.pointer.primary_down());
        let gizmo_rect = egui::Rect::from_min_size(
            Pos2::new(response.rect.right() - 146., response.rect.top() + 12.),
            egui::Vec2::splat(134.),
        );
        let modifiers = ui.input(|i| i.modifiers);
        let escape =
            ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ui.ctx().wants_keyboard_input();
        if escape || !options.enabled {
            if let Some(drag) = self.corner_drag.take() {
                self.draft = Some(drag.before);
            }
            if let Some((_, before)) = self.drag.take() {
                self.draft = Some(before);
            }
            if let Some(before) = self.ring_drag.take() {
                self.draft = Some(before);
            }
            self.ring_last = None;
            self.drawing = false;
        }
        if options.enabled && response.hovered() {
            camera.zoom_by(ui.input(|i| i.smooth_scroll_delta.y));
        }
        let pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        if pressed && response.hovered() && options.enabled {
            self.navigating = pos.is_some_and(|p| navigation::contains(response.rect, p))
                || pos.is_some_and(|p| gizmo_rect.contains(p))
                || options.pan_hold.active(modifiers)
                || options.zoom_hold.active(modifiers);
            if !self.navigating
                && !self.frozen
                && !escape
                && let Some(pos) = pos
            {
                if let Some(handle) = self.corner_at(scene, pos) {
                    if let Some(before) = self.draft.clone() {
                        let amount = before.corner_rounding(handle.index);
                        self.corner_drag = Some(CornerDrag {
                            handle,
                            before,
                            start: pos,
                            amount,
                        });
                    }
                } else if self.tool == PathTool::Ring {
                    if let Some((position, normal)) = scene.pick(pos) {
                        self.ring_drag = self.draft.clone();
                        self.ring_last = Some(pos);
                        self.update_ring(document, PathPoint { position, normal });
                    } else {
                        self.notice =
                            Some("Press on the model's side to create a complete ring".into());
                    }
                } else if let Some(index) = self.nearest_point(scene, pos) {
                    self.selected = Some(index);
                    if self.tool == PathTool::Move {
                        self.drag = self.draft.clone().map(|p| (index, p));
                    }
                } else if let Some(point) = self.point(scene, pos) {
                    if self.tool == PathTool::Insert {
                        if let Some(index) = self.insert_index(scene, pos) {
                            self.add(point, Some(index));
                        } else {
                            self.notice = Some(
                                "Click within 24 pixels of a path segment to insert a point".into(),
                            );
                        }
                    } else if self.tool == PathTool::Add {
                        self.add(point, None);
                        self.drawing = true;
                        self.last_draw = Some(pos);
                    } else {
                        self.navigating = true;
                    }
                } else {
                    self.navigating = true;
                }
            }
        }
        if options.enabled && !escape {
            if response.clicked_by(egui::PointerButton::Primary)
                && let Some(pos) = pos.filter(|p| gizmo_rect.contains(*p))
                && let Some((yaw, pitch)) =
                    super::gizmo::snap_target(camera, gizmo_rect.center(), pos)
            {
                camera.yaw = yaw;
                camera.pitch = pitch;
            }
            let secondary = response.dragged_by(egui::PointerButton::Secondary);
            let middle = response.dragged_by(egui::PointerButton::Middle);
            if secondary || middle || (primary && self.navigating) {
                let delta = ui.input(|i| i.pointer.delta());
                if options.zoom_hold.active(modifiers) {
                    camera.zoom_drag(delta);
                } else if middle || options.pan_hold.active(modifiers) {
                    camera.pan += delta;
                } else {
                    camera.orbit(delta);
                }
            } else if !self.frozen
                && let Some(pos) = pos
            {
                if let Some(drag) = &self.corner_drag {
                    let amount = (drag.amount
                        + (pos - drag.start).dot(drag.handle.axis) / drag.handle.pixels_per_amount)
                        .max(0.);
                    if let Some(path) = self.draft.as_mut()
                        && let Err(error) = path.set_corner_rounding(drag.handle.index, amount)
                    {
                        self.notice = Some(error);
                    }
                } else if self.ring_drag.is_some() {
                    if response.rect.contains(pos)
                        && self.ring_last.is_none_or(|last| {
                            last.distance(pos) >= 2. || (!primary && last != pos)
                        })
                    {
                        self.ring_last = Some(pos);
                        if let Some((position, normal)) = scene.pick(pos) {
                            self.update_ring(document, PathPoint { position, normal });
                        } else {
                            self.notice = Some("Keep the pen on the model to move the ring; the last valid ring is kept".into());
                        }
                    }
                } else if self.drag.is_some()
                    && let Some(point) = self.point(scene, pos)
                {
                    let index = self.drag.as_ref().unwrap().0;
                    if let Some(path) = self.draft.as_mut() {
                        path.points[index] = point;
                    }
                } else if primary
                    && self.drawing
                    && response.rect.contains(pos)
                    && self.last_draw.is_none_or(|last| last.distance(pos) >= 12.)
                    && let Some(point) = self.point(scene, pos)
                {
                    self.add(point, None);
                    self.last_draw = Some(pos);
                }
            }
        }
        if !primary {
            self.corner_drag = None;
            self.ring_drag = None;
            self.ring_last = None;
            self.drag = None;
            self.drawing = false;
            self.navigating = false;
        }
        self.refresh_guide(document);
        let hovered = pos
            .filter(|_| response.contains_pointer() && self.tool == PathTool::Add)
            .and_then(|pos| self.nearest_point(scene, pos));
        if let Some(path) = self.draft.as_ref() {
            let cache = self.guide.as_ref().expect("draft guide refreshed");
            let color = if cache.warning.is_some() {
                Color32::from_rgb(245, 150, 90)
            } else {
                controls::SHAPE
            };
            let points = &cache.points;
            if cache.warning.is_some() {
                painter.text(
                    response.rect.center_bottom() - egui::vec2(0., 45.),
                    egui::Align2::CENTER_BOTTOM,
                    "Orange guide: stale or unprojected · adjust points or reproject",
                    egui::FontId::proportional(12.),
                    color,
                );
            }
            for (index, pair) in points.windows(2).enumerate() {
                let front = path.mode == PathMode::Free || faces_camera(scene, &pair[0]);
                if !front && index % 4 >= 2 {
                    continue;
                }
                if let Some(segment) = scene.clip_segment([pair[0].position, pair[1].position]) {
                    painter.line_segment(
                        segment,
                        Stroke::new(
                            2_f32,
                            if front {
                                color
                            } else {
                                color.gamma_multiply(0.3)
                            },
                        ),
                    );
                }
            }
            if path.closed
                && points.len() > 2
                && let Some(segment) =
                    scene.clip_segment([points.last().unwrap().position, points[0].position])
            {
                painter.line_segment(segment, Stroke::new(2_f32, color));
            }
            for (i, point) in path.points.iter().enumerate() {
                // Dense generated rings remain readable while drawing. Point-editing
                // tools expose every handle; drawing keeps only useful landmarks.
                if !matches!(self.tool, PathTool::Move | PathTool::Insert)
                    && i != path.seam
                    && self.selected != Some(i)
                    && hovered != Some(i)
                    && (path.closed || i + 1 != path.points.len())
                {
                    continue;
                }
                if !scene.visible_depth(point.position) {
                    continue;
                }
                let center = scene.project(point.position);
                let selected = self.selected == Some(i);
                let front = path.mode == PathMode::Free || faces_camera(scene, point);
                controls::marker(
                    painter,
                    center,
                    controls::Marker::Circle,
                    if front {
                        color
                    } else {
                        color.gamma_multiply(0.3)
                    },
                    selected,
                );
                if i == path.seam && !(path.closed && path.corner_rounding(path.seam) > 0.) {
                    painter.text(
                        center + egui::vec2(8., -8.),
                        egui::Align2::LEFT_BOTTOM,
                        "Start",
                        egui::FontId::proportional(12.),
                        color,
                    );
                }
            }
            // The editable corner stays at its original control point. A rounded
            // closed seam starts at the fillet midpoint on the sampled route.
            if path.closed
                && path.corner_rounding(path.seam) > 0.
                && let Some(start) = points.first()
                && scene.visible_depth(start.position)
            {
                let center = scene.project(start.position);
                let front = path.mode == PathMode::Free || faces_camera(scene, start);
                let seam_color = if front {
                    controls::ACTION
                } else {
                    controls::ACTION.gamma_multiply(0.3)
                };
                controls::marker(painter, center, controls::Marker::Square, seam_color, false);
                painter.text(
                    center + egui::vec2(8., -8.),
                    egui::Align2::LEFT_BOTTOM,
                    "Start",
                    egui::FontId::proportional(12.),
                    seam_color,
                );
            }
        }
        for handle in self.corner_handles(scene) {
            let center = scene.project(self.draft.as_ref().unwrap().points[handle.index].position);
            painter.line_segment(
                [center, handle.position],
                Stroke::new(1_f32, controls::TOPOLOGY.gamma_multiply(0.6)),
            );
            controls::marker(
                painter,
                handle.position,
                if handle.diamond {
                    controls::Marker::Diamond
                } else {
                    controls::Marker::Circle
                },
                controls::TOPOLOGY,
                self.corner_drag
                    .as_ref()
                    .is_some_and(|d| d.handle.diamond == handle.diamond)
                    || pos.is_some_and(|p| p.distance(handle.position) <= controls::HIT_RADIUS),
            );
        }
        painter.text(
            response.rect.center_bottom() - egui::vec2(0., 26.),
            egui::Align2::CENTER_BOTTOM,
            "Paths · right-drag: orbit · middle-drag: pan · wheel: zoom · Save path to keep edits",
            egui::FontId::proportional(13.),
            controls::NEUTRAL,
        );
    }
}

fn faces_camera(scene: &Scene, point: &PathPoint) -> bool {
    let view = if scene.eye_distance == 0. {
        scene.view
    } else {
        scene.view * scene.eye_distance - point.position
    };
    point.normal.dot(view) > 0.
}

#[cfg(test)]
mod guide_tests {
    use super::*;
    use crate::sculpt::Geometry;
    fn plane_document() -> Document {
        let mesh = crate::sculpt::Mesh {
            vertices: vec![
                Vec3(-1., -1., 0.),
                Vec3(1., -1., 0.),
                Vec3(1., 1., 0.),
                Vec3(-1., 1., 0.),
            ],
            mask: vec![0.; 4],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..Default::default()
        };
        let mut document = Document::default();
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        document
    }
    fn draft() -> SurfacePath {
        SurfacePath {
            id: 0,
            name: "Draft".into(),
            target: 0,
            mode: PathMode::Surface,
            points: vec![
                PathPoint {
                    position: Vec3(-0.5, 0., 0.),
                    normal: Vec3(0., 0., 1.),
                },
                PathPoint {
                    position: Vec3(0.5, 0., 0.),
                    normal: Vec3(0., 0., 1.),
                },
            ],
            smoothing: 0.,
            closed: false,
            seam: 0,
            surface_signature: 0,
            attached_route: None,
            corner_rounding: Vec::new(),
        }
    }
    #[test]
    fn surface_guide_cache_tracks_draft_and_revision_without_mutating_document() {
        let mut document = plane_document();
        let before = document.project();
        let mut editor = PathEditor::default();
        editor.select(draft());
        editor.refresh_guide(&document);
        assert!(editor.guide.as_ref().unwrap().warning.is_none());
        assert!(editor.guide.as_ref().unwrap().points.len() > 400);
        let pointer = editor.guide.as_ref().unwrap().points.as_ptr();
        editor.refresh_guide(&document);
        assert_eq!(pointer, editor.guide.as_ref().unwrap().points.as_ptr());
        assert!(document.project() == before);
        assert_eq!(editor.draft.as_ref().unwrap().id, 0);
        editor.draft.as_mut().unwrap().points[1].position.0 = 0.25;
        editor.refresh_guide(&document);
        assert_eq!(
            editor
                .guide
                .as_ref()
                .unwrap()
                .points
                .last()
                .unwrap()
                .position
                .0,
            0.25
        );
        let mut geometry = document.geometry_snapshot();
        for p in &mut geometry.mesh.vertices {
            p.2 = 0.1;
        }
        document.replace_geometry(geometry).unwrap();
        editor.refresh_guide(&document);
        assert!(
            editor
                .guide
                .as_ref()
                .unwrap()
                .points
                .iter()
                .all(|p| (p.position.2 - 0.1).abs() < 1e-5)
        );
        assert!(
            editor
                .draft
                .as_ref()
                .unwrap()
                .points
                .iter()
                .all(|p| p.position.2 == 0.)
        );
    }
    #[test]
    fn invalid_guide_reports_once_and_stale_paths_keep_original_route() {
        let mut document = plane_document();
        let p = draft();
        let id = document
            .create_path(p.name, p.mode, p.points, p.closed, p.smoothing)
            .unwrap();
        let mut editor = PathEditor::default();
        editor.select(document.path(id).unwrap().clone());
        let mut geometry = document.geometry_snapshot();
        for p in &mut geometry.mesh.vertices {
            p.2 = 0.1;
        }
        document.replace_geometry(geometry).unwrap();
        editor.refresh_guide(&document);
        assert!(editor.notice.take().unwrap().contains("Reproject"));
        assert!(editor.guide.as_ref().unwrap().warning.is_some());
        assert!(
            editor
                .guide
                .as_ref()
                .unwrap()
                .points
                .iter()
                .all(|p| p.position.2 == 0.)
        );
        editor.refresh_guide(&document);
        assert!(editor.notice.is_none());
    }
}

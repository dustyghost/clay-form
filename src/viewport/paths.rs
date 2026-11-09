//! Session-only path drafts. Navigation never edits the draft or document.
use super::{ViewportOptions, controls, navigation, scene::Scene};
pub(crate) use crate::sculpt::paths::RingOrientation as RingAxis;
use crate::{
    camera::Camera,
    document::Document,
    sculpt::{
        Vec3,
        paths::{PathMode, PathPoint, SurfacePath},
    },
};
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use std::collections::BTreeSet;
mod hud;
pub(crate) mod transform;

#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum PathTool {
    #[default]
    Add,
    Move,
    Insert,
    Ring,
    Whole,
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PathBrush {
    Surface,
    Free,
    Ring,
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PathNavigation {
    Orbit,
    Pan,
    Zoom,
}
#[derive(Clone, Copy, PartialEq)]
struct RingPlane {
    hit: PathPoint,
    normal: Vec3,
    orientation: RingAxis,
}

#[derive(Clone, Copy)]
enum RingDragKind {
    Place,
    Tilt { start: Pos2, right: Vec3, up: Vec3 },
}

struct RingDrag {
    before: SurfacePath,
    plane: Option<RingPlane>,
    kind: RingDragKind,
}

struct GuideCache {
    draft: SurfacePath,
    revision: u64,
    points: Vec<PathPoint>,
    warning: Option<String>,
}

struct GhostPaths {
    revision: u64,
    target: u64,
    routes: Vec<(u64, PathMode, bool, Vec<PathPoint>)>,
}

#[derive(Clone, Copy)]
struct CornerHandle {
    index: usize,
    position: Pos2,
    axis: Vec2,
    pixels_per_amount: f32,
    diamond: bool,
}
struct PointDrag {
    index: usize,
    before: SurfacePath,
    start: Pos2,
    axis: Option<Vec3>,
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
    pub panel: bool,
    pub exit_brush: bool,
    insert_on_path: bool,
    suspended: Option<SurfacePath>,
    ring_move: bool,
    camera_drag: Option<PathNavigation>,
    pub frozen: bool,
    pub draft: Option<SurfacePath>,
    pub selected: Option<usize>,
    pub tool: PathTool,
    pub ring_axis: RingAxis,
    pub brush: Option<PathBrush>,
    pub commit_gesture: bool,
    pub confirm_delete: Option<u64>,
    pub notice: Option<String>,
    pub hidden_ghosts: BTreeSet<u64>,
    ghosts: Option<GhostPaths>,
    pub transform: transform::State,
    drag: Option<PointDrag>,
    draw_before: Option<SurfacePath>,
    corner_drag: Option<CornerDrag>,
    ring_drag: Option<RingDrag>,
    ring_plane: Option<RingPlane>,
    ring_last: Option<Pos2>,
    drawing: bool,
    navigation_last: Option<Pos2>,
    last_draw: Option<Pos2>,
    guide: Option<GuideCache>,
}
impl PathEditor {
    pub(crate) fn select(&mut self, path: SurfacePath) {
        self.reset_selection();
        self.ring_plane = path.ring_orientation.and_then(|orientation| {
            if !path.closed || path.mode != PathMode::Surface {
                return None;
            }
            let hit = *path.points.get(path.seam)?;
            let normal = path
                .points
                .iter()
                .zip(path.points.iter().cycle().skip(1))
                .fold(Vec3::default(), |normal, (a, b)| {
                    normal + (a.position - hit.position).cross(b.position - hit.position)
                })
                .norm();
            (normal.len() > 0.5).then_some(RingPlane {
                hit,
                normal,
                orientation,
            })
        });
        self.draft = Some(path);
    }

    pub(crate) fn clear_selection(&mut self) {
        self.reset_selection();
        self.draft = None;
        self.ring_plane = None;
    }

    fn reset_selection(&mut self) {
        self.transform.reset();
        self.selected = None;
        self.confirm_delete = None;
        self.ring_move = false;
        self.insert_on_path = false;
        self.drag = None;
        self.draw_before = None;
        self.corner_drag = None;
        self.ring_drag = None;
        self.ring_last = None;
        self.drawing = false;
        self.camera_drag = None;
        self.navigation_last = None;
    }
    pub(crate) fn accept_saved(&mut self, path: SurfacePath) {
        let selected = self.selected;
        self.select(path);
        self.selected = selected;
    }

    pub(crate) fn gesture_active(&self) -> bool {
        self.drawing
            || self.drag.is_some()
            || self.corner_drag.is_some()
            || self.ring_drag.is_some()
    }

    pub(crate) fn new_draft(document: &Document, mode: PathMode) -> SurfacePath {
        SurfacePath {
            id: 0,
            target: document.active_model_id(),
            name: format!("Path {}", document.paths().len() + 1),
            mode,
            points: Vec::new(),
            smoothing: 0.,
            closed: false,
            seam: 0,
            surface_signature: 0,
            offset: 0.,
            ring_orientation: None,
            attached_route: None,
            corner_rounding: Vec::new(),
        }
    }

    pub(crate) fn select_for_edit(
        &mut self,
        path: SurfacePath,
        document: &Document,
    ) -> Result<(), String> {
        if self.dirty(document) {
            if self.brush.is_some() && self.draft.as_ref().is_some_and(|p| p.id == 0) {
                self.suspended = self.draft.take();
            } else {
                return Err("Apply or discard the current path edits first".into());
            }
        }
        self.select(path);
        self.tool = if self.ring_plane.is_some() {
            PathTool::Ring
        } else {
            PathTool::Move
        };
        Ok(())
    }

    fn resume_draft(&mut self) {
        if let Some(draft) = self.suspended.take() {
            self.brush = Some(if draft.mode == PathMode::Free {
                PathBrush::Free
            } else {
                PathBrush::Surface
            });
            self.select(draft);
            self.tool = PathTool::Add;
        }
    }

    pub(crate) fn has_unfinished(&self, document: &Document) -> bool {
        self.suspended.is_some() || self.dirty(document)
    }

    fn editable_draft(&self) -> bool {
        self.brush.is_some()
            && self
                .draft
                .as_ref()
                .is_some_and(|path| path.id == 0 && !path.points.is_empty() && !path.closed)
    }

    fn brush_press(
        &mut self,
        document: &Document,
        scene: &Scene,
        pos: Pos2,
        brush: PathBrush,
    ) -> bool {
        if self.dirty(document) && self.draft.as_ref().is_some_and(|p| p.id != 0) {
            self.notice = Some(
                "Save or discard the current path settings before drawing another path".into(),
            );
            return true;
        }
        let active = self.draft.as_ref().map(|path| path.id);
        let mut hit = None;
        for path in self.draft.iter().chain(document.paths().iter().filter(|p| {
            p.target == document.active_model_id()
                && Some(p.id) != active
                && !self.hidden_ghosts.contains(&p.id)
        })) {
            for (index, point) in path.points.iter().enumerate().filter(|(i, _)| {
                *i == path.seam
                    || (!path.closed && *i + 1 == path.points.len())
                    || (active == Some(path.id)
                        && (self.tool == PathTool::Move || self.selected == Some(*i)))
            }) {
                let position = path.display_position(*point);
                if !scene.visible_depth(position)
                    || (path.mode == PathMode::Surface && !faces_camera(scene, point))
                {
                    continue;
                }
                let distance = scene.project(position).distance(pos);
                if distance <= controls::HIT_RADIUS
                    && hit.is_none_or(|(_, _, previous)| distance < previous)
                {
                    hit = Some((path, index, distance));
                }
            }
        }
        if let Some((path, index, _)) = hit {
            if Some(path.id) != active
                && let Err(error) = self.select_for_edit(path.clone(), document)
            {
                self.notice = Some(error);
                return true;
            }
            self.selected = Some(index);
            if let Some(plane) = self.ring_plane {
                self.tool = PathTool::Ring;
                self.ring_drag = self.draft.clone().map(|before| RingDrag {
                    before,
                    plane: Some(plane),
                    kind: RingDragKind::Place,
                });
                self.ring_last = Some(pos);
            } else {
                self.tool = PathTool::Move;
                self.start_point_drag(index, pos, None);
            }
            return true;
        }

        if let Some(path) = self.ghost_at(document, scene, pos) {
            if let Err(error) = self.select_for_edit(path.clone(), document) {
                self.notice = Some(error);
            }
            return true;
        }
        let brush = self.suspended.as_ref().map_or(brush, |draft| {
            if draft.mode == PathMode::Free {
                PathBrush::Free
            } else {
                PathBrush::Surface
            }
        });
        let mode = if brush == PathBrush::Free {
            PathMode::Free
        } else {
            PathMode::Surface
        };
        if mode == PathMode::Surface && scene.pick(pos).is_none() {
            self.camera_drag = Some(PathNavigation::Orbit);
            return true;
        }
        self.resume_draft();
        let unfinished = self.draft.as_ref().is_some_and(|p| {
            p.id == 0
                && p.mode == mode
                && p.target == document.active_model_id()
                && (p.points.is_empty() || (brush != PathBrush::Ring && !p.closed))
        });
        if !unfinished {
            self.select(Self::new_draft(document, mode));
        }
        if brush == PathBrush::Free && self.draft.as_ref().is_some_and(|p| p.points.is_empty()) {
            self.draft.as_mut().unwrap().smoothing = 1.;
        }
        self.tool = if brush == PathBrush::Ring {
            PathTool::Ring
        } else {
            PathTool::Add
        };
        false
    }

    pub(crate) fn dirty(&self, document: &Document) -> bool {
        self.draft.as_ref().is_some_and(|p| {
            if p.id == 0 {
                !p.points.is_empty()
            } else {
                document.path(p.id) != Some(p)
            }
        }) || self.transform.pending_section_scale != 1.
    }
    fn ghost_at<'a>(
        &self,
        document: &'a Document,
        scene: &Scene,
        pos: Pos2,
    ) -> Option<&'a SurfacePath> {
        let active = self.draft.as_ref().map(|p| p.id);
        self.ghosts
            .as_ref()
            .filter(|cache| {
                cache.revision == document.display_revision()
                    && cache.target == document.active_model_id()
            })?
            .routes
            .iter()
            .filter(|(id, _, _, _)| Some(*id) != active && !self.hidden_ghosts.contains(id))
            .filter_map(|(id, mode, _, route)| {
                let distance = route
                    .windows(2)
                    .filter(|pair| *mode == PathMode::Free || faces_camera(scene, &pair[0]))
                    .filter_map(|pair| scene.clip_segment([pair[0].position, pair[1].position]))
                    .map(|[a, b]| {
                        let edge = b - a;
                        let t = ((pos - a).dot(edge) / edge.length_sq().max(1e-6)).clamp(0., 1.);
                        (a + edge * t).distance(pos)
                    })
                    .min_by(f32::total_cmp)?;
                (distance <= 7.).then_some((*id, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .and_then(|(id, _)| document.path(id))
    }

    fn paint_ghosts(&mut self, document: &Document, scene: &Scene, painter: &egui::Painter) {
        let revision = document.display_revision();
        let target = document.active_model_id();
        if self
            .ghosts
            .as_ref()
            .is_none_or(|g| g.revision != revision || g.target != target)
        {
            self.ghosts = Some(GhostPaths {
                revision,
                target,
                routes: document
                    .paths()
                    .iter()
                    .filter(|p| p.target == target)
                    .map(|p| (p.id, p.mode, p.closed, p.dense_curve()))
                    .collect(),
            });
        }
        let ghosts = self.ghosts.as_ref().unwrap();
        let active = self.draft.as_ref().map(|p| p.id);
        for (id, mode, closed, points) in &ghosts.routes {
            if active == Some(*id) || self.hidden_ghosts.contains(id) || points.len() < 2 {
                continue;
            }
            if self.brush.is_some()
                && let Some(path) = document.path(*id)
            {
                for point in [
                    path.points.get(path.seam),
                    (!path.closed).then(|| path.points.last()).flatten(),
                ]
                .into_iter()
                .flatten()
                {
                    let position = path.display_position(*point);
                    if scene.visible_depth(position)
                        && (*mode == PathMode::Free || faces_camera(scene, point))
                    {
                        controls::marker(
                            painter,
                            scene.project(position),
                            controls::Marker::Circle,
                            controls::SHAPE.gamma_multiply(0.45),
                            false,
                        );
                    }
                }
            }
            let count = points.len() - usize::from(!*closed);
            for i in 0..count {
                let p = &points[i];
                let front = *mode == PathMode::Free || faces_camera(scene, p);
                if !front && i % 4 >= 2 {
                    continue;
                }
                let q = &points[(i + 1) % points.len()];
                if let Some(segment) = scene.clip_segment([p.position, q.position]) {
                    let opacity = if front { 0.45 } else { 0.2 };
                    painter.line_segment(
                        segment,
                        Stroke::new(1.5_f32, controls::SHAPE.gamma_multiply(opacity)),
                    );
                }
            }
        }
    }

    /// Geometry and draft edits invalidate the guide; camera navigation does not.
    fn refresh_guide(&mut self, document: &Document) {
        let Some(path) = &self.draft else {
            self.guide = None;
            return;
        };
        let revision = document.display_revision();
        if self.guide.as_mut().is_some_and(|cache| {
            cache.draft.offset = path.offset;
            cache.revision == revision && cache.draft == *path
        }) {
            self.transform.route_start = self
                .guide
                .as_ref()
                .and_then(|g| g.points.first())
                .map(|p| p.position);
            return;
        }
        let mut points = path.surface_curve();
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
        self.transform.route_start = points.first().map(|p| p.position);
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
                normal: draft
                    .points
                    .first()
                    .map_or(scene.view, |point| point.normal),
            })
        }
    }
    fn start_point_drag(&mut self, index: usize, start: Pos2, axis: Option<Vec3>) {
        self.drag = self.draft.clone().map(|before| PointDrag {
            index,
            before,
            start,
            axis,
        });
    }

    fn free_axes(&self, scene: &Scene) -> impl Iterator<Item = (usize, Vec3, Pos2, Pos2)> + use<> {
        let pivot = self
            .draft
            .as_ref()
            .filter(|p| {
                p.mode == PathMode::Free && self.brush.is_some() && self.tool != PathTool::Whole
            })
            .and_then(|p| p.points.get(self.selected?))
            .filter(|p| scene.visible_depth(p.position))
            .map(|p| p.position);
        let mut handles = [None; 3];
        if let Some(pivot) = pivot {
            let center = scene.project(pivot);
            for (i, handle) in handles.iter_mut().enumerate() {
                let axis = crate::sculpt::instances::axis(i);
                let direction = scene.axis_direction(pivot, axis);
                if direction.length() > 0.1 {
                    *handle = Some((i, axis, center, center + direction.normalized() * 48.));
                }
            }
        }
        handles.into_iter().flatten()
    }

    fn nearest_point(&self, scene: &Scene, pos: Pos2) -> Option<usize> {
        let path = self.draft.as_ref()?;
        let surface = path.mode == PathMode::Surface;
        path.points
            .iter()
            .enumerate()
            .filter(|(_, p)| scene.visible_depth(path.display_position(**p)))
            .filter(|(_, p)| !surface || faces_camera(scene, p))
            .map(|(i, p)| (i, scene.project(path.display_position(*p)).distance(pos)))
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
                let a = scene.project(path.display_position(path.points[i]));
                let b = scene.project(path.display_position(path.points[(i + 1) % n]));
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
    fn ring_tilt_handle(&self, scene: &Scene) -> Option<(Pos2, Pos2)> {
        let plane = self.ring_plane?;
        if self.tool == PathTool::Whole
            || self
                .draft
                .as_ref()
                .is_none_or(|p| !p.closed || p.mode != PathMode::Surface)
        {
            return None;
        }
        let position = self.draft.as_ref()?.display_position(plane.hit);
        if !scene.visible_depth(position) {
            return None;
        }
        let anchor = scene.project(position);
        Some((anchor, anchor + Vec2::new(36., -28.)))
    }

    fn update_ring(
        &mut self,
        document: &Document,
        hit: PathPoint,
        orientation: RingAxis,
        manual_normal: Option<Vec3>,
    ) {
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
        use crate::sculpt::paths::ring::{auto_section_ring, section_ring};
        let result = if let Some(axis) = manual_normal.or_else(|| orientation.vector()) {
            section_ring(document.mesh(), hit, axis).map(|points| (axis, points))
        } else {
            auto_section_ring(document.mesh(), hit)
        };
        match result {
            Ok((normal, points)) => {
                let path = self.draft.as_mut().unwrap();
                path.points = points;
                path.corner_rounding.clear();
                path.attached_route = None;
                path.closed = true;
                path.seam = 0;
                path.smoothing = 0.;
                path.ring_orientation = Some(orientation);
                self.selected = None;
                self.ring_plane = Some(RingPlane {
                    hit,
                    normal,
                    orientation,
                });
                if self.brush.is_none() {
                    self.notice = Some("Drag Tilt to angle the ring, or save the path.".into());
                }
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
        let camera_controls = hud::camera_targets(gizmo_rect);
        if self.brush.is_some() {
            self.brush_hud(ui.ctx(), response.rect, scene, document);
            hud::paint_camera_targets(painter, &camera_controls, self.camera_drag, pos);
            if response.hovered() {
                let over_camera = pos
                    .is_some_and(|pos| camera_controls.iter().any(|(_, rect)| rect.contains(pos)));
                ui.ctx().set_cursor_icon(if over_camera {
                    egui::CursorIcon::Grab
                } else {
                    egui::CursorIcon::Crosshair
                });
            }
        }
        let modifiers = ui.input(|i| i.modifiers);
        let escape =
            ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ui.ctx().wants_keyboard_input();
        if escape || !options.enabled {
            if let Some(path) = &mut self.draft {
                self.transform.cancel(path);
            }
            if let Some(drag) = self.corner_drag.take() {
                self.draft = Some(drag.before);
            }
            if let Some(drag) = self.drag.take() {
                self.draft = Some(drag.before);
            }
            if let Some(before) = self.draw_before.take() {
                self.draft = Some(before);
                self.selected = None;
            } else if escape && !self.gesture_active() && self.editable_draft() {
                self.clear_selection();
            }
            if escape {
                self.ring_move = false;
                self.insert_on_path = false;
            }
            if let Some(drag) = self.ring_drag.take() {
                self.draft = Some(drag.before);
                self.ring_plane = drag.plane;
            }
            self.ring_last = None;
            self.drawing = false;
            self.camera_drag = None;
            self.navigation_last = None;
        }
        let frame_hovered = response
            .hover_pos()
            .is_some_and(|p| navigation::contains(response.rect, p));
        navigation::paint(painter, response.rect, frame_hovered);
        if options.enabled && response.hovered() {
            camera.zoom_by(ui.input(|i| i.smooth_scroll_delta.y));
        }
        // A pen can report its first position and its down event in the same
        // frame. Use the press position, since hover can still refer to the
        // path palette or the previous contact in that frame.
        let press = ui.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } => Some((*pos, *modifiers)),
                _ => None,
            })
        });
        if options.enabled
            && !escape
            && let Some((pos, press_modifiers)) = press
            && response.rect.contains(pos)
            && ui.ctx().layer_id_at(pos) == Some(response.layer_id)
        {
            self.navigation_last = Some(pos);
            self.camera_drag = camera_controls
                .iter()
                .find(|(_, rect)| self.brush.is_some() && rect.contains(pos))
                .map(|(mode, _)| *mode)
                .or_else(|| match self.tool {
                    _ if options.zoom_hold.active(press_modifiers) => Some(PathNavigation::Zoom),
                    _ if options.pan_hold.active(press_modifiers) => Some(PathNavigation::Pan),
                    _ if navigation::contains(response.rect, pos)
                        || gizmo_rect.contains(pos)
                        || ui.input(|i| i.pointer.secondary_down() || i.pointer.middle_down()) =>
                    {
                        Some(PathNavigation::Orbit)
                    }
                    _ => None,
                });
            if self.camera_drag.is_none() && !self.frozen && self.confirm_delete.is_none() {
                if self.tool == PathTool::Whole {
                    if let Some(path) = &self.draft {
                        if !self.transform.press(path, scene, pos) {
                            self.camera_drag = Some(PathNavigation::Orbit);
                        }
                    }
                } else if let Some((_, axis, _, _)) = self
                    .free_axes(scene)
                    .filter(|(_, _, _, handle)| handle.distance(pos) <= controls::HIT_RADIUS)
                    .min_by(|a, b| a.3.distance_sq(pos).total_cmp(&b.3.distance_sq(pos)))
                    && let Some(index) = self.selected
                {
                    self.start_point_drag(index, pos, Some(axis));
                } else if self.insert_on_path {
                    if let Some(index) = self.insert_index(scene, pos)
                        && let Some(point) = self.point(scene, pos)
                    {
                        self.add(point, Some(index));
                        self.insert_on_path = false;
                        self.commit_gesture = true;
                    }
                } else if self.ring_move {
                    if let Some(plane) = self.ring_plane
                        && let Some((position, normal)) = scene.pick(pos)
                    {
                        let previous = self.ring_plane;
                        self.update_ring(
                            document,
                            PathPoint { position, normal },
                            plane.orientation,
                            (plane.orientation == RingAxis::Manual).then_some(plane.normal),
                        );
                        if self.ring_plane != previous {
                            self.ring_move = false;
                            self.commit_gesture = true;
                        }
                    }
                } else if let Some(handle) = self.corner_at(scene, pos) {
                    if let Some(before) = self.draft.clone() {
                        let amount = before.corner_rounding(handle.index);
                        self.corner_drag = Some(CornerDrag {
                            handle,
                            before,
                            start: pos,
                            amount,
                        });
                    }
                } else if let Some((_, handle)) = self.ring_tilt_handle(scene)
                    && handle.distance(pos) <= controls::HIT_RADIUS + 4.
                    && let Some(before) = self.draft.clone()
                {
                    let (right, up, _) = camera.basis();
                    self.ring_drag = Some(RingDrag {
                        before,
                        plane: self.ring_plane,
                        kind: RingDragKind::Tilt {
                            start: pos,
                            right,
                            up,
                        },
                    });
                    self.ring_last = Some(pos);
                } else if !(self
                    .brush
                    .is_some_and(|brush| self.brush_press(document, scene, pos, brush)))
                {
                    if self.tool == PathTool::Ring {
                        let before = self.draft.clone();
                        let plane = self.ring_plane;
                        if let Some(before) = before {
                            if let Some((position, normal)) = scene.pick(pos) {
                                self.ring_drag = Some(RingDrag {
                                    before,
                                    plane: self.ring_plane,
                                    kind: RingDragKind::Place,
                                });
                                self.ring_last = Some(pos);
                                self.update_ring(
                                    document,
                                    PathPoint { position, normal },
                                    plane.map_or(self.ring_axis, |p| p.orientation),
                                    plane
                                        .filter(|p| p.orientation == RingAxis::Manual)
                                        .map(|p| p.normal),
                                );
                            } else {
                                self.notice = Some(
                                    "Press on the model's side to create a complete ring".into(),
                                );
                            }
                        }
                    } else if let Some(index) = self.nearest_point(scene, pos) {
                        self.selected = Some(index);
                        if self.tool == PathTool::Move {
                            self.start_point_drag(index, pos, None);
                        }
                    } else if let Some(point) = self.point(scene, pos) {
                        if self.tool == PathTool::Insert {
                            if let Some(index) = self.insert_index(scene, pos) {
                                self.add(point, Some(index));
                            } else {
                                self.notice = Some(
                                    "Click within 24 pixels of a path segment to insert a point"
                                        .into(),
                                );
                            }
                        } else if self.tool == PathTool::Add {
                            if self.brush.is_some() {
                                self.draw_before = self.draft.clone();
                            }
                            self.add(point, None);
                            self.drawing = true;
                            self.last_draw = Some(pos);
                        } else {
                            self.camera_drag = Some(PathNavigation::Orbit);
                        }
                    } else {
                        self.camera_drag = Some(PathNavigation::Orbit);
                    }
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
                camera.roll = 0.;
            }
            let secondary = response.dragged_by(egui::PointerButton::Secondary);
            let middle = response.dragged_by(egui::PointerButton::Middle);
            if secondary || middle || (primary && self.camera_drag.is_some()) {
                let delta = if primary && self.camera_drag.is_some() && !secondary && !middle {
                    pos.map_or(Vec2::ZERO, |p| {
                        self.navigation_last
                            .replace(p)
                            .map_or(Vec2::ZERO, |last| p - last)
                    })
                } else {
                    ui.input(|i| i.pointer.delta())
                };
                let mode = if secondary || middle {
                    if options.zoom_hold.active(modifiers) {
                        PathNavigation::Zoom
                    } else if middle || options.pan_hold.active(modifiers) {
                        PathNavigation::Pan
                    } else {
                        PathNavigation::Orbit
                    }
                } else {
                    self.camera_drag.unwrap_or(PathNavigation::Orbit)
                };
                hud::paint_camera_status(ui.ctx(), painter, response.rect, mode);
                match mode {
                    PathNavigation::Zoom => camera.zoom_drag(delta),
                    PathNavigation::Pan => camera.pan += delta,
                    PathNavigation::Orbit => camera.orbit(delta),
                }
            } else if !self.frozen
                && let Some(pos) = pos
            {
                if self.tool == PathTool::Whole {
                    if let Some(path) = &mut self.draft
                        && let Err(error) = self.transform.update(path, scene, pos)
                    {
                        self.notice = Some(error);
                    }
                } else if let Some(drag) = &self.corner_drag {
                    let amount = (drag.amount
                        + (pos - drag.start).dot(drag.handle.axis) / drag.handle.pixels_per_amount)
                        .max(0.);
                    if let Some(path) = self.draft.as_mut()
                        && let Err(error) = path.set_corner_rounding(drag.handle.index, amount)
                    {
                        self.notice = Some(error);
                    }
                } else if let Some(drag) = &self.ring_drag {
                    if response.rect.contains(pos)
                        && self.ring_last.is_none_or(|last| {
                            last.distance(pos) >= 2. || (!primary && last != pos)
                        })
                    {
                        self.ring_last = Some(pos);
                        match drag.kind {
                            RingDragKind::Place => {
                                let pick = if let Some(plane) = drag.plane {
                                    pos + (scene.project(plane.hit.position)
                                        - scene.project(drag.before.display_position(plane.hit)))
                                } else {
                                    pos
                                };
                                if let Some((position, normal)) = scene.pick(pick) {
                                    let plane = self.ring_plane;
                                    let orientation =
                                        plane.map_or(self.ring_axis, |p| p.orientation);
                                    self.update_ring(
                                        document,
                                        PathPoint { position, normal },
                                        orientation,
                                        plane
                                            .filter(|p| p.orientation == RingAxis::Manual)
                                            .map(|p| p.normal),
                                    );
                                } else {
                                    self.notice = Some("Keep the pen on the model to move the ring; the last valid ring is kept".into());
                                }
                            }
                            RingDragKind::Tilt { start, right, up } => {
                                if let Some(plane) = drag.plane {
                                    let delta = pos - start;
                                    let normal = plane
                                        .normal
                                        .rotated(up, delta.x * 0.01)
                                        .rotated(right, delta.y * 0.01)
                                        .norm();
                                    self.update_ring(
                                        document,
                                        plane.hit,
                                        RingAxis::Manual,
                                        Some(normal),
                                    );
                                }
                            }
                        }
                    }
                } else if let Some(drag) = &self.drag {
                    let original = drag.before.points[drag.index];
                    let point = if let Some(axis) = drag.axis {
                        scene
                            .axis_parameter(pos, original.position, axis)
                            .zip(scene.axis_parameter(drag.start, original.position, axis))
                            .map(|(end, start)| PathPoint {
                                position: original.position + axis * (end - start),
                                ..original
                            })
                    } else {
                        let pick =
                            if drag.before.mode == PathMode::Surface && drag.before.offset != 0. {
                                scene.project(original.position) + (pos - drag.start)
                            } else {
                                pos
                            };
                        self.point(scene, pick)
                    };
                    if pos != drag.start
                        && let Some(point) = point
                        && let Some(path) = self.draft.as_mut()
                    {
                        path.points[drag.index] = point;
                    }
                } else if self.drawing
                    && response.rect.contains(pos)
                    && ui.ctx().layer_id_at(pos) == Some(response.layer_id)
                    && self.last_draw.is_none_or(|last| last.distance(pos) >= 12.)
                    && let Some(point) = self.point(scene, pos)
                {
                    self.add(point, None);
                    self.last_draw = Some(pos);
                }
            }
        }
        if !primary {
            self.commit_gesture |= self.brush.is_some()
                && !self.editable_draft()
                && self.gesture_active()
                && self
                    .draft
                    .as_ref()
                    .is_some_and(|p| p.points.len() >= if p.closed { 3 } else { 2 });
            self.transform.finish();
            self.corner_drag = None;
            self.ring_drag = None;
            self.ring_last = None;
            self.drag = None;
            self.draw_before = None;
            self.drawing = false;
            self.camera_drag = None;
            self.navigation_last = None;
        }
        self.refresh_guide(document);
        self.paint_ghosts(document, scene, painter);
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
            let color = if self.camera_drag.is_some() {
                color.gamma_multiply(0.5)
            } else {
                color
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
                if let Some(segment) = scene.clip_segment([
                    path.display_position(pair[0]),
                    path.display_position(pair[1]),
                ]) {
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
                && let Some(segment) = scene.clip_segment([
                    path.display_position(*points.last().unwrap()),
                    path.display_position(points[0]),
                ])
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
                let position = path.display_position(*point);
                if !scene.visible_depth(position) {
                    continue;
                }
                let center = scene.project(position);
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
                && scene.visible_depth(path.display_position(*start))
            {
                let center = scene.project(path.display_position(*start));
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
        for (i, _, start, handle) in self.free_axes(scene) {
            let color = controls::AXES[i];
            painter.line_segment([start, handle], Stroke::new(2_f32, color));
            controls::marker(
                painter,
                handle,
                controls::Marker::Square,
                color,
                pos.is_some_and(|p| p.distance(handle) <= controls::HIT_RADIUS),
            );
            painter.text(
                handle + Vec2::new(8., -8.),
                egui::Align2::LEFT_BOTTOM,
                ["X", "Y", "Z"][i],
                egui::FontId::proportional(12.),
                color,
            );
        }
        if let Some((anchor, handle)) = self.ring_tilt_handle(scene) {
            let color = controls::ACTION;
            painter.line_segment([anchor, handle], Stroke::new(1_f32, color));
            controls::marker(
                painter,
                handle,
                controls::Marker::Diamond,
                color,
                pos.is_some_and(|p| p.distance(handle) <= controls::HIT_RADIUS + 4.),
            );
            painter.text(
                handle + Vec2::new(9., -7.),
                egui::Align2::LEFT_BOTTOM,
                "Tilt",
                egui::FontId::proportional(12.),
                color,
            );
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
        if self.tool == PathTool::Whole
            && let Some(path) = &self.draft
        {
            self.transform.paint(path, scene, painter, pos);
        }
        if self.brush.is_none() {
            painter.text(
                response.rect.center_bottom() - egui::vec2(0., 26.),
                egui::Align2::CENTER_BOTTOM,
                "Path editing · right-drag to orbit · middle-drag to pan · wheel to zoom",
                egui::FontId::proportional(13.),
                controls::NEUTRAL,
            );
        }
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
            offset: 0.,
            ring_orientation: None,
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

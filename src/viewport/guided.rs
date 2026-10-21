//! Saved surface paths constrain ordinary on-model gestures transactionally.
use super::{
    path_follow::{PathCursor, PathRoute},
    scene::Scene,
};
use crate::{
    document::{Document, PathStrokeOptions, PathStrokeSession},
    sculpt::{
        Brush, Kind, Mesh,
        alpha::Projection,
        paths::{PathMode, PathSample},
    },
};
use eframe::egui::{self, Color32, Pos2, Stroke};

struct Guide {
    id: u64,
    route: PathRoute,
    warning: Option<String>,
}
struct Drag {
    guide: usize,
    brush: Brush,
    cursor: PathCursor,
    session: PathStrokeSession,
    pointer: Pos2,
    radius: f32,
    spacing: f32,
    deforms: bool,
    motion: bool,
    pressure_radius: bool,
    pressure_curve: f32,
}
pub(crate) struct GuidedPaths {
    pub(crate) enabled: bool,
    pub(crate) path_starts_only: bool,
    pub(crate) used_path: Option<u64>,
    revision: Option<(u64, u64)>,
    guides: Vec<Guide>,
    drag: Option<Drag>,
    blocked: bool,
}
impl Default for GuidedPaths {
    fn default() -> Self {
        Self {
            enabled: true,
            path_starts_only: false,
            used_path: None,
            revision: None,
            guides: Vec::new(),
            drag: None,
            blocked: false,
        }
    }
}
impl GuidedPaths {
    pub(crate) fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub(super) fn preview(&self) -> Option<&Mesh> {
        self.drag.as_ref().map(|d| d.session.mesh())
    }
    pub(super) fn cancel(&mut self) {
        self.drag = None;
        self.blocked = true;
    }
    fn refresh(&mut self, document: &Document) {
        let key = (document.display_revision(), document.active_model_id());
        if self.drag.is_some() || self.revision == Some(key) {
            return;
        }
        self.revision = Some(key);
        self.guides.clear();
        if !document.active_visible() {
            return;
        }
        for path in document
            .paths()
            .iter()
            .filter(|p| p.target == key.1 && p.mode == PathMode::Surface)
        {
            let dense = path.dense_curve();
            let length: f32 = dense
                .windows(2)
                .map(|p| (p[1].position - p[0].position).len())
                .sum();
            let spacing = (length / 512.).max(0.0001);
            let projected = path.samples_on_mesh(document.mesh(), spacing);
            let warning = projected.as_ref().err().cloned();
            if let Ok(samples) = projected.or_else(|_| path.sample(spacing))
                && let Ok(route) = PathRoute::new(samples, path.closed)
            {
                self.guides.push(Guide {
                    id: path.id,
                    route,
                    warning,
                });
            }
        }
    }
    pub(super) fn paint(&mut self, document: &Document, scene: &Scene, painter: &egui::Painter) {
        if !self.enabled {
            return;
        }
        self.refresh(document);
        for (index, guide) in self.guides.iter().enumerate() {
            let color = if guide.warning.is_some() {
                Color32::from_rgb(245, 150, 90)
            } else {
                Color32::from_rgb(60, 210, 190)
            };
            let count = 512;
            for i in 0..count {
                let a = guide
                    .route
                    .sample(guide.route.length() * i as f32 / count as f32);
                let b = guide
                    .route
                    .sample(guide.route.length() * (i + 1) as f32 / count as f32);
                if front(scene, &a)
                    && let Some(segment) = scene.clip_segment([a.position, b.position])
                {
                    painter.line_segment(segment, Stroke::new(1.5_f32, color));
                }
            }
            if let Some(drag) = self.drag.as_ref().filter(|d| d.guide == index) {
                let sample = guide.route.sample(drag.cursor.distance());
                painter.circle_stroke(
                    scene.project(sample.position),
                    drag.radius * scene.scale_at(sample.position),
                    Stroke::new(1.5_f32, Color32::WHITE),
                );
            }
        }
    }
    /// Returns None for an ordinary free stroke, Some for a consumed guide gesture.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn update(
        &mut self,
        document: &mut Document,
        scene: &Scene,
        brush: &Brush,
        pointer: Option<Pos2>,
        start: Option<Pos2>,
        down: bool,
        released: bool,
        pressure: f32,
        invert: bool,
        stencil: Option<Projection>,
    ) -> Option<Result<bool, String>> {
        if self.blocked {
            if !down {
                self.blocked = false;
            }
            return Some(Ok(false));
        }
        if !self.enabled {
            return None;
        }
        if self.drag.is_none() {
            let start = start?;
            let (hit, _) = scene.pick(start)?;
            self.refresh(document);
            let proximity = brush.radius.clamp(12., 40.);
            let nearest = self
                .guides
                .iter()
                .enumerate()
                .filter_map(|(index, g)| {
                    let distance = g.route.nearest(
                        start,
                        |s| visible(scene, s),
                        None,
                        proximity,
                        f32::INFINITY,
                    )?;
                    let sample = g.route.sample(distance);
                    if g.warning.is_none() && !surface_visible(scene, &sample) {
                        return None;
                    }
                    // A projected guide on another sheet must not capture this surface hit.
                    let world = proximity / scene.scale_at(hit);
                    ((sample.position - hit).len() <= world * 1.5).then_some((
                        index,
                        distance,
                        scene.project(sample.position).distance(start),
                    ))
                })
                .min_by(|a, b| a.2.total_cmp(&b.2));
            let Some((index, distance, _)) = nearest else {
                if self.path_starts_only {
                    self.blocked = down;
                    return Some(Err("Stroke ignored. Start near a surface path, or turn off Only start strokes on paths.".into()));
                }
                return None;
            };
            if let Some(error) = &self.guides[index].warning {
                self.blocked = down;
                return Some(Err(format!(
                    "Reproject this path in Editable paths: {error}"
                )));
            }
            let sample = self.guides[index].route.sample(distance);
            let radius = (brush.radius / scene.scale_at(sample.position)).clamp(0.00001, 1000.);
            let options = PathStrokeOptions {
                radius,
                pressure,
                invert,
                reverse: false,
            };
            let result = document
                .prepare_live_path_stroke(self.guides[index].id, brush, options, stencil)
                .and_then(|request| {
                    let mut session = request.begin_live();
                    session.apply_sample(sample, pressure, invert)?;
                    Ok(session)
                });
            match result {
                Ok(session) => {
                    self.used_path = Some(self.guides[index].id);
                    self.drag = Some(Drag {
                        guide: index,
                        brush: brush.clone(),
                        cursor: PathCursor::new(distance),
                        session,
                        pointer: start,
                        radius,
                        spacing: radius
                            * if brush.kind == Kind::Snake {
                                brush.spacing.min(0.1)
                            } else {
                                brush.spacing
                            },
                        motion: matches!(brush.kind, Kind::Grab | Kind::Snake),
                        pressure_radius: brush.pressure_radius,
                        pressure_curve: brush.pressure_curve,
                        deforms: brush.kind != Kind::Mask,
                    })
                }
                Err(error) => {
                    self.blocked = down;
                    return Some(Err(error));
                }
            }
        }
        let result = self
            .set_brush(scene, brush)
            .and_then(|()| self.advance(scene, pointer, pressure, invert));
        if let Err(error) = result {
            self.drag = None;
            self.blocked = down;
            return Some(Err(error));
        }
        if released || !down {
            let mut drag = self.drag.take().unwrap();
            let route = &self.guides[drag.guide].route;
            let result = (|| {
                if drag.motion
                    && let Some(sample) = drag.cursor.flush_end(route)
                {
                    drag.session.apply_sample(sample, pressure, invert)?;
                }
                let preview = drag.session.finish()?;
                let warning = preview.attachment_warning().map(str::to_owned);
                let changed = document.apply_path_stroke(preview)?;
                if let Some(warning) = warning {
                    return Err(format!(
                        "Stroke applied. Reproject the guide before using it again: {warning}"
                    ));
                }
                Ok(changed && drag.deforms)
            })();
            self.revision = None;
            return Some(result);
        }
        Some(Ok(false))
    }
    fn set_brush(&mut self, scene: &Scene, brush: &Brush) -> Result<(), String> {
        let drag = self.drag.as_mut().unwrap();
        if drag.brush == *brush {
            return Ok(());
        }
        let sample = self.guides[drag.guide].route.sample(drag.cursor.distance());
        let radius = (brush.radius / scene.scale_at(sample.position)).clamp(0.00001, 1000.);
        drag.session.set_brush(brush, radius)?;
        drag.brush = brush.clone();
        drag.radius = radius;
        drag.spacing = radius
            * if brush.kind == Kind::Snake {
                brush.spacing.min(0.1)
            } else {
                brush.spacing
            };
        drag.motion = matches!(brush.kind, Kind::Grab | Kind::Snake);
        drag.deforms |= brush.kind != Kind::Mask;
        drag.pressure_radius = brush.pressure_radius;
        drag.pressure_curve = brush.pressure_curve;
        Ok(())
    }
    fn advance(
        &mut self,
        scene: &Scene,
        pointer: Option<Pos2>,
        pressure: f32,
        invert: bool,
    ) -> Result<(), String> {
        let drag = self.drag.as_mut().unwrap();
        drag.session.begin_frame();
        let Some(pointer) = pointer else {
            return Ok(());
        };
        if pointer == drag.pointer {
            return Ok(());
        }
        let route = &self.guides[drag.guide].route;
        let scale = scene.scale_at(route.sample(drag.cursor.distance()).position);
        let travel = pointer.distance(drag.pointer) / scale * 4. + drag.radius * 2.;
        if let Some(distance) = route.nearest(
            pointer,
            |s| visible(scene, s),
            Some(drag.cursor.distance()),
            f32::MAX,
            travel,
        ) {
            // Test the traversed section against the immutable source scene. A
            // route behind another sheet must never receive hidden dabs.
            let from = drag.cursor.distance();
            let checks = (((distance - from).abs() * scale / 8.).ceil() as usize).clamp(1, 512);
            let unobscured = (1..=checks).all(|i| {
                surface_visible(
                    scene,
                    &route.sample(from + (distance - from) * i as f32 / checks as f32),
                )
            });
            if unobscured {
                for sample in drag.cursor.advance(
                    route,
                    distance,
                    (drag.spacing
                        * if drag.pressure_radius {
                            pressure.powf(drag.pressure_curve).max(0.05)
                        } else {
                            1.
                        })
                    .max(0.00001),
                )? {
                    drag.session.apply_sample(sample, pressure, invert)?;
                }
            }
        }
        drag.pointer = pointer;
        Ok(())
    }
}
fn front(scene: &Scene, sample: &PathSample) -> bool {
    let view = if scene.eye_distance == 0. {
        scene.view
    } else {
        scene.view * scene.eye_distance - sample.position
    };
    sample.normal.dot(view) > 0. && scene.visible_depth(sample.position)
}
fn visible(scene: &Scene, sample: &PathSample) -> Option<Pos2> {
    front(scene, sample).then(|| scene.project(sample.position))
}

fn surface_visible(scene: &Scene, sample: &PathSample) -> bool {
    front(scene, sample)
        && scene
            .pick(scene.project(sample.position))
            .is_some_and(|(hit, _)| {
                (hit - sample.position).len()
                    <= (0.75 / scene.scale_at(sample.position)).max(0.0001)
            })
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use crate::{camera::Camera, viewport::gpu::DisplayMesh};
    use std::sync::Arc;

    #[test]
    #[ignore = "requires CLAY_PATH_SESSION diagnostic export directory"]
    fn diagnostic_session_guided_detail_traverses_visible_surface() {
        let directory = std::path::PathBuf::from(
            std::env::var("CLAY_PATH_SESSION").expect("diagnostic export directory"),
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap())
                .unwrap();
        // The retained session shows the small, apparently stalled pass at13→14.
        let edit = manifest["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|edit| edit["from"] == 13)
            .unwrap();
        let settings = &edit["settings_at_edit_start"];
        let view = &settings["view_input"]["camera"];
        let camera = Camera {
            yaw: view["yaw"].as_f64().unwrap() as f32,
            pitch: view["pitch"].as_f64().unwrap() as f32,
            zoom: view["zoom"].as_f64().unwrap() as f32,
            ..Camera::default()
        };
        let mut document = Document::default();
        let project =
            crate::persistence::load_project(directory.join("stage-013.clay").to_str().unwrap())
                .unwrap();
        document
            .replace_project(project, "session-review.clay")
            .unwrap();
        let brush: Brush = serde_json::from_value(settings["active_brush"].clone()).unwrap();
        let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1200., 916.));
        let scene =
            Scene::from_geometry(rect, &camera, Arc::new(DisplayMesh::new(&document)), true);
        let mut guides = GuidedPaths::default();
        guides.refresh(&document);
        assert_eq!(guides.guides.len(), 1);
        assert!(
            guides.guides[0].warning.is_none(),
            "{:?}",
            guides.guides[0].warning
        );
        let route = &guides.guides[0].route;
        let samples: Vec<_> = (0..=512)
            .map(|i| route.sample(route.length() * i as f32 / 512.))
            .collect();
        let mut best = (0usize, 0usize);
        let mut start = 0;
        for (i, sample) in samples.iter().enumerate() {
            if !surface_visible(&scene, sample) {
                start = i + 1;
            } else if i + 1 - start > best.1 - best.0 {
                best = (start, i + 1);
            }
        }
        assert!(
            best.1 - best.0 > 24,
            "needs a substantial visible surface route: {best:?}"
        );
        let samples = &samples[best.0 + 2..best.1 - 2];
        let before = document.mesh().clone();
        let history = document.history_counts();
        let first = scene.project(samples[0].position);
        guides
            .update(
                &mut document,
                &scene,
                &brush,
                Some(first),
                Some(first),
                true,
                false,
                1.,
                false,
                None,
            )
            .unwrap()
            .unwrap();
        assert!(guides.dragging());
        for sample in samples.iter().skip(1) {
            let pointer = scene.project(sample.position);
            guides
                .update(
                    &mut document,
                    &scene,
                    &brush,
                    Some(pointer),
                    None,
                    true,
                    false,
                    1.,
                    false,
                    None,
                )
                .unwrap()
                .unwrap();
        }
        let preview = guides.drag.as_ref().unwrap().session.mesh();
        let changed = before
            .vertices
            .iter()
            .zip(&preview.vertices)
            .filter(|(a, b)| (**a - **b).len() > 1e-6)
            .count();
        let mut covered = 0;
        for sample in samples.iter().step_by((samples.len() / 8).max(1)) {
            let local = before
                .vertices
                .iter()
                .zip(&preview.vertices)
                .any(|(a, b)| (*a - sample.position).len() < 0.08 && (*a - *b).len() > 1e-5);
            covered += usize::from(local);
        }
        eprintln!(
            "Session stage13 visible route {} samples, {changed} moved vertices, {covered} covered sections",
            samples.len()
        );
        assert!(
            covered >= 7,
            "only the start dab was applied: {covered} sections"
        );
        assert_eq!(document.mesh(), &before, "live stroke remains private");
        let last = scene.project(samples.last().unwrap().position);
        guides
            .update(
                &mut document,
                &scene,
                &brush,
                Some(last),
                None,
                false,
                true,
                1.,
                false,
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(document.history_counts().0, history.0 + 1);
        assert!(!document.paths()[0].is_stale(document.mesh()));
    }
}

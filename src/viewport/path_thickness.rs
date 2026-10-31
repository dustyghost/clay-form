//! Inline controls edit the same private assembly preview as the sidebar.
use super::{ViewportOptions, controls, navigation, scene::Scene};
use crate::{
    document::Document,
    sculpt::{
        Vec3,
        instances::*,
        paths::{PathSample, SurfacePath},
    },
};
use eframe::egui::{self, Pos2, Vec2};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Start,
    End,
    Path(usize),
    Section(usize),
}
impl Kind {
    fn label(self) -> String {
        match self {
            Self::Start => "Start".into(),
            Self::End => "End".into(),
            Self::Path(i) => format!("Path point {}", i + 1),
            Self::Section(i) => format!("Section {}", i + 1),
        }
    }
}
#[derive(Clone, Copy)]
struct Station {
    kind: Kind,
    fraction: f32,
    origin: Vec3,
    tangent: Vec3,
}
struct Cache {
    path: SurfacePath,
    count: usize,
    spacing: f32,
    fit_count: bool,
    caps: (bool, bool),
    curve: Vec<Vec3>,
    stations: Vec<Station>,
}
impl Cache {
    fn matches(&self, path: &SurfacePath, recipe: &PathAssembly) -> bool {
        self.path == *path
            && self.count == recipe.count
            && self.spacing == recipe.spacing
            && self.fit_count == recipe.fit_count
            && self.caps == (recipe.start.is_some(), recipe.end.is_some())
    }
    fn new(path: &SurfacePath, recipe: &PathAssembly) -> Result<Self, String> {
        let placements = recipe.placements(path)?;
        let length: f32 = path
            .dense_curve()
            .windows(2)
            .map(|p| (p[1].position - p[0].position).len())
            .sum();
        let mut samples = path.sample((length / 1024.).max(1e-5))?;
        if path.closed {
            let mut seam = *samples.first().ok_or("Path has no samples")?;
            seam.distance = length;
            samples.push(seam);
        }
        if samples.len() < 2 {
            return Err("Path needs at least two samples".into());
        }
        let length = samples.last().unwrap().distance;
        let at = |kind, distance: f32| {
            let i = samples
                .partition_point(|s| s.distance < distance)
                .clamp(1, samples.len() - 1);
            station(kind, &samples[i - 1], &samples[i], distance, length)
        };
        let mut stations = vec![at(Kind::Start, 0.)];
        if !path.closed {
            stations.push(at(Kind::End, length));
        }
        // Search only the bounded display samples, once per path/layout change.
        for (index, point) in path.points.iter().enumerate() {
            let mut best = (f32::INFINITY, 0.);
            for pair in samples.windows(2) {
                let delta = pair[1].position - pair[0].position;
                let t = ((point.position - pair[0].position).dot(delta)
                    / delta.dot(delta).max(1e-12))
                .clamp(0., 1.);
                let error = point.position - (pair[0].position + delta * t);
                let error = error.dot(error);
                if error < best.0 {
                    best = (
                        error,
                        pair[0].distance + (pair[1].distance - pair[0].distance) * t,
                    );
                }
            }
            stations.push(at(Kind::Path(index), best.1));
        }
        for (index, distance) in recipe
            .section_distances(length, path.closed, placements.len())
            .into_iter()
            .enumerate()
        {
            stations.push(at(Kind::Section(index), distance));
        }
        Ok(Self {
            path: path.clone(),
            count: recipe.count,
            spacing: recipe.spacing,
            fit_count: recipe.fit_count,
            caps: (recipe.start.is_some(), recipe.end.is_some()),
            curve: samples.iter().map(|s| s.position).collect(),
            stations,
        })
    }
}
fn station(kind: Kind, a: &PathSample, b: &PathSample, distance: f32, length: f32) -> Station {
    let t = (distance - a.distance) / (b.distance - a.distance).max(1e-8);
    Station {
        kind,
        fraction: distance / length,
        origin: a.position * (1. - t) + b.position * t,
        tangent: (a.tangent * (1. - t) + b.tangent * t).norm(),
    }
}
#[derive(Clone, Copy)]
struct Handle {
    station: Station,
    anchor: Pos2,
    position: Pos2,
    axis: Vec2,
}
struct Drag {
    handle: Handle,
    pointer: Pos2,
    factor: f32,
    before: Set,
}

#[derive(Default)]
pub(crate) struct Editor {
    set: Option<u64>,
    path_points: bool,
    sections: bool,
    cache: Option<Cache>,
    drag: Option<Drag>,
    selected: Option<Kind>,
    pub(crate) updated: Option<Set>,
    pub(crate) notice: Option<String>,
    #[cfg(test)]
    pub(crate) cache_builds: usize,
}
impl Editor {
    pub(crate) fn configure(&mut self, set: Option<u64>, path_points: bool, sections: bool) {
        if self.set != set {
            self.reset();
        }
        self.set = set;
        self.path_points = path_points;
        self.sections = sections;
    }
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn show(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        document: &mut Document,
        options: ViewportOptions,
        gizmo: egui::Rect,
    ) -> bool {
        if !document.instance_preview_active() {
            self.reset();
            return false;
        }
        let Some(set) = self
            .set
            .and_then(|id| document.instances().sets.iter().find(|s| s.id == id))
            .cloned()
        else {
            return false;
        };
        let Layout::Path(recipe) = &set.layout else {
            return false;
        };
        if !recipe.bend {
            self.cache = None;
            self.drag = None;
            return false;
        }
        let Some(path) = document.path(recipe.path) else {
            return false;
        };
        if self.cache.as_ref().is_none_or(|c| !c.matches(path, recipe)) {
            match Cache::new(path, recipe) {
                Ok(cache) => {
                    self.cache = Some(cache);
                    #[cfg(test)]
                    {
                        self.cache_builds += 1;
                    }
                }
                Err(error) => {
                    self.notice = Some(error);
                    return true;
                }
            }
        }
        let cache = self.cache.as_ref().unwrap();
        let closed = cache.path.closed;
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let down = ui.input(|i| i.pointer.primary_down());
        let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if (!options.enabled || escaped)
            && let Some(drag) = self.drag.take()
        {
            match document.preview_instance_set(drag.before.clone()) {
                Ok(()) => self.updated = Some(drag.before),
                Err(error) => self.notice = Some(error),
            }
            return true;
        }
        for pair in cache.curve.windows(2) {
            if let Some(line) = scene.clip_segment([pair[0], pair[1]]) {
                painter.line_segment(
                    line,
                    egui::Stroke::new(1_f32, controls::SHAPE.gamma_multiply(0.45)),
                );
            }
        }
        let mut handles = Vec::new();
        let mut occupied = HashSet::new();
        for &station in &cache.stations {
            if (matches!(station.kind, Kind::Path(_)) && !self.path_points)
                || (matches!(station.kind, Kind::Section(_)) && !self.sections)
                || !scene.visible_depth(station.origin)
            {
                continue;
            }
            // Start/End already represent any coincident point or repeat centre.
            if matches!(station.kind, Kind::Path(_) | Kind::Section(_))
                && (station.fraction < 1e-5 || station.fraction > 1. - 1e-5)
            {
                continue;
            }
            let anchor = scene.project(station.origin);
            let direction = scene.project(station.origin + station.tangent) - anchor;
            let mut axis = if direction.length_sq() > 1e-6 {
                egui::vec2(-direction.y, direction.x).normalized()
            } else {
                egui::vec2(0., -1.)
            };
            if matches!(station.kind, Kind::Section(_)) {
                axis = -axis;
            }
            let factor = recipe.thickness.at(station.fraction, closed);
            let distance = 24. + factor * 18.;
            let bounds = response.rect.shrink(22.);
            let clear = |p| {
                bounds.contains(p)
                    && !gizmo.contains(p)
                    && ui
                        .ctx()
                        .layer_id_at(p)
                        .is_none_or(|layer| layer == response.layer_id)
            };
            if !clear(anchor + axis * distance) && clear(anchor - axis * distance) {
                axis = -axis;
            }
            let position = anchor + axis * distance;
            if !clear(position) || navigation::contains(response.rect, position) {
                continue;
            }
            let cell = (
                (position.x / 22.).floor() as i32,
                (position.y / 22.).floor() as i32,
            );
            if !occupied.insert(cell) && self.selected != Some(station.kind) {
                continue;
            }
            handles.push(Handle {
                station,
                anchor,
                position,
                axis,
            });
        }
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                egui::Id::new("path_thickness_handles"),
                handles
                    .iter()
                    .map(|h| {
                        (
                            h.station.kind.label(),
                            h.position,
                            h.axis,
                            h.station.fraction,
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let hovered = pointer.and_then(|p| {
            handles
                .iter()
                .filter(|h| h.position.distance(p) <= controls::HIT_RADIUS)
                .min_by(|a, b| {
                    a.position
                        .distance_sq(p)
                        .total_cmp(&b.position.distance_sq(p))
                })
                .copied()
        });
        for handle in &handles {
            let kind = handle.station.kind;
            let (marker, color) = match kind {
                Kind::Start | Kind::End => (controls::Marker::Square, controls::ACTION),
                Kind::Path(_) => (controls::Marker::Diamond, controls::SHAPE),
                Kind::Section(_) => (controls::Marker::Circle, controls::TOPOLOGY),
            };
            let highlighted =
                self.selected == Some(kind) || hovered.is_some_and(|h| h.station.kind == kind);
            painter.line_segment(
                [handle.anchor, handle.position],
                egui::Stroke::new(1_f32, color.gamma_multiply(0.7)),
            );
            controls::marker(painter, handle.position, marker, color, highlighted);
            if highlighted || matches!(kind, Kind::Start | Kind::End) {
                let label = format!(
                    "{} · {:.2}×",
                    kind.label(),
                    recipe.thickness.at(handle.station.fraction, closed)
                );
                let x = if handle.position.x > response.rect.center().x {
                    -12. - painter
                        .layout_no_wrap(label.clone(), egui::FontId::proportional(12.), color)
                        .size()
                        .x
                } else {
                    10.
                };
                controls::label(
                    painter,
                    handle.position + egui::vec2(x, -20.),
                    &label,
                    color,
                );
            }
        }
        let modifiers = ui.input(|i| i.modifiers);
        if options.enabled
            && response.hovered()
            && !escaped
            && !options.pan_hold.active(modifiers)
            && !options.zoom_hold.active(modifiers)
            && ui.input(|i| i.pointer.primary_pressed())
            && let (Some(handle), Some(pointer)) = (hovered, pointer)
        {
            self.selected = Some(handle.station.kind);
            self.drag = Some(Drag {
                handle,
                pointer,
                factor: recipe.thickness.at(handle.station.fraction, closed),
                before: set.clone(),
            });
        }
        let capturing = self.drag.is_some();
        if let Some(drag) = &self.drag
            && let Some(pointer) = pointer
        {
            let factor = (drag.factor + (pointer - drag.pointer).dot(drag.handle.axis) / 60.)
                .clamp(MIN_THICKNESS, MAX_THICKNESS);
            let mut candidate = drag.before.clone();
            let Layout::Path(a) = &mut candidate.layout else {
                unreachable!()
            };
            let result = if (factor - drag.factor).abs() < 1e-6 {
                Ok(())
            } else {
                a.thickness
                    .edit_at(drag.handle.station.fraction, factor, closed)
            }
            .and_then(|()| {
                if candidate == set {
                    Ok(())
                } else {
                    document.preview_instance_set(candidate.clone())
                }
            });
            match result {
                Ok(()) => self.updated = Some(candidate),
                Err(error) => self.notice = Some(error),
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if hovered.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if !down {
            self.drag = None;
        }
        capturing
    }
}

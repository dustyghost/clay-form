//! Live single-stamp previews and line guides; document edits happen on release.
use super::{gpu::Renderer, scene::Scene};
use crate::{
    document::Document,
    sculpt::{AdaptiveBudget, Brush, Dab, Mesh, StrokeMode, Vec3},
};
use eframe::egui::{self, Color32, Pos2};
pub(super) struct PendingStroke {
    pub start: Pos2,
    pub end: Pos2,
    pub brush: Brush,
    pub pressure: f32,
    pub invert: bool,
    pub anchor: (Vec3, Vec3),
}
impl PendingStroke {
    fn radius(&self) -> f32 {
        let radius = if self.brush.stroke_mode == StrokeMode::DragRect {
            self.start.distance(self.end).clamp(1., 500.)
        } else {
            self.brush.radius
        };
        radius
            * if self.brush.pressure_radius {
                self.pressure.powf(self.brush.pressure_curve).max(0.05)
            } else {
                1.
            }
    }
    fn posed_brush(&self) -> Brush {
        let mut brush = self.brush.clone();
        if brush.stroke_mode == StrokeMode::DragRect {
            brush.radius = self.start.distance(self.end).clamp(1., 500.);
            let direction = self.end - self.start;
            if direction.length_sq() > 1. {
                brush.rotation += direction.y.atan2(direction.x).to_degrees();
            }
        }
        brush
    }
    pub fn paint(&self, painter: &egui::Painter, scene: &Scene, renderer: Option<&Renderer>) {
        let brush = self.posed_brush();
        let radius = self.radius();
        let target = if brush.stroke_mode == StrokeMode::DragRect {
            self.start
        } else {
            self.end
        };
        let color = if brush.subtract ^ self.invert {
            Color32::from_rgb(255, 140, 100)
        } else {
            Color32::from_rgb(125, 210, 245)
        };
        if let Some(renderer) = renderer {
            renderer.set_brush(scene, target, radius, &brush, color);
        }
        let surface = brush.surface_cursor
            && scene.brush_cursor_with_footprint(
                painter,
                target,
                radius,
                &brush,
                color,
                renderer.is_none(),
            );
        if !surface {
            painter.circle_stroke(target, radius, egui::Stroke::new(1.5_f32, color));
            if brush.inner_radius > 0. {
                painter.circle_stroke(
                    target,
                    radius * brush.inner_radius,
                    egui::Stroke::new(1_f32, color),
                );
            }
        }
        if matches!(brush.stroke_mode, StrokeMode::Line | StrokeMode::DragRect) {
            painter.line_segment([self.start, self.end], egui::Stroke::new(1.5_f32, color));
            painter.circle_filled(self.start, 3., color);
        }
        painter.text(
            target + egui::vec2(12., -20.),
            egui::Align2::LEFT_BOTTOM,
            "Release to apply · Esc to cancel",
            egui::FontId::proportional(12.),
            color,
        );
    }
    pub fn is_stamp(&self) -> bool {
        matches!(
            self.brush.stroke_mode,
            StrokeMode::DragRect | StrokeMode::DragDot
        )
    }
    pub fn preview_mesh(&self, document: &Document, scene: &Scene) -> Mesh {
        let mut mesh = document.mesh().clone();
        let brush = self.posed_brush();
        let mut budget = AdaptiveBudget::default();
        let settings = &document.mesh_settings().adaptive;
        for dab in self.dabs(scene) {
            let dabs = document.symmetry().dabs(dab);
            mesh.adapt(&brush, &dabs, settings, &mut budget, true);
            mesh.prepare_drag_stamp(&brush, &dabs, settings);
            mesh.stamp_many(&brush, &dabs);
            mesh.adapt(&brush, &dabs, settings, &mut budget, false);
        }
        mesh
    }
    pub fn commit(self, document: &mut Document, scene: &Scene) {
        let brush = self.posed_brush();
        for dab in self.dabs(scene) {
            document.apply_dab(&brush, dab);
        }
    }
    fn dabs(&self, scene: &Scene) -> Vec<Dab> {
        let brush = self.posed_brush();
        let radius = self.radius();
        let make_dab = |(center, normal)| Dab {
            center,
            normal,
            right: scene.right,
            up: scene.up,
            radius: radius / scene.scale,
            pressure: self.pressure,
            delta: Vec3::default(),
            invert: self.invert,
        };
        let mut dabs = Vec::new();
        match brush.stroke_mode {
            StrokeMode::Line => {
                let distance = self.start.distance(self.end);
                let steps = (distance / (radius * brush.spacing).max(1.))
                    .ceil()
                    .clamp(1., 256.) as usize;
                let last = if distance < 1. { 0 } else { steps };
                for i in 0..=last {
                    let point = self.start + (self.end - self.start) * (i as f32 / steps as f32);
                    if let Some(hit) = scene.pick(point) {
                        dabs.push(make_dab(hit));
                    }
                }
            }
            StrokeMode::DragRect => {
                dabs.push(make_dab(self.anchor));
            }
            StrokeMode::DragDot => {
                if let Some(hit) = scene.pick(self.end) {
                    dabs.push(make_dab(hit));
                }
            }
            StrokeMode::Freehand => unreachable!("freehand strokes apply incrementally"),
        }
        dabs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        camera::Camera,
        sculpt::{Geometry, Kind, Primitive, PrimitiveKind},
    };

    #[test]
    fn live_stamp_is_rendered_while_picking_uses_original_surface() {
        let mut document = Document::default();
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [20, 20, 1];
        document
            .replace_geometry(Geometry::new(plane.mesh().unwrap()))
            .unwrap();
        let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(400.));
        let mut brush = Brush::new(Kind::Mask);
        brush.stroke_mode = StrokeMode::DragRect;
        brush.strength = 1.;
        let stroke = PendingStroke {
            start: rect.center(),
            end: rect.center() + egui::vec2(80., 0.),
            brush,
            pressure: 1.,
            invert: false,
            anchor: (Vec3::default(), Vec3(0., 0., 1.)),
        };
        let ctx = egui::Context::default();
        let render = |pending: Option<&PendingStroke>| {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(rect),
                    ..Default::default()
                },
                |ctx| {
                    let scene = Scene::paint_with_stroke(
                        &ctx.layer_painter(egui::LayerId::background()),
                        rect,
                        &document,
                        &Camera::default(),
                        false,
                        super::super::WireMode::Visible,
                        false,
                        None,
                        pending,
                    );
                    assert!(
                        scene.geometry.mesh.mask.iter().all(|&m| m == 0.),
                        "picking scene stays unchanged"
                    );
                },
            );
            output
                .shapes
                .into_iter()
                .find_map(|s| match s.shape {
                    egui::Shape::Mesh(mesh) => {
                        Some(mesh.vertices.iter().map(|v| v.color).collect::<Vec<_>>())
                    }
                    _ => None,
                })
                .unwrap()
        };
        let before = render(None);
        let preview = render(Some(&stroke));
        assert_ne!(
            before, preview,
            "mask preview must reach the displayed surface"
        );
        assert_eq!(
            before,
            render(None),
            "canceling restores the displayed surface"
        );
        assert!(document.mesh().mask.iter().all(|&m| m == 0.));
    }

    #[test]
    fn live_stamp_matches_commit_without_buildup_or_document_edits() {
        for mode in [StrokeMode::DragRect, StrokeMode::DragDot] {
            for adaptive in [false, true] {
                let mut document = Document::default();
                let mut plane = Primitive::for_kind(if adaptive {
                    PrimitiveKind::Cube
                } else {
                    PrimitiveKind::Plane
                });
                plane.divisions = [10, 10, 10];
                document
                    .replace_geometry(Geometry::new(plane.mesh().unwrap()))
                    .unwrap();
                document
                    .set_adaptive(crate::sculpt::AdaptiveSettings {
                        enabled: adaptive,
                        ..Default::default()
                    })
                    .unwrap();
                let original = document.geometry_snapshot();
                let history = document.history_counts();
                let dirty = document.is_dirty();
                let ctx = egui::Context::default();
                let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(400.));
                let mut scene = None;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    scene = Some(Scene::paint(
                        &ctx.layer_painter(egui::LayerId::background()),
                        rect,
                        &document,
                        &Camera::default(),
                        false,
                        super::super::WireMode::Visible,
                        false,
                    ));
                });
                let scene = scene.unwrap();
                let mut brush = Brush::new(Kind::Standard);
                brush.stroke_mode = mode;
                brush.radius = 60.;
                let stroke = PendingStroke {
                    start: rect.center(),
                    end: rect.center() + egui::vec2(60., 0.),
                    anchor: scene.pick(rect.center()).unwrap(),
                    brush,
                    pressure: 0.75,
                    invert: false,
                };
                let preview = stroke.preview_mesh(&document, &scene);
                assert!(preview.vertices != document.mesh().vertices);
                assert!(
                    preview == stroke.preview_mesh(&document, &scene),
                    "holding still must not accumulate depth"
                );
                assert!(document.geometry_snapshot() == original);
                assert_eq!(document.history_counts(), history);
                assert_eq!(document.is_dirty(), dirty);
                stroke.commit(&mut document, &scene);
                document.end_stroke();
                assert!(
                    *document.mesh() == preview,
                    "preview and released stamp must match, including adaptive topology"
                );
                assert_eq!(document.history_counts().0, history.0 + 1);
                document.undo();
                assert!(document.geometry_snapshot() == original);
            }
        }
    }
}

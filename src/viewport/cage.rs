use super::{controls, gpu::DisplayMesh, navigation, scene::Scene};
use crate::{
    document::Document,
    sculpt::{
        Axis, Vec3,
        cage::{Adjustment, Cage},
    },
};
use eframe::egui::{self, Color32, Pos2, Vec2};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
enum Handle {
    Corner(usize),
    Bend,
    Twist,
}
struct Drag {
    handle: Handle,
    start: Pos2,
    anchor: Vec3,
    edit: Option<Adjustment>,
    scratch: Option<crate::sculpt::Mesh>,
}
#[derive(Default)]
pub(super) struct Editor {
    fitted: Option<(u64, Cage)>,
    drag: Option<Drag>,
    pub preview: Option<Arc<DisplayMesh>>,
}
impl Editor {
    pub fn sync(&mut self, document: &Document, enabled: bool) {
        if !enabled || !document.active_visible() || document.primitive().is_some() {
            *self = Self::default();
        } else if self
            .fitted
            .is_none_or(|(rev, _)| rev != document.display_revision())
        {
            *self = Self {
                fitted: Cage::fit(document.mesh()).map(|c| (document.display_revision(), c)),
                ..Default::default()
            };
        }
    }
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        document: &mut Document,
        axis: Axis,
        respect_mask: bool,
        allow_press: bool,
        gizmo: egui::Rect,
    ) -> Result<bool, String> {
        let Some((_, cage)) = self.fitted else {
            return Ok(false);
        };
        let current = self.drag.as_ref().and_then(|d| d.edit);
        let deform = |p| current.map_or(p, |edit| cage.point(p, edit));
        let teal = Color32::from_rgb(78, 224, 207);
        let mut handles = Vec::<(Handle, Vec3, Pos2)>::new();
        for i in 0..8 {
            let p = cage.corner(i);
            for bit in [1, 2, 4] {
                if i & bit != 0 {
                    continue;
                }
                let q = cage.corner(i | bit);
                let mut last = deform(p);
                for j in 1..=12 {
                    let next = deform(p + (q - p) * (j as f32 / 12.));
                    if let Some(line) = scene.clip_segment([last, next]) {
                        painter.line_segment(line, egui::Stroke::new(1.4_f32, teal));
                    }
                    last = next;
                }
            }
            let world = deform(p);
            if !scene.visible_depth(world) {
                continue;
            }
            let anchor = scene.project(world);
            if !scene.rect.expand(20.).contains(anchor) {
                continue;
            }
            let mut pos = anchor;
            // Front and back corners coincide in axis-aligned views.
            while handles
                .iter()
                .any(|(_, _, other)| other.distance(pos) < 18.)
            {
                pos.x += 20.;
            }
            painter.line_segment([anchor, pos], egui::Stroke::new(1_f32, teal));
            handles.push((Handle::Corner(i), p, pos));
        }
        let along = axis.vector();
        let end = (cage.low + cage.high) * 0.5 + along * ((cage.high - cage.low).dot(along) * 0.5);
        let p = scene.project(deform(end));
        for (handle, offset) in [(Handle::Bend, -40.), (Handle::Twist, 40.)] {
            if !scene.visible_depth(deform(end)) || !scene.rect.expand(60.).contains(p) {
                break;
            }
            let mut pos = p + Vec2::new(offset, -22.);
            while handles
                .iter()
                .any(|(_, _, other)| other.distance(pos) < 21.)
            {
                pos.y -= 22.;
            }
            handles.push((handle, end, pos));
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let hit_at = |pos| {
            handles
                .iter()
                .filter(|(_, _, p)| p.distance(pos) <= 12.)
                .min_by(|a, b| a.2.distance(pos).total_cmp(&b.2.distance(pos)))
                .copied()
        };
        let hit = pointer.and_then(hit_at);
        for &(h, _, pos) in &handles {
            let selected = self.drag.as_ref().is_some_and(|d| d.handle == h)
                || hit.is_some_and(|target| target.0 == h);
            let color = if selected {
                Color32::WHITE
            } else if matches!(h, Handle::Corner(_)) {
                teal
            } else {
                controls::ACTION
            };
            let radius = if matches!(h, Handle::Corner(_)) {
                5.5
            } else {
                10.
            };
            painter.circle_filled(pos, radius, color);
            painter.circle_stroke(pos, radius, egui::Stroke::new(1.5_f32, Color32::BLACK));
            if h == Handle::Bend || h == Handle::Twist {
                painter.text(
                    pos,
                    egui::Align2::CENTER_CENTER,
                    if h == Handle::Bend { "B" } else { "T" },
                    egui::FontId::proportional(13.),
                    Color32::BLACK,
                );
            }
        }
        if let Some(edit) = current {
            let label = match edit {
                Adjustment::Pull(_, d) => {
                    format!("Pull   ΔX {:+.2}   ΔY {:+.2}   ΔZ {:+.2}", d.0, d.1, d.2)
                }
                Adjustment::Bend(axis, a) => {
                    format!("Bend along {axis:?}   {:+.1}°", a.to_degrees())
                }
                Adjustment::Twist(axis, a) => {
                    format!("Twist around {axis:?}   {:+.1}°", a.to_degrees())
                }
            };
            painter.text(
                scene.rect.center_bottom() - Vec2::new(0., 28.),
                egui::Align2::CENTER_BOTTOM,
                label,
                egui::FontId::proportional(16.),
                Color32::WHITE,
            );
        }
        if self.drag.is_none()
            && allow_press
            && response.contains_pointer()
            && let Some(start) = ui.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        ..
                    } => Some(*pos),
                    _ => None,
                })
            })
            && let Some((handle, anchor, _)) = hit_at(start)
            && !gizmo.contains(start)
            && !navigation::contains(scene.rect, start)
        {
            self.drag = Some(Drag {
                handle,
                start,
                anchor,
                edit: None,
                scratch: None,
            });
        }
        let Some(drag) = self.drag.as_mut() else {
            return Ok(false);
        };
        if pointer.is_none() && !ui.input(|i| i.pointer.primary_down()) {
            self.drag = None;
            self.preview = None;
            return Ok(true);
        }
        if let Some(pos) = pointer {
            let angle =
                ((pos.x - drag.start.x) * 0.012).clamp(-std::f32::consts::PI, std::f32::consts::PI);
            let edit = match drag.handle {
                Handle::Corner(i) => Adjustment::Pull(
                    i,
                    scene.at_depth(pos, drag.anchor) - scene.at_depth(drag.start, drag.anchor),
                ),
                Handle::Bend => Adjustment::Bend(axis, angle),
                Handle::Twist => Adjustment::Twist(axis, angle),
            };
            if ui.input(|i| i.pointer.primary_released()) {
                self.drag = None;
                self.preview = None;
                document.apply_cage(cage, edit, respect_mask)?;
                ui.ctx().request_repaint();
            } else if drag.edit != Some(edit) {
                let vertices = cage.vertices(document.mesh(), edit, respect_mask)?;
                self.preview = Some(Arc::new(scene.geometry.cage_preview(
                    document,
                    vertices,
                    &mut drag.scratch,
                )));
                drag.edit = Some(edit);
                ui.ctx().request_repaint();
            }
        }
        Ok(true)
    }
}

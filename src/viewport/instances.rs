//! Mark the placement or set being edited without capturing viewport input.
use super::*;
use egui::{Rect, Stroke};

pub(crate) const COPY_COLOR: Color32 = Color32::from_rgb(255, 198, 82);
pub(crate) const SET_COLOR: Color32 = Color32::from_rgb(101, 216, 243);

fn brackets(painter: &egui::Painter, rect: Rect, color: Color32) {
    let length = (rect.width().min(rect.height()) * 0.25).clamp(3., 12.);
    for (corner, dx, dy) in [
        (rect.left_top(), length, length),
        (rect.right_top(), -length, length),
        (rect.left_bottom(), length, -length),
        (rect.right_bottom(), -length, -length),
    ] {
        let points = [
            corner + Vec2::new(dx, 0.),
            corner,
            corner + Vec2::new(0., dy),
        ];
        for stroke in [
            Stroke::new(4_f32, Color32::from_black_alpha(220)),
            Stroke::new(2_f32, color),
        ] {
            painter.line_segment([points[0], points[1]], stroke);
            painter.line_segment([points[1], points[2]], stroke);
        }
    }
}

fn paint_copy_wire(scene: &Scene, painter: &egui::Painter, owner: usize) {
    // Keep the surface readable inside the frame, including when two copies overlap.
    let faces = scene.instance_faces(owner);
    for tri in faces.iter().step_by(faces.len().div_ceil(160).max(1)) {
        let points = tri.map(|i| scene.geometry.mesh.vertices[i]);
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            if let Some(line) = scene.clip_segment([points[a], points[b]]) {
                painter.line_segment(line, Stroke::new(1_f32, COPY_COLOR.gamma_multiply(0.65)));
            }
        }
    }
}

pub(super) fn paint(
    scene: &Scene,
    painter: &egui::Painter,
    document: &Document,
    editor: Option<u64>,
) {
    let editor = editor.filter(|_| document.instance_preview_active());
    let selected = document.selected_instance;
    let Some(id) = editor.or_else(|| selected.map(|(id, _)| id)) else {
        return;
    };
    let Some(set) = document.instances().sets.iter().find(|s| s.id == id) else {
        return;
    };
    let slot = selected
        .filter(|(owner, _)| *owner == id && !set.joined())
        .map(|(_, slot)| slot);
    let color = if slot.is_some() {
        COPY_COLOR
    } else {
        SET_COLOR
    };
    let view = navigation::inner(scene.rect).shrink(4.);
    if !view.is_positive() {
        return;
    }
    let painter = painter.with_clip_rect(view);
    let mut anchor = None;
    let mut count = 0;
    if let Ok(placements) = document.instance_placements() {
        for (i, &(owner, copy, _)) in placements.iter().enumerate() {
            if owner != id || slot.is_some_and(|s| s != copy) {
                continue;
            }
            count += 1;
            let display_owner = document.model_count() + i;
            let frame = scene
                .instance_bounds(display_owner)
                .expand(4.)
                .intersect(view);
            if frame.is_positive() {
                if slot.is_some() {
                    paint_copy_wire(scene, &painter, display_owner);
                }
                brackets(&painter, frame, color);
                anchor = Some(frame);
            }
        }
    }
    let state = if editor.is_some() {
        "Editing"
    } else {
        "Selected"
    };
    let mut label = format!("{state} · {}", set.target_name(slot));
    if !set.visible
        || slot.is_some_and(|slot| set.adjustments.iter().any(|a| a.slot == slot && !a.visible))
    {
        label.push_str(" · Hidden");
    } else if set.joined() {
        label.push_str(" · Joined assembly");
    } else if slot.is_none() {
        label.push_str(&format!(
            " · {count} visible {}",
            if count == 1 { "copy" } else { "copies" }
        ));
    } else if anchor.is_none() {
        label.push_str(" · Out of view");
    }
    let anchor = anchor.filter(|_| slot.is_some() || set.joined());
    let galley = painter.layout(
        label,
        egui::FontId::proportional(13.),
        color,
        (view.width() - 24.).max(1.),
    );
    let size = galley.size() + Vec2::new(12., 6.);
    let mut position = view.left_top() + Vec2::new(4., 24.);
    let corner = anchor.map(|frame| {
        // Point the callout inward, away from the viewport navigation controls along the edges.
        if frame.center().x > view.center().x {
            position = frame.left_top() - Vec2::new(size.x + 12., size.y + 5.);
            frame.left_top()
        } else {
            position = frame.right_top() + Vec2::new(12., -size.y - 5.);
            frame.right_top()
        }
    });
    position.x = position
        .x
        .clamp(view.left(), (view.right() - size.x).max(view.left()));
    position.y = position
        .y
        .clamp(view.top(), (view.bottom() - size.y).max(view.top()));
    let badge = Rect::from_min_size(position, size);
    if let Some(corner) = corner {
        painter.line_segment([badge.clamp(corner), corner], Stroke::new(1_f32, color));
    }
    painter.rect_filled(badge, 3., Color32::from_rgb(22, 28, 37));
    painter.galley(badge.min + Vec2::new(6., 3.), galley, color);
}

#[derive(Default)]
pub(crate) struct CopyMove {
    drag: Option<MoveDrag>,
    pub(crate) updated: Option<crate::sculpt::instances::Set>,
}
struct MoveDrag {
    target: (u64, usize),
    start: Pos2,
    pivot: Vec3,
    axis: Option<(Vec3, Vec2)>,
    before: crate::sculpt::instances::Set,
}
impl CopyMove {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn show(
        &mut self,
        scene: &Scene,
        painter: &egui::Painter,
        response: &egui::Response,
        document: &mut Document,
        options: ViewportOptions,
        press: Option<Pos2>,
        gizmo: Rect,
    ) -> bool {
        use crate::sculpt::instances::Adjustment;
        let Some(target @ (id, slot)) = document.selected_instance.filter(|(id, _)| {
            options.instance_editor == Some(*id) && document.instance_preview_active()
        }) else {
            self.drag = None;
            return false;
        };
        let Some(set) = document
            .instances()
            .sets
            .iter()
            .find(|s| s.id == id && !s.joined())
        else {
            self.drag = None;
            return false;
        };
        let Some(pivot) = document.instance_placements().ok().and_then(|resolved| {
            resolved
                .iter()
                .find(|(s, n, _)| (*s, *n) == target)
                .map(|(_, _, p)| p.transform.origin)
        }) else {
            self.drag = None;
            return false;
        };
        let ctx = painter.ctx();
        if self.drag.as_ref().is_some_and(|d| d.target != target) {
            self.drag = None;
        }
        let origin = scene.project(pivot);
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let mut hit = None;
        #[cfg(test)]
        let mut handles = vec![("Move freely", origin)];
        if scene.visible_depth(pivot) {
            for (axis, projected, color, label) in transform::world_axes(scene, pivot) {
                let mut tip = origin + projected.normalized() * 65.;
                if !navigation::inner(scene.rect).contains(tip) {
                    tip = origin - projected.normalized() * 65.;
                }
                #[cfg(test)]
                handles.push((label, tip));
                let hovered = press
                    .or(pointer)
                    .is_some_and(|p| p.distance(tip) < controls::HIT_RADIUS);
                painter.arrow(origin, tip - origin, Stroke::new(2_f32, color));
                controls::marker(painter, tip, controls::Marker::Circle, color, hovered);
                controls::label(painter, tip + Vec2::new(6., -16.), label, color);
                if hovered {
                    hit = Some(Some((axis, projected)));
                }
            }
            let centre = press
                .or(pointer)
                .is_some_and(|p| p.distance(origin) < controls::HIT_RADIUS);
            controls::marker(
                painter,
                origin,
                controls::Marker::Diamond,
                COPY_COLOR,
                centre,
            );
            if centre {
                hit = Some(None);
            }
        }
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("instance_move_handles"), handles));
        let mods = ctx.input(|i| i.modifiers);
        if options.enabled
            && !options.pan_hold.active(mods)
            && !options.zoom_hold.active(mods)
            && let Some((pos, axis)) = press.zip(hit)
            && response.rect.contains(pos)
            && !gizmo.contains(pos)
            && !navigation::contains(scene.rect, pos)
            && ctx.layer_id_at(pos) == Some(response.layer_id)
        {
            self.drag = Some(MoveDrag {
                target,
                start: pos,
                pivot,
                axis,
                before: set.clone(),
            });
        }
        let Some(drag) = &self.drag else { return false };
        if !options.enabled || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            let before = drag.before.clone();
            if document.preview_instance_set(before.clone()).is_ok() {
                self.updated = Some(before);
            }
            self.drag = None;
            return true;
        }
        if let Some(point) = pointer {
            let delta = drag.axis.map_or_else(
                || scene.at_depth(point, drag.pivot) - scene.at_depth(drag.start, drag.pivot),
                |(axis, projected)| {
                    transform::axis_motion(scene, drag.pivot, axis, projected, drag.start, point)
                },
            );
            let mut candidate = drag.before.clone();
            if delta != Vec3::default() {
                let a = if let Some(i) = candidate.adjustments.iter().position(|a| a.slot == slot) {
                    &mut candidate.adjustments[i]
                } else {
                    candidate.adjustments.push(Adjustment::new(slot));
                    candidate.adjustments.last_mut().unwrap()
                };
                a.offset = a.offset + delta;
            }
            if candidate != *set && document.preview_instance_set(candidate.clone()).is_ok() {
                self.updated = Some(candidate);
            }
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.drag = None;
        }
        true
    }
}

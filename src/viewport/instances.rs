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

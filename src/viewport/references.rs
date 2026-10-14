use crate::{
    document::Document,
    reference::{Placement, ReferenceImage},
};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct ReferenceOverlay {
    textures: Vec<(Arc<[u8]>, egui::TextureHandle)>,
    drag: Option<Drag>,
    pub selected: Option<usize>,
}
struct Drag {
    index: usize,
    start: Pos2,
    rect: Rect,
    viewport: Rect,
    before: Placement,
    preview: Placement,
    resize: bool,
}
pub(super) fn image_rect(image: &ReferenceImage, placement: Placement, viewport: Rect) -> Rect {
    let aspect = image.pixels.size[1] as f32 / image.pixels.size[0] as f32;
    let width = (placement.width * viewport.width()).min(viewport.height() * 0.85 / aspect);
    let size = Vec2::new(width, width * aspect);
    let center = viewport.min
        + Vec2::new(
            placement.center[0] * viewport.width(),
            placement.center[1] * viewport.height(),
        );
    let center = Pos2::new(
        center.x.clamp(
            viewport.left() + size.x / 2.,
            viewport.right() - size.x / 2.,
        ),
        center.y.clamp(
            viewport.top() + size.y / 2.,
            viewport.bottom() - size.y / 2.,
        ),
    );
    Rect::from_center_size(center, size)
}
impl ReferenceOverlay {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub fn cancel_drag(&mut self) {
        self.drag = None;
        self.selected = None;
    }
    pub fn show(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        document: &mut Document,
        options: super::ViewportOptions,
    ) {
        let (enabled, arranging, visible) = (
            options.enabled,
            options.arrange_references,
            options.references_visible,
        );
        let viewport = response.rect;
        self.textures.retain(|(pixels, _)| {
            document
                .references()
                .iter()
                .any(|image| Arc::ptr_eq(pixels, &image.pixels.rgba))
        });
        if viewport.width() <= 1. || viewport.height() <= 1. {
            self.drag = None;
            return;
        }
        if !visible || !arranging || !enabled || !ui.input(|i| i.focused) {
            self.drag = None;
        }
        if !visible {
            return;
        }
        if self.drag.as_ref().is_some_and(|d| {
            d.viewport != viewport
                || document
                    .references()
                    .get(d.index)
                    .is_none_or(|image| image.placement != d.before)
        }) {
            self.drag = None;
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        if arranging
            && enabled
            && response.hovered()
            && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
            && let Some(pos) = pointer
            && !super::navigation::contains(viewport, pos)
            && let Some((index, image, rect)) = document
                .references()
                .iter()
                .enumerate()
                .rev()
                .filter(|(_, image)| image.placement.visible && !image.placement.locked)
                .map(|(i, image)| (i, image, image_rect(image, image.placement, viewport)))
                .find(|(_, _, rect)| rect.expand(8.).contains(pos))
        {
            self.selected = Some(index);
            self.drag = Some(Drag {
                index,
                start: pos,
                rect,
                viewport,
                before: image.placement,
                preview: image.placement,
                resize: pos.distance(rect.right_bottom()) <= 16.,
            });
        }
        if let Some(drag) = &mut self.drag
            && let Some(pos) = pointer
        {
            let image = &document.references()[drag.index];
            if drag.resize {
                let delta = pos - drag.start;
                let aspect = drag.rect.height() / drag.rect.width();
                let width = (drag.rect.width()
                    + (delta.x + delta.y * aspect) / (1. + aspect * aspect))
                    .max(viewport.width() * 0.05)
                    .min(viewport.width() * 0.9)
                    .min(viewport.height() * 0.85 / aspect);
                drag.preview.width = (width / viewport.width()).clamp(0.05, 0.9);
                let center = drag.rect.min + Vec2::new(width, width * aspect) * 0.5;
                drag.preview.center = [
                    (center.x - viewport.left()) / viewport.width(),
                    (center.y - viewport.top()) / viewport.height(),
                ]
                .map(|n| n.clamp(0., 1.));
            } else {
                let center = drag.rect.center() + (pos - drag.start);
                drag.preview.center = [
                    (center.x - viewport.left()) / viewport.width(),
                    (center.y - viewport.top()) / viewport.height(),
                ]
                .map(|n| n.clamp(0., 1.));
            }
            let clamped = image_rect(image, drag.preview, viewport);
            drag.preview.center = [
                (clamped.center().x - viewport.left()) / viewport.width(),
                (clamped.center().y - viewport.top()) / viewport.height(),
            ];
            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                let _ = document.place_reference(drag.index, drag.preview);
                self.drag = None;
            } else if !ui.input(|i| i.pointer.primary_down()) {
                self.drag = None;
            }
        }
        for (index, image) in document.references().iter().enumerate() {
            if !image.placement.visible {
                continue;
            }
            let placement = self
                .drag
                .as_ref()
                .filter(|d| d.index == index)
                .map_or(image.placement, |d| d.preview);
            let rect = image_rect(image, placement, viewport);
            let texture = if let Some((_, texture)) = self
                .textures
                .iter()
                .find(|(pixels, _)| Arc::ptr_eq(pixels, &image.pixels.rgba))
            {
                texture
            } else {
                let texture = ui.ctx().load_texture(
                    "reference image",
                    egui::ColorImage::from_rgba_unmultiplied(image.pixels.size, &image.pixels.rgba),
                    egui::TextureOptions::LINEAR,
                );
                self.textures.push((image.pixels.rgba.clone(), texture));
                &self.textures.last().unwrap().1
            };
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE.gamma_multiply(placement.opacity),
            );
            if arranging {
                let color = if placement.locked {
                    Color32::GRAY
                } else {
                    Color32::from_rgb(80, 205, 210)
                };
                painter.rect_stroke(
                    rect,
                    0.,
                    egui::Stroke::new(1_f32, color),
                    egui::StrokeKind::Inside,
                );
                if !placement.locked {
                    painter.rect_filled(
                        Rect::from_center_size(
                            rect.right_bottom() - Vec2::splat(5.),
                            Vec2::splat(10.),
                        ),
                        2.,
                        color,
                    );
                }
                if enabled
                    && response.hovered()
                    && pointer.is_some_and(|p| rect.contains(p))
                    && !placement.locked
                {
                    ui.ctx().set_cursor_icon(
                        if pointer.unwrap().distance(rect.right_bottom()) <= 16. {
                            egui::CursorIcon::ResizeNwSe
                        } else {
                            egui::CursorIcon::Grab
                        },
                    );
                }
            }
        }
    }
}

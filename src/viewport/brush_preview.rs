//! Brush outlines can be painted independently of sculpting or pointer ownership.
use super::*;

impl Viewport {
    pub(super) fn paint_brush_outline(
        &self,
        scene: &Scene,
        painter: &egui::Painter,
        position: Pos2,
        radius: f32,
        brush: &Brush,
        color: Color32,
    ) {
        let surface = if brush.surface_cursor {
            if let Some(renderer) = &self.renderer {
                renderer.set_brush(scene, position, radius, brush, color);
                scene.brush_cursor_with_footprint(painter, position, radius, brush, color, false)
            } else {
                scene.brush_cursor(painter, position, radius, brush, color)
            }
        } else {
            false
        };
        if !surface {
            for fraction in [1., brush.inner_radius] {
                if fraction > 0. {
                    painter.circle_stroke(
                        position,
                        radius * fraction,
                        egui::Stroke::new(1.5_f32, color),
                    );
                }
            }
        }
    }
}

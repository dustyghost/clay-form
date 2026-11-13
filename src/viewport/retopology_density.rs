use super::*;
impl Viewport {
    pub(super) fn paint_retopology_density(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        document: &mut Document,
        options: ViewportOptions,
    ) -> bool {
        let Some((mode, radius, strength)) = self.session.density_paint.filter(|_| options.enabled)
        else {
            if self.session.density_last.take().is_some() {
                document.end_stroke();
            }
            return false;
        };
        let down = response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        let click = response.clicked_by(egui::PointerButton::Primary);
        let pointer = response
            .interact_pointer_pos()
            .or_else(|| response.hover_pos());
        let mut brush = crate::sculpt::Brush::new(Kind::Mask);
        brush.radius = radius;
        brush.strength = strength;
        let color = match mode {
            crate::sculpt::retopology::DensityPaint::More => Color32::from_rgb(245, 125, 60),
            crate::sculpt::retopology::DensityPaint::Less => Color32::from_rgb(70, 160, 250),
            crate::sculpt::retopology::DensityPaint::Automatic => Color32::LIGHT_GRAY,
        };
        if let Some(pos) = response.hover_pos() {
            self.paint_brush_outline(scene, painter, pos, radius, &brush, color);
        }
        let navigation = ui.input(|i| {
            options.pan_hold.active(i.modifiers) || options.zoom_hold.active(i.modifiers)
        });
        let mut consumed = self.session.density_last.is_some();
        if (down || click)
            && !navigation
            && let Some(pos) = pointer
        {
            let previous = self.session.density_last.unwrap_or(pos);
            let distance = previous.distance(pos);
            if self.session.density_last.is_none() || distance >= (radius * 0.15).max(2.) {
                let steps = (distance / (radius * 0.15).max(2.)).ceil().clamp(1., 64.) as usize;
                for i in 1..=steps {
                    let point = previous + (pos - previous) * (i as f32 / steps as f32);
                    if let Some((center, normal)) = scene.pick(point) {
                        let dab = crate::sculpt::Dab {
                            center,
                            normal,
                            right: scene.right,
                            up: scene.up,
                            radius: radius / scene.scale_at(center),
                            pressure: options.pressure,
                            delta: Vec3::default(),
                            invert: false,
                        };
                        match document.paint_retopology_density(&brush, dab, mode) {
                            Ok(_) => {
                                consumed = true;
                                self.session.density_last = Some(pos);
                            }
                            Err(e) => {
                                self.status = Some(e);
                                document.end_stroke();
                            }
                        }
                    }
                }
                if consumed {
                    ui.ctx().request_repaint();
                }
            }
        }
        if !down && self.session.density_last.take().is_some() {
            document.end_stroke();
        }
        consumed
    }
}

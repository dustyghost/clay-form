//! The evaluated surface is inspectable. Transform handles act on the selected source.
use super::*;
use std::sync::Arc;
pub(super) const RESULT_OWNER: usize = usize::MAX;

impl Viewport {
    pub(crate) fn set_boolean_result(&mut self, document: &Document, mesh: &crate::sculpt::Mesh) {
        self.boolean_display = Some(Arc::new(gpu::DisplayMesh::with_boolean_result(
            document, mesh,
        )));
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn show_boolean(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        document: &mut Document,
        camera: &mut Camera,
        options: ViewportOptions,
        gizmo: egui::Rect,
    ) {
        let rect = response.rect;
        let scene = if let Some(display) = self.boolean_display.clone() {
            Scene::paint_cached_preview(
                painter,
                rect,
                camera,
                self.renderer.as_mut(),
                display,
                options.wire,
                options.wire_mode,
                options.wire_triangles,
            )
        } else {
            Scene::paint_with_renderer(
                painter,
                rect,
                document,
                camera,
                options.wire,
                options.wire_mode,
                options.wire_triangles,
                self.renderer.as_mut(),
            )
        };
        if let Some(renderer) = &self.renderer {
            renderer.set_backfaces(options.backface_opacity);
            renderer.set_polygroups(options.show_polygroups);
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let modifiers = ui.input(|i| i.modifiers);
        let transform = options.transform_mode != TransformMode::Sculpt
            && options.transform_operation != TransformOperation::Cage;
        if transform
            && options.enabled
            && response.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.primary_pressed())
            && let Some(pos) = pointer
            && !gizmo.contains(pos)
            && !navigation::contains(rect, pos)
            && !options.zoom_hold.active(modifiers)
            && !options.pan_hold.active(modifiers)
            && self.session.transform.can_drag(
                document,
                &scene,
                options.transform_mode,
                options.transform_operation,
                pos,
            )
        {
            self.session.gesture = Some(Gesture::Transform);
        }
        if self.session.gesture == Some(Gesture::Transform) {
            if options.enabled
                && let Err(e) = self.session.transform.update(
                    document,
                    &scene,
                    options.transform_mode,
                    options.transform_operation,
                    pointer,
                    true,
                    options.respect_mask,
                )
            {
                self.status = Some(e);
            }
            if !options.enabled || !ui.input(|i| i.pointer.primary_down()) {
                self.session.transform.finish_drag();
                document.end_stroke();
                self.session.gesture = None;
            }
        } else if options.enabled {
            if response.hovered() {
                camera.zoom_by(ui.input(|i| i.smooth_scroll_delta.y));
            }
            if response.dragged() {
                let delta = ui.input(|i| i.pointer.delta());
                if options.zoom_hold.active(modifiers) {
                    camera.zoom_drag(delta);
                } else if options.pan_hold.active(modifiers)
                    || response.dragged_by(egui::PointerButton::Middle)
                {
                    camera.pan += delta;
                } else {
                    camera.orbit(delta);
                }
            } else if response.clicked_by(egui::PointerButton::Primary)
                && let Some(pos) = pointer
            {
                if gizmo.contains(pos) {
                    if let Some((yaw, pitch)) =
                        super::gizmo::snap_target(camera, gizmo.center(), pos)
                    {
                        camera.yaw = yaw;
                        camera.pitch = pitch;
                        camera.roll = 0.;
                    }
                } else if self.boolean_current
                    && let Some((owner, _, _)) = scene.pick_model(pos)
                {
                    if owner < document.model_count() && owner != document.active_model_index() {
                        if let Err(e) = document.select_submodel(owner) {
                            self.status = Some(e);
                        }
                    } else if owner != RESULT_OWNER && owner >= document.model_count() {
                        document.select_instance_owner(owner);
                    }
                }
            }
        }
        if transform {
            self.session.transform.paint(
                document,
                &scene,
                painter,
                options.transform_mode,
                options.transform_operation,
            );
        }
        navigation::paint(
            painter,
            rect,
            response
                .hover_pos()
                .is_some_and(|p| navigation::contains(rect, p)),
        );
        draw_gizmo(camera, painter, gizmo, response.hover_pos());
        controls::label(
            painter,
            rect.center_bottom() + Vec2::new(-75., -52.),
            &self.boolean_notice,
            controls::NEUTRAL,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{BooleanRole, Geometry, Mesh, ModelMetadata, Primitive, Submodel};
    #[test]
    fn boolean_stack_display_picks_the_result_and_keeps_regular_rows_separate() {
        let solid = Primitive {
            size: Vec3(0.5, 0.5, 0.5),
            divisions: [1; 3],
            ..Default::default()
        };
        let center = solid.mesh().unwrap();
        let mut geometry = Geometry::new(center.clone());
        geometry.scene.active_metadata.boolean_role = BooleanRole::Subtract;
        for (role, x) in [(BooleanRole::Regular, 1.), (BooleanRole::Add, -1.)] {
            let mut p = solid.clone();
            p.center.0 = x;
            geometry.submodels.push(Submodel {
                name: role.label().into(),
                metadata: ModelMetadata {
                    boolean_role: role,
                    ..Default::default()
                },
                mesh: p.mesh().unwrap(),
                mesh_settings: Default::default(),
            });
        }
        let mut document = Document::default();
        document.replace_geometry(geometry).unwrap();
        let rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.));
        for (mesh, center_owner) in [(Mesh::default(), None), (center, Some(RESULT_OWNER))] {
            let display = Arc::new(gpu::DisplayMesh::with_boolean_result(&document, &mesh));
            let scene = Scene::from_geometry(rect, &Camera::default(), display, true);
            let owner = |x| {
                scene
                    .pick_model(scene.project(Vec3(x, 0., 0.)))
                    .map(|hit| hit.0)
            };
            assert_eq!(owner(0.), center_owner);
            assert_eq!(owner(1.), Some(1));
            assert_eq!(owner(-1.), None);
        }
    }
}

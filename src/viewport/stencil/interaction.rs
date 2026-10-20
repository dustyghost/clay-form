//! Direct stencil placement. Drags preview privately and commit only on release.
use super::*;
use crate::viewport::controls::{self, Marker};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Handle {
    Move,
    Rotate,
    Corner,
    Width,
    Height,
}
impl Handle {
    fn label(self) -> &'static str {
        match self {
            Self::Move => "Move stencil",
            Self::Rotate => "Rotate · Shift: snap 15°",
            Self::Corner => "Resize · Shift: stretch",
            Self::Width => "Resize width",
            Self::Height => "Resize height",
        }
    }
}
struct Drag {
    handle: Handle,
    original: Stencil,
    start: Vec2,
    preview: Stencil,
}
#[derive(Default)]
pub(in crate::viewport) struct Controls {
    drag: Option<Drag>,
    capture: bool,
    hide_handles: bool,
}
impl Controls {
    pub fn capturing(&self) -> bool {
        self.capture || self.drag.is_some()
    }
    pub fn preview(&self) -> Option<&Stencil> {
        self.drag.as_ref().map(|d| &d.preview)
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        document: &mut Document,
        scene: &Scene,
        enabled: bool,
    ) -> bool {
        let (pointer, pressed, down, escape, shift, focused) = ui.input(|i| {
            (
                i.pointer.interact_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.key_pressed(egui::Key::Escape),
                i.modifiers.shift,
                i.focused,
            )
        });
        let mut consumed = self.capture;
        if !down {
            self.capture = false;
        }
        if !enabled || escape || !focused {
            consumed |= self.drag.take().is_some();
            self.capture |= consumed && down;
        }
        let Some(original) = document.stencil().cloned() else {
            self.drag = None;
            return consumed;
        };
        if self
            .drag
            .as_ref()
            .is_some_and(|drag| drag.original != original)
        {
            self.drag = None;
            self.capture |= down;
            consumed = true;
        }
        let mut edited = original.clone();
        let mut remove = false;
        let area = egui::Area::new(egui::Id::new("stencil_canvas_controls"))
            .order(egui::Order::Foreground)
            .movable(false)
            .fixed_pos(scene.rect.left_top() + Vec2::new(12., 42.))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width((scene.rect.width() - 48.).clamp(160., 330.));
                    ui.add_enabled_ui(enabled && self.drag.is_none(), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.checkbox(&mut edited.enabled, "Stencil enabled");
                            let mut handles = !self.hide_handles;
                            ui.checkbox(&mut handles, "Handles");
                            self.hide_handles = !handles;
                        });
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Reset placement").clicked() {
                                edited.position = [0.; 2];
                                edited.rotation = 0.;
                                let size = edited.surface.as_ref().map_or(0.5, |frame| {
                                    scene.rect.width().min(scene.rect.height())
                                        / scene.scale_at(frame.origin)
                                        * 0.5
                                });
                                edited.size = [size; 2];
                            }
                            remove = ui.button("Remove stencil").clicked();
                        });
                        ui.add(
                            egui::Slider::new(&mut edited.opacity, 0. ..=1.).text("Guide opacity"),
                        );
                    });
                });
            });
        let over_palette = pointer.is_some_and(|p| area.response.rect.contains(p));
        if over_palette && pressed && enabled {
            self.capture = true;
        }
        consumed |= (over_palette && enabled) || self.capture;
        if remove {
            let _ = document.set_stencil(None);
            self.drag = None;
            return true;
        }
        if edited != original {
            let _ = document.set_stencil(Some(edited));
            return true;
        }
        let displayed = self.preview().unwrap_or(&original);
        let handles = handles(displayed, scene);
        let hover = pointer.and_then(|p| {
            handles
                .iter()
                .find(|(_, q)| {
                    scene.rect.shrink(controls::HIT_RADIUS).contains(*q)
                        && p.distance(*q) <= controls::HIT_RADIUS
                })
                .copied()
        });
        let handle_hover = enabled
            && !self.hide_handles
            && !over_palette
            && hover.is_some()
            && pointer.is_some_and(|p| {
                scene.rect.contains(p) && ui.ctx().layer_id_at(p) == Some(ui.layer_id())
            });
        consumed |= handle_hover;
        if handle_hover {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if !self.hide_handles {
            for &(handle, point) in &handles {
                if !scene.rect.shrink(controls::HIT_RADIUS).contains(point) {
                    continue;
                }
                controls::marker(
                    ui.painter(),
                    point,
                    if handle == Handle::Rotate {
                        Marker::Circle
                    } else {
                        Marker::Square
                    },
                    if handle == Handle::Move {
                        controls::ACTION
                    } else {
                        controls::SHAPE
                    },
                    hover.is_some_and(|(h, p)| h == handle && p == point),
                );
            }
            if let Some((handle, point)) = hover {
                controls::label(
                    ui.painter(),
                    point + Vec2::new(15., 12.),
                    handle.label(),
                    controls::NEUTRAL,
                );
            }
        }
        if handle_hover
            && focused
            && !escape
            && pressed
            && !self.capture
            && let Some((handle, _)) = hover
            && let Some(pointer) = pointer
        {
            self.capture = true;
            consumed = true;
            if let Some(start) = plane_point(&original, scene, pointer) {
                self.drag = Some(Drag {
                    handle,
                    start,
                    original: original.clone(),
                    preview: original.clone(),
                });
            }
        }
        if let Some(drag) = &mut self.drag {
            if let Some(point) = pointer.and_then(|p| plane_point(&drag.original, scene, p)) {
                drag.preview = transformed(&drag.original, drag.handle, drag.start, point, shift);
            }
            consumed = true;
            ui.ctx().request_repaint();
            if !down {
                let preview = self.drag.take().unwrap().preview;
                let _ = document.set_stencil(Some(preview));
            }
        }
        consumed
    }
}
fn plane_point(stencil: &Stencil, scene: &Scene, point: Pos2) -> Option<Vec2> {
    if let Some(frame) = &stencil.surface {
        let (origin, ray) = scene.ray(point);
        let normal = frame.right.cross(frame.up);
        let divisor = ray.dot(normal);
        if divisor.abs() < 0.02 {
            return None;
        }
        let world = origin + ray * ((frame.origin - origin).dot(normal) / divisor);
        if !scene.visible_depth(world) {
            return None;
        }
        let offset = world - frame.origin;
        Some(Vec2::new(offset.dot(frame.right), offset.dot(frame.up)))
    } else {
        let offset = (point - scene.rect.center()) / scene.rect.width().min(scene.rect.height());
        Some(Vec2::new(offset.x, -offset.y))
    }
}
fn handles(stencil: &Stencil, scene: &Scene) -> Vec<(Handle, Pos2)> {
    let projection = project(stencil, scene);
    let (s, c) = stencil.rotation.to_radians().sin_cos();
    let mut corners = Vec::new();
    for [x, y] in [[-0.5, 0.5], [0.5, 0.5], [0.5, -0.5], [-0.5, -0.5]] {
        let x = x * projection.size[0];
        let y = y * projection.size[1];
        let point = projection.frame.origin
            + projection.frame.right * (projection.position[0] + x * c - y * s)
            + projection.frame.up * (projection.position[1] + x * s + y * c);
        if !scene.visible_depth(point) {
            return Vec::new();
        }
        corners.push(scene.project(point));
    }
    let top = corners[0].lerp(corners[1], 0.5);
    let bottom = corners[2].lerp(corners[3], 0.5);
    let outward = (top - bottom).normalized();
    let mut result = vec![
        (Handle::Move, top + outward * 19.),
        (Handle::Rotate, top + outward * 49.),
        (Handle::Width, corners[1].lerp(corners[2], 0.5)),
        (Handle::Height, bottom),
    ];
    result.extend(corners.into_iter().map(|p| (Handle::Corner, p)));
    result
}
fn transformed(
    original: &Stencil,
    handle: Handle,
    start: Vec2,
    current: Vec2,
    shift: bool,
) -> Stencil {
    let mut result = original.clone();
    let center = Vec2::new(original.position[0], original.position[1]);
    if handle == Handle::Move {
        let position = center + current - start;
        result.position = [
            position.x.clamp(-10000., 10000.),
            position.y.clamp(-10000., 10000.),
        ];
    } else if handle == Handle::Rotate {
        let a = start - center;
        let b = current - center;
        if a.length_sq() > 1e-10 && b.length_sq() > 1e-10 {
            let angle = original.rotation + (b.y.atan2(b.x) - a.y.atan2(a.x)).to_degrees();
            result.rotation = (if shift {
                (angle / 15.).round() * 15.
            } else {
                angle
            } + 180.)
                .rem_euclid(360.)
                - 180.;
        }
    } else {
        let (s, c) = original.rotation.to_radians().sin_cos();
        let local = |p: Vec2| {
            let p = p - center;
            Vec2::new(p.x * c + p.y * s, -p.x * s + p.y * c)
        };
        let a = local(start);
        let b = local(current);
        let ratio = |x: f32, y: f32| {
            if x.abs() > 1e-6 {
                (y / x).max(0.001)
            } else {
                1.
            }
        };
        let factors = if handle == Handle::Corner && !shift {
            let scale = (a.dot(b) / a.length_sq().max(1e-10)).max(0.001);
            [scale; 2]
        } else {
            [
                if handle == Handle::Height {
                    1.
                } else {
                    ratio(a.x, b.x)
                },
                if handle == Handle::Width {
                    1.
                } else {
                    ratio(a.y, b.y)
                },
            ]
        };
        result.size = std::array::from_fn(|i| (original.size[i] * factors[i]).clamp(0.001, 10000.));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{camera::Camera, sculpt::alpha::Asset, viewport::gpu::DisplayMesh};
    use std::sync::Arc;
    #[test]
    fn stencil_controls_surface_drag_uses_plane_coordinates_in_both_cameras() {
        let document = Document::default();
        for perspective in [false, true] {
            let scene = Scene::from_geometry(
                egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 800.)),
                &Camera {
                    perspective,
                    yaw: 0.4,
                    pitch: 0.2,
                    ..Default::default()
                },
                Arc::new(DisplayMesh::new(&document)),
                false,
            );
            let frame = Frame {
                origin: Vec3(0., 0., 0.6),
                right: Vec3(1., 0., 0.),
                up: Vec3(0., 1., 0.),
            };
            let mut stencil = Stencil::new(Asset {
                name: "Test".into(),
                category: String::new(),
                image: Alpha::Soft,
            });
            stencil.surface = Some(frame.clone());
            stencil.rotation = 30.;
            for uv in [Vec2::new(0.2, -0.1), Vec2::new(-0.3, 0.4)] {
                let screen = scene.project(frame.origin + frame.right * uv.x + frame.up * uv.y);
                assert!((plane_point(&stencil, &scene, screen).unwrap() - uv).length() < 1e-5);
            }
            let result = transformed(
                &stencil,
                Handle::Move,
                Vec2::ZERO,
                Vec2::new(0.2, -0.1),
                false,
            );
            assert_eq!(result.position, [0.2, -0.1]);
            assert_eq!(result.surface, stencil.surface);
            let start = Vec2::new(30_f32.to_radians().cos(), 30_f32.to_radians().sin()) * 0.25;
            let width = transformed(&stencil, Handle::Width, start, start * 2., false);
            assert!((width.size[0] - 1.).abs() < 1e-6);
            assert_eq!(width.size[1], 0.5);
        }
    }
}

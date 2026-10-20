//! View-fixed and planar surface stencil projection and placement controls.
use super::scene::Scene;
mod interaction;
use crate::{
    document::Document,
    sculpt::{
        Alpha, Vec3,
        alpha::{Frame, Projection, Stencil},
    },
};
use eframe::egui::{self, Color32, Pos2, Vec2};

pub(super) fn project(stencil: &Stencil, scene: &Scene) -> Projection {
    let (frame, view, distance, position, size) = if let Some(frame) = &stencil.surface {
        (
            frame.clone(),
            frame.right.cross(frame.up),
            0.,
            stencil.position,
            stencil.size,
        )
    } else {
        let unit = scene.rect.width().min(scene.rect.height()) / scene.scale;
        let pan = (scene.rect.center() - scene.center) / scene.scale;
        (
            Frame {
                origin: Vec3::default(),
                right: scene.right,
                up: scene.up,
            },
            scene.view,
            scene.eye_distance,
            [
                stencil.position[0] * unit + pan.x,
                stencil.position[1] * unit - pan.y,
            ],
            [stencil.size[0] * unit, stencil.size[1] * unit],
        )
    };
    Projection {
        image: stencil.asset.image.clone(),
        frame,
        view,
        distance,
        position,
        size,
        rotation: stencil.rotation,
    }
}
pub(super) fn attach(document: &mut Document, scene: &Scene, pos: Pos2) -> Result<(), String> {
    let (origin, normal) = scene
        .pick(pos)
        .ok_or("Click a visible part of the active model")?;
    let mut stencil = document.stencil().cloned().ok_or("Place a stencil first")?;
    let right = (scene.right - normal * scene.right.dot(normal)).norm();
    if right.len() < 0.5 {
        return Err("Orbit to face the surface before attaching".into());
    }
    if stencil.surface.is_none() {
        let scale = scene.rect.width().min(scene.rect.height()) / scene.scale_at(origin);
        stencil.size = stencil.size.map(|v| v * scale);
    }
    stencil.surface = Some(Frame {
        origin,
        right,
        up: normal.cross(right).norm(),
    });
    stencil.position = [0.; 2];
    document.set_stencil(Some(stencil))
}
#[derive(Default)]
pub(super) struct Overlay {
    pub controls: interaction::Controls,
    texture: Option<(Alpha, egui::TextureHandle)>,
}
impl Overlay {
    pub fn paint(&mut self, stencil: Option<&Stencil>, scene: &Scene, painter: &egui::Painter) {
        let Some(stencil) = stencil.filter(|s| s.enabled) else {
            return;
        };
        super::controls::label(
            painter,
            scene.rect.left_bottom() + Vec2::new(12., -40.),
            &format!("Stencil: {}", stencil.asset.name),
            Color32::from_rgb(160, 225, 215),
        );
        if stencil.opacity <= 0. {
            return;
        }
        if self
            .texture
            .as_ref()
            .is_none_or(|(a, _)| *a != stencil.asset.image)
        {
            let a = &stencil.asset.image;
            let pixels = (0..256 * 256)
                .map(|i| {
                    Color32::from_rgba_unmultiplied(
                        130,
                        220,
                        210,
                        (a.sample((i % 256) as f32 / 255., (i / 256) as f32 / 255.) * 255.) as u8,
                    )
                })
                .collect();
            self.texture = Some((
                a.clone(),
                painter.ctx().load_texture(
                    "placed stencil",
                    egui::ColorImage::new([256, 256], pixels),
                    egui::TextureOptions::LINEAR,
                ),
            ));
        }
        let projection = project(stencil, scene);
        let (s, c) = stencil.rotation.to_radians().sin_cos();
        let point = |u: f32, v: f32| {
            let x = (u - 0.5) * projection.size[0];
            let y = (0.5 - v) * projection.size[1];
            projection.frame.origin
                + projection.frame.right * (projection.position[0] + x * c - y * s)
                + projection.frame.up * (projection.position[1] + x * s + y * c)
        };
        let corners = [point(0., 0.), point(1., 0.), point(1., 1.), point(0., 1.)];
        if corners.iter().any(|p| !scene.visible_depth(*p)) {
            return;
        }
        let mut mesh = egui::Mesh::with_texture(self.texture.as_ref().unwrap().1.id());
        // A small grid keeps perspective interpolation aligned with the projector.
        for y in 0..=16 {
            for x in 0..=16 {
                let uv = Pos2::new(x as f32 / 16., y as f32 / 16.);
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: scene.project(point(uv.x, uv.y)),
                    uv,
                    color: Color32::WHITE.gamma_multiply(stencil.opacity),
                });
            }
        }
        for y in 0..16 {
            for x in 0..16 {
                let i = y * 17 + x;
                mesh.indices
                    .extend_from_slice(&[i, i + 1, i + 17, i + 1, i + 18, i + 17]);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
        for i in 0..4 {
            painter.line_segment(
                [
                    scene.project(corners[i]),
                    scene.project(corners[(i + 1) % 4]),
                ],
                egui::Stroke::new(1_f32, Color32::from_rgb(100, 180, 175)),
            );
        }
        let label = scene.project(corners[0]) + Vec2::new(4., -4.);
        painter.text(
            label,
            egui::Align2::LEFT_BOTTOM,
            format!(
                "{} · {}",
                stencil.asset.name,
                if stencil.surface.is_some() {
                    "surface stencil"
                } else {
                    "view stencil"
                }
            ),
            egui::FontId::proportional(12.),
            Color32::from_rgb(160, 225, 215),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        camera::Camera,
        sculpt::{Brush, Geometry, Kind, Primitive, PrimitiveKind, StrokeMode, alpha::Asset},
        viewport::{gpu::DisplayMesh, stroke::PendingStroke},
    };
    use std::sync::Arc;
    fn scene(d: &Document, camera: &Camera) -> Scene {
        Scene::from_geometry(
            egui::Rect::from_min_size(Pos2::new(20., 50.), Vec2::new(900., 700.)),
            camera,
            Arc::new(DisplayMesh::new(d)),
            false,
        )
    }
    fn stencil() -> Stencil {
        Stencil::new(Asset {
            name: "Test".into(),
            category: String::new(),
            image: Alpha::Image {
                width: 2,
                height: 2,
                pixels: vec![0., 1., 0., 1.].into(),
            },
        })
    }
    #[test]
    fn alpha_view_projection_matches_screen_coverage_with_pan_zoom_orbit_and_perspective() {
        let d = Document::default();
        for perspective in [false, true] {
            for zoom in [0.4, 2.3] {
                let camera = Camera {
                    perspective,
                    zoom,
                    pan: Vec2::new(25., -35.),
                    yaw: 0.3,
                    pitch: 0.2,
                    ..Default::default()
                };
                let scene = scene(&d, &camera);
                let mut stencil = stencil();
                stencil.position = [0.1, 0.2];
                stencil.rotation = 35.;
                let projection = project(&stencil, &scene);
                let unit = scene.rect.height();
                let (s, c) = 35_f32.to_radians().sin_cos();
                let screen = scene.rect.center()
                    + Vec2::new(0.1 * unit, -0.2 * unit)
                    + Vec2::new(c, -s) * (unit * stencil.size[0] * 0.25);
                for depth in [-0.5, 0., 0.5] {
                    let point = scene.at_depth(screen, scene.view * depth);
                    assert!((projection.weight(point) - 0.75).abs() < 1e-5);
                }
            }
        }
    }
    #[test]
    fn alpha_surface_attachment_stays_fixed_when_orbiting_and_stamp_preview_matches_commit() {
        let mut d = Document::default();
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [30, 30, 1];
        d.replace_geometry(Geometry::new(plane.mesh().unwrap()))
            .unwrap();
        let initial = scene(&d, &Camera::default());
        d.set_stencil(Some(stencil())).unwrap();
        attach(&mut d, &initial, initial.rect.center()).unwrap();
        let fixed = project(d.stencil().unwrap(), &initial);
        let orbit = scene(
            &d,
            &Camera {
                yaw: 0.5,
                pitch: 0.4,
                zoom: 2.,
                ..Default::default()
            },
        );
        let moved = project(d.stencil().unwrap(), &orbit);
        for p in &d.mesh().vertices {
            assert_eq!(fixed.weight(*p), moved.weight(*p));
        }
        for mode in [StrokeMode::DragRect, StrokeMode::DragDot, StrokeMode::Line] {
            let before = d.geometry_snapshot();
            let history = d.history_counts();
            let mut brush = Brush::new(Kind::Standard);
            brush.stroke_mode = mode;
            let start = initial.rect.center();
            let stroke = PendingStroke {
                start,
                end: start + Vec2::new(80., 0.),
                anchor: initial.pick(start).unwrap(),
                brush,
                stencil: Some(fixed.clone()),
                pressure: 0.5,
                invert: false,
            };
            if stroke.is_stamp() {
                let preview = stroke.preview_mesh(&d, &initial);
                assert!(d.geometry_snapshot() == before);
                assert_eq!(d.history_counts(), history);
                assert_eq!(preview, stroke.preview_mesh(&d, &initial));
                stroke.commit(&mut d, &initial);
                d.end_stroke();
                assert_eq!(*d.mesh(), preview);
            } else {
                stroke.commit(&mut d, &initial);
                d.end_stroke();
            }
            assert_ne!(d.mesh().vertices, before.mesh.vertices);
            assert_eq!(d.history_counts().0, history.0 + 1);
            d.undo();
            assert!(d.geometry_snapshot() == before);
        }
    }
    #[test]
    fn alpha_adaptive_stencil_preview_matches_commit_and_undo_in_both_projections() {
        for perspective in [false, true] {
            for mode in [StrokeMode::DragRect, StrokeMode::DragDot, StrokeMode::Line] {
                let mut d = Document::default();
                let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
                cube.divisions = [8, 8, 8];
                d.replace_geometry(Geometry::new(cube.mesh().unwrap()))
                    .unwrap();
                d.set_adaptive(crate::sculpt::AdaptiveSettings {
                    enabled: true,
                    relative: true,
                    detail: 12.,
                    ..Default::default()
                })
                .unwrap();
                d.set_symmetry(crate::sculpt::Symmetry {
                    enabled: true,
                    ..Default::default()
                })
                .unwrap();
                let scene = scene(
                    &d,
                    &Camera {
                        perspective,
                        ..Default::default()
                    },
                );
                let before = d.geometry_snapshot();
                let mut brush = Brush::new(Kind::Standard);
                brush.stroke_mode = mode;
                let start = scene.rect.center() + Vec2::new(25., 0.);
                let stroke = PendingStroke {
                    start,
                    end: start + Vec2::new(65., 0.),
                    anchor: scene.pick(start).unwrap(),
                    brush,
                    stencil: Some(project(&stencil(), &scene)),
                    pressure: 0.7,
                    invert: false,
                };
                let preview = stroke.preview_mesh(&d, &scene);
                assert!(d.geometry_snapshot() == before);
                assert!(preview.vertices.len() > before.mesh.vertices.len());
                stroke.commit(&mut d, &scene);
                d.end_stroke();
                assert_eq!(*d.mesh(), preview);
                d.mesh().validate_closed().unwrap();
                d.undo();
                assert!(d.geometry_snapshot() == before);
            }
        }
    }
}

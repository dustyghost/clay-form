//! Selection-local direct manipulation. Geometry stays a draft until release.
use super::*;
use crate::sculpt::CellNotch;
use crate::viewport::transform::rotation::{Ring, RotationDrag};

const SIZE: f32 = 85.;
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Handle {
    Free,
    Axis(Vec3),
    Plane(Vec3),
    Ring(Vec3),
}
pub(super) struct DirectDrag {
    pub gizmo: bool,
    pub direction: Vec3,
    pivot: Vec3,
    handle: Handle,
    cell: Option<CellNotch>,
    rotation: Option<RotationDrag>,
}
impl DirectDrag {
    pub(super) fn cell_target(&self, scene: &Scene, anchor: Vec3) -> Option<Pos2> {
        let cell = self.cell.as_ref()?;
        let target = anchor + cell.travel();
        if !scene.visible_depth(target) {
            return None;
        }
        let start = scene.project(anchor);
        let projected = scene.project(target);
        // Straight-on, an inward face push has no screen direction. Offer a down-drag
        // of one cell at the local zoom, with the ring at that same pen position.
        if self.handle == Handle::Free
            && matches!(cell, CellNotch::Face(_))
            && projected.distance(start) < 12.
        {
            Some(start + egui::Vec2::Y * (scene.scale_at(anchor) * cell.travel().len()).max(14.))
        } else {
            Some(projected)
        }
    }
}
fn axes() -> [(Vec3, Color32, &'static str); 3] {
    [
        (Vec3(1., 0., 0.), controls::AXES[0], "X"),
        (Vec3(0., 1., 0.), controls::AXES[1], "Y"),
        (Vec3(0., 0., 1.), controls::AXES[2], "Z"),
    ]
}
fn ring(axis: Vec3, pivot: Vec3, color: Color32, label: &'static str) -> Ring {
    Ring {
        axis,
        pivot,
        color,
        label,
        radius: SIZE,
    }
}
fn vertices(mesh: &Mesh, components: &BTreeSet<Component>) -> Vec<usize> {
    let polygons = mesh.polygons();
    components
        .iter()
        .flat_map(|c| match *c {
            Component::Face(f) => polygons[f].vertices.clone(),
            Component::Edge(e) => e.to_vec(),
            Component::Vertex(v) => vec![v],
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn plane_point(scene: &Scene, pointer: Pos2, pivot: Vec3, normal: Vec3) -> Option<Vec3> {
    let (origin, ray) = scene.ray(pointer);
    let denominator = ray.dot(normal);
    (denominator.abs() > 0.05).then(|| origin + ray * ((pivot - origin).dot(normal) / denominator))
}
fn trackball(scene: &Scene, point: Pos2, pivot: Vec3) -> Vec3 {
    let p = (point - scene.project(pivot)) / SIZE;
    let z = (1. - p.length_sq()).max(0.).sqrt();
    (scene.right * p.x - scene.up * p.y + scene.view * z).norm()
}
impl ModelingState {
    /// All selected components must belong to one symmetry orbit.
    pub(super) fn move_cell(
        options: ModelingOptions,
        components: &BTreeSet<Component>,
        mesh: &Mesh,
        anchor: Vec3,
        symmetry: &ModelingSymmetry,
    ) -> Option<CellNotch> {
        if options.tool != ModelTool::Move {
            return None;
        }
        let first = *components.first()?;
        let copies: Vec<Component> = match first {
            Component::Face(f) if options.mode == ComponentMode::Face && options.snap_face_cell => {
                symmetry
                    .face_copies(f)
                    .into_iter()
                    .map(Component::Face)
                    .collect()
            }
            Component::Edge(e) if options.mode == ComponentMode::Edge && options.snap_cell => {
                symmetry
                    .edges(&[e])
                    .into_iter()
                    .map(Component::Edge)
                    .collect()
            }
            _ => return None,
        };
        if !components.iter().all(|c| copies.contains(c)) {
            return None;
        }
        let distance = |c: &Component| {
            let pivot = match *c {
                Component::Face(f) => {
                    let Some(q) = mesh.quads.get(f) else {
                        return f32::INFINITY;
                    };
                    q.iter()
                        .fold(Vec3::default(), |sum, &v| sum + mesh.vertices[v])
                        * 0.25
                }
                Component::Edge([a, b]) => (mesh.vertices[a] + mesh.vertices[b]) * 0.5,
                _ => return f32::INFINITY,
            };
            (pivot - anchor).len()
        };
        match *components
            .iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))?
        {
            Component::Face(f) => mesh.face_cell_notch(f),
            Component::Edge(e) => mesh.cell_notch(e),
            _ => None,
        }
    }

    fn transform_frame(&self, document: &Document) -> Option<(Vec3, Vec3)> {
        let vertices = self.selected_vertices(document.mesh());
        let anchor = self
            .selection_anchor
            .or_else(|| vertices.first().map(|&v| document.mesh().vertices[v]))?;
        let symmetry = self.symmetry.as_ref()?.1.as_ref().ok()?;
        let (_, pivot) = document
            .mesh()
            .component_transform_frame(&vertices, anchor, symmetry);
        Some((anchor, pivot))
    }
    pub(super) fn begin_gizmo(
        &mut self,
        document: &Document,
        scene: &Scene,
        options: ModelingOptions,
        point: Pos2,
        handle: Handle,
    ) -> Result<(), String> {
        let (anchor, _) = self
            .transform_frame(document)
            .ok_or("Select components for the gizmo")?;
        self.begin_direct(
            document,
            scene,
            options,
            point,
            self.selected.clone(),
            anchor,
            handle,
            true,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn begin_direct(
        &mut self,
        document: &Document,
        scene: &Scene,
        options: ModelingOptions,
        point: Pos2,
        components: BTreeSet<Component>,
        anchor: Vec3,
        handle: Handle,
        gizmo: bool,
    ) -> Result<(), String> {
        let symmetry = ModelingSymmetry::new(document.mesh(), document.symmetry())?;
        let (_, pivot) = document.mesh().component_transform_frame(
            &vertices(document.mesh(), &components),
            anchor,
            &symmetry,
        );
        self.edit_drag = Some(EditDrag {
            start: point,
            direct: Some(DirectDrag {
                gizmo,
                direction: options.transform_direction,
                pivot,
                handle,
                cell: Self::move_cell(options, &components, document.mesh(), anchor, &symmetry),
                rotation: if let Handle::Ring(axis) = handle {
                    Some(ring(axis, pivot, Color32::WHITE, "").begin(scene, point))
                } else {
                    None
                },
            }),
            rail: None,
            initial_amount: options.amount,
            moved: false,
            scale: scene.scale_at(pivot),
            components,
            anchor,
            revision: document.display_revision(),
            amount: if options.tool == ModelTool::Scale {
                1.
            } else {
                0.
            },
            result: None,
        });
        Ok(())
    }
    pub(super) fn advance_direct_transform(
        &mut self,
        document: &Document,
        scene: &Scene,
        point: Option<Pos2>,
        mut options: ModelingOptions,
    ) {
        let Some(drag) = &mut self.edit_drag else {
            return;
        };
        let Some(direct) = &mut drag.direct else {
            return;
        };
        let Some(point) = point else {
            return;
        };
        drag.moved |= point.distance(drag.start) > 3.;
        if !drag.moved {
            return;
        }
        let delta = point - drag.start;
        let pivot = direct.pivot;
        let mut snapped = None;
        let (direction, amount) = match options.tool {
            ModelTool::Move => {
                let displacement = match direct.handle {
                    Handle::Axis(axis) => {
                        let amount = scene
                            .axis_parameter(point, pivot, axis)
                            .zip(scene.axis_parameter(drag.start, pivot, axis))
                            .map(|(a, b)| a - b)
                            .unwrap_or_else(|| {
                                let projected = scene.axis_direction(pivot, axis);
                                delta.dot(projected)
                                    / (drag.scale * projected.length_sq().max(0.01))
                            });
                        axis * amount
                    }
                    Handle::Plane(normal) => match (
                        plane_point(scene, point, pivot, normal),
                        plane_point(scene, drag.start, pivot, normal),
                    ) {
                        (Some(a), Some(b)) => a - b,
                        _ => Vec3::default(),
                    },
                    _ => scene.at_depth(point, pivot) - scene.at_depth(drag.start, pivot),
                };
                let displacement = direct
                    .cell
                    .as_ref()
                    .and_then(|cell| {
                        if direct.handle != Handle::Free {
                            if cell.reached(displacement) {
                                snapped = Some(cell);
                                return Some(cell.travel());
                            }
                            return None;
                        }
                        let screen =
                            direct.cell_target(scene, drag.anchor)? - scene.project(drag.anchor);
                        if screen.length() < 12. {
                            return None;
                        }
                        let progress = delta.dot(screen) / screen.length_sq();
                        let radius = (screen.length() * 0.08).clamp(3., 10.);
                        let fraction =
                            (screen.dot(delta) / delta.length_sq().max(1.)).clamp(0., 1.);
                        if (screen - delta * fraction).length() <= radius
                            || cell.reached(displacement)
                        {
                            snapped = Some(cell);
                            Some(cell.travel())
                        } else if matches!(cell, CellNotch::Face(_))
                            && (0. ..1.).contains(&progress)
                            && (delta - screen * progress).length() <= radius
                        {
                            Some(cell.travel() * progress)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(displacement);
                (displacement.norm(), displacement.len())
            }
            ModelTool::Scale => {
                let (axis, travel) = if let Handle::Axis(axis) = direct.handle {
                    let projected = scene.axis_direction(pivot, axis).normalized();
                    (axis, delta.dot(projected))
                } else {
                    (Vec3::default(), -delta.y)
                };
                (axis, (travel / 150.).exp().clamp(0.001, 1000.))
            }
            ModelTool::Rotate => {
                if let Some(rotation) = &mut direct.rotation {
                    rotation.advance(scene, point);
                    (rotation.axis, rotation.angle.to_degrees())
                } else {
                    let a = trackball(scene, drag.start, pivot);
                    let b = trackball(scene, point, pivot);
                    let cross = a.cross(b);
                    let angle = cross.len().atan2(a.dot(b)).to_degrees();
                    let axis = if cross.len() < 1e-6 && a.dot(b) < 0. {
                        let fallback = a.cross(scene.up);
                        if fallback.len() > 1e-6 {
                            fallback.norm()
                        } else {
                            a.cross(scene.right).norm()
                        }
                    } else {
                        cross.norm()
                    };
                    (axis, angle)
                }
            }
            _ => return,
        };
        direct.direction = direction;
        options.transform_direction = direction;
        drag.amount = amount;
        // The grid plane and depth were measured on press. Use that plan throughout
        // the gesture, including when the pointer retreats out of snap range.
        drag.result = Some(if let Some(cell) = snapped {
            ModelingSymmetry::new(document.mesh(), document.symmetry()).and_then(|symmetry| {
                super::edit::EditOutcome::notch(
                    document.mesh(),
                    cell,
                    amount,
                    options.protect_mask,
                    &symmetry,
                )
            })
        } else {
            options.snap_cell = false;
            options.snap_face_cell = false;
            Self::edit_result(document, &drag.components, drag.anchor, options, amount)
        });
    }
    pub(super) fn paint_transform_gizmo(
        &self,
        document: &Document,
        scene: &Scene,
        painter: &egui::Painter,
        options: ModelingOptions,
        pointer: Option<Pos2>,
    ) -> Option<Handle> {
        if !options.tool.is_transform() || options.interaction != ModelInteraction::Gizmo {
            return None;
        }
        let (_, mut pivot) = self.transform_frame(document)?;
        if let Some(drag) = &self.edit_drag
            && let Some(direct) = &drag.direct
        {
            pivot = direct.pivot;
            if options.tool == ModelTool::Move && drag.result.as_ref().is_some_and(|r| r.is_ok()) {
                pivot = pivot + direct.direction * drag.amount;
            }
        }
        if !scene.visible_depth(pivot) {
            return None;
        }
        let center = scene.project(pivot);
        let mut candidates = vec![];
        if let Some(p) = pointer
            && p.distance(center) <= 10.
        {
            candidates.push((0., Handle::Free));
        }
        if options.tool == ModelTool::Rotate {
            for (axis, color, label) in axes() {
                let ring = ring(axis, pivot, color, label);
                let hit = pointer
                    .map(|p| ring.nearest(scene, p).0)
                    .filter(|&d| d < 9.);
                ring.paint(scene, painter, hit.is_some());
                if let Some(d) = hit {
                    candidates.push((d + 1., Handle::Ring(axis)));
                }
            }
        } else {
            for (axis, color, label) in axes() {
                let projected = scene.axis_direction(pivot, axis);
                if projected.length() < 0.15 {
                    continue;
                }
                let end = center + projected.normalized() * (projected.length() * SIZE).max(55.);
                let hit = pointer.is_some_and(|p| {
                    let line = end - center;
                    let t = ((p - center).dot(line) / line.length_sq()).clamp(0.22, 1.);
                    p.distance(end) < 12. || p.distance(center + line * t) < 7.
                });
                let stroke = egui::Stroke::new(if hit { 3.5 } else { 2_f32 }, color);
                if options.tool == ModelTool::Move {
                    painter.arrow(center, end - center, stroke);
                } else {
                    painter.line_segment([center, end], stroke);
                    painter.rect_filled(
                        egui::Rect::from_center_size(end, Vec2::splat(10.)),
                        1.,
                        color,
                    );
                }
                controls::label(painter, end + Vec2::new(8., -14.), label, color);
                if hit {
                    candidates.push((1., Handle::Axis(axis)));
                }
            }
            if options.tool == ModelTool::Move {
                let axes = axes();
                for (a, b, n, label) in [(0, 1, 2, "XY"), (1, 2, 0, "YZ"), (2, 0, 1, "XZ")] {
                    let u = scene.axis_direction(pivot, axes[a].0);
                    let v = scene.axis_direction(pivot, axes[b].0);
                    let area = (u.x * v.y - u.y * v.x).abs();
                    if area < 0.15 {
                        continue;
                    }
                    let points = [
                        center + u * 18. + v * 18.,
                        center + u * 34. + v * 18.,
                        center + u * 34. + v * 34.,
                        center + u * 18. + v * 34.,
                    ];
                    let hit = pointer.is_some_and(|p| {
                        let q = p - center;
                        let det = u.x * v.y - u.y * v.x;
                        let x = (q.x * v.y - q.y * v.x) / det;
                        let y = (u.x * q.y - u.y * q.x) / det;
                        (15. ..=37.).contains(&x) && (15. ..=37.).contains(&y)
                    });
                    let color = axes[n].1;
                    painter.add(egui::Shape::convex_polygon(
                        points.to_vec(),
                        color.gamma_multiply(if hit { 0.5 } else { 0.2 }),
                        egui::Stroke::new(1_f32, color),
                    ));
                    if hit {
                        candidates.push((2., Handle::Plane(axes[n].0)));
                        controls::label(painter, points[2] + Vec2::new(5., 5.), label, color);
                    }
                }
            }
        }
        painter.circle_filled(center, 7., controls::NEUTRAL);
        painter.circle_stroke(center, 8., egui::Stroke::new(1.5_f32, Color32::BLACK));
        controls::label(
            painter,
            center + Vec2::new(-20., 13.),
            if options.tool == ModelTool::Scale {
                "ALL"
            } else {
                "FREE"
            },
            controls::NEUTRAL,
        );
        candidates
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, h)| h)
    }
}

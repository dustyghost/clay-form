//! Pen selection uses a private draft, so cancellation restores the prior selection.
use super::*;

pub(super) struct SelectionDrag {
    pub current: BTreeSet<Component>,
    change: SelectionChange,
    mode: ComponentMode,
    span: SelectionSpan,
    flat_angle: f32,
    last: Pos2,
    visited: BTreeSet<Component>,
}
impl ModelingState {
    pub fn selection(&self) -> &BTreeSet<Component> {
        self.selection_drag
            .as_ref()
            .map_or(&self.selected, |d| &d.current)
    }
}
impl Viewport {
    pub(super) fn selection_target(
        component: Component,
        point: Pos2,
        span: SelectionSpan,
        flat_angle: f32,
        mesh: &Mesh,
        scene: &Scene,
        symmetry: Option<&ModelingSymmetry>,
    ) -> Result<Vec<Component>, String> {
        let targets = match (component, span) {
            (
                _,
                SelectionSpan::Connected
                | SelectionSpan::Flat
                | SelectionSpan::Group
                | SelectionSpan::Border,
            ) => Self::region_target(component, span, flat_angle, mesh),
            (Component::Edge(e), SelectionSpan::Loop) => mesh
                .expand_edge_loop(&[e])
                .map(|es| es.into_iter().map(Component::Edge).collect()),
            (Component::Edge(e), SelectionSpan::Ring) => mesh
                .select_edge_ring(e)
                .map(|es| es.into_iter().map(Component::Edge).collect()),
            (Component::Face(f), SelectionSpan::Loop) => {
                let polygons = mesh.polygons();
                let edge = polygons[f]
                    .edges()
                    .filter_map(|(a, b)| {
                        let [aa, bb] = scene.clip_segment([mesh.vertices[a], mesh.vertices[b]])?;
                        let direction = bb - aa;
                        let t = ((point - aa).dot(direction) / direction.length_sq().max(1e-6))
                            .clamp(0., 1.);
                        Some((point.distance(aa + direction * t), [a, b]))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .ok_or("No visible edge for this face loop")?
                    .1;
                mesh.select_face_loop(f, edge)
                    .map(|fs| fs.into_iter().map(Component::Face).collect())
            }
            _ => Ok(vec![component]),
        }?;
        let Some(symmetry) = symmetry else {
            return Ok(targets);
        };
        Ok(targets
            .into_iter()
            .flat_map(|c| match c {
                Component::Face(f) => symmetry
                    .face_copies(f)
                    .into_iter()
                    .map(Component::Face)
                    .collect::<Vec<_>>(),
                Component::Vertex(v) => symmetry
                    .vertex_copies(v)
                    .into_iter()
                    .map(Component::Vertex)
                    .collect(),
                Component::Edge(e) => symmetry
                    .edge_copies(e)
                    .into_iter()
                    .map(|[a, b]| Component::Edge([a.min(b), a.max(b)]))
                    .collect(),
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_selection_stroke(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        scene: &Scene,
        document: &Document,
        model: ModelingOptions,
        gizmo: egui::Rect,
        pressed: bool,
    ) {
        let allowed = |p| {
            response.rect.contains(p)
                && !navigation::contains(response.rect, p)
                && !gizmo.contains(p)
        };
        let events = ui.input(|i| i.events.clone());
        let down = events.iter().position(|e| {
            matches!(
                e,
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                }
            )
        });
        let symmetry = self
            .modeling
            .symmetry
            .as_ref()
            .and_then(|(_, r)| r.as_ref().ok());
        let fresh = self.modeling.selection_drag.is_none();
        if fresh {
            if !pressed || self.session.gesture.is_some() {
                return;
            }
            let origin = down
                .and_then(|i| match events[i] {
                    egui::Event::PointerButton { pos, .. } => Some(pos),
                    _ => None,
                })
                .or_else(|| response.interact_pointer_pos());
            let Some(origin) = origin.filter(|p| allowed(*p)) else {
                return;
            };
            let Some(component) = Self::component_at(origin, model.mode, document, scene) else {
                return;
            };
            if let Err(error) = Self::selection_target(
                component,
                origin,
                model.span,
                model.flat_angle,
                document.mesh(),
                scene,
                symmetry,
            ) {
                self.status = Some(error);
                return;
            }
            let modifiers = ui.input(|i| i.modifiers);
            let change = if modifiers.ctrl || modifiers.command {
                SelectionChange::Remove
            } else if modifiers.shift {
                SelectionChange::Add
            } else {
                model.change
            };
            self.modeling.selection_anchor = scene.pick(origin).map(|h| h.0);
            self.modeling.selection_drag = Some(SelectionDrag {
                current: if change == SelectionChange::Replace {
                    BTreeSet::new()
                } else {
                    self.modeling.selected.clone()
                },
                change,
                mode: model.mode,
                span: model.span,
                flat_angle: model.flat_angle,
                last: origin,
                visited: BTreeSet::new(),
            });
            self.session.gesture = Some(Gesture::Modeling);
        }
        let Some(drag) = self.modeling.selection_drag.as_mut() else {
            return;
        };
        let mut path = vec![drag.last];
        // Keep coalesced pen samples in order, rather than taking a shortcut
        // from the previous frame to the final pointer position.
        for event in events
            .iter()
            .skip(if fresh { down.unwrap_or(0) } else { 0 })
        {
            match event {
                egui::Event::PointerMoved(p)
                | egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    ..
                } => path.push(*p),
                _ => {}
            }
        }
        for end in path {
            let start = drag.last;
            // Sample short segments so a fast drag does not skip faces between
            // input events. Picking still depth-tests each sample.
            let steps = (start.distance(end) / 4.).ceil().max(1.) as usize;
            for step in 0..=steps {
                let p = start.lerp(end, step as f32 / steps as f32);
                if !allowed(p) {
                    continue;
                }
                let Some(component) = Self::component_at(p, drag.mode, document, scene) else {
                    continue;
                };
                if !drag.visited.insert(component) {
                    continue;
                }
                match Self::selection_target(
                    component,
                    p,
                    drag.span,
                    drag.flat_angle,
                    document.mesh(),
                    scene,
                    symmetry,
                ) {
                    Ok(targets) => {
                        for target in targets {
                            if drag.change == SelectionChange::Remove {
                                drag.current.remove(&target);
                            } else {
                                drag.current.insert(target);
                            }
                        }
                    }
                    Err(error) => self.status = Some(error),
                }
            }
            drag.last = end;
        }
    }
}

impl ModelingState {
    pub fn convert_selection(&mut self, mesh: &Mesh, mode: ComponentMode, boundary: bool) {
        let vertices: BTreeSet<_> = self.selected_vertices(mesh).into_iter().collect();
        let faces = mesh.polygons();
        self.selected = match mode {
            ComponentMode::Vertex => vertices.into_iter().map(Component::Vertex).collect(),
            ComponentMode::Face => faces
                .iter()
                .enumerate()
                .filter(|(_, p)| p.visible && p.vertices.iter().all(|v| vertices.contains(v)))
                .map(|(f, _)| Component::Face(f))
                .collect(),
            ComponentMode::Edge => {
                let mut incidence = std::collections::BTreeMap::<[usize; 2], usize>::new();
                for (f, p) in faces.iter().enumerate() {
                    if self.selected.contains(&Component::Face(f)) {
                        for (a, b) in p.edges() {
                            *incidence.entry([a.min(b), a.max(b)]).or_default() += 1;
                        }
                    }
                }
                mesh.polygon_edges()
                    .into_iter()
                    .filter(|e| {
                        if boundary {
                            incidence.get(e) == Some(&1)
                        } else {
                            e.iter().all(|v| vertices.contains(v))
                        }
                    })
                    .map(Component::Edge)
                    .collect()
            }
        };
    }
}
impl Viewport {
    fn region_target(
        seed: Component,
        span: SelectionSpan,
        flat_angle: f32,
        mesh: &Mesh,
    ) -> Result<Vec<Component>, String> {
        let faces = mesh.polygons();
        let seed_face = match seed {
            Component::Face(f) => Some(f),
            Component::Edge(e) => faces
                .iter()
                .position(|p| p.visible && e.iter().all(|v| p.vertices.contains(v))),
            Component::Vertex(v) => faces
                .iter()
                .position(|p| p.visible && p.vertices.contains(&v)),
        }
        .ok_or("Choose a visible component")?;
        let n = |f: usize| {
            let p = &faces[f];
            let a = mesh.vertices[p.vertices[0]];
            (mesh.vertices[p.vertices[1]] - a)
                .cross(mesh.vertices[p.vertices[2]] - a)
                .norm()
        };
        let normal = n(seed_face);
        let cosine = flat_angle.to_radians().cos();
        let mut adjacent = std::collections::BTreeMap::<[usize; 2], Vec<usize>>::new();
        for (f, p) in faces.iter().enumerate() {
            for (a, b) in p.edges() {
                adjacent.entry([a.min(b), a.max(b)]).or_default().push(f);
            }
        }
        if span == SelectionSpan::Border {
            let Component::Edge(e) = seed else {
                return Err("Boundary target requires Edge mode".into());
            };
            if adjacent.get(&e).is_none_or(|fs| fs.len() != 1) {
                return Err("Choose an open boundary edge".into());
            }
            let mut edges = BTreeSet::from([e]);
            let mut pending = vec![e];
            while let Some(e) = pending.pop() {
                for (&other, fs) in &adjacent {
                    if fs.len() == 1
                        && faces[fs[0]].visible
                        && other.iter().any(|v| e.contains(v))
                        && edges.insert(other)
                    {
                        pending.push(other);
                    }
                }
            }
            return Ok(edges.into_iter().map(Component::Edge).collect());
        }
        let mut selected = BTreeSet::from([seed_face]);
        let mut pending = vec![seed_face];
        if span == SelectionSpan::Group {
            selected = faces
                .iter()
                .enumerate()
                .filter(|(_, p)| p.visible && p.group == faces[seed_face].group)
                .map(|(f, _)| f)
                .collect();
        } else {
            while let Some(f) = pending.pop() {
                for (a, b) in faces[f].edges() {
                    for &next in &adjacent[&[a.min(b), a.max(b)]] {
                        let p = &faces[next];
                        if !p.visible
                            || (span == SelectionSpan::Flat && n(next).dot(normal) + 1e-6 < cosine)
                        {
                            continue;
                        }
                        if selected.insert(next) {
                            pending.push(next);
                        }
                    }
                }
            }
        }
        Ok(match seed {
            Component::Face(_) => selected.into_iter().map(Component::Face).collect(),
            Component::Edge(_) => selected
                .into_iter()
                .flat_map(|f| {
                    faces[f]
                        .edges()
                        .map(|(a, b)| Component::Edge([a.min(b), a.max(b)]))
                        .collect::<Vec<_>>()
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            Component::Vertex(_) => selected
                .into_iter()
                .flat_map(|f| faces[f].vertices.iter().copied().map(Component::Vertex))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        })
    }
}

#[cfg(test)]
mod angle_tests {
    use super::*;
    #[test]
    fn modeling_flat_island_angle_uses_seed_normal_and_connected_visible_faces() {
        let mut mesh = Mesh {
            vertices: vec![Vec3(0., -0.5, 0.), Vec3(0., 0.5, 0.)],
            triangles: vec![],
            quads: vec![],
            mask: vec![],
            creases: vec![],
            materials: Vec::new(),
            vertex_groups: vec![],
            polygroups: vec![],
            hidden_faces: vec![],
        };
        let mut x = 0.;
        let mut z = 0.;
        for (i, degrees) in [0_f32, 20., 40., 60.].into_iter().enumerate() {
            let angle = degrees.to_radians();
            x += angle.cos();
            z -= angle.sin();
            mesh.vertices.extend([Vec3(x, -0.5, z), Vec3(x, 0.5, z)]);
            let a = i * 2;
            let q = [a, a + 2, a + 3, a + 1];
            mesh.quads.push(q);
            mesh.triangles
                .extend([[q[0], q[1], q[2]], [q[0], q[2], q[3]]]);
        }
        let a = mesh.vertices.len();
        mesh.vertices.extend([
            Vec3(10., -0.5, 0.),
            Vec3(11., -0.5, 0.),
            Vec3(11., 0.5, 0.),
            Vec3(10., 0.5, 0.),
        ]);
        mesh.quads.push([a, a + 1, a + 2, a + 3]);
        mesh.triangles
            .extend([[a, a + 1, a + 2], [a, a + 2, a + 3]]);
        mesh.mask = vec![0.; mesh.vertices.len()];
        mesh.validate().unwrap();
        let target = |mesh: &Mesh, angle| {
            Viewport::region_target(Component::Face(0), SelectionSpan::Flat, angle, mesh).unwrap()
        };
        assert_eq!(target(&mesh, 0.), vec![Component::Face(0)]);
        assert_eq!(
            target(&mesh, 25.),
            vec![Component::Face(0), Component::Face(1)]
        );
        assert_eq!(
            target(&mesh, 45.),
            vec![Component::Face(0), Component::Face(1), Component::Face(2)]
        );
        assert_eq!(target(&mesh, 180.).len(), 4); // Disconnected parallel face remains excluded.
        mesh.hidden_faces = vec![false; mesh.triangles.len()];
        mesh.hidden_faces[2] = true;
        mesh.hidden_faces[3] = true;
        assert_eq!(target(&mesh, 180.), vec![Component::Face(0)]);
    }
}

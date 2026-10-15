//! Pen selection uses a private draft, so cancellation restores the prior selection.
use super::*;

pub(super) struct SelectionDrag {
    pub current: BTreeSet<Component>,
    change: SelectionChange,
    mode: ComponentMode,
    span: SelectionSpan,
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
        mesh: &Mesh,
        scene: &Scene,
        symmetry: Option<&ModelingSymmetry>,
    ) -> Result<Vec<Component>, String> {
        let targets = match (component, span) {
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
            if !pressed || self.gesture.is_some() {
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
            self.modeling.selection_drag = Some(SelectionDrag {
                current: if change == SelectionChange::Replace {
                    BTreeSet::new()
                } else {
                    self.modeling.selected.clone()
                },
                change,
                mode: model.mode,
                span: model.span,
                last: origin,
                visited: BTreeSet::new(),
            });
            self.gesture = Some(Gesture::Modeling);
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

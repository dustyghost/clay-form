//! Pen-friendly component targeting and transactional extrusion previews.
use super::*;
use crate::sculpt::{Extrusion, LoopCut, Mesh, ModelingSymmetry, Symmetry};
use std::collections::BTreeSet;
mod selection;
use selection::SelectionDrag;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ComponentMode {
    #[default]
    Face,
    Edge,
    Vertex,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ModelTool {
    #[default]
    Select,
    Extrude,
    LoopCut,
}
impl ModelTool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Extrude => "Extrude",
            Self::LoopCut => "Loop cut",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SelectionChange {
    #[default]
    Replace,
    Add,
    Remove,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SelectionSpan {
    #[default]
    Single,
    Loop,
    Ring,
}
#[derive(Clone, Copy)]
pub(crate) struct ModelingOptions {
    pub mode: ComponentMode,
    pub tool: ModelTool,
    pub change: SelectionChange,
    pub span: SelectionSpan,
    pub join: bool,
    pub loop_count: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Component {
    Face(usize),
    Edge([usize; 2]),
    Vertex(usize),
}
#[derive(Default)]
pub(crate) struct ModelingState {
    pub selected: BTreeSet<Component>,
    pub revision: u64,
    symmetry: Option<(Symmetry, Result<ModelingSymmetry, String>)>,
    drag: Option<ExtrudeDrag>,
    selection_drag: Option<SelectionDrag>,
    loop_preview: Option<LoopPreview>,
    loop_drag: Option<LoopDrag>,
    pub requested_loop_count: Option<u32>,
}
struct LoopPreview {
    edge: [usize; 2],
    count: u32,
    position: f32,
    result: Result<LoopCut, String>,
}
struct LoopDrag {
    edge: [usize; 2],
    start: Pos2,
    start_parameter: Option<f32>,
    position: f32,
    count: u32,
    revision: u64,
    moved: bool,
}
struct ExtrudeDrag {
    start: Pos2,
    scale: f32,
    faces: Vec<usize>,
    revision: u64,
    distance: f32,
    join: bool,
    result: Option<Result<Extrusion, String>>,
}
impl ModelingState {
    pub fn sync(&mut self, document: &Document) {
        if self.revision != document.display_revision() {
            self.symmetry = None;
            self.selected.clear();
            self.selection_drag = None;
            self.drag = None;
            self.loop_drag = None;
            self.loop_preview = None;
            self.revision = document.display_revision();
        }
    }
    pub fn cancel(&mut self) {
        self.selection_drag = None;
        self.drag = None;
        self.loop_drag = None;
        self.loop_preview = None;
    }
    pub fn dragging(&self) -> bool {
        self.drag.is_some() || self.loop_drag.is_some() || self.selection_drag.is_some()
    }
    pub fn selected_vertices(&self, mesh: &Mesh) -> Vec<usize> {
        let faces = mesh.polygons();
        self.selected
            .iter()
            .flat_map(|c| match *c {
                Component::Face(i) => faces.get(i).map(|p| p.vertices.clone()).unwrap_or_default(),
                Component::Edge(e) => e.to_vec(),
                Component::Vertex(v) => vec![v],
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn preview(&self) -> Option<&Mesh> {
        self.drag
            .as_ref()?
            .result
            .as_ref()?
            .as_ref()
            .ok()
            .map(|r| &r.mesh)
    }
    pub fn update_preview(
        &mut self,
        document: &Document,
        point: Option<Pos2>,
        options: Option<ModelingOptions>,
        enabled: bool,
        escape: bool,
    ) {
        self.sync(document);
        if !enabled || escape || options.is_none() {
            self.cancel();
            return;
        }
        if self
            .symmetry
            .as_ref()
            .is_none_or(|(settings, _)| settings != document.symmetry())
        {
            self.cancel();
            self.symmetry = Some((
                document.symmetry().clone(),
                ModelingSymmetry::new(document.mesh(), document.symmetry()),
            ));
            if let Some((_, Ok(symmetry))) = &self.symmetry {
                self.selected = self
                    .selected
                    .iter()
                    .flat_map(|c| match *c {
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
                    .collect();
            }
        }
        let options = options.unwrap();
        if options.tool != ModelTool::Select {
            self.selection_drag = None;
        }
        if options.tool != ModelTool::LoopCut {
            self.loop_drag = None;
            self.loop_preview = None;
        }
        if options.tool != ModelTool::Extrude {
            self.drag = None;
            return;
        }
        if let Some(drag) = &mut self.drag
            && let Some(point) = point
        {
            let distance = (drag.start.y - point.y) / drag.scale;
            if distance != drag.distance || drag.join != options.join || drag.result.is_none() {
                drag.distance = distance;
                drag.join = options.join;
                drag.result = Some(
                    self.symmetry
                        .as_ref()
                        .unwrap()
                        .1
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|symmetry| {
                            document.mesh().extrude_symmetric(
                                &drag.faces,
                                distance,
                                options.join,
                                symmetry,
                            )
                        }),
                );
            }
        }
    }
}
impl Viewport {
    pub(crate) fn set_model_edges(&mut self, edges: Vec<[usize; 2]>, document: &Document) {
        self.modeling.selected = edges.iter().copied().map(Component::Edge).collect();
        self.modeling.revision = document.display_revision();
        self.set_edge_selection(edges, document);
    }
    fn component_at(
        point: Pos2,
        mode: ComponentMode,
        document: &Document,
        scene: &Scene,
    ) -> Option<Component> {
        let mesh = document.mesh();
        match mode {
            ComponentMode::Edge => Self::edge_at(point, document, scene).map(Component::Edge),
            ComponentMode::Face => {
                let triangle = mesh.triangles[scene.pick_face(point)?];
                mesh.polygons()
                    .iter()
                    .position(|p| p.visible && triangle.iter().all(|v| p.vertices.contains(v)))
                    .map(Component::Face)
            }
            ComponentMode::Vertex => {
                let visible: BTreeSet<_> = mesh
                    .triangles
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mesh.face_visible(*i))
                    .flat_map(|(_, t)| t.iter().copied())
                    .collect();
                visible
                    .into_iter()
                    .filter_map(|v| {
                        let p = scene.project(mesh.vertices[v]);
                        let distance = p.distance(point);
                        if distance > 12. {
                            return None;
                        }
                        // Sample just inside the incident surface at silhouettes.
                        let hit = scene.pick(p).or_else(|| scene.pick(p.lerp(point, 0.1)))?;
                        ((hit.0 - mesh.vertices[v]).len() < 2. / scene.scale_at(mesh.vertices[v]))
                            .then_some((distance, v))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, v)| Component::Vertex(v))
            }
        }
    }
    fn paint_component(
        component: Component,
        mesh: &Mesh,
        scene: &Scene,
        painter: &egui::Painter,
        color: Color32,
    ) {
        match component {
            Component::Vertex(v) => {
                if let Some(&p) = mesh.vertices.get(v) {
                    painter.circle_filled(scene.project(p), 5., color);
                }
            }
            Component::Edge([a, b]) => {
                if let Some(points) = scene.clip_segment([mesh.vertices[a], mesh.vertices[b]]) {
                    painter.line_segment(points, egui::Stroke::new(3_f32, color));
                }
            }
            Component::Face(i) => {
                if let Some(p) = mesh.polygons().get(i) {
                    for (a, b) in p.edges() {
                        if let Some(points) =
                            scene.clip_segment([mesh.vertices[a], mesh.vertices[b]])
                        {
                            painter.line_segment(points, egui::Stroke::new(3_f32, color));
                        }
                    }
                }
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_modeling(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        scene: &Scene,
        document: &mut Document,
        options: ViewportOptions,
        gizmo: egui::Rect,
    ) -> bool {
        let Some(model) = options.modeling else {
            return false;
        };
        self.modeling.sync(document);
        if model.mode == ComponentMode::Edge {
            let edges = self
                .modeling
                .selected
                .iter()
                .filter_map(|c| {
                    if let Component::Edge(e) = c {
                        Some(*e)
                    } else {
                        None
                    }
                })
                .collect();
            self.set_edge_selection(edges, document);
        }
        let point = response
            .hover_pos()
            .or_else(|| response.interact_pointer_pos());
        let eligible =
            point.filter(|p| !navigation::contains(response.rect, *p) && !gizmo.contains(*p));
        let hover = eligible.and_then(|p| Self::component_at(p, model.mode, document, scene));
        let selected = Color32::from_rgb(70, 225, 240);
        let highlighted = Color32::from_rgb(255, 205, 95);
        if let Some(drag) = &self.modeling.drag {
            if let Some(Ok(result)) = &drag.result {
                for &edge in &result.rim {
                    if let Some(points) = scene.clip_segment(edge) {
                        painter.line_segment(points, egui::Stroke::new(3_f32, highlighted));
                    }
                }
            }
        } else {
            for &component in self.modeling.selection() {
                Self::paint_component(component, document.mesh(), scene, painter, selected);
            }
            if let Some(component) = self
                .modeling
                .loop_drag
                .as_ref()
                .map(|d| Component::Edge(d.edge))
                .or(hover)
            {
                let targets = if model.tool == ModelTool::Select {
                    Self::selection_target(
                        component,
                        point.unwrap(),
                        model.span,
                        document.mesh(),
                        scene,
                        self.modeling
                            .symmetry
                            .as_ref()
                            .and_then(|(_, r)| r.as_ref().ok()),
                    )
                    .unwrap_or_else(|_| vec![component])
                } else {
                    vec![component]
                };
                for target in targets {
                    Self::paint_component(target, document.mesh(), scene, painter, highlighted);
                }
            }
        }
        let symmetry_error = self
            .modeling
            .symmetry
            .as_ref()
            .and_then(|(_, r)| r.as_ref().err())
            .cloned();
        let hint = if let Some(error) = &symmetry_error {
            error.clone()
        } else if let Some(drag) = &self.modeling.drag {
            match &drag.result {
                Some(Ok(result)) => format!(
                    "Extrude {:.3}{} · release to apply · Esc cancels",
                    result.distance,
                    if result.joined {
                        " · Tunnel snapped"
                    } else {
                        ""
                    }
                ),
                Some(Err(error)) => format!("{error} · Esc cancels"),
                None => "Drag up to extrude out, down to push in".into(),
            }
        } else if let Some(drag) = &self.modeling.loop_drag {
            if let Some(LoopPreview {
                result: Err(error), ..
            }) = &self.modeling.loop_preview
            {
                format!("{error} · Esc cancels")
            } else {
                format!(
                    "Loop cut · {} loops · {:.0}% · release to apply · wheel: count · Esc cancels",
                    drag.count,
                    drag.position * 100.
                )
            }
        } else if self.modeling.selection_drag.is_some() {
            format!(
                "Selecting · {} selected · release to keep · Esc cancels",
                self.modeling.selection().len()
            )
        } else {
            format!(
                "Modeling · {:?} · {} · {} selected · Esc exits",
                model.mode,
                model.tool.label(),
                self.modeling.selected.len()
            )
        };
        painter.text(
            navigation::inner(response.rect).left_bottom() + Vec2::new(8., -8.),
            egui::Align2::LEFT_BOTTOM,
            hint,
            egui::FontId::proportional(13.),
            Color32::WHITE,
        );
        if symmetry_error.is_some() {
            self.modeling.cancel();
            if self.gesture == Some(Gesture::Modeling) {
                self.gesture = None;
                self.wait_for_release();
            }
            return eligible.is_some_and(|p| scene.pick_model(p).is_some());
        }
        if !options.enabled {
            self.modeling.cancel();
            if self.gesture == Some(Gesture::Modeling) {
                self.gesture = None;
            }
            return true;
        }
        let lost_pointer = self.gesture == Some(Gesture::Modeling)
            && ui.input(|i| {
                !i.pointer.button_released(egui::PointerButton::Primary)
                    && (!i.pointer.primary_down()
                        || i.events
                            .iter()
                            .any(|e| matches!(e, egui::Event::PointerGone)))
            });
        if lost_pointer || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.modeling.cancel();
            self.gesture = None;
            self.wait_for_release();
            return true;
        }
        if self.gesture.is_some_and(|g| g != Gesture::Modeling) {
            return false;
        }
        let pressed =
            response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed());
        let clicked = response.clicked_by(egui::PointerButton::Primary);
        if model.tool == ModelTool::Select {
            self.update_selection_stroke(
                ui,
                response,
                scene,
                document,
                model,
                gizmo,
                pressed || clicked,
            );
        }
        if self.gesture.is_none() && (pressed || clicked) && eligible.is_some() && hover.is_some() {
            self.gesture = Some(Gesture::Modeling);
            if model.tool == ModelTool::Extrude
                && let Some(Component::Face(face)) = hover
            {
                let faces = if self.modeling.selected.contains(&Component::Face(face)) {
                    self.modeling
                        .selected
                        .iter()
                        .filter_map(|c| {
                            if let Component::Face(i) = c {
                                Some(*i)
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    vec![face]
                };
                self.modeling.drag = Some(ExtrudeDrag {
                    start: point.unwrap(),
                    scale: scene.scale_at(scene.pick(point.unwrap()).unwrap().0),
                    faces,
                    revision: document.display_revision(),
                    distance: 0.,
                    join: model.join,
                    result: None,
                });
            }
        }
        if model.tool == ModelTool::LoopCut {
            let parameter = |edge: [usize; 2], point| {
                let a = document.mesh().vertices[edge[0]];
                let axis = document.mesh().vertices[edge[1]] - a;
                scene
                    .axis_parameter(point, a, axis.norm())
                    .map(|d| d / axis.len().max(1e-8))
            };
            if self.modeling.loop_drag.is_none()
                && self.gesture == Some(Gesture::Modeling)
                && (pressed || clicked)
                && let Some(Component::Edge(edge)) = hover
            {
                self.modeling.loop_drag = Some(LoopDrag {
                    edge,
                    start: point.unwrap(),
                    start_parameter: parameter(edge, point.unwrap()),
                    position: 0.5,
                    count: model.loop_count.clamp(1, LoopCut::MAX_COUNT),
                    revision: document.display_revision(),
                    moved: false,
                });
            }
            if let Some(drag) = &mut self.modeling.loop_drag {
                drag.count = model.loop_count.clamp(1, LoopCut::MAX_COUNT);
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll != 0. {
                    drag.count = (drag.count as i32 + if scroll > 0. { 1 } else { -1 })
                        .clamp(1, LoopCut::MAX_COUNT as i32)
                        as u32;
                    self.modeling.requested_loop_count = Some(drag.count);
                }
                if let Some(p) = point {
                    drag.moved |= p.distance(drag.start) > 3.;
                    if drag.moved
                        && let (Some(start), Some(current)) =
                            (drag.start_parameter, parameter(drag.edge, p))
                    {
                        drag.position = 0.5 + current - start;
                    }
                }
                drag.position = LoopCut::center(drag.count, drag.position);
            }
            let target = self
                .modeling
                .loop_drag
                .as_ref()
                .map(|d| (d.edge, d.count, d.position))
                .or_else(|| {
                    if let Some(Component::Edge(e)) = hover {
                        Some((e, model.loop_count.clamp(1, LoopCut::MAX_COUNT), 0.5))
                    } else {
                        None
                    }
                });
            if let Some((edge, count, position)) = target {
                if self
                    .modeling
                    .loop_preview
                    .as_ref()
                    .is_none_or(|p| p.edge != edge || p.count != count || p.position != position)
                {
                    self.modeling.loop_preview = Some(LoopPreview {
                        edge,
                        count,
                        position,
                        result: self
                            .modeling
                            .symmetry
                            .as_ref()
                            .unwrap()
                            .1
                            .as_ref()
                            .map_err(Clone::clone)
                            .and_then(|symmetry| {
                                document
                                    .mesh()
                                    .insert_symmetric_edge_loops(edge, count, position, symmetry)
                            }),
                    });
                }
                if let Some(LoopPreview {
                    result: Ok(cut), ..
                }) = &self.modeling.loop_preview
                {
                    for &edge in &cut.cuts {
                        if let Some(points) = scene.clip_segment(edge) {
                            painter.line_segment(points, egui::Stroke::new(3_f32, highlighted));
                        }
                    }
                }
            } else {
                self.modeling.loop_preview = None;
            }
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && self.gesture == Some(Gesture::Modeling)
        {
            self.gesture = None;
            if let Some(drag) = self.modeling.drag.take() {
                if let Some(result) = drag.result {
                    match result.and_then(|r| {
                        document.commit_modeling(
                            r.mesh,
                            if r.joined {
                                "Extrude tunnel"
                            } else {
                                "Extrude faces"
                            },
                            drag.revision,
                        )
                    }) {
                        Ok(()) => {}
                        Err(e) => self.status = Some(e),
                    }
                }
                self.modeling.sync(document);
            } else if let Some(drag) = self.modeling.loop_drag.take() {
                if let Some(preview) = self.modeling.loop_preview.take() {
                    if let Err(e) = preview.result.and_then(|cut| {
                        document.commit_modeling(cut.mesh, "Insert edge loops", drag.revision)
                    }) {
                        self.status = Some(e);
                    }
                    self.modeling.sync(document);
                }
            } else if let Some(drag) = self.modeling.selection_drag.take() {
                self.modeling.selected = drag.current;
                if model.mode == ComponentMode::Edge {
                    let edges = self
                        .modeling
                        .selected
                        .iter()
                        .filter_map(|c| {
                            if let Component::Edge(e) = c {
                                Some(*e)
                            } else {
                                None
                            }
                        })
                        .collect();
                    self.set_edge_selection(edges, document);
                }
            }
            return true;
        }
        // Preserve empty-space, navigation-frame and gizmo navigation; consume model
        // presses even if the chosen component has no hit (e.g. a face in vertex mode).
        self.gesture == Some(Gesture::Modeling)
            || eligible.is_some_and(|p| scene.pick_model(p).is_some())
    }
}

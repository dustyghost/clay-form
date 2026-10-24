//! Shared preview/commit path for measured component operations.
use super::*;
use crate::sculpt::ComponentTransform;
pub(super) struct EditOutcome {
    pub mesh: Mesh,
    pub amount: f32,
    pub retain_caps: bool,
    pub notice: &'static str,
}
impl EditOutcome {
    fn new(mesh: Mesh, amount: f32, retain_caps: bool) -> Self {
        Self {
            mesh,
            amount,
            retain_caps,
            notice: "",
        }
    }
}
pub(super) struct EditDrag {
    pub start: Pos2,
    pub direct: Option<super::transform::DirectDrag>,
    pub rail: Option<egui::Vec2>,
    pub initial_amount: f32,
    pub moved: bool,
    pub scale: f32,
    pub components: BTreeSet<Component>,
    pub anchor: Vec3,
    pub revision: u64,
    pub amount: f32,
    pub result: Option<Result<EditOutcome, String>>,
}
impl ModelTool {
    pub fn is_edit(self) -> bool {
        self.construction_kind().is_some()
            || matches!(
                self,
                Self::Move
                    | Self::Rotate
                    | Self::Scale
                    | Self::Inset
                    | Self::Bevel
                    | Self::Fuse
                    | Self::Slide
            )
    }
}
impl ModelingState {
    pub(super) fn cap_selection(
        source: &Mesh,
        out: &Mesh,
        selected: &BTreeSet<Component>,
    ) -> BTreeSet<Component> {
        selected
            .iter()
            .filter_map(|c| {
                if let Component::Face(f) = *c {
                    let new = if f < source.quads.len() {
                        f
                    } else {
                        out.quads.len() + f - source.quads.len()
                    };
                    (new < out.polygons().len()).then_some(Component::Face(new))
                } else {
                    None
                }
            })
            .collect()
    }

    pub(super) fn update_edit_preview(
        &mut self,
        document: &Document,
        point: Option<Pos2>,
        options: ModelingOptions,
    ) {
        if !options.tool.is_edit() {
            self.edit_drag = None;
            return;
        }
        if let Some(drag) = &mut self.edit_drag {
            if drag.direct.is_some() {
                return;
            }
            if let Some(point) = point {
                drag.moved |= point.distance(drag.start) > 3.;
                if !drag.moved {
                    return;
                }
                let travel = drag.start.y - point.y;
                drag.amount = match options.tool {
                    ModelTool::EdgeSplit | ModelTool::EdgeCollapse => {
                        let rail = drag.rail.unwrap_or(egui::Vec2::ZERO);
                        let fraction = drag.initial_amount
                            + (point - drag.start).dot(rail) / rail.length_sq().max(1.);
                        if options.tool == ModelTool::EdgeSplit {
                            fraction.clamp(0.001, 0.999)
                        } else {
                            fraction.clamp(0., 1.)
                        }
                    }
                    ModelTool::Rotate => travel,
                    ModelTool::Scale => (travel / 150.).exp(),
                    _ => travel / drag.scale,
                };
            }
            drag.result = Some(Self::edit_result(
                document,
                &drag.components,
                drag.anchor,
                options,
                drag.amount,
            ));
        }
    }
    pub(super) fn edit_result(
        document: &Document,
        components: &BTreeSet<Component>,
        anchor: Vec3,
        options: ModelingOptions,
        amount: f32,
    ) -> Result<EditOutcome, String> {
        if !amount.is_finite() {
            return Err("Amount must be finite".into());
        }
        let mesh = document.mesh();
        let polygons = mesh.polygons();
        let vertices: BTreeSet<_> = components
            .iter()
            .flat_map(|c| match *c {
                Component::Vertex(v) => vec![v],
                Component::Edge(e) => e.to_vec(),
                Component::Face(f) => polygons
                    .get(f)
                    .map(|p| p.vertices.clone())
                    .unwrap_or_default(),
            })
            .collect();
        if let Some(kind) = options.tool.construction_kind() {
            let faces: Vec<_> = components
                .iter()
                .filter_map(|c| {
                    if let Component::Face(f) = c {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .collect();
            let edges: Vec<_> = components
                .iter()
                .filter_map(|c| {
                    if let Component::Edge(e) = c {
                        Some(*e)
                    } else {
                        None
                    }
                })
                .collect();
            let points: Vec<_> = components
                .iter()
                .filter_map(|c| {
                    if let Component::Vertex(v) = c {
                        Some(*v)
                    } else {
                        None
                    }
                })
                .collect();
            let c = options.construction;
            let result = mesh.build_components(
                kind,
                &faces,
                &edges,
                &points,
                anchor,
                crate::sculpt::ComponentBuildOptions {
                    amount,
                    width: c.width,
                    round: c.round,
                    triangulate: c.triangulate,
                    crease: c.crease,
                    other_side: c.other_side,
                },
                &ModelingSymmetry::new(mesh, document.symmetry())?,
            )?;
            mesh.check_model_protection(&result, options.protect_mask)?;
            return Ok(EditOutcome::new(result, amount, false));
        }
        if matches!(
            options.tool,
            ModelTool::Extrude | ModelTool::Inset | ModelTool::Bevel | ModelTool::Fuse
        ) {
            let faces: Vec<_> = components
                .iter()
                .filter_map(|c| {
                    if let Component::Face(f) = c {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .collect();
            let symmetry = ModelingSymmetry::new(mesh, document.symmetry())?;
            let result = if options.tool == ModelTool::Fuse || options.tool == ModelTool::Extrude {
                let r = if options.tool == ModelTool::Fuse {
                    mesh.extrude_symmetric_contact(
                        &faces,
                        amount,
                        crate::sculpt::ExtrudeContact {
                            join: options.join,
                            loops: true,
                            snap: options.snap,
                        },
                        &symmetry,
                    )?
                } else {
                    mesh.extrude_model(&faces, amount, options.join, options.individual, &symmetry)?
                };
                EditOutcome {
                    mesh: r.mesh,
                    amount: r.distance,
                    retain_caps: !r.joined && !r.collapsed,
                    notice: if r.partial_contact {
                        "Some regions snapped; others have no compatible contact"
                    } else if r.varying_distance {
                        "Regions snapped at different depths; amount shows requested travel"
                    } else if r.joined && r.collapsed {
                        "Regions snapped to caps and edge loops"
                    } else if r.joined {
                        "Tunnel snapped"
                    } else if r.collapsed {
                        "Edge loop snapped"
                    } else if options.tool == ModelTool::Fuse {
                        "No compatible contact; free extrusion"
                    } else {
                        ""
                    },
                }
            } else {
                let kind = if options.tool == ModelTool::Inset {
                    crate::sculpt::Construction::Inset
                } else {
                    crate::sculpt::Construction::Bevel
                };
                EditOutcome::new(
                    mesh.construct_symmetric(&faces, amount, kind, options.individual, &symmetry)?,
                    amount,
                    true,
                )
            };
            mesh.check_model_protection(&result.mesh, options.protect_mask)?;
            return Ok(result);
        }
        if options.tool == ModelTool::Slide {
            let edges: Vec<_> = components
                .iter()
                .filter_map(|c| {
                    if let Component::Edge(e) = c {
                        Some(*e)
                    } else {
                        None
                    }
                })
                .collect();
            let result = mesh.slide_model_edges(
                &edges,
                amount,
                options.axis.vector(),
                anchor,
                &ModelingSymmetry::new(mesh, document.symmetry())?,
            )?;
            mesh.check_model_protection(&result, options.protect_mask)?;
            return Ok(EditOutcome::new(result, amount, false));
        }
        if options.tool == ModelTool::LoopCut {
            let edge = components
                .iter()
                .find_map(|c| {
                    if let Component::Edge(e) = c {
                        Some(*e)
                    } else {
                        None
                    }
                })
                .ok_or("Select an edge to repeat the cut")?;
            let result = mesh
                .insert_symmetric_edge_loops(
                    edge,
                    options.loop_count,
                    amount,
                    &ModelingSymmetry::new(mesh, document.symmetry())?,
                )?
                .mesh;
            mesh.check_model_protection(&result, options.protect_mask)?;
            return Ok(EditOutcome::new(result, amount, false));
        }
        let direct = options.interaction != ModelInteraction::Axis;
        let axis = if direct {
            if options.tool == ModelTool::Scale && options.interaction == ModelInteraction::Free {
                Vec3::default()
            } else {
                options.transform_direction
            }
        } else if options.axis == ModelAxis::Normal {
            let mut normal = Vec3::default();
            for p in &polygons {
                if p.vertices.iter().all(|v| vertices.contains(v)) {
                    let a = mesh.vertices[p.vertices[0]];
                    normal = normal
                        + (mesh.vertices[p.vertices[1]] - a)
                            .cross(mesh.vertices[p.vertices[2]] - a);
                }
            }
            if normal.len() < 1e-8 {
                return Err("Normal axis needs a face region with a shared direction".into());
            }
            normal.norm()
        } else {
            options.axis.vector()
        };
        if direct && axis.len() < 1e-8 && options.tool != ModelTool::Scale {
            return Ok(EditOutcome::new(mesh.clone(), amount, false));
        }
        let transform = match options.tool {
            ModelTool::Move => ComponentTransform::Move(axis * amount),
            ModelTool::Rotate => ComponentTransform::Rotate(axis, amount.to_radians()),
            ModelTool::Scale => ComponentTransform::Scale(
                if !direct && options.axis == ModelAxis::Uniform {
                    Vec3::default()
                } else {
                    axis
                },
                amount,
            ),
            _ => return Err("Choose a component edit".into()),
        };
        mesh.transform_components(
            &vertices.into_iter().collect::<Vec<_>>(),
            anchor,
            transform,
            options.protect_mask,
            &ModelingSymmetry::new(mesh, document.symmetry())?,
        )
        .map(|mesh| EditOutcome::new(mesh, amount, false))
    }
    pub(super) fn commit_edit(
        &mut self,
        document: &mut Document,
        mut options: ModelingOptions,
    ) -> Result<(), String> {
        self.notice = None;
        let Some(drag) = self.edit_drag.take() else {
            return Ok(());
        };
        if let Some(direct) = &drag.direct {
            if !drag.moved && direct.gizmo {
                return Ok(());
            }
            if drag.moved {
                options.transform_direction = direct.direction;
            }
        }
        let result = if !drag.moved {
            if !options.tap_repeat && !options.tool.instant() {
                return Ok(());
            }
            Self::edit_result(
                document,
                &drag.components,
                drag.anchor,
                options,
                options.amount,
            )?
        } else {
            drag.result.ok_or("Drag to preview the edit")??
        };
        if result.mesh == *document.mesh() {
            return Ok(());
        }
        let caps = if result.retain_caps
            && matches!(
                options.tool,
                ModelTool::Inset | ModelTool::Bevel | ModelTool::Extrude
            ) {
            Self::cap_selection(document.mesh(), &result.mesh, &drag.components)
        } else {
            BTreeSet::new()
        };
        let amount = result.amount;
        document.commit_modeling(result.mesh, options.tool.label(), drag.revision)?;
        self.sync(document);
        let mut last = options;
        last.amount = amount;
        self.notice = (!result.notice.is_empty()).then_some(result.notice);
        self.last_edit = Some(last);
        self.requested_amount = Some(amount);
        if options.tool.is_transform() {
            self.requested_direction = Some(options.transform_direction);
        }
        self.selection_anchor = Some(
            if drag.direct.is_some() && options.tool == ModelTool::Move {
                drag.anchor + options.transform_direction * amount
            } else {
                drag.anchor
            },
        );
        if matches!(
            options.tool,
            ModelTool::Move | ModelTool::Rotate | ModelTool::Scale | ModelTool::Slide
        ) {
            self.selected = drag.components;
        } else {
            self.selected = caps;
        }
        Ok(())
    }
    pub fn apply_amount(
        &mut self,
        document: &mut Document,
        options: ModelingOptions,
    ) -> Result<(), String> {
        self.notice = None;
        self.sync(document);
        let vertices = self.selected_vertices(document.mesh());
        let anchor = self
            .selection_anchor
            .or_else(|| vertices.first().map(|&v| document.mesh().vertices[v]))
            .ok_or("Select components first")?;
        let result = Self::edit_result(document, &self.selected, anchor, options, options.amount)?;
        if result.mesh == *document.mesh() {
            return Ok(());
        }
        let caps = if result.retain_caps
            && matches!(
                options.tool,
                ModelTool::Inset | ModelTool::Bevel | ModelTool::Extrude
            ) {
            Self::cap_selection(document.mesh(), &result.mesh, &self.selected)
        } else {
            BTreeSet::new()
        };
        let mut options = options;
        options.amount = result.amount;
        document.commit_modeling(result.mesh, options.tool.label(), self.revision)?;
        if !matches!(
            options.tool,
            ModelTool::Move | ModelTool::Rotate | ModelTool::Scale | ModelTool::Slide
        ) {
            self.selected = caps;
        }
        self.revision = document.display_revision();
        self.notice = (!result.notice.is_empty()).then_some(result.notice);
        self.last_edit = Some(options);
        self.requested_amount = Some(options.amount);
        self.symmetry = None;
        Ok(())
    }
}
impl Viewport {
    pub(super) fn update_edit_gesture(
        &mut self,
        document: &Document,
        scene: &Scene,
        model: ModelingOptions,
        hover: Option<Component>,
        point: Option<Pos2>,
        pressed: bool,
    ) {
        if !pressed || self.session.gesture.is_some() {
            return;
        }
        let (Some(component), Some(point)) = (hover, point) else {
            return;
        };
        let targets = if self.modeling.selected.contains(&component) {
            Ok(self.modeling.selected.iter().copied().collect::<Vec<_>>())
        } else {
            Self::selection_target(
                component,
                point,
                model.span,
                model.flat_angle,
                document.mesh(),
                scene,
                None,
            )
        };
        match targets {
            Ok(targets) => {
                let anchor = scene.pick(point).map_or(Vec3::default(), |h| h.0);
                if model.tool.is_transform() && model.interaction == ModelInteraction::Gizmo {
                    self.modeling.selected = targets.into_iter().collect();
                    self.modeling.selection_anchor = Some(anchor);
                    self.modeling.symmetry = None;
                    self.session.gesture = Some(Gesture::Modeling);
                    return;
                }
                if model.tool.is_transform() && model.interaction == ModelInteraction::Free {
                    match self.modeling.begin_direct(
                        document,
                        scene,
                        model,
                        point,
                        targets.into_iter().collect(),
                        anchor,
                        super::transform::Handle::Free,
                        false,
                    ) {
                        Ok(()) => self.session.gesture = Some(Gesture::Modeling),
                        Err(e) => self.status = Some(e),
                    }
                    return;
                }
                self.modeling.edit_drag = Some(EditDrag {
                    direct: None,
                    start: point,
                    rail: if let Component::Edge([a, b]) = component {
                        let anchor = scene.pick(point).map_or(Vec3::default(), |h| h.0);
                        let delta = scene.project(document.mesh().vertices[b])
                            - scene.project(document.mesh().vertices[a]);
                        Some(
                            if (document.mesh().vertices[a] - anchor).len()
                                <= (document.mesh().vertices[b] - anchor).len()
                            {
                                delta
                            } else {
                                -delta
                            },
                        )
                    } else {
                        None
                    },
                    initial_amount: model.amount,
                    moved: false,
                    scale: scene.scale_at(scene.pick(point).map_or(Vec3::default(), |h| h.0)),
                    components: targets.into_iter().collect(),
                    anchor: scene.pick(point).map_or(Vec3::default(), |h| h.0),
                    revision: document.display_revision(),
                    amount: 0.,
                    result: None,
                });
                self.session.gesture = Some(Gesture::Modeling);
            }
            Err(e) => self.status = Some(e),
        }
    }
}

impl ModelingState {
    pub fn topology_edit(
        &mut self,
        document: &mut Document,
        edit: crate::sculpt::TopologyEdit,
        protect: bool,
    ) -> Result<(), String> {
        self.sync(document);
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        let mut vertices = Vec::new();
        for c in &self.selected {
            match *c {
                Component::Face(f) => faces.push(f),
                Component::Edge(e) => edges.push(e),
                Component::Vertex(v) => vertices.push(v),
            }
        }
        let symmetry = ModelingSymmetry::new(document.mesh(), document.symmetry())?;
        let result = document
            .mesh()
            .edit_model_topology(edit, &faces, &edges, &vertices, &symmetry)?;
        document.mesh().check_model_protection(&result, protect)?;
        document.commit_modeling(result, edit.label(), self.revision)?;
        self.sync(document);
        Ok(())
    }
}

impl ModelingState {
    pub fn repeat_last(
        &mut self,
        document: &mut Document,
        mode: ComponentMode,
        protect_mask: bool,
    ) -> Result<(), String> {
        let mut options = self
            .last_edit
            .ok_or("Complete a measured edit before repeating")?;
        if mode != options.mode {
            return Err("Choose the same component mode as the last edit before repeating".into());
        }
        options.protect_mask = protect_mask;
        self.apply_amount(document, options)
    }
}

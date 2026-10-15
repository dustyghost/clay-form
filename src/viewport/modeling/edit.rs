//! Shared preview/commit path for measured component operations.
use super::*;
use crate::sculpt::ComponentTransform;
pub(super) struct EditDrag {
    pub start: Pos2,
    pub scale: f32,
    pub components: BTreeSet<Component>,
    pub anchor: Vec3,
    pub revision: u64,
    pub amount: f32,
    pub result: Option<Result<Mesh, String>>,
}
impl ModelTool {
    pub fn is_edit(self) -> bool {
        matches!(
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
            if let Some(point) = point {
                let travel = drag.start.y - point.y;
                drag.amount = match options.tool {
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
    fn edit_result(
        document: &Document,
        components: &BTreeSet<Component>,
        anchor: Vec3,
        options: ModelingOptions,
        amount: f32,
    ) -> Result<Mesh, String> {
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
            let result = if options.tool == ModelTool::Fuse {
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
                .mesh
            } else if options.tool == ModelTool::Extrude {
                mesh.extrude_model(&faces, amount, options.join, options.individual, &symmetry)?
                    .mesh
            } else {
                let kind = match options.tool {
                    ModelTool::Inset => crate::sculpt::Construction::Inset,
                    ModelTool::Bevel => crate::sculpt::Construction::Bevel,
                    _ => crate::sculpt::Construction::Extrude,
                };
                mesh.construct_symmetric(&faces, amount, kind, options.individual, &symmetry)?
            };
            mesh.check_model_protection(&result, options.protect_mask)?;
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
            return Ok(result);
        }
        let axis = if options.axis == ModelAxis::Normal {
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
        let transform = match options.tool {
            ModelTool::Move => ComponentTransform::Move(axis * amount),
            ModelTool::Rotate => ComponentTransform::Rotate(axis, amount.to_radians()),
            ModelTool::Scale => ComponentTransform::Scale(
                if options.axis == ModelAxis::Uniform {
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
    }
    pub(super) fn commit_edit(
        &mut self,
        document: &mut Document,
        options: ModelingOptions,
    ) -> Result<(), String> {
        let Some(drag) = self.edit_drag.take() else {
            return Ok(());
        };
        let result = drag.result.ok_or("Drag to preview the edit")??;
        let caps = if matches!(
            options.tool,
            ModelTool::Inset | ModelTool::Bevel | ModelTool::Extrude
        ) {
            Self::cap_selection(document.mesh(), &result, &drag.components)
        } else {
            BTreeSet::new()
        };
        document.commit_modeling(result, options.tool.label(), drag.revision)?;
        self.sync(document);
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
        self.sync(document);
        let vertices = self.selected_vertices(document.mesh());
        let anchor = vertices
            .first()
            .map(|&v| document.mesh().vertices[v])
            .ok_or("Select components first")?;
        let result = Self::edit_result(document, &self.selected, anchor, options, options.amount)?;
        let caps = if matches!(
            options.tool,
            ModelTool::Inset | ModelTool::Bevel | ModelTool::Extrude
        ) {
            Self::cap_selection(document.mesh(), &result, &self.selected)
        } else {
            BTreeSet::new()
        };
        document.commit_modeling(result, options.tool.label(), self.revision)?;
        if !matches!(
            options.tool,
            ModelTool::Move | ModelTool::Rotate | ModelTool::Scale | ModelTool::Slide
        ) {
            self.selected = caps;
        }
        self.revision = document.display_revision();
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
        if !pressed || self.gesture.is_some() {
            return;
        }
        let (Some(component), Some(point)) = (hover, point) else {
            return;
        };
        let targets = if self.modeling.selected.contains(&component) {
            Ok(self.modeling.selected.iter().copied().collect::<Vec<_>>())
        } else {
            Self::selection_target(component, point, model.span, document.mesh(), scene, None)
        };
        match targets {
            Ok(targets) => {
                self.modeling.edit_drag = Some(EditDrag {
                    start: point,
                    scale: scene.scale_at(scene.pick(point).map_or(Vec3::default(), |h| h.0)),
                    components: targets.into_iter().collect(),
                    anchor: scene.pick(point).map_or(Vec3::default(), |h| h.0),
                    revision: document.display_revision(),
                    amount: 0.,
                    result: None,
                });
                self.gesture = Some(Gesture::Modeling);
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

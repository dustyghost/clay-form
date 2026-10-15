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
        matches!(self, Self::Move | Self::Rotate | Self::Scale)
    }
}
impl ModelingState {
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
        document.commit_modeling(result, options.tool.label(), drag.revision)?;
        self.sync(document);
        self.selected = drag.components;
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
        document.commit_modeling(result, options.tool.label(), self.revision)?;
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

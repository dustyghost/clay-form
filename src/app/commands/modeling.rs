//! Modeling command execution after shared availability and edit-boundary checks.
use super::*;

impl App {
    pub(super) fn dispatch_modeling(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::ModelingMode(active) => if active {
                self.document.require_modeling()
            } else {
                Ok(())
            }
            .map(|()| {
                self.viewport.cancel(&mut self.document);
                self.ui.modeling.active = active;
                self.ui.edges.selecting = false;
                self.ui.selection_tool = None;
                self.ui.slice_active = false;
                self.ui.arrange_references = false;
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                if active {
                    self.ui.wire = true;
                }
            }),
            Command::ModelingComponent(mode) => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.sync(&self.document);
                self.viewport
                    .modeling
                    .convert_selection(self.document.mesh(), mode, false);
                self.viewport.set_edge_selection(Vec::new(), &self.document);
                self.ui.modeling.mode = mode;
                self.ui.modeling.span = crate::viewport::SelectionSpan::Single;
                self.ui
                    .modeling
                    .set_tool(crate::viewport::ModelTool::Select);
                Ok(())
            }
            Command::ClearModelSelection => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.selected.clear();
                self.viewport.set_edge_selection(Vec::new(), &self.document);
                Ok(())
            }
            Command::ModelBoundary => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.sync(&self.document);
                self.viewport.modeling.convert_selection(
                    self.document.mesh(),
                    crate::viewport::ComponentMode::Edge,
                    true,
                );
                self.ui.modeling.mode = crate::viewport::ComponentMode::Edge;
                self.ui
                    .modeling
                    .set_tool(crate::viewport::ModelTool::Select);
                self.ui.modeling.span = crate::viewport::SelectionSpan::Single;
                Ok(())
            }
            Command::ModelTopology(edit) => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.topology_edit(
                    &mut self.document,
                    edit,
                    self.ui.modeling.protect_mask,
                )
            }
            Command::ModelRepeat => {
                self.viewport.cancel(&mut self.document);
                self.viewport
                    .modeling
                    .repeat_last(
                        &mut self.document,
                        self.ui.modeling.mode,
                        self.ui.modeling.protect_mask,
                    )
                    .map(|()| {
                        if let Some(last) = self.viewport.modeling.last_edit {
                            self.ui.modeling.load_options(last);
                        }
                    })
            }
            Command::ModelApply => {
                self.viewport.cancel(&mut self.document);
                let options = self.ui.modeling.options();
                self.viewport
                    .modeling
                    .apply_amount(&mut self.document, options)
            }
            Command::ModelDissolveLoops => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.sync(&self.document);
                let edges = self.viewport.selected_edges(&self.document).to_vec();
                let revision = self.document.display_revision();
                self.document
                    .require_modeling()
                    .and_then(|()| {
                        let symmetry = crate::sculpt::ModelingSymmetry::new(
                            self.document.mesh(),
                            self.document.symmetry(),
                        )?;
                        let mesh = self
                            .document
                            .mesh()
                            .dissolve_loops(&symmetry.edges(&edges))?;
                        symmetry.validate_result(&mesh)?;
                        self.document
                            .mesh()
                            .check_model_protection(&mesh, self.ui.modeling.protect_mask)?;
                        Ok(mesh)
                    })
                    .and_then(|mesh| {
                        self.document
                            .commit_modeling(mesh, "Dissolve selected loops", revision)
                    })
                    .map(|()| {
                        self.viewport.modeling.sync(&self.document);
                        self.viewport.set_edge_selection(Vec::new(), &self.document);
                    })
            }
            Command::ModelSelectLoop => {
                self.viewport.cancel(&mut self.document);
                self.viewport.modeling.sync(&self.document);
                let edges = self.viewport.selected_edges(&self.document).to_vec();
                crate::sculpt::ModelingSymmetry::new(self.document.mesh(), self.document.symmetry())
                    .and_then(|symmetry| {
                        self.document
                            .mesh()
                            .expand_edge_loop(&edges)
                            .map(|edges| symmetry.edges(&edges))
                    })
                    .map(|edges| {
                        self.viewport.set_model_edges(edges, &self.document);
                    })
            }
            Command::GroupModelSelection(existing, remove) => {
                self.viewport.modeling.sync(&self.document);
                let vertices = self
                    .viewport
                    .modeling
                    .selected_vertices(self.document.mesh());
                let faces = (self.ui.modeling.mode == crate::viewport::ComponentMode::Face)
                    .then(|| self.viewport.modeling.selected_faces());
                let edges = self.viewport.selected_edges(&self.document).to_vec();
                self.document
                    .group_model_selection(
                        &vertices,
                        faces.as_deref(),
                        &self.ui.modeling.group_name,
                        existing,
                        remove,
                    )
                    .map(|target| {
                        self.ui.modeling.group = target.unwrap_or(0);
                        self.ui.selected_vertex_group =
                            target.map(|g| (self.document.active_model_index(), g));
                        self.ui.vertex_group_rename = target
                            .map(|g| self.document.mesh().vertex_groups[g].name.clone())
                            .unwrap_or_default();
                        if target.is_some_and(|i| !self.document.mesh().vertex_groups[i].visible) {
                            self.viewport.modeling.sync(&self.document);
                        } else {
                            self.viewport.modeling.revision = self.document.display_revision();
                            self.viewport.set_edge_selection(edges, &self.document);
                        }
                    })
            }
            _ => unreachable!("command routed to the wrong feature handler"),
        }
    }
}

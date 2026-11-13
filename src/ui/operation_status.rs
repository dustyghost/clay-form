//! Persistent feedback for the next viewport gesture, independent of panel placement.
use super::*;
use crate::viewport::{SelectionTool, TransformMode};

#[derive(Debug)]
struct OperationSummary {
    mode: String,
    target: String,
    hint: String,
    return_action: Option<Action>,
}

impl UiState {
    fn operation_summary(&self, document: &Document, ctx: &egui::Context) -> OperationSummary {
        let model = document
            .model(document.active_model_index())
            .map_or("No model", |model| model.name);
        let mut summary = OperationSummary {
            mode: "Sculpt".into(),
            target: model.into(),
            hint: String::new(),
            return_action: None,
        };
        if self.mesh_busy {
            summary.mode = "Mesh operation in progress".into();
            summary.hint = "You can navigate while this runs. Cancel to stop the operation.".into();
            return summary;
        }
        if self.projection.open {
            summary.mode = match self.projection.mode {
                SurfaceMode::Projection => "Project details preview",
                SurfaceMode::Relaxation => "Relax mesh preview",
                SurfaceMode::Retopology => "Guided retopology preview",
            }
            .into();
            summary.hint =
                "Apply keeps the preview as an undoable edit. Cancel returns to the model.".into();
            return summary;
        }
        if self.fusion.open {
            summary.mode = "Join meshes preview".into();
            summary.hint = "Choose the two surfaces, then Preview and Apply. Cancel keeps the original models.".into();
            return summary;
        }
        if document.instance_preview_active() {
            summary.mode = "Array / instance preview".into();
            summary.hint =
                "Apply keeps the preview. Cancel restores the previous arrangement.".into();
            return summary;
        }
        if self.paths_active {
            summary.mode = if self.paths_preview_active {
                "Path stroke preview"
            } else {
                "Edit paths"
            }
            .into();
            summary.hint = if self.paths_preview_active {
                "Apply stroke keeps the preview. Cancel preview keeps the path for further editing."
            } else {
                "Save the path to keep its edits. Finish paths returns to your previous tool."
            }
            .into();
            return summary;
        }
        let held = !ctx.wants_keyboard_input()
            && ctx.input(|input| {
                input.focused
                    && (self.preferences.mask_hold.active(input)
                        || self.preferences.visibility_hold.active(input))
            });
        if self.mask_active && held {
            let preset = &self.preferences.mask_preset;
            summary.mode = format!(
                "Temporary mask · {} · {}",
                preset.shape.label(),
                if preset.erase ^ ctx.input(|input| input.modifiers.alt) {
                    "Erase"
                } else {
                    "Add"
                }
            );
            summary.hint = "Release the selection hold to restore your tool. Escape cancels an unfinished outline.".into();
            return summary;
        }
        if self.viewport_menu.visibility_was_active && held {
            let settings = self.preferences.visibility_selection;
            summary.mode = format!(
                "Temporary visibility · {} · {}{}",
                settings.shape.label(),
                if settings.hide ^ ctx.input(|input| input.modifiers.alt) {
                    "Hide"
                } else {
                    "Isolate"
                },
                if settings.flip { " · Flipped" } else { "" }
            );
            summary.hint = "Release the selection hold to restore your tool. Hidden faces remain in the model.".into();
            return summary;
        }
        if self.materials.sampling {
            summary.mode = "Sample appearance".into();
            summary.hint =
                "Click the model to sample into the Paint inputs. Escape cancels sampling.".into();
            return summary;
        }
        if self.arrange_references {
            summary.mode = "Arrange reference images".into();
            summary.target = "Reference images".into();
            summary.hint = "Drag an image to move it. Escape finishes arranging.".into();
            summary.return_action = Some(Action::ArrangeReferences);
            return summary;
        }
        if self.slice_active {
            summary.mode = format!(
                "Slice · {} · {}{}",
                self.slice_options.mode.label(),
                if self.slice_path_mode { "Path" } else { "Line" },
                if self.slice_options.flip {
                    " · Flipped"
                } else {
                    ""
                }
            );
            summary.hint = if self.slice_path_mode {
                "Place and edit points. Apply cut keeps the result. Escape cancels and leaves Slice."
            } else {
                "Drag across the model. Release applies the cut. Escape cancels and leaves Slice."
            }.into();
            summary.return_action = Some(Action::Slice);
            return summary;
        }
        if let Some(tool) = self.selection_tool {
            summary.mode = match tool {
                SelectionTool::Visibility(settings) => format!(
                    "Visibility · {} · {}",
                    settings.shape.label(),
                    if settings.hide { "Hide" } else { "Isolate" }
                ),
                _ => tool.label().into(),
            };
            summary.hint = format!("{} · Escape cancels the gesture", tool.instructions());
            summary.return_action = Some(Action::Sculpt);
            return summary;
        }
        if self.modeling.active {
            summary.mode = format!(
                "Modeling · {:?} · {}",
                self.modeling.mode,
                self.modeling.tool.label()
            );
            summary.target = format!("{model} · {} selected", self.modeling.count);
            summary.hint = if let Some(reason) =
                self.edge_unavailable(&Action::PenModeling, document)
            {
                reason.into()
            } else if self.modeling.preview_amount.is_some() {
                "Release to apply the preview. Escape cancels the edit and keeps your selection."
                    .into()
            } else if self.modeling.count == 0 {
                "Choose components in the viewport. Changing tools keeps the selection.".into()
            } else {
                "Drag to preview an edit. Release applies it; Escape cancels the gesture.".into()
            };
            summary.return_action = Some(Action::PenModeling);
            return summary;
        }
        if self.edges.selecting {
            summary.mode = "Select edges".into();
            summary.target = format!("{model} · {} edges selected", self.edges.selected.len());
            summary.hint = "Select edges in the viewport, then choose an edge operation.".into();
            summary.return_action = Some(Action::SelectEdges);
            return summary;
        }
        if self.transform_mode != TransformMode::Sculpt {
            summary.mode = format!("Transform · {}", self.transform_operation.label());
            summary.hint = format!(
                "{} · Escape cancels the drag",
                self.transform_operation.hint()
            );
            summary.return_action = Some(Action::Sculpt);
            return summary;
        }
        if self.instances.placing {
            summary.mode = "Place objects".into();
            summary.hint = "Place objects on the active surface. Escape ends placement.".into();
            return summary;
        }
        if let Some(brush) = self
            .brush_index_for_context(document, ctx)
            .and_then(|index| document.brushes().get(index))
        {
            summary.mode = if brush.kind == Kind::Paint {
                format!("Paint · {}", brush.name)
            } else if brush.kind == Kind::Mask {
                format!("Mask · {}", brush.name)
            } else {
                format!("Sculpt · {}", brush.name)
            };
            if brush.kind == Kind::Paint {
                if let Some(channel) = document.texture_paint {
                    summary.target = document
                        .mesh()
                        .uv
                        .textures
                        .iter()
                        .find(|image| image.channel == channel)
                        .map_or_else(
                            || format!("{model} · Missing image target"),
                            |image| format!("{model} · Image: {}", image.name),
                        );
                    summary.hint = if document.mesh().uv.current(document.mesh()) {
                        "Strokes change the selected image. Colour and material inputs affect future strokes."
                    } else {
                        "Generate current UVs in UV / textures before painting this image."
                    }.into();
                } else {
                    summary.target =
                        format!("{model} · Vertices · {}", brush.paint_channels.label());
                    summary.hint = "Strokes change vertex appearance. Colour and material inputs affect future strokes.".into();
                }
            } else {
                use crate::sculpt::StrokeMode;
                summary.hint = match brush.effective_stroke_mode() {
                    StrokeMode::Freehand if brush.kind == Kind::Mask => "Drag to apply mask protection along the surface. Reverse the brush effect to erase protection.",
                    StrokeMode::Freehand => "Drag to apply the sculpting brush along the surface.",
                    StrokeMode::Line => "Drag a straight brush stroke. Release to apply; Escape cancels.",
                    StrokeMode::DragRect => "Drag to size and rotate a brush stamp. Release to apply; Escape cancels.",
                    StrokeMode::DragDot => "Drag to position a brush stamp. Release to apply; Escape cancels.",
                }.into();
                if brush.kind == Kind::Mask {
                    summary.target = format!("{model} · Mask protection");
                }
            }
        }
        if document.primitive().is_some() {
            summary.hint = "Convert the primitive in Submodels to begin sculpting.".into();
        } else if !document.active_visible() {
            summary.hint = "Show the active model in Submodels to work on it.".into();
        }
        summary
    }

    /// Keep this strip present when panels are collapsed or Zen mode is active.
    pub(crate) fn operation_status(
        &self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let summary = self.operation_summary(document, ui.ctx());
        ui.horizontal_wrapped(|ui| {
            ui.strong(&summary.mode);
            ui.label(format!("Target: {}", summary.target));
            if let Some(action) = summary.return_action {
                let reason = self.action_unavailable(&action, document);
                let response =
                    ui.add_enabled(reason.is_none(), egui::Button::new("Return to sculpt"));
                let response = if let Some(reason) = reason {
                    response.on_disabled_hover_text(reason)
                } else {
                    response
                };
                if response.clicked() {
                    commands.push(Command::Action(action));
                }
            }
            let finish = if self.mesh_busy {
                Some(("Cancel operation", Command::CancelMeshJob))
            } else if self.projection.open {
                Some(("Cancel preview", Command::CancelProjection))
            } else if self.fusion.open {
                Some(("Cancel join", Command::CancelFusion))
            } else if document.instance_preview_active() {
                Some((
                    "Cancel preview",
                    Command::Instances(crate::command::InstanceCommand::Cancel),
                ))
            } else if self.paths_preview_active {
                Some((
                    "Cancel preview",
                    Command::Path(crate::command::PathCommand::CancelPreview),
                ))
            } else if self.paths_active {
                Some((
                    "Finish paths",
                    Command::Path(crate::command::PathCommand::Close),
                ))
            } else {
                None
            };
            if let Some((label, command)) = finish {
                let reason = crate::app::policy::EditorPolicy::new(self, document)
                    .command(&command)
                    .err();
                if ui
                    .add_enabled(reason.is_none(), egui::Button::new(label))
                    .on_disabled_hover_text(reason.unwrap_or(""))
                    .clicked()
                {
                    commands.push(command);
                }
            }
        });
        ui.label(&summary.hint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::{ComponentMode, ModelTool};

    #[test]
    fn operation_feedback_tracks_mode_selection_and_actual_paint_target() {
        let mut state = UiState::default();
        let mut document = Document::default();
        let ctx = egui::Context::default();
        let summary = state.operation_summary(&document, &ctx);
        assert!(summary.mode.starts_with("Sculpt"));
        assert!(summary.hint.contains("sculpting brush"));
        assert!(!summary.hint.to_lowercase().contains("paint"));
        state.modeling.active = true;
        state.modeling.mode = ComponentMode::Edge;
        state.modeling.tool = ModelTool::Slide;
        let summary = state.operation_summary(&document, &ctx);
        assert!(summary.mode.contains("Edge"));
        assert!(summary.target.contains("0 selected"));
        assert_eq!(summary.return_action, Some(Action::PenModeling));
        state.modeling.active = false;
        state.slice_active = true;
        state.slice_options.mode = crate::sculpt::SliceMode::Trim;
        assert!(
            state
                .operation_summary(&document, &ctx)
                .mode
                .contains("Trim")
        );
        state.slice_active = false;
        state.selected = document
            .append_brush(crate::sculpt::Brush::new(Kind::Paint))
            .unwrap();
        let summary = state.operation_summary(&document, &ctx);
        assert!(summary.mode.starts_with("Paint"));
        assert!(summary.target.contains("Vertices"));
        assert!(summary.hint.contains("future strokes"));
        state.selected = document
            .brushes()
            .iter()
            .position(|brush| brush.kind == Kind::Mask)
            .unwrap();
        let summary = state.operation_summary(&document, &ctx);
        assert!(summary.mode.starts_with("Mask"));
        assert!(summary.target.contains("Mask protection"));
        assert!(summary.hint.contains("protection"));
    }

    #[test]
    fn released_selection_hold_immediately_restores_the_normal_feedback() {
        let mut state = UiState::default();
        let document = Document::default();
        state.mask_active = true;
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert!(
                !state
                    .operation_summary(&document, ctx)
                    .mode
                    .starts_with("Temporary mask")
            );
        });
    }
}

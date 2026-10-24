//! Shared controls for drop-down menus and persistent movable menu windows.
use super::*;

impl UiState {
    pub(super) fn builtin_menu(
        &mut self,
        ui: &mut egui::Ui,
        menu: BuiltinMenu,
        document: &Document,
        camera: &crate::camera::Camera,
        commands: &mut Vec<Command>,
    ) {
        let glyph = match menu {
            BuiltinMenu::Brushes => Glyph::Plus,
            BuiltinMenu::Transform => Glyph::Transform,
            BuiltinMenu::Slice => Glyph::Slice,
            BuiltinMenu::Primitive => Glyph::Primitive,
            BuiltinMenu::Edit => Glyph::Edit,
            BuiltinMenu::Settings => Glyph::Settings,
            BuiltinMenu::Symmetry => Glyph::Symmetry,
            BuiltinMenu::Visibility | BuiltinMenu::Submodels => Glyph::Layers,
            BuiltinMenu::Mesh => Glyph::Mesh,
            BuiltinMenu::Modeling => Glyph::Edit,
            BuiltinMenu::Wireframe => Glyph::Wire,
            BuiltinMenu::Mask => Glyph::Mask,
            BuiltinMenu::View => Glyph::Camera,
            BuiltinMenu::References => Glyph::View,
        };
        let label = menu.label();
        if self
            .preferences
            .menu_sections
            .iter()
            .any(|s| s.menu == menu)
        {
            let response = ui.add(design::button(ui, glyph, label).selected(true))
                .on_hover_text("Docked as a collapsible section. Click to expand; right-click for layout options.");
            if response.clicked() {
                self.preferences
                    .menu_sections
                    .iter_mut()
                    .find(|s| s.menu == menu)
                    .unwrap()
                    .open = true;
            }
            self.section_context(response, SidebarSection::Builtin(menu));
            return;
        }
        if self.preferences.pinned_menus.iter().any(|p| p.menu == menu) {
            if ui
                .add(design::button(ui, glyph, label).selected(true))
                .on_hover_text("Pinned open. Click to unpin and return to a drop-down menu.")
                .clicked()
            {
                self.preferences.pinned_menus.retain(|p| p.menu != menu);
            }
            return;
        }
        let clicked = design::menu(
            ui,
            glyph,
            label,
            (menu == BuiltinMenu::Symmetry && document.symmetry().enabled)
                || (menu == BuiltinMenu::Modeling && self.modeling.active),
            menu == BuiltinMenu::Modeling,
            |ui| {
                ui.set_min_width(280.);
                self.menu_placement_controls(ui, menu, false);
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height((ui.ctx().content_rect().height() - 180.).max(120.))
                    .show(ui, |ui| {
                        self.builtin_menu_contents(menu, ui, document, camera, commands)
                    });
            },
        );
        if clicked && !self.mesh_busy {
            if menu == BuiltinMenu::Transform
                && self.transform_mode == crate::viewport::TransformMode::Sculpt
            {
                commands.push(Command::TransformMode(
                    crate::viewport::TransformMode::World,
                ));
            }
            if menu == BuiltinMenu::Slice && !self.slice_active && document.primitive().is_none() {
                commands.push(Command::SliceTool(true));
            }
        }
    }

    pub(super) fn menu_placement_controls(
        &mut self,
        ui: &mut egui::Ui,
        menu: BuiltinMenu,
        pinned: bool,
    ) -> bool {
        let mut unpin = false;
        ui.horizontal(|ui| {
            ui.weak("Panel");
            if design::icon_button(
                ui,
                if pinned { Glyph::Unpin } else { Glyph::Pin },
                if pinned {
                    "Unpin menu"
                } else {
                    "Pin menu open"
                },
            )
            .clicked()
            {
                if pinned {
                    unpin = true;
                } else {
                    let pos = ui.min_rect().min;
                    self.preferences.pin_menu(menu, [pos.x, pos.y]);
                }
                ui.close_kind(egui::UiKind::Menu);
            }
            self.menu_dock_options(ui, menu);
        });
        unpin
    }

    pub(super) fn pinned_menus(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        camera: &crate::camera::Camera,
        commands: &mut Vec<Command>,
    ) {
        self.pinned_menu_rects.clear();
        for pinned in self.preferences.pinned_menus.clone() {
            let mut open = true;
            let mut unpin = false;
            let enabled = self.pending.is_none()
                && self.primitive_warning.is_none()
                && !self.icon_capture_open()
                && !self.recovery_open
                && !self.settings_open
                && !self.layout_open;
            let max_height = (ctx.content_rect().height() - 80.).max(120.);
            let response = egui::Window::new(pinned.menu.label())
                .id(egui::Id::new(("pinned_menu", pinned.menu)))
                .open(&mut open)
                .enabled(enabled)
                .current_pos(pinned.position)
                .constrain_to(ctx.content_rect())
                .default_width(320.)
                .resizable(false)
                .collapsible(false)
                .max_height(max_height)
                .show(ctx, |ui| {
                    // Placement controls must stay reachable when the menu body scrolls.
                    unpin = self.menu_placement_controls(ui, pinned.menu, true);
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt(("pinned_menu_body", pinned.menu))
                        .max_height((max_height - 80.).max(40.))
                        .show(ui, |ui| {
                            self.builtin_menu_contents(pinned.menu, ui, document, camera, commands);
                        });
                });
            if let Some(response) = response {
                // Include the closing frame so its click cannot edit the model underneath.
                self.pinned_menu_rects.push(response.response.rect);
                if let Some(current) = self
                    .preferences
                    .pinned_menus
                    .iter_mut()
                    .find(|p| p.menu == pinned.menu)
                {
                    let position = response.response.rect.min;
                    current.position = [position.x, position.y];
                }
            }
            if !open || unpin {
                self.preferences
                    .pinned_menus
                    .retain(|p| p.menu != pinned.menu);
            }
        }
    }

    pub(crate) fn pointer_over_pinned_menu(&self, ctx: &egui::Context) -> bool {
        ctx.pointer_hover_pos()
            .is_some_and(|pos| self.pinned_menu_rects.iter().any(|rect| rect.contains(pos)))
    }

    pub(super) fn builtin_menu_contents(
        &mut self,
        menu: BuiltinMenu,
        ui: &mut egui::Ui,
        document: &Document,
        camera: &crate::camera::Camera,
        commands: &mut Vec<Command>,
    ) {
        let popup = ui
            .stack()
            .iter()
            .any(|stack| stack.info.kind == Some(egui::UiKind::Menu));
        match menu {
            BuiltinMenu::Modeling => self.modeling_function_menu(ui, document, commands, popup),
            BuiltinMenu::References => self.reference_controls(ui, document, commands),
            BuiltinMenu::Brushes => {
                if ui
                    .button("Alpha Drag")
                    .customize(Action::NewBrush("Alpha Drag".into()))
                    .clicked()
                {
                    commands.push(Command::AddAlphaDragBrush);
                    if popup {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                }
                for kind in Kind::ALL {
                    if ui
                        .button(format!("{kind:?}"))
                        .customize(Action::NewBrush(format!("{kind:?}")))
                        .clicked()
                    {
                        commands.push(Command::AddBrush(kind));
                        if popup {
                            ui.close_kind(egui::UiKind::Menu);
                        }
                    }
                }
            }
            BuiltinMenu::Transform => self.transform_controls(ui, commands),
            BuiltinMenu::Slice => self.slice_controls(ui, document, commands),
            BuiltinMenu::Primitive => {
                if document.primitive().is_some() {
                    self.primitive_controls(ui, document, commands);
                } else if ui
                    .add_enabled(
                        !self.mesh_busy,
                        design::button(ui, Glyph::Primitive, "Create primitive…"),
                    )
                    .customize(Action::Primitive)
                    .clicked()
                {
                    commands.push(Command::RequestPrimitive);
                    if popup {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                }
            }
            BuiltinMenu::Edit => {
                for (action, command, label, available) in [
                    (
                        Action::Undo,
                        Command::Undo,
                        document.undo_label(),
                        document.can_undo(),
                    ),
                    (
                        Action::Redo,
                        Command::Redo,
                        document.redo_label(),
                        document.can_redo(),
                    ),
                ] {
                    let shortcut = self
                        .preferences
                        .bindings
                        .iter()
                        .find(|(a, _)| *a == action)
                        .and_then(|(_, binding)| binding.as_ref())
                        .map_or(String::new(), |b| b.label());
                    let text = format!(
                        "{}{}",
                        action.label(),
                        label.map_or(String::new(), |s| format!(" {s}"))
                    );
                    if ui
                        .add_enabled(
                            available && !self.mesh_busy && self.pending.is_none(),
                            design::button(
                                ui,
                                if action == Action::Undo {
                                    Glyph::Undo
                                } else {
                                    Glyph::Redo
                                },
                                text,
                            )
                            .shortcut_text(shortcut),
                        )
                        .customize(action.clone())
                        .clicked()
                    {
                        commands.push(command);
                        if popup {
                            ui.close_kind(egui::UiKind::Menu);
                        }
                    }
                }
                ui.separator();
                let (undo, redo) = document.history_counts();
                ui.label(format!("{undo} undo steps • {redo} redo steps"));
                ui.label("Geometry and masks • history resets on New/Open");
            }
            BuiltinMenu::Settings => {
                if ui
                    .add(design::button(
                        ui,
                        Glyph::Export,
                        "Export diagnostic bundle…",
                    ))
                    .clicked()
                {
                    self.diagnostics_open = true;
                    if popup {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                }
                if ui
                    .add(design::button(ui, Glyph::Restore, "Recovery saves…"))
                    .customize(Action::RecoverySettings)
                    .clicked()
                {
                    commands.push(Command::Action(Action::RecoverySettings));
                    if popup {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                }
                if ui
                    .add(design::button(ui, Glyph::Settings, "Key settings…"))
                    .customize(Action::KeySettings)
                    .clicked()
                {
                    self.settings_open = true;
                    if popup {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                }
                if ui
                    .add(design::button(ui, Glyph::Layers, "Customize workspace…"))
                    .customize(Action::CustomizeWorkspace)
                    .clicked()
                {
                    self.layout_open = true;
                    if popup {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                }
            }
            BuiltinMenu::Visibility => {
                let hidden = document.mesh().hidden_face_count();
                ui.label(format!(
                    "{hidden} / {} faces hidden",
                    document.mesh().triangles.len()
                ));
                ui.add_enabled_ui(!self.mesh_busy && document.primitive().is_none(), |ui| {
                        for (tool, action) in [
                            (crate::viewport::SelectionTool::Rectangle, Action::SelectRectangle),
                            (crate::viewport::SelectionTool::Lasso, Action::SelectLasso),
                            (crate::preferences::VisibilitySettings {shape: crate::preferences::VisibilityShape::Line, ..self.preferences.visibility_selection}.tool(), Action::SelectVisibilityLine),
                            (crate::preferences::VisibilitySettings {shape: crate::preferences::VisibilityShape::Path, ..self.preferences.visibility_selection}.tool(), Action::SelectVisibilityPath),
                        ] {
                            if ui.selectable_label(self.selection_tool == Some(tool), action.label()).on_hover_text("Selects face centers through all depths, keeping quads whole. Line and closed path use the chosen Hide/Isolate operation.").customize(action.clone()).clicked() {
                                commands.push(Command::Action(action));
                                if popup { ui.close_kind(egui::UiKind::Menu); }
                            }
                        }
                        if ui.add(design::button(ui, Glyph::View, "Show all")).customize(Action::ShowAllFaces).clicked() { commands.push(Command::ShowAllFaces); }
                        if ui.add(design::button(ui, Glyph::Restore, "Invert visibility")).customize(Action::InvertVisibility).clicked() { commands.push(Command::InvertVisibility); }
                        if ui.add_enabled(hidden > 0 && hidden < document.mesh().triangles.len() && document.active_visible(), design::button(ui, Glyph::Layers, "Split hidden"))
                            .on_hover_text("Move hidden faces into a new submodel. Keeps masks and groups; boundaries remain open. Resets subdivision levels. Undo restores the original.").customize(Action::SplitHidden).clicked() {
                            commands.push(Command::SplitHidden);
                        }
                    });
                ui.separator();
                ui.label(format!(
                    "Hold {}: remembered Hide/Isolate",
                    self.preferences.visibility_hold.label()
                ));
                ui.label(format!(
                    "Tool menu: {}",
                    self.preferences
                        .shortcut(&Action::MaskVisibilityMenu)
                        .map_or("Unbound".into(), |s| s.label())
                ));
                ui.label("Rectangle click: hide group; last visible group flips visibility");
                ui.label("Rectangle click background: show all");
                ui.label("Rectangle drag empty background: invert");
                ui.label("Release hold or Escape: cancel unfinished selection");
                ui.weak("Selects face centers through all depths; keeps quads whole.");
            }
            BuiltinMenu::Mesh => self.mesh_controls(ui, document, commands),
            BuiltinMenu::Submodels => self.submodel_controls(ui, document, commands),
            BuiltinMenu::Wireframe => {
                use crate::viewport::WireMode;
                design::checkbox(ui, &mut self.wire, "Show wireframe").customize(Action::Wireframe);
                ui.radio_value(
                    &mut self.wire_mode,
                    WireMode::Visible,
                    "Visible surface (hide occluded edges)",
                )
                .customize(Action::WireVisible);
                ui.radio_value(
                    &mut self.wire_mode,
                    WireMode::Front,
                    "Front facing (show through submodels)",
                )
                .customize(Action::WireFront);
                ui.radio_value(
                    &mut self.wire_mode,
                    WireMode::Transparent,
                    "Transparent (all edges)",
                )
                .customize(Action::WireTransparent);
                design::checkbox(ui, &mut self.wire_triangles, "Show triangle diagonals")
                    .customize(Action::WireTriangles);
            }
            BuiltinMenu::Symmetry => {
                self.mirror_controls(ui, document, commands);
                ui.separator();
                ui.strong("Brush symmetry");
                let mut symmetry = document.symmetry().clone();
                design::checkbox(ui, &mut symmetry.enabled, "Enable symmetry")
                    .customize(Action::Symmetry);

                ui.label("Mirror planes");
                ui.horizontal(|ui| {
                    for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
                        design::checkbox(ui, &mut symmetry.mirror[i], *label);
                    }
                });
                design::checkbox(ui, &mut symmetry.radial, "Radial symmetry");
                ui.add_enabled_ui(symmetry.radial, |ui| {
                    egui::ComboBox::from_label("Radial axis")
                        .selected_text(format!("{:?}", symmetry.radial_axis))
                        .show_ui(ui, |ui| {
                            for axis in Axis::ALL {
                                ui.selectable_value(
                                    &mut symmetry.radial_axis,
                                    axis,
                                    format!("{axis:?}"),
                                );
                            }
                        });
                    ui.add(egui::Slider::new(&mut symmetry.radial_count, 2..=16).text("Repeats"));
                });
                ui.separator();
                ui.label("Symmetry center (object coordinates)");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut symmetry.center.0)
                            .speed(0.01)
                            .range(-1_000_000. ..=1_000_000.)
                            .prefix("X "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut symmetry.center.1)
                            .speed(0.01)
                            .range(-1_000_000. ..=1_000_000.)
                            .prefix("Y "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut symmetry.center.2)
                            .speed(0.01)
                            .range(-1_000_000. ..=1_000_000.)
                            .prefix("Z "),
                    );
                });
                if ui
                    .add(design::button(ui, Glyph::Transform, "Center on origin"))
                    .customize(Action::CenterSymmetry)
                    .clicked()
                {
                    symmetry.center = crate::sculpt::Vec3::default();
                }
                design::checkbox(ui, &mut self.show_symmetry_guides, "Show symmetry lines")
                    .customize(Action::Guides);
                ui.add(egui::Slider::new(&mut self.guide_opacity, 0.1..=1.).text("Guide opacity"));
                ui.label("Mirror planes and radial repeats can be combined.");

                if symmetry != *document.symmetry() {
                    commands.push(Command::SetSymmetry(symmetry));
                }
            }
            BuiltinMenu::Mask => {
                self.temporary_mask_controls(ui, document, commands);
                let mut threshold = document
                    .model(document.active_model_index())
                    .unwrap()
                    .metadata
                    .mask_threshold;
                if ui.add_enabled(!self.mesh_busy, egui::Slider::new(&mut threshold, 0. ..=1.).text("Selection threshold").custom_formatter(|v,_| format!("{:.0}%",v*100.)).custom_parser(|text| text.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v|v/100.))).on_hover_text("Minimum vertex mask value for grouping, deletion and extraction. Zero-valued vertices are always excluded. Complete faces must meet the threshold at every corner.").changed() { commands.push(Command::MaskThreshold(threshold)); }
                ui.add_enabled_ui(!self.mesh_busy && document.primitive().is_none(), |ui| {
                        ui.strong("Mask through");
                        if ui.add_enabled(document.active_visible(), egui::Button::new("Line").selected(matches!(self.selection_tool, Some(crate::viewport::SelectionTool::MaskLine { .. })))).on_hover_text("Draw a dividing line from empty space. Add or erase masking on one side, front to back; never cuts geometry.").customize(Action::MaskLine).clicked() {
                            commands.push(Command::Action(Action::MaskLine));
                            if popup { ui.close_kind(egui::UiKind::Menu); }
                        }
                        if ui.add_enabled(document.active_visible(), egui::Button::new("Mask lasso").selected(self.selection_tool == Some(crate::viewport::SelectionTool::MaskLasso))).on_hover_text("Drag to add masking; Alt to erase. Selects vertices through all depths. Hidden faces and their boundary vertices are protected.").customize(Action::MaskLasso).clicked() {
                            commands.push(Command::Action(Action::MaskLasso));
                            if popup { ui.close_kind(egui::UiKind::Menu); }
                        }
                        if ui.add(design::button(ui, Glyph::Mask, "Clear mask")).customize(Action::ClearMask).clicked() { commands.push(Command::ClearMask); }
                        if ui.add(design::button(ui, Glyph::Restore, "Invert mask")).on_hover_text("Swap masked and unmasked areas, preserving soft mask values on every subdivision level.").customize(Action::InvertMask).clicked() { commands.push(Command::InvertMask); }
                        ui.horizontal(|ui| {
                            if ui.add(design::button(ui, Glyph::Plus, "Expand mask")).on_hover_text("Spread soft mask values by one mesh edge per click.").customize(Action::ExpandMask).clicked() { commands.push(Command::GrowMask(true)); }
                            if ui.add(design::button(ui, Glyph::Minus, "Contract mask")).on_hover_text("Tighten the mask by one mesh edge per click.").customize(Action::ContractMask).clicked() { commands.push(Command::GrowMask(false)); }
                        });
                        let mut intensity = document.mask_peak();
                        ui.horizontal(|ui| {
                            if ui.add_enabled(document.active_visible(), egui::Button::new("Blur mask")).on_hover_text("Soften the mask by one mesh-edge ring on every subdivision level, including hidden faces. Geometry and saved vertex group weights stay unchanged.").customize(Action::BlurMask).clicked() { commands.push(Command::BlurMask); }
                            if ui.add_enabled(document.active_visible(), egui::Button::new("Sharpen mask")).on_hover_text("Tighten soft mask transitions around 50% on every subdivision level, including hidden faces. Geometry and saved vertex group weights stay unchanged.").customize(Action::SharpenMask).clicked() { commands.push(Command::SharpenMask); }
                        });
                        if ui.add(egui::Slider::new(&mut intensity, 0. ..=1.).text("Mask intensity").custom_formatter(|v,_| format!("{:.0}%",v*100.)).custom_parser(|text| text.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v|v/100.))).on_hover_text("Scale the mask on every subdivision level while preserving its soft falloff. Stored group weights stay unchanged.").changed() { commands.push(Command::MaskIntensity(intensity)); }
                        if ui.add(design::button(ui, Glyph::Delete, "Delete masked")).on_hover_text("Remove faces meeting the mask threshold at every vertex. Creates open boundaries and resets subdivision levels and closed-surface modes. Undo restores everything.").customize(Action::DeleteMasked).clicked() { commands.push(Command::DeleteMasked); }
                    });
            }
            BuiltinMenu::View => {
                let mut perspective = camera.perspective;
                if design::checkbox(ui, &mut perspective, "Perspective view")
                    .customize(Action::Perspective)
                    .on_hover_text("Show depth and foreshortening. Turn off for orthographic comparisons against drawings.")
                    .changed() {
                    commands.push(Command::SetPerspective(perspective));
                }
                if perspective {
                    let mut degrees = camera.field_of_view;
                    if ui.add(egui::Slider::new(&mut degrees, 20. ..=75.).text("Field of view").suffix("°"))
                        .on_hover_text("Wider angles emphasize depth; narrower angles flatten proportions. Preserves the apparent size of the visible models.")
                        .changed() {
                        commands.push(Command::SetFieldOfView(degrees));
                    }
                }
                ui.separator();
                design::checkbox(ui, &mut self.show_polygroups, "Show vertex group colors")
                    .customize(Action::PolygroupColors);
                design::checkbox(ui, &mut self.show_backfaces, "Faint back-facing surfaces").customize(Action::Backfaces).on_hover_text("Show the reverse side of open surfaces translucently. Front-facing surfaces stay solid.");
                if self.show_backfaces {
                    ui.add(
                        egui::Slider::new(&mut self.backface_opacity, 0.02..=0.5)
                            .text("Back-face opacity")
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.))
                            .custom_parser(|text| {
                                text.trim()
                                    .trim_end_matches('%')
                                    .trim()
                                    .parse::<f64>()
                                    .ok()
                                    .map(|v| v / 100.)
                            }),
                    );
                }
                ui.separator();
                if ui
                    .add(design::button(ui, Glyph::Plus, "Zoom in"))
                    .customize(Action::ZoomIn)
                    .clicked()
                {
                    commands.push(Command::ZoomIn);
                }
                if ui
                    .add(design::button(ui, Glyph::Minus, "Zoom out"))
                    .customize(Action::ZoomOut)
                    .clicked()
                {
                    commands.push(Command::ZoomOut);
                }
                let mut value = camera.zoom;
                ui.add(
                    egui::Slider::new(&mut value, 0.2..=8.)
                        .logarithmic(true)
                        .text("Zoom")
                        .custom_formatter(|value, _| format!("{:.0}%", value * 100.))
                        .custom_parser(|text| {
                            text.trim()
                                .trim_end_matches('%')
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .map(|percent| percent / 100.)
                        }),
                );
                if value != camera.zoom {
                    commands.push(Command::SetZoom(value));
                }
                if ui
                    .add(design::button(ui, Glyph::Restore, "Reset zoom to 100%"))
                    .customize(Action::ResetZoom)
                    .clicked()
                {
                    commands.push(Command::SetZoom(1.));
                }
                if ui
                    .add(design::button(ui, Glyph::Restore, "Reset view"))
                    .customize(Action::ResetView)
                    .clicked()
                {
                    commands.push(Command::ResetView);
                }
            }
        }
    }
}

impl UiState {
    pub(super) fn slice_controls(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        ui.add_enabled_ui(
            !self.mesh_busy && self.pending.is_none() && document.primitive().is_none(),
            |ui| {
                if ui
                    .selectable_label(
                        self.slice_active,
                        if self.slice_active {
                            "Done slicing"
                        } else {
                            "Start slicing"
                        },
                    )
                    .customize(Action::Slice)
                    .clicked()
                {
                    commands.push(Command::SliceTool(!self.slice_active));
                }
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.slice_path_mode, false, "Line");
                    ui.selectable_value(&mut self.slice_path_mode, true, "Path");
                    for mode in crate::sculpt::SliceMode::ALL {
                        ui.selectable_value(&mut self.slice_options.mode, mode, mode.label());
                    }
                });
                if self.slice_options.mode != crate::sculpt::SliceMode::Polygroups {
                    design::checkbox(ui, &mut self.slice_options.cap, "Cap cuts").on_hover_text(
                        "Close the cut with flat faces; requires a closed source surface",
                    );
                }
                if self.slice_options.mode == crate::sculpt::SliceMode::Trim {
                    design::checkbox(ui, &mut self.slice_options.flip, "Flip kept side");
                }
                ui.label("Active model · through all depths · resets subdivision levels");
                ui.separator();
                if ui.button("Mask through · Line").on_hover_text("Switch to non-destructive line masking. Keeps geometry and subdivision levels.").customize(Action::MaskLine).clicked() {
                    commands.push(Command::Action(Action::MaskLine));
                    ui.close_kind(egui::UiKind::Menu);
                }
                ui.label("Mask only · preserves geometry and subdivision levels");
            },
        );
    }
}

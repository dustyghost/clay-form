//! Shared controls for drop-down menus and persistent movable menu windows.
use super::*;
use crate::preferences::PinnedMenu;

impl UiState {
    pub(super) fn builtin_menu(
        &mut self,
        ui: &mut egui::Ui,
        menu: BuiltinMenu,
        document: &Document,
        zoom: f32,
        commands: &mut Vec<Command>,
    ) {
        let glyph = match menu {
            BuiltinMenu::Edit => Glyph::Edit,
            BuiltinMenu::Settings | BuiltinMenu::Symmetry => Glyph::Settings,
            BuiltinMenu::Visibility | BuiltinMenu::Submodels => Glyph::Layers,
            BuiltinMenu::Mesh => Glyph::Mesh,
            BuiltinMenu::Wireframe => Glyph::Wire,
            BuiltinMenu::Mask => Glyph::Mask,
            BuiltinMenu::View => Glyph::View,
        };
        if self
            .preferences
            .menu_sections
            .iter()
            .any(|s| s.menu == menu)
        {
            let response = ui.add(design::button(ui, glyph, menu.label()).selected(true))
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
                .add(design::button(ui, glyph, menu.label()).selected(true))
                .on_hover_text("Pinned open. Click to unpin and return to a drop-down menu.")
                .clicked()
            {
                self.preferences.pinned_menus.retain(|p| p.menu != menu);
            }
            return;
        }
        design::menu(ui, glyph, menu.label(), |ui| {
            if ui
                .button("Pin menu open")
                .on_hover_text(
                    "Keep these controls in a floating window. Drag its title bar to move it.",
                )
                .clicked()
            {
                let position = ui.min_rect().min;
                self.preferences.pinned_menus.push(PinnedMenu {
                    menu,
                    position: [position.x, position.y],
                });
                ui.close_kind(egui::UiKind::Menu);
            }
            self.menu_dock_options(ui, menu);
            ui.separator();
            self.builtin_menu_contents(menu, ui, document, zoom, commands);
        });
    }

    pub(super) fn pinned_menus(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        zoom: f32,
        commands: &mut Vec<Command>,
    ) {
        self.pinned_menu_rects.clear();
        for pinned in self.preferences.pinned_menus.clone() {
            let mut open = true;
            let mut unpin = false;
            let enabled = self.pending.is_none()
                && self.primitive_warning.is_none()
                && !self.icon_capture_open()
                && !self.settings_open
                && !self.layout_open;
            let response = egui::Window::new(pinned.menu.label())
                .id(egui::Id::new(("pinned_menu", pinned.menu)))
                .open(&mut open)
                .enabled(enabled)
                .current_pos(pinned.position)
                .constrain_to(ctx.content_rect())
                .default_width(320.)
                .resizable(false)
                .collapsible(false)
                .max_height((ctx.content_rect().height() - 80.).max(120.))
                .vscroll(true)
                .show(ctx, |ui| {
                    unpin = ui
                        .button("Unpin menu")
                        .on_hover_text(
                            "Close this window and return its controls to the toolbar menu.",
                        )
                        .clicked();
                    self.menu_dock_options(ui, pinned.menu);
                    ui.separator();
                    self.builtin_menu_contents(pinned.menu, ui, document, zoom, commands);
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
        zoom: f32,
        commands: &mut Vec<Command>,
    ) {
        let popup = ui
            .stack()
            .iter()
            .any(|stack| stack.info.kind == Some(egui::UiKind::Menu));
        match menu {
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
                            egui::Button::new(text).shortcut_text(shortcut),
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
                        if ui.selectable_label(self.select_active, "Select rectangle").customize(Action::SelectRectangle).clicked() {
                            commands.push(Command::SelectTool(!self.select_active));
                            if popup { ui.close_kind(egui::UiKind::Menu); }
                        }
                        if ui.button("Show all").customize(Action::ShowAllFaces).clicked() { commands.push(Command::ShowAllFaces); }
                        if ui.button("Invert visibility").customize(Action::InvertVisibility).clicked() { commands.push(Command::InvertVisibility); }
                        if ui.add_enabled(hidden > 0 && hidden < document.mesh().triangles.len() && document.active_visible(), egui::Button::new("Split hidden"))
                            .on_hover_text("Move hidden faces into a new submodel. Keeps masks and groups; boundaries remain open. Resets subdivision levels. Undo restores the original.").customize(Action::SplitHidden).clicked() {
                            commands.push(Command::SplitHidden);
                        }
                    });
                ui.separator();
                ui.label("Ctrl+Shift drag: isolate rectangle • Alt: hide");
                ui.label("Ctrl+Shift click: hide group; last visible group flips visibility");
                ui.label("Ctrl+Shift click background: show all");
                ui.label("Ctrl+Shift drag empty background: invert");
                ui.label("Space: move rectangle • Esc: cancel");
                ui.weak("Selects face centers through all depths; keeps quads whole.");
            }
            BuiltinMenu::Mesh => self.mesh_controls(ui, document, commands),
            BuiltinMenu::Submodels => self.submodel_controls(ui, document, commands),
            BuiltinMenu::Wireframe => {
                use crate::viewport::WireMode;
                ui.checkbox(&mut self.wire, "Show wireframe")
                    .customize(Action::Wireframe);
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
                ui.checkbox(&mut self.wire_triangles, "Show triangle diagonals")
                    .customize(Action::WireTriangles);
            }
            BuiltinMenu::Symmetry => {
                let mut symmetry = document.symmetry().clone();
                ui.checkbox(&mut symmetry.enabled, "Enable symmetry")
                    .customize(Action::Symmetry);

                ui.label("Mirror planes");
                ui.horizontal(|ui| {
                    for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
                        ui.checkbox(&mut symmetry.mirror[i], *label);
                    }
                });
                ui.checkbox(&mut symmetry.radial, "Radial symmetry");
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
                ui.checkbox(&mut self.show_symmetry_guides, "Show symmetry lines")
                    .customize(Action::Guides);
                ui.add(egui::Slider::new(&mut self.guide_opacity, 0.1..=1.).text("Guide opacity"));
                ui.label("Mirror planes and radial repeats can be combined.");

                if symmetry != *document.symmetry() {
                    commands.push(Command::SetSymmetry(symmetry));
                }
            }
            BuiltinMenu::Mask => {
                let mut threshold = document
                    .model(document.active_model_index())
                    .unwrap()
                    .metadata
                    .mask_threshold;
                if ui.add_enabled(!self.mesh_busy, egui::Slider::new(&mut threshold, 0. ..=1.).text("Selection threshold").custom_formatter(|v,_| format!("{:.0}%",v*100.)).custom_parser(|text| text.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v|v/100.))).on_hover_text("Minimum vertex mask value for grouping, deletion and extraction. Zero-valued vertices are always excluded. Complete faces must meet the threshold at every corner.").changed() { commands.push(Command::MaskThreshold(threshold)); }
                ui.add_enabled_ui(!self.mesh_busy && document.primitive().is_none(), |ui| {
                        if ui.add(design::button(ui, Glyph::Mask, "Clear mask")).customize(Action::ClearMask).clicked() { commands.push(Command::ClearMask); }
                        if ui.add(design::button(ui, Glyph::Restore, "Invert mask")).on_hover_text("Swap masked and unmasked areas, preserving soft mask values on every subdivision level.").customize(Action::InvertMask).clicked() { commands.push(Command::InvertMask); }
                        ui.horizontal(|ui| {
                            if ui.button("Expand mask").on_hover_text("Spread soft mask values by one mesh edge per click.").customize(Action::ExpandMask).clicked() { commands.push(Command::GrowMask(true)); }
                            if ui.button("Contract mask").on_hover_text("Tighten the mask by one mesh edge per click.").customize(Action::ContractMask).clicked() { commands.push(Command::GrowMask(false)); }
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
                ui.checkbox(&mut self.show_polygroups, "Show vertex group colors")
                    .customize(Action::PolygroupColors);
                ui.checkbox(&mut self.show_backfaces, "Faint back-facing surfaces").customize(Action::Backfaces).on_hover_text("Show the reverse side of open surfaces translucently. Front-facing surfaces stay solid.");
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
                let mut value = zoom;
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
                if value != zoom {
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

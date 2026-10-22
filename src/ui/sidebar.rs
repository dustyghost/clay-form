//! Whole menus share the sidebar ordering with ordinary tools and custom sections.
use super::*;
use crate::preferences::Placement;

impl UiState {
    pub(super) fn menu_dock_options(&mut self, ui: &mut egui::Ui, menu: BuiltinMenu) {
        for (glyph, label, dock) in [
            (Glyph::DockLeft, "Make collapsible on left", Dock::Left),
            (Glyph::DockRight, "Make collapsible on right", Dock::Right),
        ] {
            if design::icon_button(ui, glyph, label).clicked() {
                self.preferences.dock_menu(menu, dock);
                if ui
                    .stack()
                    .iter()
                    .any(|s| s.info.kind == Some(egui::UiKind::Menu))
                {
                    ui.close_kind(egui::UiKind::Menu);
                }
            }
        }
    }

    pub(super) fn builtin_section(
        &mut self,
        ui: &mut egui::Ui,
        menu: BuiltinMenu,
        document: &Document,
        camera: &crate::camera::Camera,
        commands: &mut Vec<Command>,
    ) {
        let open = self
            .preferences
            .menu_sections
            .iter()
            .find(|s| s.menu == menu)
            .unwrap()
            .open;
        let header = egui::CollapsingHeader::new(menu.label())
            .id_salt(("builtin_section", menu))
            .open(Some(open))
            .show(ui, |ui| {
                self.menu_placement_controls(ui, menu, false);
                ui.separator();
                self.builtin_menu_contents(menu, ui, document, camera, commands)
            });
        if header.header_response.clicked() {
            self.preferences
                .menu_sections
                .iter_mut()
                .find(|s| s.menu == menu)
                .unwrap()
                .open = !open;
        }
        self.section_context(header.header_response, SidebarSection::Builtin(menu));
    }

    pub(super) fn section_context(&mut self, response: egui::Response, section: SidebarSection) {
        response
            .on_hover_text("Right-click to reorder or move this section.")
            .context_menu(|ui| {
                let dock = self.preferences.section_dock(section).unwrap();
                let sections = self.preferences.sidebar_sections(dock);
                let index = sections.iter().position(|s| *s == section).unwrap();
                if ui
                    .add_enabled(index > 0, egui::Button::new("Move section up"))
                    .clicked()
                {
                    self.preferences.move_section(section, false);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        index + 1 < sections.len(),
                        egui::Button::new("Move section down"),
                    )
                    .clicked()
                {
                    self.preferences.move_section(section, true);
                    ui.close();
                }
                let (target, label) = if dock == Dock::Left {
                    (Dock::Right, "Move to right sidebar")
                } else {
                    (Dock::Left, "Move to left sidebar")
                };
                if ui.button(label).clicked() {
                    match section {
                        SidebarSection::Tool(tool) => {
                            self.preferences
                                .panels
                                .iter_mut()
                                .find(|p| p.tool == tool)
                                .unwrap()
                                .dock = target
                        }
                        SidebarSection::Builtin(menu) => self.preferences.dock_menu(menu, target),
                        SidebarSection::Custom(id) => {
                            self.preferences
                                .menus
                                .iter_mut()
                                .find(|m| m.id == id)
                                .unwrap()
                                .placement = if target == Dock::Left {
                                Placement::LeftPanel
                            } else {
                                Placement::RightPanel
                            }
                        }
                    }
                    ui.close();
                }
                if !matches!(section, SidebarSection::Tool(_))
                    && ui.button("Return to toolbar menu").clicked()
                {
                    match section {
                        SidebarSection::Builtin(menu) => self.preferences.undock_menu(menu),
                        SidebarSection::Custom(id) => {
                            self.preferences
                                .menus
                                .iter_mut()
                                .find(|m| m.id == id)
                                .unwrap()
                                .placement = Placement::Menu
                        }
                        _ => {}
                    }
                    ui.close();
                }
                ui.separator();
                match section {
                    SidebarSection::Builtin(menu) => {
                        if ui.button("Float menu").clicked() {
                            let pos = ui.min_rect().min;
                            self.preferences.pin_menu(menu, [pos.x, pos.y]);
                            ui.close();
                        }
                        customization::sidebar_options(
                            ui,
                            if dock == Dock::Left {
                                Placement::LeftPanel
                            } else {
                                Placement::RightPanel
                            },
                        );
                    }
                    SidebarSection::Custom(id) => {
                        let menu = self.preferences.menus.iter().find(|m| m.id == id).unwrap();
                        customization::menu_options(ui, menu);
                    }
                    SidebarSection::Tool(tool) => {
                        customization::sidebar_options(
                            ui,
                            if dock == Dock::Left {
                                Placement::LeftPanel
                            } else {
                                Placement::RightPanel
                            },
                        );
                        ui.separator();
                        customization::action_options(ui, &Action::ShowPanel(tool));
                    }
                }
            });
    }

    pub(super) fn sidebar_order_editor(&mut self, ui: &mut egui::Ui) {
        ui.weak("Reorder all sidebar sections here, or right-click a section header.");
        for dock in [Dock::Left, Dock::Right] {
            ui.strong(if dock == Dock::Left {
                "Left sidebar order"
            } else {
                "Right sidebar order"
            });
            let sections = self.preferences.sidebar_sections(dock);
            for (index, section) in sections.iter().enumerate() {
                ui.push_id(("sidebar_order", index, format!("{dock:?}")), |ui| {
                    ui.horizontal(|ui| {
                        if ui.add_enabled(index > 0, egui::Button::new("Up")).clicked() {
                            self.preferences.move_section(*section, false);
                        }
                        if ui
                            .add_enabled(index + 1 < sections.len(), egui::Button::new("Down"))
                            .clicked()
                        {
                            self.preferences.move_section(*section, true);
                        }
                        let label = match section {
                            SidebarSection::Tool(tool) => tool.label(),
                            SidebarSection::Builtin(menu) => menu.label(),
                            SidebarSection::Custom(id) => {
                                &self
                                    .preferences
                                    .menus
                                    .iter()
                                    .find(|m| m.id == *id)
                                    .unwrap()
                                    .name
                            }
                        };
                        ui.label(label);
                    });
                });
            }
        }
    }
}

use super::*;

#[derive(Clone)]
struct Locator {
    key: String,
    destination: String,
    section: Option<String>,
    expires: f64,
    destination_scrolled: bool,
    control_scrolled: bool,
}
fn locator_id() -> egui::Id {
    egui::Id::new("search_control_locator")
}
/// Shared action widgets report their stable identity regardless of whether they
/// are shown in a toolbar, a shelf, a floating menu or a docked panel.
pub(crate) fn highlight_response(response: &egui::Response, key: &str) {
    let ctx = &response.ctx;
    let now = ctx.input(|i| i.time);
    let mut scroll = false;
    let matched = ctx.data_mut(|data| {
        let Some(locator) = data
            .get_temp_mut_or_default::<Option<Locator>>(locator_id())
            .as_mut()
        else {
            return false;
        };
        if now > locator.expires {
            return false;
        }
        if locator.key == key {
            scroll = !locator.control_scrolled;
            locator.control_scrolled = true;
            true
        } else if locator.destination == key && !locator.control_scrolled {
            scroll = !locator.destination_scrolled;
            locator.destination_scrolled = true;
            true
        } else {
            false
        }
    });
    if matched {
        ctx.layer_painter(response.layer_id).rect_stroke(
            response.rect.expand(3.),
            5.,
            egui::Stroke::new(2_f32, ctx.style().visuals.selection.stroke.color),
            egui::StrokeKind::Outside,
        );
        if scroll {
            response.scroll_to_me(Some(egui::Align::Center));
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(120));
    }
}
pub(crate) fn mark_destination(ui: &mut egui::Ui, key: &str) {
    let rect = egui::Rect::from_min_size(
        ui.next_widget_position(),
        egui::vec2(ui.available_width(), 22.),
    );
    let response = ui.interact(
        rect,
        ui.id().with(("search_destination", key)),
        egui::Sense::hover(),
    );
    highlight_response(&response, key);
}
pub(crate) fn requested_section(ctx: &egui::Context, section: &str) -> Option<bool> {
    ctx.data(|data| data.get_temp::<Option<Locator>>(locator_id()))
        .flatten()
        .and_then(|locator| (locator.section.as_deref() == Some(section)).then_some(true))
}
pub(crate) fn is_requested(ctx: &egui::Context, key: &str) -> bool {
    ctx.data(|data| data.get_temp::<Option<Locator>>(locator_id()))
        .flatten()
        .is_some_and(|locator| locator.key == key)
}
fn destination_key(destination: &Destination) -> String {
    match destination {
        Destination::Panel(tool) => format!("panel:{tool:?}"),
        Destination::Menu(menu) => format!("menu:{menu:?}"),
        Destination::Custom(id) => format!("custom:{id}"),
        Destination::Project => "project".into(),
        Destination::Paint => "paint".into(),
        Destination::Workspace => "workspace".into(),
        Destination::Keys => "keys".into(),
        Destination::Capacity => "mesh_capacity".into(),
        Destination::Viewport => "viewport_controls".into(),
        Destination::Recovery => "recovery".into(),
        Destination::Uv(true) => "textures".into(),
        Destination::Uv(false) => "uv".into(),
    }
}
impl UiState {
    pub(super) fn reveal_search_entry(&mut self, ctx: &egui::Context, entry: &Entry) {
        if let Some(reveal) = &mut self.search.reveal {
            let mut base = self.preferences.clone();
            restore_generated_layout(&reveal.workspace, &reveal.shown, &reveal.moved, &mut base);
            reveal.workspace = base.workspace("Before search reveal".into());
            reveal.moved.clear();
        }
        if self.search.reveal.is_none() {
            self.search.reveal = Some(Reveal {
                workspace: self.preferences.workspace("Before search reveal".into()),
                shown: self.preferences.workspace("Before search reveal".into()),
                moved: Vec::new(),
                project: false,
                paint: false,
                active: self.preferences.active_workspace,
                zen: self.zen_mode,
                narrow_panel: self.narrow_panel,
                label: entry.name.clone(),
                dialogs: Dialogs {
                    layout: self.layout_open,
                    keys: self.settings_open,
                    capacity: self.mesh_capacity_open,
                    viewport: self.viewport_controls.editor_open,
                    recovery: self.recovery_open,
                    uv: self.uv.open,
                },
            });
        }
        if let Some(reveal) = &mut self.search.reveal {
            reveal.label = entry.name.clone();
            reveal.project = false;
            reveal.paint = false;
        }
        self.set_zen_mode(false);
        // Reuse existing placements. Only hidden/unplaced destinations need a
        // temporary floating placement, which the persistence snapshot filters.
        self.layout_open = false;
        self.settings_open = false;
        self.mesh_capacity_open = false;
        self.viewport_controls.editor_open = false;
        self.recovery_open = false;
        self.uv.open = false;
        match &entry.destination {
            Destination::Project => {
                if let Some(reveal) = &mut self.search.reveal {
                    reveal.project = true;
                }
            }
            Destination::Paint => {
                if let Some(reveal) = &mut self.search.reveal {
                    reveal.paint = true;
                }
            }
            Destination::Panel(tool) => {
                if let Some(panel) = self
                    .preferences
                    .panels
                    .iter_mut()
                    .find(|panel| panel.tool == *tool)
                {
                    panel.open = true;
                    if panel.dock == Dock::Hidden {
                        panel.dock = Dock::Floating;
                    }
                    if panel.dock == Dock::Floating {
                        panel.position = [
                            ctx.content_rect().left() + 24.,
                            ctx.content_rect().top() + 90.,
                        ];
                    }
                }
            }
            Destination::Menu(menu) => {
                if let Some(section) = self
                    .preferences
                    .menu_sections
                    .iter_mut()
                    .find(|section| section.menu == *menu)
                {
                    section.open = true;
                } else if let Some(pinned) = self
                    .preferences
                    .pinned_menus
                    .iter_mut()
                    .find(|pinned| pinned.menu == *menu)
                {
                    pinned.position = [
                        ctx.content_rect().left() + 24.,
                        ctx.content_rect().top() + 90.,
                    ];
                } else {
                    self.preferences.pin_menu(
                        *menu,
                        [
                            ctx.content_rect().left() + 24.,
                            ctx.content_rect().top() + 90.,
                        ],
                    );
                }
            }
            Destination::Custom(id) => {
                if let Some(menu) = self
                    .preferences
                    .menus
                    .iter_mut()
                    .find(|menu| menu.id == *id)
                {
                    menu.expanded = true;
                    if matches!(menu.placement, Placement::Hidden | Placement::Menu) {
                        menu.placement = Placement::Floating;
                    }
                    if menu.placement == Placement::Floating {
                        menu.position = [
                            ctx.content_rect().left() + 24.,
                            ctx.content_rect().top() + 90.,
                        ];
                    }
                }
            }
            Destination::Workspace => self.layout_open = true,
            Destination::Keys => self.settings_open = true,
            Destination::Capacity => self.mesh_capacity_open = true,
            Destination::Viewport => {
                if let Some(kind) = &entry.navigation {
                    if !self
                        .preferences
                        .viewport_controls
                        .iter()
                        .any(|control| &control.kind == kind)
                    {
                        self.reveal_viewport_control(kind.clone());
                    }
                } else {
                    self.viewport_controls.editor_open = true;
                }
            }
            Destination::Recovery => self.recovery_open = true,
            Destination::Uv(texture) => {
                self.uv.open = true;
                self.uv.texture_tab = *texture;
            }
        }
        let dock = match &entry.destination {
            Destination::Panel(tool) => self
                .preferences
                .panels
                .iter()
                .find(|p| p.tool == *tool)
                .map(|p| p.dock),
            Destination::Menu(menu) => self
                .preferences
                .menu_sections
                .iter()
                .find(|s| s.menu == *menu)
                .map(|s| s.dock),
            Destination::Custom(id) => self
                .preferences
                .menus
                .iter()
                .find(|m| m.id == *id)
                .and_then(|m| match m.placement {
                    Placement::LeftPanel | Placement::Left => Some(Dock::Left),
                    Placement::RightPanel | Placement::Right => Some(Dock::Right),
                    _ => None,
                }),
            _ => None,
        };
        if matches!(dock, Some(Dock::Left | Dock::Right)) {
            self.narrow_panel = dock;
        }
        if let Some(reveal) = &mut self.search.reveal {
            reveal.shown = self.preferences.workspace("Shown by search".into());
        }
        let section = entry.section.clone();
        let expires = ctx.input(|i| i.time) + 12.;
        ctx.data_mut(|data| {
            data.insert_temp(
                locator_id(),
                Some(Locator {
                    key: entry.key.clone(),
                    destination: destination_key(&entry.destination),
                    section,
                    expires,
                    destination_scrolled: false,
                    control_scrolled: false,
                }),
            )
        });
    }
    /// Window position can change because egui clamps or rounds it. Only the
    /// actual window move response records an intentional placement change.
    pub(crate) fn search_placement_drag(&mut self, section: SidebarSection, dragged: bool) {
        if dragged
            && let Some(reveal) = &mut self.search.reveal
            && !reveal.moved.contains(&section)
        {
            reveal.moved.push(section);
        }
    }
    pub(crate) fn search_has_reveal(&self) -> bool {
        self.search.reveal.is_some()
    }
    /// Save preference changes made during a reveal, but keep the original
    /// workspace arrangement on disk until the artist explicitly keeps it.
    pub(crate) fn preferences_for_save(&self) -> Preferences {
        let mut preferences = self.preferences.clone();
        if let Some(reveal) = &self.search.reveal
            && preferences.active_workspace == reveal.active
        {
            restore_generated_layout(
                &reveal.workspace,
                &reveal.shown,
                &reveal.moved,
                &mut preferences,
            );
        }
        preferences.sync_active_workspace();
        preferences
    }
    pub(crate) fn finish_search_reveal(&mut self, ctx: &egui::Context, keep: bool) {
        if let Some(reveal) = self.search.reveal.take() {
            // Switching to another saved workspace is an explicit layout choice;
            // never restore a snapshot from the previous workspace over it.
            if !keep && self.preferences.active_workspace == reveal.active {
                restore_generated_layout(
                    &reveal.workspace,
                    &reveal.shown,
                    &reveal.moved,
                    &mut self.preferences,
                );
                self.set_zen_mode(reveal.zen);
                self.narrow_panel = reveal.narrow_panel;
                self.layout_open = reveal.dialogs.layout;
                self.settings_open = reveal.dialogs.keys;
                self.mesh_capacity_open = reveal.dialogs.capacity;
                self.viewport_controls.editor_open = reveal.dialogs.viewport;
                self.recovery_open = reveal.dialogs.recovery;
                self.uv.open = reveal.dialogs.uv;
            }
        }
        ctx.data_mut(|data| data.remove::<Option<Locator>>(locator_id()));
    }
    pub(crate) fn search_reveal_banner(&mut self, ctx: &egui::Context) {
        if self
            .search
            .reveal
            .as_ref()
            .is_some_and(|r| r.active != self.preferences.active_workspace)
        {
            self.finish_search_reveal(ctx, true);
        }
        let Some(label) = self.search.reveal.as_ref().map(|r| r.label.clone()) else {
            return;
        };
        let mut finish = None;
        egui::TopBottomPanel::top("search_reveal_notice").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Showing {label}"));
                if ui.button("Return to workspace").clicked() {
                    finish = Some(false);
                }
                if ui
                    .button("Keep layout")
                    .on_hover_text(
                        "Keep the revealed panels in this workspace and save their positions.",
                    )
                    .clicked()
                {
                    finish = Some(true);
                }
            });
        });
        if let Some(keep) = finish {
            self.finish_search_reveal(ctx, keep);
        }
    }
}

// Revert only values introduced by the reveal. Content, colours, shortcuts and
// controls explicitly changed by the artist remain intact. Moving a revealed
// panel yourself also counts as an intentional placement.
fn restore_generated_layout(
    before: &Workspace,
    shown: &Workspace,
    moved: &[SidebarSection],
    preferences: &mut Preferences,
) {
    for panel in &mut preferences.panels {
        if let (Some(old), Some(temp)) = (
            before.panels.iter().find(|p| p.tool == panel.tool),
            shown.panels.iter().find(|p| p.tool == panel.tool),
        ) {
            if old.dock != temp.dock
                && panel.dock == temp.dock
                && !moved.contains(&SidebarSection::Tool(panel.tool))
            {
                panel.dock = old.dock;
            }
            if old.open != temp.open
                && panel.open == temp.open
                && !moved.contains(&SidebarSection::Tool(panel.tool))
            {
                panel.open = old.open;
            }
            if old.position != temp.position && !moved.contains(&SidebarSection::Tool(panel.tool)) {
                panel.position = old.position;
            }
        }
    }
    for menu in &mut preferences.menus {
        if let (Some(old), Some(temp)) = (
            before.menus.iter().find(|m| m.id == menu.id),
            shown.menus.iter().find(|m| m.id == menu.id),
        ) {
            if old.placement != temp.placement
                && menu.placement == temp.placement
                && !moved.contains(&SidebarSection::Custom(menu.id))
            {
                menu.placement = old.placement;
            }
            if old.expanded != temp.expanded
                && menu.expanded == temp.expanded
                && !moved.contains(&SidebarSection::Custom(menu.id))
            {
                menu.expanded = old.expanded;
            }
            if old.position != temp.position && !moved.contains(&SidebarSection::Custom(menu.id)) {
                menu.position = old.position;
            }
        }
    }
    preferences.pinned_menus.retain_mut(|menu| {
        let old = before.pinned_menus.iter().find(|p| p.menu == menu.menu);
        let temp = shown.pinned_menus.iter().find(|p| p.menu == menu.menu);
        if let Some(temp) = temp {
            if let Some(old) = old {
                if old.position != temp.position
                    && !moved.contains(&SidebarSection::Builtin(menu.menu))
                {
                    menu.position = old.position;
                }
            } else if !moved.contains(&SidebarSection::Builtin(menu.menu)) {
                return false;
            }
        }
        true
    });
    for section in &mut preferences.menu_sections {
        if let (Some(old), Some(temp)) = (
            before.menu_sections.iter().find(|s| s.menu == section.menu),
            shown.menu_sections.iter().find(|s| s.menu == section.menu),
        ) && old.open != temp.open
            && section.open == temp.open
        {
            section.open = old.open;
        }
    }
}
impl UiState {
    pub(crate) fn search_revealed_project(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if self.search.reveal.as_ref().is_some_and(|r| r.project) {
            let mut open = true;
            egui::Window::new("Project")
                .id(egui::Id::new("search_project_menu"))
                .open(&mut open)
                .default_pos([24., 110.])
                .resizable(false)
                .show(ctx, |ui| self.project_menu_contents(ui, document, commands));
            if !open && let Some(reveal) = &mut self.search.reveal {
                reveal.project = false;
            }
        }
        if self.search.reveal.as_ref().is_some_and(|r| r.paint) {
            let mut open = true;
            egui::Window::new("Paint controls").id(egui::Id::new("search_paint_controls")).open(&mut open).default_pos([24., 110.]).default_width(340.).max_height((ctx.content_rect().height() - 140.).max(160.)).vscroll(true).show(ctx, |ui| {
                mark_destination(ui, "paint");
                if let Some(index) = self.paint_input_index(document) {
                    let mut brush = document.brushes()[index].clone();
                    ui.weak(format!("Paint input: {}. The active sculpting tool stays selected.", brush.name));
                    self.paint_controls(ui, &mut brush, document, commands);
                    if brush != document.brushes()[index] { commands.push(Command::UpdateBrush(index, brush)); }
                } else {
                    ui.label("Create a Paint brush in Brush library > Add brush to configure paint inputs.");
                }
            });
            if !open && let Some(reveal) = &mut self.search.reveal {
                reveal.paint = false;
            }
        }
    }
}

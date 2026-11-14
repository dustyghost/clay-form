//! Search resolves live action identities, never cached document indices.
use super::*;
use crate::preferences::{Placement, ViewportControlKind, Workspace};
mod catalog;
mod settings;
pub(crate) use settings::{requested_any_section, setting_response};
mod reveal;
pub(crate) use reveal::{highlight_response, is_requested, mark_destination, requested_section};
#[cfg(test)]
mod tests;

#[derive(Default)]
pub(crate) struct SearchState {
    pub open: bool,
    query: String,
    selected: Option<String>,
    focus: bool,
    reveal: Option<Reveal>,
}
struct Reveal {
    workspace: Workspace,
    shown: Workspace,
    moved: Vec<SidebarSection>,
    project: bool,
    paint: bool,
    active: Option<usize>,
    zen: bool,
    narrow_panel: Option<Dock>,
    label: String,
    dialogs: Dialogs,
}
#[derive(Clone, Copy)]
struct Dialogs {
    layout: bool,
    keys: bool,
    capacity: bool,
    viewport: bool,
    recovery: bool,
    uv: bool,
}
#[derive(Clone, Debug, PartialEq)]
enum Destination {
    Project,
    Paint,
    Panel(Tool),
    Menu(BuiltinMenu),
    Custom(u64),
    Workspace,
    Keys,
    Capacity,
    Viewport,
    Recovery,
    Uv(bool),
}
#[derive(Clone)]
struct Entry {
    key: String,
    name: String,
    kind: &'static str,
    description: String,
    aliases: String,
    location: String,
    destination: Destination,
    action: Option<Action>,
    navigation: Option<ViewportControlKind>,
    section: Option<String>,
}
impl UiState {
    pub(crate) fn open_search(&mut self) {
        // The visible Search entry can be used while Key settings is waiting for
        // a binding. Search input must never be captured by that window.
        self.cancel_shortcut_assignment();
        self.search.open = true;
        self.search.focus = true;
        self.search.selected = None;
        self.search.query.clear();
    }
    pub(crate) fn search_open(&self) -> bool {
        self.search.open
    }
    /// The modal is shown before the viewport so its pointer and keyboard events
    /// are claimed before a sculpt gesture or an ordinary shortcut can see them.
    pub(crate) fn search_dialog(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if !self.search.open {
            return;
        }
        let entries = self.search_entries(document);
        let mut requested = None;
        let mut close = false;
        egui::Modal::new(egui::Id::new("universal_search")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 48.).clamp(220., 620.));
            ui.horizontal(|ui| {
                ui.heading("Search Clay Form");
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.search.query)
                    .id_salt("universal_search_query")
                    .desired_width(f32::INFINITY)
                    .hint_text("Find a tool, setting, brush or model...")
                    .char_limit(160),
            );
            if self.search.focus {
                input.request_focus();
                self.search.focus = false;
            }
            if input.changed() { self.search.selected = None; }
            let matches = ranked(&entries, &self.search.query);
            let mut stale_selection = self.search.selected.as_ref().is_some_and(|key| !matches.iter().any(|entry| &entry.key == key));
            let mut selected = matches.iter().position(|entry| {
                self.search.selected.as_deref() == Some(entry.key.as_str())
            }).unwrap_or(0);
            let mut moved = false;
            if input.has_focus() {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                    selected = (selected + 1).min(matches.len().saturating_sub(1));
                    moved = true;
                }
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                    selected = selected.saturating_sub(1);
                    moved = true;
                }
            }
            if moved { stale_selection = false; }
            if !stale_selection { self.search.selected = matches.get(selected).map(|entry| entry.key.clone()); }
            ui.weak(format!("{} results · arrows to choose · Enter to run or reveal · Esc to close", matches.len()));
            let list_height = (ctx.content_rect().height() * 0.40).clamp(90., 340.);
            egui::ScrollArea::vertical().id_salt("search_results").max_height(list_height).show(ui, |ui| {
                for (index, entry) in matches.iter().enumerate() {
                    let shortcut = entry.action.as_ref().and_then(|action| self.preferences.shortcut(action)).map(|shortcut| shortcut.label()).unwrap_or_default();
                    let label = format!("{}  ·  {}{}", entry.name, entry.kind, if shortcut.is_empty() { String::new() } else { format!("  ·  {shortcut}") });
                    let response = ui.add_sized([ui.available_width(), 34.], egui::Button::new(label).selected(!stale_selection && index == selected).wrap());
                    if moved && index == selected {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.clicked() {
                        self.search.selected = Some(entry.key.clone());
                        selected = index;
                        stale_selection = false;
                    }
                    if response.double_clicked() {
                        requested = Some((entry.key.clone(), SearchRequest::Default));
                    }
                }
            });
            if stale_selection {
                ui.label("The selected result changed or was removed. Choose another result to continue.");
                ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            } else if let Some(entry) = matches.get(selected) {
                ui.separator();
                ui.strong(&entry.name);
                ui.label(&entry.description);
                ui.weak(format!("Location: {}", entry.location));
                let unavailable = entry.action.as_ref().and_then(|action| self.action_unavailable(action, document));
                if let Some(reason) = unavailable {
                    ui.label(format!("Unavailable: {reason}"));
                    if let Some(key) = prerequisite_key(reason)
                        && let Some(prerequisite) = entries.iter().find(|entry| entry.key == key)
                        && ui.button(format!("Show prerequisite: {}", prerequisite.name)).clicked()
                    {
                        requested = Some((prerequisite.key.clone(), SearchRequest::Reveal));
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if let Some(action) = &entry.action
                        && !super::brush_controls::is_control(action)
                        && ui.add_enabled(unavailable.is_none(), egui::Button::new("Run")).clicked()
                    {
                        requested = Some((entry.key.clone(), SearchRequest::Run));
                    }
                    if ui.button("Show in UI").clicked() {
                        requested = Some((entry.key.clone(), SearchRequest::Reveal));
                    }
                    if entry.action.is_some() {
                        if ui.button("Assign shortcut").clicked() {
                            requested = Some((entry.key.clone(), SearchRequest::Bind));
                        }
                        if ui.button("Add to menu / shelf").clicked() {
                            requested = Some((entry.key.clone(), SearchRequest::Shelf));
                        }
                    }
                    if (entry.action.is_some() || entry.navigation.is_some())
                        && ui.add_enabled(self.preferences.viewport_controls.len() < 128, egui::Button::new("Add to viewport")).clicked()
                    {
                        requested = Some((entry.key.clone(), SearchRequest::Viewport));
                    }
                });
                if (input.has_focus() || input.lost_focus()) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                    requested = Some((entry.key.clone(), SearchRequest::Default));
                }
            } else {
                ui.label("No matches. Try a tool name, part of a name, or words such as cut, roll or save.");
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                close = true;
            }
        });
        if let Some((key, request)) = requested {
            // Rebuild from the current scene before acting. No stored result can
            // select a deleted brush or a previous project's object by index.
            if let Some(entry) = self
                .search_entries(document)
                .into_iter()
                .find(|entry| entry.key == key)
            {
                self.search.open = false;
                match request {
                    SearchRequest::Default | SearchRequest::Run => {
                        if let Some(action) = &entry.action
                            && !super::brush_controls::is_control(action)
                        {
                            if self.action_enabled(action, document) {
                                commands.push(Command::Action(action.clone()));
                            } else {
                                self.search.open = true;
                            }
                        } else {
                            self.reveal_search_entry(ctx, &entry);
                        }
                    }
                    SearchRequest::Reveal => self.reveal_search_entry(ctx, &entry),
                    SearchRequest::Bind => {
                        if let Some(action) = entry.action {
                            self.finish_search_reveal(ctx, false);
                            self.begin_shortcut_assignment(action, document, ctx);
                        }
                    }
                    SearchRequest::Shelf => {
                        if let Some(action) = entry.action {
                            self.finish_search_reveal(ctx, false);
                            self.begin_add_to_menu(action, document);
                        }
                    }
                    SearchRequest::Viewport => {
                        self.finish_search_reveal(ctx, false);
                        if let Some(kind) = entry
                            .navigation
                            .or_else(|| entry.action.map(ViewportControlKind::Action))
                        {
                            self.add_viewport_control(kind);
                        }
                    }
                }
            }
        }
        if close {
            self.search.open = false;
        }
    }
}
#[derive(Clone, Copy)]
enum SearchRequest {
    Default,
    Run,
    Reveal,
    Bind,
    Shelf,
    Viewport,
}

fn normalise(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn ranked<'a>(entries: &'a [Entry], query: &str) -> Vec<&'a Entry> {
    let query = normalise(query);
    let mut results: Vec<_> = entries
        .iter()
        .filter_map(|entry| score(entry, &query).map(|score| (score, entry)))
        .collect();
    results.sort_by(|(sa, a), (sb, b)| {
        sb.cmp(sa)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.key.cmp(&b.key))
    });
    results.into_iter().map(|(_, entry)| entry).collect()
}
fn score(entry: &Entry, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(if entry.action.is_some() { 10 } else { 0 });
    }
    let name = normalise(&entry.name);
    if name == query {
        return Some(1000);
    }
    let aliases = normalise(&entry.aliases);
    // A deliberate synonym such as "cut" for Slice should beat incidental
    // substrings such as "shortcuts" and longer names such as "Cut one cell".
    if aliases.split_whitespace().any(|alias| alias == query) {
        return Some(if entry.action.is_some() || entry.navigation.is_some() {
            975
        } else {
            950
        });
    }
    if name.starts_with(query) {
        return Some(900);
    }
    if name.contains(query) {
        return Some(800);
    }
    let text = format!(
        "{name} {aliases} {} {}",
        normalise(entry.kind),
        normalise(&entry.location)
    );
    let mut result = 0;
    for token in query.split_whitespace() {
        if name.split_whitespace().any(|word| word.starts_with(token)) {
            result += 70;
        } else if aliases
            .split_whitespace()
            .any(|word| word.starts_with(token))
        {
            result += 60;
        } else if text.contains(token) {
            result += 30;
        } else if token.chars().count() >= 4
            && text.split_whitespace().any(|word| one_typo(token, word))
        {
            result += 10;
        } else {
            return None;
        }
    }
    Some(result)
}
/// One insertion, removal, substitution or adjacent transposition, bounded by
/// the short query rather than a quadratic allocation for every catalogue row.
fn one_typo(a: &str, b: &str) -> bool {
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let first = a
        .iter()
        .zip(&b)
        .position(|(a, b)| a != b)
        .unwrap_or(a.len().min(b.len()));
    if first == a.len().min(b.len()) {
        return true;
    }
    if a.len() == b.len() {
        a[first + 1..] == b[first + 1..]
            || (first + 1 < a.len()
                && a[first] == b[first + 1]
                && a[first + 1] == b[first]
                && a[first + 2..] == b[first + 2..])
    } else if a.len() > b.len() {
        a[first + 1..] == b[first..]
    } else {
        a[first..] == b[first + 1..]
    }
}

/// Routes correspond to exact shared-policy prerequisites and existing live
/// catalogue identities. Unknown reasons keep their explanation without a
/// speculative navigation target.
fn prerequisite_key(reason: &str) -> Option<&'static str> {
    Some(match reason {
        "Generate current UVs in UV / textures before painting the image" => "setting:UV islands",
        "Choose an assigned paint image in Textures first" => "setting:Texture library",
        "Select a Paint brush before filling material"
        | "This brush is no longer in the project. Choose a brush from the Brushes panel" => {
            "action:ShowPanel(Library)"
        }
        "This custom menu no longer exists. Open Workspace to create or restore one" => {
            "setting:Custom menus and shelves"
        }
        "This modelling preset no longer exists. Choose a current preset" => {
            "action:ModelSavePreset"
        }
        "This Mask preset no longer exists. Choose a Mask brush in Mask / visibility" => {
            "menu:Mask"
        }
        "Load an alpha and enable its stencil first" => "setting:Alpha library",
        "Remove an unused reference image before adding another"
        | "Add a reference image first" => "menu:References",
        "Switch to Dynamic mode first" => "setting:Dynamic mesh resolution",
        "Switch to Subdivision mode before adding a level"
        | "Switch to the highest subdivision level before subdividing"
        | "Use a mesh with a single subdivision level before editing component groups" => {
            "setting:Subdivision levels"
        }
        "Enable Modeling and choose a tool first"
        | "Enable Modeling before saving its settings as a preset"
        | "Choose an editing tool in Modeling before applying an amount"
        | "Select a face in the viewport first"
        | "Select an edge in the viewport first"
        | "Select a vertex in the viewport first"
        | "Select an edge loop in the viewport first"
        | "Select components in the viewport first"
        | "Switch to Edge mode and select a loop first"
        | "Switch to Face mode and select faces first"
        | "Choose a selection group in Modeling first" => "action:PenModeling",
        "Select a model with a surface in Submodels first"
        | "Choose a selection group on the active model first"
        | "Show the active submodel before editing its edges."
        | "Convert the primitive in Submodels before editing its edges."
        | "Convert the primitive in Submodels to a sculpting mesh first"
        | "Convert the primitive to a sculpting mesh first."
        | "Show the active model in Submodels first"
        | "Show the active model and all of its faces first."
        | "Remove an unused submodel to free a model slot first"
        | "Choose a model with another model below it in Submodels"
        | "Select a primitive with a surface to convert"
        | "Enable the Thickness modifier on the active model first"
        | "Apply or disable the Thickness modifier in Submodels to edit the base mesh."
        | "Apply or remove thickness modifier first."
        | "Apply or remove the target's Thickness modifier before editing its surface"
        | "Choose Edit source for this array, or Make editable before editing its surface" => {
            "action:ShowPanel(Submodels)"
        }
        "Enable a transform tool before resetting its pivot" => "action:ShowPanel(Transform)",
        "Choose Closed path in Mask / visibility first" => "menu:Visibility",
        "Choose the Mask line tool first" => "menu:Mask",
        "Show all faces before remeshing"
        | "Show all faces before smooth subdivision."
        | "Hide some faces, leaving some visible, before splitting the model" => "menu:Visibility",
        _ => return None,
    })
}

use super::*;

#[test]
fn aliases_typos_and_exact_names_find_live_controls() {
    let ui = UiState::default();
    let document = Document::default();
    let entries = ui.search_entries(&document);
    assert_eq!(ranked(&entries, "cut")[0].action, Some(Action::Slice));
    assert_eq!(
        ranked(&entries, "roll view")[0].navigation,
        Some(ViewportControlKind::Roll)
    );
    assert!(
        ranked(&entries, "slcie")
            .iter()
            .any(|e| e.action == Some(Action::Slice))
    );
    assert_eq!(
        ranked(&entries, "Mesh capacity")[0].destination,
        Destination::Capacity
    );
    assert!(ranked(&entries, "nonesuchxyz").is_empty());
}
#[test]
fn catalogue_refreshes_removed_brushes_and_workspace_names() {
    let mut ui = UiState::default();
    let document = Document::default();
    ui.preferences.save_workspace_as("My sculpt desk").unwrap();
    assert!(
        ui.search_entries(&document)
            .iter()
            .any(|e| e.name == "My sculpt desk")
    );
    ui.preferences.delete_workspace(0);
    assert!(
        !ui.search_entries(&document)
            .iter()
            .any(|e| e.name == "My sculpt desk")
    );
    let action = Action::SelectBrush("deleted-brush".into());
    ui.preferences.bindings.push((action.clone(), None));
    let stale = ui
        .search_entries(&document)
        .into_iter()
        .find(|e| e.action.as_ref() == Some(&action))
        .unwrap();
    assert!(!ui.action_enabled(stale.action.as_ref().unwrap(), &document));
}
#[test]
fn reveal_restores_hidden_panel_zen_and_persisted_arrangement() {
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    ui.preferences
        .panels
        .iter_mut()
        .find(|p| p.tool == Tool::Mesh)
        .unwrap()
        .dock = Dock::Hidden;
    ui.preferences.save_workspace_as("Sculpt desk").unwrap();
    ui.set_zen_mode(true);
    let original = ui.preferences.clone();
    let entry = ui
        .search_entries(&Document::default())
        .into_iter()
        .find(|e| e.action == Some(Action::ShowPanel(Tool::Mesh)))
        .unwrap();
    ui.reveal_search_entry(&ctx, &entry);
    assert!(!ui.zen_mode);
    assert_eq!(
        ui.preferences
            .panels
            .iter()
            .find(|p| p.tool == Tool::Mesh)
            .unwrap()
            .dock,
        Dock::Floating
    );
    assert_eq!(ui.preferences_for_save(), original);
    ui.preferences
        .bind(
            Action::Save,
            Shortcut::new(egui::Key::S, egui::Modifiers::ALT),
        )
        .unwrap();
    assert!(
        ui.preferences_for_save()
            .shortcut(&Action::Save)
            .unwrap()
            .alt
    );
    ui.finish_search_reveal(&ctx, false);
    assert!(ui.zen_mode);
    assert_eq!(ui.preferences.panels, original.panels);
    assert!(ui.preferences.shortcut(&Action::Save).unwrap().alt);
}
#[test]
fn keeping_reveal_or_switching_workspace_does_not_restore_old_layout() {
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    ui.preferences.save_workspace_as("First").unwrap();
    ui.preferences.save_workspace_as("Second").unwrap();
    let entry = ui
        .search_entries(&Document::default())
        .into_iter()
        .find(|e| e.name == "Slice options")
        .unwrap();
    ui.reveal_search_entry(&ctx, &entry);
    assert!(
        ui.preferences
            .pinned_menus
            .iter()
            .any(|p| p.menu == BuiltinMenu::Slice)
    );
    ui.finish_search_reveal(&ctx, true);
    assert!(
        ui.preferences
            .pinned_menus
            .iter()
            .any(|p| p.menu == BuiltinMenu::Slice)
    );
    ui.reveal_search_entry(&ctx, &entry);
    ui.preferences.switch_workspace(0);
    let chosen = ui.preferences.clone();
    ui.finish_search_reveal(&ctx, false);
    assert_eq!(ui.preferences, chosen);
}
#[test]
fn search_keyboard_closes_without_leaking_escape() {
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    ui.open_search();
    let mut commands = Vec::new();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        ui.search_dialog(ctx, &Document::default(), &mut commands)
    });
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(input, |ctx| {
        ui.search_dialog(ctx, &Document::default(), &mut commands);
        assert!(!ctx.input(|i| i.key_pressed(egui::Key::Escape)));
    });
    assert!(!ui.search_open());
    assert!(commands.is_empty());
}

#[test]
fn reveal_uses_visible_custom_shelf_when_standard_panel_is_hidden() {
    let mut ui = UiState::default();
    ui.preferences
        .panels
        .iter_mut()
        .find(|p| p.tool == Tool::Brush)
        .unwrap()
        .dock = Dock::Hidden;
    let id = ui.preferences.add_menu();
    let menu = ui
        .preferences
        .menus
        .iter_mut()
        .find(|m| m.id == id)
        .unwrap();
    menu.name = "Favourite controls".into();
    menu.placement = Placement::Top;
    menu.items.push(crate::preferences::CustomItem::action(
        Action::BrushStrength,
    ));
    // Ignore the default brush strip so the custom destination is unambiguous.
    for menu in &mut ui.preferences.menus {
        if menu.id != id {
            menu.placement = Placement::Hidden;
        }
    }
    let entry = ui
        .search_entries(&Document::default())
        .into_iter()
        .find(|entry| entry.action == Some(Action::BrushStrength))
        .unwrap();
    assert_eq!(entry.destination, Destination::Custom(id));
    assert!(entry.location.contains("Favourite controls"));
    ui.reveal_search_entry(&egui::Context::default(), &entry);
    assert_eq!(
        ui.preferences
            .panels
            .iter()
            .find(|p| p.tool == Tool::Brush)
            .unwrap()
            .dock,
        Dock::Hidden
    );
}

#[test]
fn reveal_keeps_explicit_content_and_appearance_changes() {
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    let entry = ui
        .search_entries(&Document::default())
        .into_iter()
        .find(|e| e.name == "Slice options")
        .unwrap();
    ui.reveal_search_entry(&ctx, &entry);
    ui.preferences.appearance = crate::preferences::Appearance::warm();
    let id = ui.preferences.add_menu();
    let appearance = ui.preferences.appearance.clone();
    let saved = ui.preferences_for_save();
    assert_eq!(saved.appearance, appearance);
    assert!(saved.menus.iter().any(|m| m.id == id));
    assert!(
        !saved
            .pinned_menus
            .iter()
            .any(|m| m.menu == BuiltinMenu::Slice)
    );
    ui.finish_search_reveal(&ctx, false);
    assert_eq!(ui.preferences.appearance, appearance);
    assert!(ui.preferences.menus.iter().any(|m| m.id == id));
}

#[test]
fn named_path_reveal_preserves_tool_and_selection() {
    use crate::sculpt::{
        Vec3,
        paths::{PathMode, PathPoint},
    };
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    let mut document = Document::default();
    document
        .create_path(
            "Chin contour".into(),
            PathMode::Free,
            vec![
                PathPoint {
                    position: Vec3(0., 0., 0.),
                    normal: Vec3(0., 1., 0.),
                },
                PathPoint {
                    position: Vec3(1., 0., 0.),
                    normal: Vec3(0., 1., 0.),
                },
            ],
            false,
            0.,
        )
        .unwrap();
    let original = document.project();
    let selected = ui.selected;
    let entry = ui
        .search_entries(&document)
        .into_iter()
        .find(|e| e.name == "Chin contour")
        .unwrap();
    ui.reveal_search_entry(&ctx, &entry);
    assert_eq!(entry.section.as_deref(), Some("saved_paths"));
    assert!(!ui.paths_active);
    assert_eq!(ui.selected, selected);
    assert!(document.project() == original);
}

#[test]
fn configured_shortcut_labels_are_searchable() {
    let mut ui = UiState::default();
    ui.preferences
        .bind(
            Action::Save,
            Shortcut::new(egui::Key::J, egui::Modifiers::CTRL),
        )
        .unwrap();
    let entries = ui.search_entries(&Document::default());
    assert!(
        ranked(&entries, "Ctrl J")
            .iter()
            .any(|e| e.action == Some(Action::Save))
    );
}

#[test]
fn removed_selected_result_does_not_execute_a_different_result() {
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    ui.open_search();
    let mut commands = Vec::new();
    let document = Document::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        ui.search_dialog(ctx, &document, &mut commands)
    });
    ui.search.selected = Some("workspace:0:Removed desk".into());
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(input, |ctx| ui.search_dialog(ctx, &document, &mut commands));
    assert!(commands.is_empty());
    assert!(ui.search_open());
    assert_eq!(
        ui.search.selected.as_deref(),
        Some("workspace:0:Removed desk")
    );
}

#[test]
fn prerequisite_routes_resolve_shared_policy_reasons_to_live_controls() {
    let ui = UiState::default();
    let document = Document::default();
    let entries = ui.search_entries(&document);
    for action in [Action::Remesh, Action::ModelApply, Action::FillMaterial] {
        let reason = ui
            .action_unavailable(&action, &document)
            .expect("requires preparation");
        let key = prerequisite_key(reason).expect("concrete prerequisite route");
        assert!(
            entries.iter().any(|entry| entry.key == key),
            "missing route for {reason}"
        );
    }
    assert!(prerequisite_key("Nothing to undo").is_none());
}

#[test]
fn enter_runs_the_selected_result_after_query_field_surrenders_focus() {
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    let document = Document::default();
    let mut commands = Vec::new();
    ui.open_search();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        ui.search_dialog(ctx, &document, &mut commands)
    });
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::Text("zen mode".into()));
    let _ = ctx.run(input, |ctx| ui.search_dialog(ctx, &document, &mut commands));
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(input, |ctx| {
        ui.search_dialog(ctx, &document, &mut commands);
        assert!(!ctx.input(|i| i.key_pressed(egui::Key::Enter)));
    });
    assert!(!ui.search_open());
    assert!(matches!(
        commands.as_slice(),
        [Command::Action(Action::ToggleZen)]
    ));
}

fn shortcut_frame(
    ui: &mut UiState,
    ctx: &egui::Context,
    document: &Document,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1200., 900.),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            ui.search_dialog(ctx, document, &mut Vec::new());
            // Match the application order: an assignment requested by Search
            // reaches the key editor in the same frame.
            ui.settings(ctx, document);
        },
    )
}

fn shortcut_key(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn opening_search_cancels_an_existing_shortcut_capture_before_typing() {
    let ctx = egui::Context::default();
    let mut ui = UiState {
        settings_open: true,
        capturing: Some(Action::ToggleZen),
        ..Default::default()
    };
    let document = Document::default();
    let bindings = ui.preferences.bindings.clone();

    // This is the same opener used by the visible Search entry, which remains
    // available while shortcut capture intentionally suppresses global keys.
    ui.open_search();
    assert!(ui.capturing.is_none());
    shortcut_frame(&mut ui, &ctx, &document, vec![]);
    shortcut_frame(
        &mut ui,
        &ctx,
        &document,
        vec![
            shortcut_key(egui::Key::C, true),
            egui::Event::Text("capacity".into()),
        ],
    );
    assert!(ui.search_open());
    assert_eq!(ui.search.query, "capacity");
    assert!(ui.capturing.is_none());
    assert_eq!(ui.preferences.bindings, bindings);
}

#[test]
fn keyboard_assignment_does_not_bind_its_activation_key() {
    for activation in [egui::Key::Enter, egui::Key::Space] {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let mut ui = UiState::default();
        let document = Document::default();
        let original = ui.preferences.shortcut(&Action::ToggleZen).cloned();
        ui.open_search();
        ui.search.query = "zen mode".into();
        shortcut_frame(&mut ui, &ctx, &document, vec![]);
        shortcut_frame(&mut ui, &ctx, &document, vec![]);

        // Reach the actual button through keyboard navigation, without relying
        // on generated egui IDs or the number of controls before it.
        let mut assignment_focused = false;
        for _ in 0..16 {
            shortcut_frame(
                &mut ui,
                &ctx,
                &document,
                vec![shortcut_key(egui::Key::Tab, true)],
            );
            let output = shortcut_frame(
                &mut ui,
                &ctx,
                &document,
                vec![shortcut_key(egui::Key::Tab, false)],
            );
            let button = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Assign shortcut" => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            });
            assignment_focused = ctx
                .memory(|memory| memory.focused())
                .and_then(|id| ctx.read_response(id))
                .zip(button)
                .is_some_and(|(response, point)| response.rect.contains(point));
            if assignment_focused {
                break;
            }
        }
        assert!(assignment_focused, "Assign shortcut is keyboard reachable");

        shortcut_frame(
            &mut ui,
            &ctx,
            &document,
            vec![shortcut_key(activation, true)],
        );
        assert!(!ui.search_open());
        assert!(ui.settings_open);
        assert_eq!(ui.capturing, Some(Action::ToggleZen));
        assert_eq!(
            ui.preferences.shortcut(&Action::ToggleZen),
            original.as_ref()
        );

        shortcut_frame(
            &mut ui,
            &ctx,
            &document,
            vec![shortcut_key(activation, false)],
        );
        // The same key remains a valid intentional shortcut on a new press.
        shortcut_frame(
            &mut ui,
            &ctx,
            &document,
            vec![shortcut_key(activation, true)],
        );
        assert!(ui.capturing.is_none());
        assert_eq!(
            ui.preferences.shortcut(&Action::ToggleZen),
            Some(&crate::preferences::Shortcut::new(
                activation,
                egui::Modifiers::NONE
            ))
        );
    }
}

#[test]
fn temporary_pinned_menu_is_not_persisted_after_automatic_window_clamping() {
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    let document = Document::default();
    let entry = ui
        .search_entries(&document)
        .into_iter()
        .find(|e| e.name == "Slice options")
        .unwrap();
    ui.reveal_search_entry(&ctx, &entry);
    // Model a viewport resize between reveal and the next layout pass. Egui must
    // move the generated window into bounds without interpreting it as a drag.
    let outside = [680., 640.];
    ui.preferences
        .pinned_menus
        .iter_mut()
        .find(|p| p.menu == BuiltinMenu::Slice)
        .unwrap()
        .position = outside;
    ui.search
        .reveal
        .as_mut()
        .unwrap()
        .shown
        .pinned_menus
        .iter_mut()
        .find(|p| p.menu == BuiltinMenu::Slice)
        .unwrap()
        .position = outside;
    for _ in 0..3 {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(720., 680.),
                )),
                ..Default::default()
            },
            |ctx| {
                ui.pinned_menus(ctx, &document, &Camera::default(), &mut Vec::new());
            },
        );
    }
    assert_ne!(
        ui.preferences
            .pinned_menus
            .iter()
            .find(|p| p.menu == BuiltinMenu::Slice)
            .unwrap()
            .position,
        outside
    );
    assert!(
        !ui.preferences_for_save()
            .pinned_menus
            .iter()
            .any(|p| p.menu == BuiltinMenu::Slice)
    );
    ui.finish_search_reveal(&ctx, false);
    assert!(
        !ui.preferences
            .pinned_menus
            .iter()
            .any(|p| p.menu == BuiltinMenu::Slice)
    );
}

#[test]
fn explicit_window_drag_keeps_a_newly_revealed_menu() {
    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    let entry = ui
        .search_entries(&Document::default())
        .into_iter()
        .find(|e| e.name == "Slice options")
        .unwrap();
    ui.reveal_search_entry(&ctx, &entry);
    ui.search_placement_drag(SidebarSection::Builtin(BuiltinMenu::Slice), true);
    ui.preferences
        .pinned_menus
        .iter_mut()
        .find(|p| p.menu == BuiltinMenu::Slice)
        .unwrap()
        .position = [80., 120.];
    assert!(
        ui.preferences_for_save()
            .pinned_menus
            .iter()
            .any(|p| p.menu == BuiltinMenu::Slice)
    );
    ui.finish_search_reveal(&ctx, false);
    assert_eq!(
        ui.preferences
            .pinned_menus
            .iter()
            .find(|p| p.menu == BuiltinMenu::Slice)
            .unwrap()
            .position,
        [80., 120.]
    );
}

#[test]
fn direct_model_reference_and_asset_prerequisites_have_live_destinations() {
    let entries = UiState::default().search_entries(&Document::default());
    for reason in [
        "Convert the primitive in Submodels to a sculpting mesh first",
        "Show the active model in Submodels first",
        "Remove an unused submodel to free a model slot first",
        "Apply or remove the target's Thickness modifier before editing its surface",
        "Choose Edit source for this array, or Make editable before editing its surface",
        "Add a reference image first",
        "Remove an unused reference image before adding another",
        "Load an alpha and enable its stencil first",
    ] {
        let key = prerequisite_key(reason).expect("supported prerequisite");
        assert!(
            entries.iter().any(|e| e.key == key),
            "missing target for {reason}"
        );
    }
}

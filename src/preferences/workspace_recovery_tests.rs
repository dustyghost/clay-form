use super::*;

#[test]
fn default_layout_preserves_named_arrangement_and_shared_bindings() {
    let mut prefs = Preferences::default();
    prefs.save_workspace_as("My sculpting").unwrap();
    prefs.panels[0].dock = Dock::Hidden;
    prefs.viewport_controls.clear();
    prefs.appearance = Appearance::warm();
    prefs
        .bind(Action::Save, Shortcut::new(Key::F8, Modifiers::NONE))
        .unwrap();
    let bindings = prefs.bindings.clone();
    let previous = prefs.workspace("My sculpting".into());

    assert_eq!(prefs.start_default_workspace(), "My sculpting");
    assert_eq!(prefs.bindings, bindings);
    assert_eq!(prefs.workspaces, vec![previous.clone()]);
    assert_eq!(prefs.active_workspace, None);
    assert!(!prefs.viewport_controls.is_empty());
    prefs.switch_workspace(0);
    assert_eq!(prefs.workspace("My sculpting".into()), previous);
    assert_eq!(prefs.bindings, bindings);
}

#[test]
fn unnamed_layout_reset_keeps_a_restart_safe_recovery_workspace() {
    let mut prefs = Preferences::default();
    let menu_id = prefs.add_menu();
    prefs.menus.last_mut().unwrap().name = "Favourite brushes".into();
    prefs
        .menus
        .last_mut()
        .unwrap()
        .items
        .push(CustomItem::action(Action::ClearMask));
    prefs
        .bind(
            Action::OpenMenu(menu_id),
            Shortcut::new(Key::F9, Modifiers::NONE),
        )
        .unwrap();
    let previous = prefs.workspace("Before layout reset".into());
    let bindings = prefs.bindings.clone();
    let name = prefs.start_default_workspace();
    assert_eq!(name, "Before layout reset");
    assert_eq!(prefs.bindings, bindings);

    let mut restored: Preferences =
        serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
    restored.validate().unwrap();
    restored.switch_workspace(0);
    assert_eq!(restored.workspace(name), previous);
    assert_eq!(restored.bindings, bindings);
    restored.active_workspace = None;
    assert_eq!(restored.start_default_workspace(), "Before layout reset 2");
    assert_eq!(restored.workspaces.len(), 2);
}

#[test]
fn restoring_hidden_panels_keeps_saved_workspaces_and_contents() {
    let mut prefs = Preferences::default();
    prefs.panels[0].dock = Dock::Hidden;
    prefs.menus[0].placement = Placement::Hidden;
    let contents = prefs.menus[0].items.clone();
    prefs.save_workspace_as("Hidden controls").unwrap();
    let saved = prefs.workspaces.clone();
    assert_eq!(prefs.restore_hidden_panels(), 2);
    assert_eq!(prefs.panels[0].dock, Dock::Left);
    assert!(prefs.panels[0].open);
    assert_eq!(prefs.menus[0].placement, Placement::LeftPanel);
    assert_eq!(prefs.menus[0].items, contents);
    assert_eq!(prefs.workspaces, saved);
    assert_eq!(prefs.restore_hidden_panels(), 0);
}

#[test]
fn floating_recovery_handles_all_window_types_without_moving_visible_windows() {
    let mut prefs = Preferences::default();
    prefs.panels[0].dock = Dock::Floating;
    prefs.panels[0].position = [1800., -200.];
    prefs.menus[0].placement = Placement::Floating;
    prefs.menus[0].position = [-400., 2000.];
    prefs.pinned_menus.push(PinnedMenu {
        menu: BuiltinMenu::Mask,
        position: [3000., 900.],
    });
    prefs.panels[1].dock = Dock::Floating;
    prefs.panels[1].position = [100., 100.];
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    assert_eq!(prefs.recover_floating_panels(bounds), 3);
    for position in [
        prefs.panels[0].position,
        prefs.menus[0].position,
        prefs.pinned_menus[0].position,
    ] {
        assert!((16. ..=560.).contains(&position[0]));
        assert!((16. ..=440.).contains(&position[1]));
    }
    assert_eq!(prefs.panels[1].position, [100., 100.]);
    assert_eq!(prefs.recover_floating_panels(bounds), 0);
}

#[test]
fn fresh_workspace_collapses_advanced_panels_without_changing_saved_values() {
    let prefs = Preferences::default();
    for panel in &prefs.panels {
        assert_eq!(
            panel.open,
            matches!(panel.tool, Tool::Library | Tool::Brush | Tool::Submodels)
        );
    }
    let mut old = serde_json::to_value(prefs).unwrap();
    for panel in old["panels"].as_array_mut().unwrap() {
        panel["open"] = true.into();
    }
    let restored: Preferences = serde_json::from_value(old).unwrap();
    assert!(restored.panels.iter().all(|p| p.open));
}

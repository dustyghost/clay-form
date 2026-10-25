use super::*;

#[test]
fn previous_brush_menu_gains_inner_radius_and_preserves_custom_placement() {
    for placement in [Placement::Top, Placement::LeftPanel, Placement::Floating] {
        let mut prefs = Preferences::default();
        prefs.brush_controls_version = 1;
        prefs.menus[0].placement = placement;
        prefs.menus[0].columns = 6;
        prefs.menus[0].collapsible = false;
        prefs.menus[0]
            .items
            .retain(|i| i.action != Some(Action::BrushInnerRadius));
        prefs.save_workspace_as("My workspace").unwrap();
        let path = std::env::temp_dir().join(format!(
            "clay-brush-controls-v2-{}-{placement:?}.json",
            std::process::id()
        ));
        save(&path, &prefs).unwrap();
        let migrated = load(&path).unwrap();
        assert_eq!(
            migrated.menus[0].placement,
            if placement == Placement::Top {
                Placement::LeftPanel
            } else {
                placement
            }
        );
        assert_eq!(
            migrated.menus[0].items[2].action,
            Some(Action::BrushInnerRadius)
        );
        assert_eq!(migrated.workspaces[0].menus[0], migrated.menus[0]);
        save(&path, &migrated).unwrap();
        assert_eq!(load(&path).unwrap(), migrated);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn brush_controls_migrate_once_preserving_workspaces_and_shortcuts() {
    for occupied in [false, true] {
        let mut prefs = Preferences::default();
        prefs.menus.clear();
        prefs
            .bindings
            .retain(|(a, _)| !matches!(a, Action::OpenMenu(_)));
        prefs.next_menu_id = 1;
        let custom_id = prefs.add_menu();
        prefs.menus[0].name = "My tools".into();
        prefs.menus[0]
            .items
            .push(CustomItem::action(Action::Wireframe));
        prefs.save_workspace_as("Sculpt").unwrap();
        if occupied {
            prefs
                .bind(Action::Wireframe, Shortcut::new(Key::S, Modifiers::NONE))
                .unwrap();
        }
        let custom = prefs.menus[0].clone();
        let mut old = serde_json::to_value(&prefs).unwrap();
        old.as_object_mut()
            .unwrap()
            .remove("brush_controls_version");
        let path = std::env::temp_dir().join(format!(
            "clay-brush-controls-{}-{occupied}.json",
            std::process::id()
        ));
        crate::persistence::write_json(&path.to_string_lossy(), &old).unwrap();
        let mut migrated = load(&path).unwrap();
        assert_eq!(migrated.menus.len(), 2);
        assert_eq!(migrated.menus[0], custom);
        let brush = migrated.menus[1].clone();
        assert_ne!(brush.id, custom_id);
        assert_eq!(brush.placement, Placement::LeftPanel);
        assert_eq!(migrated.workspaces[0].menus[1], brush);
        assert_eq!(
            migrated.shortcut(&Action::OpenMenu(brush.id)).is_some(),
            !occupied
        );
        assert_eq!(
            migrated.shortcut(&Action::Save),
            Some(&Shortcut::new(Key::S, Modifiers::CTRL))
        );
        if occupied {
            assert_eq!(
                migrated.shortcut(&Action::Wireframe),
                Some(&Shortcut::new(Key::S, Modifiers::NONE))
            );
        }
        migrated.menus[1].placement = Placement::Hidden;
        migrated.menus[1].items.remove(0);
        save(&path, &migrated).unwrap();
        assert_eq!(load(&path).unwrap(), migrated);
        let mut reset = migrated.clone();
        reset.reset_key_bindings();
        assert_eq!(
            reset.shortcut(&Action::OpenMenu(brush.id)),
            Some(&Shortcut::new(Key::S, Modifiers::NONE))
        );
        reset.validate().unwrap();
        migrated.menus.remove(1);
        save(&path, &migrated).unwrap();
        assert_eq!(
            load(&path).unwrap(),
            migrated,
            "a removed default is not recreated"
        );
        std::fs::remove_file(path).unwrap();
    }
}

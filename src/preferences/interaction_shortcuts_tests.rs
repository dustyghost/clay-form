use super::*;

fn legacy() -> Preferences {
    let mut prefs = Preferences {
        interaction_shortcuts_version: 0,
        ..Default::default()
    };
    for (action, binding) in &mut prefs.bindings {
        match action {
            Action::SwapPaintColours => *binding = Some(Shortcut::new(Key::Z, Modifiers::NONE)),
            Action::MaskVisibilityMenu => *binding = Some(Shortcut::new(Key::X, Modifiers::NONE)),
            _ => {}
        }
    }
    prefs
}
fn reload(prefs: &Preferences, suffix: &str) -> Preferences {
    let path = std::env::temp_dir().join(format!(
        "clay-interaction-{}-{suffix}.json",
        std::process::id()
    ));
    save(&path, prefs).unwrap();
    let result = load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    result
}
#[test]
fn interaction_shortcuts_migrate_stock_pair_once_and_reset_to_new_defaults() {
    let mut old = legacy();
    old.selection_mode = SelectionMode::Slice;
    let mut migrated = reload(&old, "stock");
    assert_eq!(
        migrated.shortcut(&Action::SwapPaintColours),
        Some(&Shortcut::new(Key::X, Modifiers::NONE))
    );
    assert_eq!(
        migrated.shortcut(&Action::MaskVisibilityMenu),
        Some(&Shortcut::new(Key::Z, Modifiers::NONE))
    );
    assert_eq!(migrated.selection_mode, SelectionMode::Slice);
    assert_eq!(migrated.interaction_shortcuts_version, 1);
    assert_eq!(
        migrated.shortcut(&Action::Slice),
        None,
        "Ctrl+V must remain untouched"
    );
    migrated.clear_binding(&Action::SwapPaintColours);
    migrated.clear_binding(&Action::MaskVisibilityMenu);
    migrated
        .bind(
            Action::SwapPaintColours,
            Shortcut::new(Key::Z, Modifiers::NONE),
        )
        .unwrap();
    migrated
        .bind(
            Action::MaskVisibilityMenu,
            Shortcut::new(Key::X, Modifiers::NONE),
        )
        .unwrap();
    let mut saved = reload(&migrated, "once");
    assert_eq!(
        saved.bindings, migrated.bindings,
        "deliberately restored old keys must survive"
    );
    saved.reset_key_bindings();
    assert_eq!(saved.bindings, Preferences::default().bindings);
}
#[test]
fn interaction_shortcuts_preserve_custom_unbound_and_conflicting_choices() {
    for case in 0..4 {
        let mut old = legacy();
        if case < 2 {
            old.clear_binding(&Action::SwapPaintColours);
            if case == 1 {
                old.bind(Action::Wireframe, Shortcut::new(Key::Z, Modifiers::NONE))
                    .unwrap();
            }
        } else {
            old.clear_binding(&Action::MaskVisibilityMenu);
            if case == 3 {
                old.bind(
                    Action::MaskVisibilityMenu,
                    Shortcut::new(Key::M, Modifiers::ALT),
                )
                .unwrap();
            }
        }
        let migrated = reload(&old, &format!("custom-{case}"));
        assert_eq!(migrated.bindings, old.bindings);
        assert_eq!(migrated.interaction_shortcuts_version, 1);
        assert_eq!(
            reload(&migrated, &format!("again-{case}")).bindings,
            migrated.bindings
        );
    }
}

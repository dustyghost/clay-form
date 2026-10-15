use super::*;

fn frame(
    state: &mut UiState,
    ctx: &egui::Context,
    doc: &Document,
    events: Vec<egui::Event>,
) -> (egui::FullOutput, Vec<Command>) {
    let mut commands = Vec::new();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400., 1000.),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            commands.extend(state.toolbar(ctx, doc, &crate::camera::Camera::default()));
            commands.extend(state.custom_surfaces(ctx, doc));
            commands.extend(state.brush_panel(
                ctx,
                doc,
                "Test",
                1.,
                &crate::camera::Camera::default(),
            ));
            egui::CentralPanel::default().show(ctx, |ui| {
                // Same response hook used by the ordinary toolbar and tool panels.
                ui.button("Source mask button").customize(Action::ClearMask);
                if state.modeling.active {
                    state.modeling_controls(ui, doc, &mut commands);
                }
            });
            state.settings(ctx, doc);
        },
    );
    (output, commands)
}
fn text_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing UI text: {label}"))
}
fn click(
    state: &mut UiState,
    ctx: &egui::Context,
    doc: &Document,
    pos: egui::Pos2,
    button: egui::PointerButton,
) -> (egui::FullOutput, Vec<Command>) {
    frame(
        state,
        ctx,
        doc,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        state,
        ctx,
        doc,
        vec![egui::Event::PointerButton {
            pos,
            button,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    )
}
#[test]
fn source_context_menu_builds_a_shelf_and_captures_shift_number() {
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.);
    let doc = Document::default();
    let mut state = UiState::default();
    frame(&mut state, &ctx, &doc, vec![]);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let source = text_center(&output, "Source mask button");
    click(
        &mut state,
        &ctx,
        &doc,
        source,
        egui::PointerButton::Secondary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let add = text_center(&output, "Add to custom menu / shelf…");
    click(&mut state, &ctx, &doc, add, egui::PointerButton::Primary);
    assert!(state.layout_open);
    assert_eq!(state.customization.chosen, Some(Action::ClearMask));
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let new = text_center(&output, "+ New menu / shelf");
    click(&mut state, &ctx, &doc, new, egui::PointerButton::Primary);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let add = text_center(&output, "Add selected: Clear mask");
    click(&mut state, &ctx, &doc, add, egui::PointerButton::Primary);
    assert_eq!(
        state.preferences.menus[0].items[0].action,
        Some(Action::ClearMask)
    );
    state.layout_open = false;
    state.preferences.menus[0].placement = Placement::Top;
    frame(&mut state, &ctx, &doc, vec![]);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let custom = text_center(&output, "Clear mask");
    let (_, commands) = click(&mut state, &ctx, &doc, custom, egui::PointerButton::Primary);
    assert!(matches!(
        commands.as_slice(),
        [Command::Action(Action::ClearMask)]
    ));
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let source = text_center(&output, "Source mask button");
    click(
        &mut state,
        &ctx,
        &doc,
        source,
        egui::PointerButton::Secondary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let assign = text_center(&output, "Assign shortcut…");
    click(&mut state, &ctx, &doc, assign, egui::PointerButton::Primary);
    assert_eq!(state.capturing, Some(Action::ClearMask));
    frame(
        &mut state,
        &ctx,
        &doc,
        vec![
            egui::Event::Key {
                key: egui::Key::Exclamationmark,
                physical_key: Some(egui::Key::Num1),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::SHIFT,
            },
            egui::Event::Text("!".into()),
        ],
    );
    assert_eq!(
        state.preferences.shortcut(&Action::ClearMask),
        Some(&Shortcut::new(egui::Key::Num1, egui::Modifiers::SHIFT))
    );
    assert!(state.capturing.is_none());
    assert!(
        state.customization.key_search.ends_with("mask"),
        "captured punctuation must not enter the search box"
    );
    assert!(!doc.is_dirty());
}

#[test]
fn either_sidebar_can_create_and_collapse_custom_menus() {
    for (dock, placement, x) in [
        (Dock::Left, Placement::LeftPanel, 60.),
        (Dock::Right, Placement::RightPanel, 1330.),
    ] {
        let ctx = egui::Context::default();
        ctx.style_mut(|s| s.animation_time = 0.);
        let doc = Document::default();
        let mut state = UiState::default();
        // Keep the panel contents short so the test can target its empty background.
        for panel in &mut state.preferences.panels {
            panel.open = false;
        }
        assert!(state.preferences.panels.iter().any(|p| p.dock == dock));
        frame(&mut state, &ctx, &doc, vec![]);
        frame(&mut state, &ctx, &doc, vec![]);
        click(
            &mut state,
            &ctx,
            &doc,
            egui::pos2(x, 750.),
            egui::PointerButton::Secondary,
        );
        let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
        let add = text_center(&output, "New collapsible menu here…");
        click(&mut state, &ctx, &doc, add, egui::PointerButton::Primary);
        assert!(state.layout_open);
        assert_eq!(state.preferences.menus.len(), 1);
        assert_eq!(state.preferences.menus[0].placement, placement);
        assert!(state.preferences.menus[0].collapsible);
        state.preferences.menus[0].name = "My sidebar menu".into();
        state.preferences.menus[0]
            .items
            .push(CustomItem::action(Action::ClearMask));
        state.layout_open = false;
        frame(&mut state, &ctx, &doc, vec![]);
        let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
        let button = text_center(&output, "Clear mask");
        let (_, commands) = click(&mut state, &ctx, &doc, button, egui::PointerButton::Primary);
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, Command::Action(Action::ClearMask)))
        );
        let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
        let header = text_center(&output, "My sidebar menu");
        click(&mut state, &ctx, &doc, header, egui::PointerButton::Primary);
        assert!(!state.preferences.menus[0].expanded);
        frame(&mut state, &ctx, &doc, vec![]);
        let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
        assert!(
            !output.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Clear mask")
            )
        );
        let header = text_center(&output, "My sidebar menu");
        click(
            &mut state,
            &ctx,
            &doc,
            header,
            egui::PointerButton::Secondary,
        );
        let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
        let edit = text_center(&output, "Edit this menu…");
        click(&mut state, &ctx, &doc, edit, egui::PointerButton::Primary);
        assert!(state.layout_open);
        assert_eq!(
            state.customization.selected_menu,
            Some(state.preferences.menus[0].id)
        );
    }
}

#[test]
fn toolbar_switches_workspace_and_applies_colours() {
    let ctx = egui::Context::default();
    let doc = Document::default();
    let mut state = UiState::default();
    state.preferences.save_workspace_as("Sculpt").unwrap();
    state.preferences.save_workspace_as("Light tools").unwrap();
    state.preferences.appearance = Appearance::light();
    state.preferences.switch_workspace(0);
    frame(&mut state, &ctx, &doc, vec![]);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let selector = text_center(&output, "Workspace: Sculpt");
    click(
        &mut state,
        &ctx,
        &doc,
        selector,
        egui::PointerButton::Primary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Light tools"),
        egui::PointerButton::Primary,
    );
    assert_eq!(state.preferences.active_workspace, Some(1));
    assert_eq!(
        ctx.style().visuals.panel_fill,
        egui::Color32::from_rgb(228, 231, 235)
    );
    assert_eq!(
        crate::preferences::viewport_background(&ctx),
        egui::Color32::from_rgb(190, 197, 205)
    );
}

#[test]
fn custom_item_and_menu_removal_preserve_other_workspaces_and_shortcuts() {
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.);
    let doc = Document::default();
    let mut state = UiState::default();
    let id = state.preferences.add_menu();
    state.preferences.menus[0].name = "Removable tools".into();
    state.preferences.menus[0].collapsible = false;
    state.preferences.menus[0].placement = Placement::LeftPanel;
    state.preferences.menus[0].items = vec![
        CustomItem::action(Action::ClearMask),
        CustomItem::action(Action::InvertMask),
    ];
    state
        .preferences
        .bind(
            Action::OpenMenu(id),
            Shortcut::new(egui::Key::Num1, egui::Modifiers::SHIFT),
        )
        .unwrap();
    state
        .preferences
        .save_workspace_as("Keep original")
        .unwrap();
    state.preferences.save_workspace_as("Modify copy").unwrap();
    for panel in &mut state.preferences.panels {
        panel.open = false;
    }
    frame(&mut state, &ctx, &doc, vec![]);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Clear mask"),
        egui::PointerButton::Secondary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let (_, commands) = click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Remove item from this menu"),
        egui::PointerButton::Primary,
    );
    assert!(commands.is_empty());
    assert_eq!(state.preferences.menus[0].items.len(), 1);
    assert_eq!(
        state.preferences.menus[0].items[0].action,
        Some(Action::InvertMask)
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Removable tools"),
        egui::PointerButton::Secondary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Delete this custom menu"),
        egui::PointerButton::Primary,
    );
    assert!(state.preferences.menus.is_empty());
    assert!(state.preferences.shortcut(&Action::OpenMenu(id)).is_some());
    state.activate_workspace(&ctx, 0);
    assert_eq!(state.preferences.menus[0].items.len(), 2);
    state.activate_workspace(&ctx, 1);
    assert!(state.preferences.menus.is_empty());
}

#[test]
fn unavailable_custom_buttons_can_still_be_removed() {
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.);
    let doc = Document::default();
    let mut state = UiState::default();
    state.preferences.add_menu();
    state.preferences.menus[0].placement = Placement::LeftPanel;
    state.preferences.menus[0].items.push(CustomItem {
        action: Some(Action::Undo),
        label: "Unavailable undo".into(),
    });
    assert!(!state.action_enabled(&Action::Undo, &doc));
    frame(&mut state, &ctx, &doc, vec![]);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let undo = text_center(&output, "Unavailable undo");
    let (_, commands) = click(&mut state, &ctx, &doc, undo, egui::PointerButton::Primary);
    assert!(commands.is_empty());
    click(&mut state, &ctx, &doc, undo, egui::PointerButton::Secondary);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Remove item from this menu"),
        egui::PointerButton::Primary,
    );
    assert!(state.preferences.menus[0].items.is_empty());
}

fn sidebar_text(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.text() == label && text.pos.x < 300. && text.pos.y > 100. =>
            {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .next()
        .unwrap_or_else(|| panic!("Missing sidebar text: {label}"))
}

#[test]
fn whole_menu_converts_collapses_reorders_with_tools_and_returns_to_toolbar() {
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.);
    let doc = Document::default();
    let mut state = UiState::default();
    for panel in &mut state.preferences.panels {
        panel.open = false;
    }
    frame(&mut state, &ctx, &doc, vec![]);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Mask"),
        egui::PointerButton::Primary,
    );
    frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        ctx.data(|d| {
            d.get_temp::<egui::Rect>(egui::Id::new(("icon_button", "Make collapsible on left")))
        })
        .unwrap()
        .center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(state.preferences.menu_sections[0].menu, BuiltinMenu::Mask);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let (_, commands) = click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Invert mask"),
        egui::PointerButton::Primary,
    );
    assert!(commands.iter().any(|c| matches!(c, Command::InvertMask)));
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    // The slider and its numeric editor are retained, not just action buttons.
    text_center(&output, "Mask intensity");
    click(
        &mut state,
        &ctx,
        &doc,
        sidebar_text(&output, "Mask"),
        egui::PointerButton::Primary,
    );
    assert!(!state.preferences.menu_sections[0].open);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        sidebar_text(&output, "Mask"),
        egui::PointerButton::Secondary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Move section down"),
        egui::PointerButton::Primary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    assert!(sidebar_text(&output, "Brush library").y < sidebar_text(&output, "Mask").y);
    assert_eq!(
        state.preferences.sidebar_sections(Dock::Left)[1],
        SidebarSection::Builtin(BuiltinMenu::Mask)
    );
    click(
        &mut state,
        &ctx,
        &doc,
        sidebar_text(&output, "Mask"),
        egui::PointerButton::Secondary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Return to toolbar menu"),
        egui::PointerButton::Primary,
    );
    assert!(state.preferences.menu_sections.is_empty());
    state.preferences.validate().unwrap();
}

#[test]
fn custom_menu_converts_as_one_section_and_moves_past_builtin_tools() {
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.);
    let doc = Document::default();
    let mut state = UiState::default();
    for panel in &mut state.preferences.panels {
        panel.open = false;
    }
    let id = state.preferences.add_menu();
    state.preferences.menus[0].name = "My whole menu".into();
    state.preferences.menus[0].items = vec![CustomItem::action(Action::InvertMask)];
    frame(&mut state, &ctx, &doc, vec![]);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "My whole menu"),
        egui::PointerButton::Primary,
    );
    frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        ctx.data(|d| {
            d.get_temp::<egui::Rect>(egui::Id::new(("icon_button", "Make collapsible on left")))
        })
        .unwrap()
        .center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(state.preferences.menus[0].placement, Placement::LeftPanel);
    assert!(state.preferences.menus[0].collapsible);
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    let (_, commands) = click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Invert mask"),
        egui::PointerButton::Primary,
    );
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::Action(Action::InvertMask)))
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        sidebar_text(&output, "My whole menu"),
        egui::PointerButton::Secondary,
    );
    let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
    click(
        &mut state,
        &ctx,
        &doc,
        text_center(&output, "Move section down"),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        state.preferences.sidebar_sections(Dock::Left)[1],
        SidebarSection::Custom(id)
    );
    state.preferences.validate().unwrap();
}

#[test]
fn typing_in_brush_settings_keeps_the_preset_that_received_focus() {
    for temporary in [false, true] {
        let mut state = UiState::default();
        let doc = Document::default();
        let ctx = egui::Context::default();
        let render = |state: &mut UiState, modifiers, events| {
            let mut commands = Vec::new();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500., 1000.),
                    )),
                    modifiers,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        state.tool_controls(Tool::Brush, ui, &doc, "Test", 1., &mut commands);
                    });
                },
            );
            (output, commands)
        };
        let held = if temporary {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        };
        let (output, _) = render(&mut state, held, vec![]);
        let name = if temporary { "Smooth" } else { "Standard" };
        let pos = text_center(&output, name);
        render(
            &mut state,
            held,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: held,
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: held,
                },
            ],
        );
        assert!(ctx.wants_keyboard_input());
        let changed = if temporary {
            egui::Modifiers::NONE
        } else {
            egui::Modifiers::SHIFT
        };
        let (_, commands) = render(&mut state, changed, vec![egui::Event::Text("X".into())]);
        let expected = doc.brushes().iter().position(|b| b.name == name).unwrap();
        assert!(commands.iter().any(|command| matches!(command, Command::UpdateBrush(index, brush) if *index == expected && brush.name.contains('X'))));
        assert_eq!(state.selected, 0);
    }
}

#[test]
fn modeling_buttons_offer_menu_and_shortcut_customization() {
    use crate::viewport::{ComponentMode as Mode, ModelTool as Tool};
    for (label, mode, tool, action) in [
        (
            "Individual",
            Mode::Face,
            Tool::Select,
            Action::ModelSelectSingle,
        ),
        ("Loop", Mode::Face, Tool::Select, Action::ModelSelectLoops),
        (
            "Parallel edges",
            Mode::Edge,
            Tool::Select,
            Action::ModelSelectRing,
        ),
        (
            "Dissolve selected loops",
            Mode::Edge,
            Tool::Select,
            Action::ModelDissolveLoops,
        ),
        ("Face", Mode::Face, Tool::Select, Action::ModelFaces),
        ("Edge", Mode::Face, Tool::Select, Action::ModelEdges),
        ("Vertex", Mode::Face, Tool::Select, Action::ModelVertices),
        ("Extrude", Mode::Face, Tool::Select, Action::ModelExtrude),
        ("Loop cut", Mode::Edge, Tool::Select, Action::ModelLoopCut),
        ("Select", Mode::Face, Tool::Extrude, Action::ModelSelect),
        ("Add", Mode::Face, Tool::Select, Action::ModelAdd),
        ("Remove", Mode::Face, Tool::Select, Action::ModelRemove),
        ("Replace", Mode::Face, Tool::Select, Action::ModelReplace),
        (
            "Join matching faces",
            Mode::Face,
            Tool::Extrude,
            Action::ModelJoin,
        ),
        ("+", Mode::Edge, Tool::LoopCut, Action::ModelMoreLoops),
        ("−", Mode::Edge, Tool::LoopCut, Action::ModelFewerLoops),
        (
            "Clear selection",
            Mode::Face,
            Tool::Select,
            Action::ModelClearSelection,
        ),
        (
            "Select loop",
            Mode::Edge,
            Tool::Select,
            Action::ModelSelectLoop,
        ),
        (
            "Create group from selection",
            Mode::Face,
            Tool::Select,
            Action::ModelCreateGroup,
        ),
        (
            "Add to group",
            Mode::Face,
            Tool::Select,
            Action::ModelAddToGroup,
        ),
        (
            "Remove from group",
            Mode::Face,
            Tool::Select,
            Action::ModelRemoveFromGroup,
        ),
    ] {
        let ctx = egui::Context::default();
        ctx.style_mut(|s| s.animation_time = 0.);
        let mut doc = Document::default();
        doc.group_model_selection(&[0], None, "Test", None, false)
            .unwrap();
        let mut state = UiState::default();
        state.modeling.active = true;
        state.modeling.mode = mode;
        state.modeling.tool = tool;
        state.modeling.count = 1;
        state.modeling.loop_count = 2;
        frame(&mut state, &ctx, &doc, vec![]);
        let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
        // This harness renders the controls in both a dock and the central panel.
        // Use the central copy; its expanded content is not clipped by dock scrolling.
        let central_text = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .rev()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(t) if t.galley.text() == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing central UI text: {label}"))
        };
        let group = central_text(&output, "Selection to vertex group");
        click(&mut state, &ctx, &doc, group, egui::PointerButton::Primary);
        let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
        let pos = central_text(&output, label);
        let (_, commands) = click(&mut state, &ctx, &doc, pos, egui::PointerButton::Secondary);
        assert!(commands.is_empty(), "Right click must not execute {label}");
        let (output, _) = frame(&mut state, &ctx, &doc, vec![]);
        text_center(&output, "Assign shortcut…");
        let add = text_center(&output, "Add to custom menu / shelf…");
        click(&mut state, &ctx, &doc, add, egui::PointerButton::Primary);
        assert_eq!(state.customization.chosen, Some(action.clone()), "{label}");
        assert!(Action::ALL.contains(&action));
        assert_eq!(
            serde_json::from_str::<Action>(&serde_json::to_string(&action).unwrap()).unwrap(),
            action
        );
    }
}

//! Opt-in visual QA of the customization windows through the actual egui renderer.
use super::ui_capture::UiCapture;
use crate::{
    document::Document,
    preferences::{Action, Appearance, CustomItem, Placement, Shortcut},
    ui::UiState,
};
use eframe::egui;

#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn custom_interface_offscreen_preview() {
    {
        let size = [1440, 1000];
        let mut capture = UiCapture::new(size);
        let mut rgba = Vec::new();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let document = Document::default();
        let mut ui = UiState::default();
        ui.preferences.compact_toolbar = true;
        ui.preferences.add_menu();
        ui.preferences.menus[0].name = "Sculpt essentials".into();
        ui.preferences.menus[0].items = [
            Action::ClearMask,
            Action::InvertMask,
            Action::Subdivide,
            Action::Wireframe,
            Action::SelectBrush(document.brushes()[0].identity()),
        ]
        .into_iter()
        .map(CustomItem::action)
        .collect();
        ui.preferences.add_menu();
        ui.preferences.menus[1].name = "Quick tools".into();
        ui.preferences.menus[1].placement = Placement::Top;
        ui.preferences.menus[1].columns = 5;
        ui.preferences.menus[1].items = ui.preferences.menus[0].items.clone();
        ui.preferences
            .bind(
                Action::Wireframe,
                Shortcut::new(egui::Key::Num1, egui::Modifiers::SHIFT),
            )
            .unwrap();
        for (placement, name) in [
            (Placement::LeftPanel, "My sculpt tools"),
            (Placement::RightPanel, "My mesh tools"),
        ] {
            ui.preferences.add_menu();
            let menu = ui.preferences.menus.last_mut().unwrap();
            menu.name = name.into();
            menu.placement = placement;
            menu.columns = 1;
            menu.items = vec![
                CustomItem::action(Action::ClearMask),
                CustomItem::action(Action::Wireframe),
            ];
        }
        ui.preferences.save_workspace_as("Sculpting").unwrap();
        for name in [
            "layout",
            "keys",
            "dark",
            "light",
            "warm",
            "pinned",
            "collapsible",
        ] {
            ui.layout_open = name == "layout";
            ui.settings_open = name == "keys";
            if name == "light" {
                ui.preferences.appearance = Appearance::light();
            }
            if name == "warm" {
                ui.preferences.appearance = Appearance::warm();
            }
            if name == "pinned" {
                use crate::preferences::{BuiltinMenu, PinnedMenu};
                ui.preferences.appearance = Appearance::default();
                ui.preferences.pinned_menus = vec![
                    PinnedMenu {
                        menu: BuiltinMenu::Mask,
                        position: [340., 180.],
                    },
                    PinnedMenu {
                        menu: BuiltinMenu::Symmetry,
                        position: [780., 180.],
                    },
                ];
            }
            if name == "collapsible" {
                use crate::preferences::{BuiltinMenu, Dock, SidebarSection, Tool};
                ui.preferences.dock_menu(BuiltinMenu::Mask, Dock::Left);
                ui.preferences.dock_menu(BuiltinMenu::Symmetry, Dock::Right);
                for panel in &mut ui.preferences.panels {
                    panel.open = false;
                }
                ui.preferences
                    .move_section(SidebarSection::Tool(Tool::Library), false);
                ui.preferences
                    .move_section(SidebarSection::Tool(Tool::Library), false);
            }
            for _ in 0..4 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(size[0] as f32, size[1] as f32),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        ui.toolbar(ctx, &document, &crate::camera::Camera::default());
                        ui.custom_surfaces(ctx, &document);
                        ui.brush_panel(
                            ctx,
                            &document,
                            "Tablet",
                            1.,
                            &crate::camera::Camera::default(),
                        );
                        ui.settings(ctx, &document);
                        egui::CentralPanel::default().show(ctx, |panel| {
                            panel.painter().rect_filled(
                                panel.max_rect(),
                                0.,
                                crate::preferences::viewport_background(ctx),
                            );
                        });
                    },
                );
                rgba = capture.render(&ctx, output);
            }
            if let Ok(path) = std::env::var("CLAY_CUSTOM_UI_PREVIEW") {
                capture.save(format!("{path}-{name}.png"), &rgba);
            }
        }
    }
}

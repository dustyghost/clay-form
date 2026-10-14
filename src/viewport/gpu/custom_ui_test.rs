//! Opt-in visual QA of the customization windows through the actual egui renderer.
use crate::{
    document::Document,
    preferences::{Action, Appearance, CustomItem, Placement, Shortcut},
    ui::UiState,
};
use eframe::{
    egui,
    glow::{self, HasContext},
};
use std::sync::Arc;

#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn custom_interface_offscreen_preview() {
    unsafe {
        let egl = super::tests::Egl::new();
        let gl = Arc::new(glow::Context::from_loader_function(|name| {
            (egl.get_proc)(std::ffi::CString::new(name).unwrap().as_ptr())
        }));
        let size = [1440, 1000];
        let texture = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            size[0],
            size[1],
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        let framebuffer = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        assert_eq!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER),
            glow::FRAMEBUFFER_COMPLETE
        );
        let mut painter = eframe::egui_glow::Painter::new(gl.clone(), "", None, false).unwrap();
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
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
                gl.clear_color(0.07, 0.07, 0.07, 1.);
                gl.clear(glow::COLOR_BUFFER_BIT);
                let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
                painter.paint_and_update_textures(
                    [size[0] as u32, size[1] as u32],
                    output.pixels_per_point,
                    &shapes,
                    &output.textures_delta,
                );
            }
            assert_eq!(gl.get_error(), glow::NO_ERROR);
            if let Ok(path) = std::env::var("CLAY_CUSTOM_UI_PREVIEW") {
                let mut rgba = vec![0; (size[0] * size[1] * 4) as usize];
                gl.read_pixels(
                    0,
                    0,
                    size[0],
                    size[1],
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(&mut rgba)),
                );
                let flipped: Vec<_> = rgba
                    .chunks_exact(size[0] as usize * 4)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image::save_buffer(
                    format!("{path}-{name}.png"),
                    &flipped,
                    size[0] as u32,
                    size[1] as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
        painter.destroy();
        gl.delete_framebuffer(framebuffer);
        gl.delete_texture(texture);
    }
}

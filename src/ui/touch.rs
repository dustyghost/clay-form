use super::*;
use crate::preferences::{TouchDrag, TouchScroll, TouchSettings};

impl UiState {
    pub(super) fn touch_settings(&mut self, ui: &mut egui::Ui, document: &Document) {
        egui::CollapsingHeader::new("Touch and trackpad gestures")
            .id_salt("touch_settings")
            .show(ui, |ui| {
                let catalog: Vec<_> = self.action_catalog(document).into_iter()
                    .map(|action| { let label = self.action_label(&action, document); (action, label) })
                    .collect();
                let settings = &mut self.preferences.touch;
                ui.checkbox(&mut settings.enabled, "Enable touch gestures");
                ui.weak("Fingers navigate; pen and mouse sculpt. Changes save automatically.");
                ui.add_enabled_ui(settings.enabled, |ui| {
                    for (label, drag) in [("One-finger drag", &mut settings.one_finger), ("Two-finger drag", &mut settings.two_finger)] {
                        egui::ComboBox::from_label(label).selected_text(drag.label()).show_ui(ui, |ui| {
                            for value in TouchDrag::ALL { ui.selectable_value(drag, value, value.label()); }
                        });
                    }
                    ui.checkbox(&mut settings.pinch, "Pinch to zoom");
                    ui.checkbox(&mut settings.reverse_pinch, "Reverse pinch direction");
                    ui.checkbox(&mut settings.twist, "Two-finger twist to roll view");
                    ui.checkbox(&mut settings.reverse_twist, "Reverse twist direction");
                    for (label, action) in [("Two-finger tap", &mut settings.two_tap), ("Three-finger tap", &mut settings.three_tap)] {
                        let selected = action.as_ref().map_or("Disabled", |action|
                            catalog.iter().find(|(a, _)| a == action).map_or("Unavailable action", |(_, label)| label.as_str()));
                        egui::ComboBox::from_label(label).width(210.).selected_text(selected).show_ui(ui, |ui| {
                            ui.selectable_value(action, None, "Disabled");
                            for (value, label) in &catalog { ui.selectable_value(action, Some(value.clone()), label); }
                        });
                    }
                    ui.add(egui::Slider::new(&mut settings.drag_speed, 0.1..=4.).text("Drag sensitivity"));
                    ui.add(egui::Slider::new(&mut settings.pinch_speed, 0.1..=4.).text("Pinch sensitivity"));
                    ui.add(egui::Slider::new(&mut settings.twist_speed, 0.1..=4.).text("Twist sensitivity"));
                    ui.add(egui::Slider::new(&mut settings.tap_seconds, 0.1..=0.8).suffix(" s").text("Maximum tap time"));
                    ui.add(egui::Slider::new(&mut settings.tap_slop, 2. ..=30.).suffix(" px").text("Tap movement tolerance"));
                    ui.separator();
                    egui::ComboBox::from_label("Trackpad scrolling").selected_text(settings.scroll.label()).show_ui(ui, |ui| {
                        for value in TouchScroll::ALL { ui.selectable_value(&mut settings.scroll, value, value.label()); }
                    });
                    ui.weak("If your trackpad scrolls in wheel steps, choose Pan all scrolling. This also changes the mouse wheel.");
                    ui.checkbox(&mut settings.button_taps, "Use right / middle clicks as two / three-finger taps");
                    ui.weak("For trackpads that send taps as mouse clicks. Also maps short mouse right / middle clicks; drags still navigate.");
                    ui.weak("Native trackpad pinch depends on OS support. You can map pinch to Ctrl+wheel and taps to Undo / Redo shortcuts in your desktop gesture settings.");
                });
                if ui.button("Restore default touch gestures").clicked() {
                    *settings = TouchSettings::default();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        state: &mut UiState,
        document: &Document,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(620., 760.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    state.touch_settings(ui, document);
                });
            },
        )
    }
    fn center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing {label}"))
    }
    fn click(state: &mut UiState, document: &Document, ctx: &egui::Context, pos: egui::Pos2) {
        for pressed in [true, false] {
            frame(
                state,
                document,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    fn open(state: &mut UiState, document: &Document, ctx: &egui::Context) {
        ctx.style_mut(|s| s.animation_time = 0.);
        let output = frame(state, document, ctx, vec![]);
        click(
            state,
            document,
            ctx,
            center(&output, "Touch and trackpad gestures"),
        );
    }

    #[test]
    fn touch_settings_enable_and_reset_with_real_controls() {
        let mut state = UiState::default();
        let document = Document::default();
        let ctx = egui::Context::default();
        open(&mut state, &document, &ctx);
        let output = frame(&mut state, &document, &ctx, vec![]);
        click(
            &mut state,
            &document,
            &ctx,
            center(&output, "Enable touch gestures"),
        );
        assert!(!state.preferences.touch.enabled);
        let output = frame(&mut state, &document, &ctx, vec![]);
        click(
            &mut state,
            &document,
            &ctx,
            center(&output, "Restore default touch gestures"),
        );
        assert_eq!(state.preferences.touch, TouchSettings::default());
    }

    #[test]
    #[cfg(target_os = "linux")]
    #[ignore = "manual touch settings visual review; requires Vulkan"]
    fn touch_settings_offscreen_preview() {
        let mut state = UiState::default();
        let document = Document::default();
        let ctx = egui::Context::default();
        let mut capture = crate::viewport::UiCapture::new([620, 760]);
        ctx.style_mut(|s| s.animation_time = 0.);
        let output = frame(&mut state, &document, &ctx, vec![]);
        let pos = center(&output, "Touch and trackpad gestures");
        // Render setup frames too: they upload the font atlas and white texture.
        capture.render(&ctx, output);
        for pressed in [true, false] {
            let output = frame(
                &mut state,
                &document,
                &ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            capture.render(&ctx, output);
        }
        let mut rgba = Vec::new();
        for _ in 0..3 {
            let output = frame(&mut state, &document, &ctx, vec![]);
            rgba = capture.render(&ctx, output);
        }
        capture.save("/tmp/clay-touch-settings.png", &rgba);
    }
}

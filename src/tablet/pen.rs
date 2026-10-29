//! Recover a pen-owned pointer independently of egui-winit's first-finger pointer.
use eframe::egui::{self, Event, PointerButton, Pos2, TouchPhase};
use std::collections::BTreeSet;

#[derive(Default)]
pub(crate) struct PenInput {
    pen: Option<u64>,
    position: Pos2,
    fingers: BTreeSet<u64>,
    suppressed: BTreeSet<u64>,
}

pub(crate) struct Routed {
    pub pen_events: Vec<Event>,
    pub reset_gestures: bool,
}

impl PenInput {
    pub fn route(&mut self, input: &mut egui::RawInput, is_pen: impl Fn(u64) -> bool) -> Routed {
        let events = std::mem::take(&mut input.events);
        // Include the release frame and quick down/up strokes in the arbitration.
        let pen_frame = self.pen.is_some()
            || events
                .iter()
                .any(|e| matches!(e, Event::Touch { id, .. } if is_pen(id.0)));
        let mut reset_gestures = pen_frame && self.pen.is_none();
        if pen_frame {
            self.suppressed.extend(&self.fingers);
        }
        let owns_pointer = pen_frame || !self.suppressed.is_empty();
        let mut pen_events = Vec::new();
        for event in events {
            match event {
                Event::Touch { id, phase, pos, .. } if is_pen(id.0) => {
                    pen_events.push(event.clone());
                    match phase {
                        TouchPhase::Start if self.pen.is_none() => {
                            // A finger may already own egui's emulated primary button.
                            // Release it before giving the pen its own down event.
                            if !self.fingers.is_empty() {
                                input.events.push(button(pos, false, input.modifiers));
                            }
                            self.pen = Some(id.0);
                            self.position = pos;
                            input.events.push(Event::PointerMoved(pos));
                            input.events.push(button(pos, true, input.modifiers));
                        }
                        TouchPhase::Move if self.pen.is_none() || self.pen == Some(id.0) => {
                            self.position = pos;
                            input.events.push(Event::PointerMoved(pos));
                        }
                        TouchPhase::End | TouchPhase::Cancel if self.pen == Some(id.0) => {
                            self.position = pos;
                            input.events.push(Event::PointerMoved(pos));
                            input.events.push(button(pos, false, input.modifiers));
                            input.events.push(Event::PointerGone);
                            self.pen = None;
                        }
                        _ => {}
                    }
                }
                Event::Touch { id, phase, .. } => {
                    let suppress = pen_frame || !self.suppressed.is_empty();
                    match phase {
                        TouchPhase::Start => {
                            self.fingers.insert(id.0);
                            if suppress {
                                self.suppressed.insert(id.0);
                            }
                        }
                        TouchPhase::End | TouchPhase::Cancel => {
                            self.fingers.remove(&id.0);
                            self.suppressed.remove(&id.0);
                        }
                        _ => {}
                    }
                    if !suppress {
                        input.events.push(event);
                    }
                }
                Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone
                    if owns_pointer => {}
                Event::WindowFocused(false) => {
                    reset_gestures = true;
                    self.cancel(input);
                    pen_events.push(event.clone());
                    input.events.push(event);
                }
                _ => input.events.push(event),
            }
        }
        if !input.focused {
            self.cancel(input);
            pen_events.push(Event::WindowFocused(false));
        }
        Routed {
            pen_events,
            reset_gestures: reset_gestures || !input.focused,
        }
    }

    fn cancel(&mut self, input: &mut egui::RawInput) {
        if self.pen.take().is_some() {
            input
                .events
                .push(button(self.position, false, input.modifiers));
            input.events.push(Event::PointerGone);
        }
        self.fingers.clear();
        self.suppressed.clear();
    }
}

fn button(pos: Pos2, pressed: bool, modifiers: egui::Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn touch(id: u64, phase: TouchPhase) -> Event {
        Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(id),
            phase,
            pos: egui::pos2(id as f32, 20.),
            // Both finger and pen deliberately have pressure.
            force: Some(0.4),
        }
    }
    fn run(router: &mut PenInput, events: Vec<Event>) -> (egui::RawInput, Routed) {
        let mut input = egui::RawInput {
            events,
            ..Default::default()
        };
        let output = router.route(&mut input, |id| id == 2);
        (input, output)
    }
    #[test]
    fn pen_bypasses_gestures_and_preserves_pressure() {
        let mut router = PenInput::default();
        let (input, routed) = run(&mut router, vec![touch(2, TouchPhase::Start)]);
        assert!(routed.reset_gestures);
        assert!(matches!(
            routed.pen_events[0],
            Event::Touch {
                force: Some(0.4),
                ..
            }
        ));
        assert!(
            !input
                .events
                .iter()
                .any(|e| matches!(e, Event::Touch { .. }))
        );
        assert!(
            input
                .events
                .iter()
                .any(|e| matches!(e, Event::PointerButton { pressed: true, .. }))
        );
        let (input, _) = run(&mut router, vec![touch(2, TouchPhase::Move)]);
        assert_eq!(input.events, vec![Event::PointerMoved(egui::pos2(2., 20.))]);
        let (input, _) = run(&mut router, vec![touch(2, TouchPhase::End)]);
        assert!(
            input
                .events
                .iter()
                .any(|e| matches!(e, Event::PointerButton { pressed: false, .. }))
        );
        assert!(router.pen.is_none());
    }
    #[test]
    fn pressure_sensitive_fingers_still_navigate() {
        let mut router = PenInput::default();
        let events = vec![touch(1, TouchPhase::Start), touch(3, TouchPhase::Start)];
        let (input, routed) = run(&mut router, events.clone());
        assert_eq!(input.events, events);
        assert!(routed.pen_events.is_empty());
        assert!(!routed.reset_gestures);
    }
    #[test]
    fn finger_first_does_not_steal_pen_and_stays_suppressed_until_lift() {
        let mut router = PenInput::default();
        run(&mut router, vec![touch(1, TouchPhase::Start)]);
        let (input, _) = run(
            &mut router,
            vec![
                touch(2, TouchPhase::Start),
                touch(1, TouchPhase::Move),
                Event::PointerMoved(egui::pos2(100., 100.)),
            ],
        );
        assert_eq!(
            input.events.last(),
            Some(&button(egui::pos2(2., 20.), true, egui::Modifiers::NONE))
        );
        run(&mut router, vec![touch(2, TouchPhase::End)]);
        let (input, _) = run(&mut router, vec![touch(1, TouchPhase::Move)]);
        assert!(input.events.is_empty());
        let (input, _) = run(
            &mut router,
            vec![
                touch(1, TouchPhase::End),
                button(Pos2::ZERO, false, egui::Modifiers::NONE),
            ],
        );
        assert!(input.events.is_empty());
        let events = vec![touch(1, TouchPhase::Start)];
        assert_eq!(run(&mut router, events.clone()).0.events, events);
    }
    #[test]
    fn palm_after_pen_does_not_interrupt_stroke() {
        let mut router = PenInput::default();
        run(&mut router, vec![touch(2, TouchPhase::Start)]);
        let (input, _) = run(
            &mut router,
            vec![
                touch(1, TouchPhase::Start),
                touch(1, TouchPhase::End),
                button(Pos2::ZERO, false, egui::Modifiers::NONE),
                touch(2, TouchPhase::Move),
            ],
        );
        assert_eq!(input.events, vec![Event::PointerMoved(egui::pos2(2., 20.))]);
        assert_eq!(router.pen, Some(2));
    }
    #[test]
    fn cancellation_and_focus_loss_release_pen() {
        for terminal in [touch(2, TouchPhase::Cancel), Event::WindowFocused(false)] {
            let mut router = PenInput::default();
            run(&mut router, vec![touch(2, TouchPhase::Start)]);
            let (input, _) = run(&mut router, vec![terminal]);
            assert!(router.pen.is_none());
            assert!(
                input
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::PointerButton { pressed: false, .. }))
            );
        }
    }
    #[test]
    fn quick_stroke_retains_both_button_edges() {
        let (input, _) = run(
            &mut PenInput::default(),
            vec![touch(2, TouchPhase::Start), touch(2, TouchPhase::End)],
        );
        let edges: Vec<_> = input
            .events
            .iter()
            .filter_map(|e| match e {
                Event::PointerButton { pressed, .. } => Some(*pressed),
                _ => None,
            })
            .collect();
        assert_eq!(edges, vec![true, false]);
    }
    #[test]
    fn pen_does_not_rotate_camera_but_finger_drag_does() {
        use crate::{camera::Camera, preferences::TouchSettings, touch::TouchInput};
        for id in [1, 2] {
            let ctx = egui::Context::default();
            let mut router = PenInput::default();
            let mut touch_input = TouchInput::default();
            let mut camera = Camera::default();
            for (phase, x) in [(TouchPhase::Start, 50.), (TouchPhase::Move, 150.)] {
                let mut event = touch(id, phase);
                if let Event::Touch { pos, .. } = &mut event {
                    *pos = egui::pos2(x, 50.);
                }
                let (input, routed) = run(&mut router, vec![event]);
                if routed.reset_gestures {
                    touch_input = TouchInput::default();
                }
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let output = touch_input.update(
                            ui,
                            &TouchSettings::default(),
                            &mut camera,
                            true,
                            |_| true,
                        );
                        assert_eq!(output.blocks_pointer, id == 1);
                    });
                });
            }
            assert_eq!(camera.yaw == 0., id == 2);
        }
    }
    #[test]
    fn finger_force_never_overrides_pen_pressure() {
        let mut router = PenInput::default();
        let mut pressure = super::super::NativePressure::default();
        let (_, routed) = run(
            &mut router,
            vec![touch(1, TouchPhase::Start), touch(2, TouchPhase::Start)],
        );
        assert_eq!(pressure.update(&routed.pen_events), Some(0.4));
        let (_, routed) = run(&mut router, vec![touch(1, TouchPhase::End)]);
        assert_eq!(pressure.update(&routed.pen_events), Some(0.4));
        let (_, routed) = run(&mut router, vec![touch(2, TouchPhase::Cancel)]);
        assert_eq!(pressure.update(&routed.pen_events), None);
    }
    #[test]
    fn pen_hover_moves_cursor_without_starting_a_stroke() {
        let mut router = PenInput::default();
        let (input, _) = run(&mut router, vec![touch(2, TouchPhase::Move)]);
        assert_eq!(input.events, vec![Event::PointerMoved(egui::pos2(2., 20.))]);
        assert!(router.pen.is_none());
    }
    #[test]
    fn unfocused_input_releases_a_held_pen_without_a_focus_event() {
        let mut router = PenInput::default();
        run(&mut router, vec![touch(2, TouchPhase::Start)]);
        let mut input = egui::RawInput {
            focused: false,
            ..Default::default()
        };
        let routed = router.route(&mut input, |_| false);
        assert!(routed.reset_gestures);
        assert!(router.pen.is_none());
        assert!(
            input
                .events
                .iter()
                .any(|e| matches!(e, Event::PointerButton { pressed: false, .. }))
        );
    }
    #[test]
    fn routed_pen_sculpts_with_default_touch_gestures_and_records_undo() {
        let mut app = crate::App::default();
        let ctx = egui::Context::default();
        let mut router = PenInput::default();
        let original = app.document.mesh().vertices.clone();
        for phase in [
            None,
            Some(TouchPhase::Start),
            Some(TouchPhase::Move),
            Some(TouchPhase::End),
        ] {
            let events = phase
                .map(|phase| {
                    let mut event = touch(2, phase);
                    if let Event::Touch { pos, .. } = &mut event {
                        *pos = egui::pos2(500., 400.);
                    }
                    vec![event]
                })
                .unwrap_or_default();
            let (mut input, _) = run(&mut router, events);
            input.screen_rect = Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1000., 800.),
            ));
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            });
        }
        assert!(app.document.is_dirty());
        assert!(app.document.can_undo());
        assert_eq!(app.camera.yaw, 0.);
        assert_eq!(app.camera.pitch, 0.);
        assert_ne!(app.document.mesh().vertices, original);
        app.document.undo();
        assert_eq!(app.document.mesh().vertices, original);
    }
}

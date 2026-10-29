//! Recognize contact gestures before the viewport handles emulated mouse input.
use crate::{
    camera::Camera,
    preferences::{Action, TouchDrag, TouchScroll, TouchSettings},
};
use eframe::egui::{self, Event, Pos2, TouchPhase, Vec2};
use std::collections::BTreeMap;

type ContactId = (u64, u64);
struct Contact {
    origin: Pos2,
    position: Pos2,
}

#[derive(Default)]
pub(crate) struct TouchInput {
    contacts: BTreeMap<ContactId, Contact>,
    baseline: Option<(usize, Pos2, Vec2)>,
    started: f64,
    peak: usize,
    moved: bool,
    rejected: bool,
    released: bool,
    button: Option<(egui::PointerButton, Pos2, f64, bool)>,
    pan_scroll: bool,
    frame: Option<u64>,
    cached: Output,
}

#[derive(Clone, Default)]
pub(crate) struct Output {
    pub blocks_pointer: bool,
    pub actions: Vec<Action>,
}

impl TouchInput {
    /// Called once per frame after overlays have established their hit regions.
    pub fn update(
        &mut self,
        ui: &egui::Ui,
        settings: &TouchSettings,
        camera: &mut Camera,
        enabled: bool,
        accepts: impl Fn(Pos2) -> bool,
    ) -> Output {
        let frame = ui.ctx().cumulative_frame_nr();
        if self.frame == Some(frame) {
            // egui can run another layout pass. Never navigate or dispatch twice.
            if settings.enabled && (self.pan_scroll || settings.scroll == TouchScroll::Disabled) {
                ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
            }
            return Output {
                blocks_pointer: self.cached.blocks_pointer,
                actions: Vec::new(),
            };
        }
        self.frame = Some(frame);
        let (events, now, focused, pointer) =
            ui.input(|i| (i.events.clone(), i.time, i.focused, i.pointer.hover_pos()));
        let enabled = enabled && focused;
        let mut output = self.contacts(&events, now, settings, camera, enabled, &accepts);
        if !focused {
            self.contacts.clear();
            self.baseline = None;
            self.button = None;
        }
        if settings.enabled {
            if enabled && !self.rejected {
                self.navigate(settings, camera);
            }
            self.trackpad(
                &events,
                now,
                settings,
                camera,
                enabled,
                pointer,
                &accepts,
                &mut output,
            );
            // egui retains smoothing after the final wheel event. Consume that tail too,
            // so a pan never turns into a zoom on the following repaint.
            if self.pan_scroll || settings.scroll == TouchScroll::Disabled {
                ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
            }
        }
        self.cached = output.clone();
        output
    }

    fn contacts(
        &mut self,
        events: &[Event],
        now: f64,
        settings: &TouchSettings,
        camera: &mut Camera,
        enabled: bool,
        accepts: &impl Fn(Pos2) -> bool,
    ) -> Output {
        let mut output = Output {
            blocks_pointer: settings.enabled && !self.contacts.is_empty(),
            ..Default::default()
        };
        if !enabled || !settings.enabled {
            self.rejected = true;
        }
        for event in events {
            let Event::Touch {
                device_id,
                id,
                phase,
                pos,
                ..
            } = event
            else {
                continue;
            };
            let key = (device_id.0, id.0);
            // Even the final release must not leak through as a sculpt or gizmo click.
            output.blocks_pointer |= settings.enabled;
            match phase {
                TouchPhase::Start => {
                    if enabled && settings.enabled && !self.rejected {
                        self.navigate(settings, camera);
                    }
                    if self.contacts.is_empty() {
                        self.started = now;
                        self.peak = 0;
                        self.moved = false;
                        self.released = false;
                        self.rejected = !enabled || !settings.enabled;
                    }
                    self.rejected |= !accepts(*pos)
                        || !pos.is_finite()
                        || self.released
                        || self
                            .contacts
                            .keys()
                            .any(|(device, _)| *device != device_id.0)
                        || self.contacts.contains_key(&key);
                    self.contacts.insert(
                        key,
                        Contact {
                            origin: *pos,
                            position: *pos,
                        },
                    );
                    self.peak = self.peak.max(self.contacts.len());
                    self.baseline = self.measure();
                }
                TouchPhase::Move | TouchPhase::End | TouchPhase::Cancel => {
                    if let Some(contact) = self.contacts.get_mut(&key) {
                        self.rejected |= !pos.is_finite() || *phase == TouchPhase::Cancel;
                        self.moved |= pos.distance(contact.origin) > settings.tap_slop;
                        contact.position = *pos;
                        if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
                            // A quick move and release may share one frame. Apply its
                            // last motion before changing the contact-count baseline.
                            if enabled && settings.enabled && !self.rejected {
                                self.navigate(settings, camera);
                            }
                            self.contacts.remove(&key);
                            self.released = true;
                            self.baseline = self.measure();
                            if self.contacts.is_empty()
                                && !self.rejected
                                && !self.moved
                                && now - self.started <= settings.tap_seconds as f64
                            {
                                let action = match self.peak {
                                    2 => &settings.two_tap,
                                    3 => &settings.three_tap,
                                    _ => &None,
                                };
                                output.actions.extend(action.clone());
                            }
                        }
                    }
                }
            }
        }
        output
    }

    fn measure(&self) -> Option<(usize, Pos2, Vec2)> {
        let count = self.contacts.len();
        if count == 0 {
            return None;
        }
        let center = self
            .contacts
            .values()
            .fold(Vec2::ZERO, |sum, c| sum + c.position.to_vec2())
            / count as f32;
        let span = if count == 2 {
            let mut contacts = self.contacts.values();
            let first = contacts.next().unwrap().position;
            contacts.next().unwrap().position - first
        } else {
            Vec2::ZERO
        };
        Some((count, center.to_pos2(), span))
    }

    fn navigate(&mut self, settings: &TouchSettings, camera: &mut Camera) {
        let Some(current) = self.measure() else {
            return;
        };
        let Some(previous) = self.baseline else {
            self.baseline = Some(current);
            return;
        };
        // A third finger or any release ends navigation until every finger is lifted.
        // Reset the baseline on contact changes so adding a finger never jumps the camera.
        if self.moved && !self.released && self.peak <= 2 && current.0 == previous.0 {
            let drag = if current.0 == 1 {
                settings.one_finger
            } else {
                settings.two_finger
            };
            Self::drag(camera, drag, (current.1 - previous.1) * settings.drag_speed);
            if current.0 == 2 {
                let old_span = previous.2.length();
                let span = current.2.length();
                if settings.pinch && old_span > 1. && span > 1. {
                    Self::pinch(camera, span / old_span, settings);
                }
                // Near-coincident fingers have no stable angle. Rebase there instead
                // of producing a half-turn as fingers cross each other.
                if settings.twist && old_span >= 24. && span >= 24. {
                    let cross = previous.2.x * current.2.y - previous.2.y * current.2.x;
                    let dot = previous.2.dot(current.2);
                    Self::twist(camera, cross.atan2(dot), settings);
                }
            }
            self.baseline = Some(current);
        }
    }

    fn drag(camera: &mut Camera, drag: TouchDrag, delta: Vec2) {
        if !delta.is_finite() {
            return;
        }
        match drag {
            TouchDrag::Disabled => {}
            TouchDrag::Orbit => camera.orbit(delta),
            TouchDrag::Pan => camera.pan += delta,
            TouchDrag::Zoom => camera.zoom_drag(delta),
        }
    }

    fn pinch(camera: &mut Camera, factor: f32, settings: &TouchSettings) {
        if factor.is_finite() && factor > 0. {
            let exponent = settings.pinch_speed * if settings.reverse_pinch { -1. } else { 1. };
            camera.set_zoom(camera.zoom * factor.powf(exponent));
        }
    }

    fn twist(camera: &mut Camera, radians: f32, settings: &TouchSettings) {
        let direction = if settings.reverse_twist { -1. } else { 1. };
        camera.roll_by(radians * settings.twist_speed * direction);
    }

    #[allow(clippy::too_many_arguments)]
    fn trackpad(
        &mut self,
        events: &[Event],
        now: f64,
        settings: &TouchSettings,
        camera: &mut Camera,
        enabled: bool,
        pointer: Option<Pos2>,
        accepts: &impl Fn(Pos2) -> bool,
        output: &mut Output,
    ) {
        let available = enabled && !output.blocks_pointer && pointer.is_some_and(accepts);
        if !enabled || !settings.button_taps {
            self.button = None;
        }
        for event in events {
            match event {
                Event::Zoom(factor) if available && settings.pinch => {
                    Self::pinch(camera, *factor, settings)
                }
                Event::Rotate(radians) if available && settings.twist => {
                    Self::twist(camera, *radians, settings)
                }
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                } => {
                    self.pan_scroll = modifiers.ctrl
                        || modifiers.command
                        || match settings.scroll {
                            TouchScroll::PixelPan => *unit == egui::MouseWheelUnit::Point,
                            TouchScroll::AllPan | TouchScroll::Disabled => true,
                            TouchScroll::Zoom => false,
                        };
                    if available && self.pan_scroll {
                        let scale = match unit {
                            egui::MouseWheelUnit::Point => 1.,
                            egui::MouseWheelUnit::Line => 24.,
                            egui::MouseWheelUnit::Page => 400.,
                        };
                        // Some OS gesture mappers send pinch as Ctrl+wheel.
                        if modifiers.ctrl || modifiers.command {
                            if settings.pinch {
                                Self::pinch(camera, (delta.y * scale * 0.002).exp(), settings);
                            }
                        } else if settings.scroll != TouchScroll::Disabled {
                            Self::drag(
                                camera,
                                settings.two_finger,
                                *delta * scale * settings.drag_speed,
                            );
                        }
                    }
                }
                Event::PointerButton {
                    button,
                    pos,
                    pressed,
                    modifiers,
                } if settings.button_taps
                    && matches!(
                        button,
                        egui::PointerButton::Secondary | egui::PointerButton::Middle
                    ) =>
                {
                    if *pressed {
                        self.button = (available && modifiers.is_none())
                            .then_some((*button, *pos, now, false));
                    } else if let Some((started_button, start, time, moved)) = self.button.take()
                        && started_button == *button
                        && available
                        && !moved
                        && pos.distance(start) <= settings.tap_slop
                        && now - time <= settings.tap_seconds as f64
                    {
                        output.blocks_pointer = true;
                        output
                            .actions
                            .extend(if *button == egui::PointerButton::Secondary {
                                settings.two_tap.clone()
                            } else {
                                settings.three_tap.clone()
                            });
                    }
                }
                Event::PointerMoved(pos) => {
                    if let Some((_, start, _, moved)) = &mut self.button {
                        *moved |= pos.distance(*start) > settings.tap_slop;
                    }
                }
                _ => {}
            }
        }
    }
}

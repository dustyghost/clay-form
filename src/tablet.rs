//! Tablet pressure service; platform readers publish normalized pressure here.
use eframe::egui;
#[cfg(any(target_os = "windows", test))]
mod contact_types;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "windows", test))]
pub(crate) mod pen;
#[cfg(target_os = "windows")]
mod windows;

/// Install native pen classification before starting eframe's event loop.
pub fn configure_native_input(_options: &mut eframe::NativeOptions) {
    #[cfg(target_os = "windows")]
    windows::configure(_options);
}

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
struct TabletState {
    pressure: f32,
    updated_at: Instant,
    status: String,
}
type SharedState = Arc<Mutex<TabletState>>;
pub(crate) struct Tablet {
    state: SharedState,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    native: NativePressure,
    #[cfg(target_os = "windows")]
    pen_input: pen::PenInput,
}
impl Tablet {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(TabletState {
            pressure: 1.,
            updated_at: Instant::now() - std::time::Duration::from_secs(10),
            status: if cfg!(target_os = "linux") {
                "Searching for pen pressure devices"
            } else {
                "Using pressure from native pointer events when available"
            }
            .into(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        #[cfg(target_os = "linux")]
        let worker = Some(linux::spawn(state.clone(), stop.clone()));
        #[cfg(not(target_os = "linux"))]
        let worker = None;
        Self {
            state,
            stop,
            worker,
            native: NativePressure::default(),
            #[cfg(target_os = "windows")]
            pen_input: pen::PenInput::default(),
        }
    }
    pub fn pressure(&self) -> Option<f32> {
        let state = self.state.lock().unwrap();
        (state.updated_at.elapsed().as_secs_f32() < 0.5).then_some(state.pressure)
    }
    pub fn status(&self) -> String {
        let status = self.state.lock().unwrap().status.clone();
        #[cfg(target_os = "windows")]
        let status = format!("{status}; {}", windows::status());
        status
    }
    pub fn update_native(&mut self, events: &[egui::Event]) -> Option<f32> {
        #[cfg(target_os = "windows")]
        let events = {
            // Windows pressure was already extracted from classified pen contacts
            // in prepare_input; finger force must never replace it here.
            let _ = events;
            &[]
        };
        self.native.update(events)
    }
    pub fn prepare_input(&mut self, _input: &mut egui::RawInput) -> bool {
        #[cfg(target_os = "windows")]
        {
            let pens = windows::pen_contacts(_input);
            let routed = self.pen_input.route(_input, |id| pens.contains(&id));
            self.native.update(&routed.pen_events);
            routed.reset_gestures
        }
        #[cfg(not(target_os = "windows"))]
        false
    }
}

#[derive(Default)]
struct NativePressure {
    contact: Option<(egui::TouchDeviceId, egui::TouchId, Option<f32>)>,
}
impl NativePressure {
    fn update(&mut self, events: &[egui::Event]) -> Option<f32> {
        for event in events {
            match event {
                egui::Event::Touch {
                    device_id,
                    id,
                    phase,
                    force,
                    ..
                } => {
                    let matches = self
                        .contact
                        .as_ref()
                        .is_some_and(|(d, i, _)| d == device_id && i == id);
                    match phase {
                        egui::TouchPhase::Start if self.contact.is_none() => {
                            self.contact = Some((
                                *device_id,
                                *id,
                                force.filter(|p| p.is_finite()).map(|p| p.clamp(0., 1.)),
                            ))
                        }
                        egui::TouchPhase::Move if matches => {
                            if let Some(p) = force.filter(|p| p.is_finite()) {
                                self.contact.as_mut().unwrap().2 = Some(p.clamp(0., 1.));
                            }
                        }
                        egui::TouchPhase::End | egui::TouchPhase::Cancel if matches => {
                            self.contact = None
                        }
                        _ => {}
                    }
                }
                egui::Event::WindowFocused(false) => self.contact = None,
                _ => {}
            }
        }
        self.contact.and_then(|(_, _, p)| p)
    }
}

impl Drop for Tablet {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_pressure_survives_frames_without_touch_events_until_release() {
        let mut native = NativePressure::default();
        let event = |phase, force| egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(2),
            phase,
            pos: egui::Pos2::ZERO,
            force,
        };
        assert_eq!(
            native.update(&[event(egui::TouchPhase::Start, Some(0.2))]),
            Some(0.2)
        );
        assert_eq!(native.update(&[]), Some(0.2));
        assert_eq!(
            native.update(&[event(egui::TouchPhase::Move, None)]),
            Some(0.2)
        );
        assert_eq!(
            native.update(&[event(egui::TouchPhase::Move, Some(0.8))]),
            Some(0.8)
        );
        assert_eq!(
            native.update(&[event(egui::TouchPhase::End, Some(0.))]),
            None
        );
        assert_eq!(native.update(&[]), None);
    }
}

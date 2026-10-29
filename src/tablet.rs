//! Tablet pressure service; platform readers publish normalized pressure here.
use eframe::egui;
#[cfg(target_os = "linux")]
mod linux;
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
        }
    }
    pub fn pressure(&self) -> Option<f32> {
        let state = self.state.lock().unwrap();
        (state.updated_at.elapsed().as_secs_f32() < 0.5).then_some(state.pressure)
    }
    pub fn status(&self) -> String {
        self.state.lock().unwrap().status.clone()
    }
    pub fn update_native(&mut self, events: &[egui::Event]) -> Option<f32> {
        self.native.update(events)
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

//! Tablet pressure service; platform readers publish normalized pressure here.
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
}
impl Tablet {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(TabletState {
            pressure: 1.,
            updated_at: Instant::now() - std::time::Duration::from_secs(10),
            status: "No Huion pressure device detected".into(),
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
        }
    }
    pub fn pressure(&self) -> Option<f32> {
        let state = self.state.lock().unwrap();
        (state.updated_at.elapsed().as_secs_f32() < 0.5).then_some(state.pressure)
    }
    pub fn status(&self) -> String {
        self.state.lock().unwrap().status.clone()
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

//! Opt-in test trace of actual viewport input, edits and callback preparation.
//! Elapsed CPU timestamps do not measure GPU completion or OS presentation.
use std::{cell::RefCell, time::Instant};

#[derive(Debug, serde::Serialize)]
pub(crate) enum Stage {
    Input {
        revision: u64,
        time: f64,
        pointer: Option<[f32; 2]>,
        primary_down: bool,
        pointer_events: usize,
    },
    Scene {
        revision: u64,
    },
    Dab {
        point: [f32; 2],
        before: u64,
        after: u64,
    },
    Callback {
        revision: Option<u64>,
    },
    Complete {
        revision: u64,
        stroke_active: bool,
    },
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct Sample {
    pub frame: usize,
    pub elapsed_ms: f64,
    pub stage: Stage,
}

struct Trace {
    started: Instant,
    frame: usize,
    samples: Vec<Sample>,
}
thread_local! {
    static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
}

pub(crate) struct Capture;
impl Capture {
    pub fn start() -> Self {
        TRACE.with(|trace| {
            let mut trace = trace.borrow_mut();
            assert!(trace.is_none(), "nested viewport trace");
            *trace = Some(Trace {
                started: Instant::now(),
                frame: 0,
                samples: Vec::new(),
            });
        });
        Self
    }
    pub fn finish(self) -> Vec<Sample> {
        TRACE.with(|trace| trace.borrow_mut().take().unwrap().samples)
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        TRACE.with(|trace| {
            trace.borrow_mut().take();
        });
    }
}
pub(super) fn frame(ui: &eframe::egui::Ui, revision: u64) {
    if !TRACE.with(|trace| {
        let mut trace = trace.borrow_mut();
        if let Some(trace) = trace.as_mut() {
            trace.frame += 1;
            true
        } else {
            false
        }
    }) {
        return;
    }
    let stage = ui.input(|input| Stage::Input {
        revision,
        time: input.time,
        pointer: input.pointer.latest_pos().map(|p| [p.x, p.y]),
        primary_down: input.pointer.primary_down(),
        pointer_events: input
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    eframe::egui::Event::PointerMoved(_)
                        | eframe::egui::Event::PointerButton { .. }
                )
            })
            .count(),
    });
    record(stage);
}
pub(crate) fn record(stage: Stage) {
    TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.samples.push(Sample {
                frame: trace.frame,
                elapsed_ms: trace.started.elapsed().as_secs_f64() * 1000.,
                stage,
            });
        }
    });
}

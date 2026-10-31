//! Test-only timings inside the production scene constructor.
use std::{cell::RefCell, collections::BTreeMap, time::Instant};
struct Trace {
    last: Instant,
    times: BTreeMap<&'static str, f64>,
}
thread_local! { static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) }; }
pub(super) fn mark(name: &'static str) {
    TRACE.with_borrow_mut(|trace| {
        if let Some(trace) = trace {
            let now = Instant::now();
            *trace.times.entry(name).or_default() +=
                now.duration_since(trace.last).as_secs_f64() * 1000.;
            trace.last = now;
        }
    });
}
pub(in crate::viewport) fn capture<T>(f: impl FnOnce() -> T) -> (T, BTreeMap<&'static str, f64>) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TRACE.with_borrow_mut(|trace| *trace = None);
        }
    }
    let _reset = Reset;
    TRACE.with_borrow_mut(|trace| {
        *trace = Some(Trace {
            last: Instant::now(),
            times: BTreeMap::new(),
        })
    });
    let result = f();
    mark("finish");
    (
        result,
        TRACE.with_borrow_mut(|trace| trace.take().unwrap().times),
    )
}

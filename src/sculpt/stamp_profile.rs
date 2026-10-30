//! Opt-in phase timings for actual sculpt proposals and safety checks.
use super::*;
use std::{cell::RefCell, collections::BTreeMap, time::Instant};

struct Trace {
    last: Instant,
    stages: BTreeMap<&'static str, f64>,
    counts: BTreeMap<&'static str, usize>,
}
thread_local! { static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) }; }
pub(super) fn mark(name: &'static str) {
    TRACE.with_borrow_mut(|trace| {
        if let Some(trace) = trace {
            let now = Instant::now();
            *trace.stages.entry(name).or_default() +=
                now.duration_since(trace.last).as_secs_f64() * 1000.;
            *trace.counts.entry(name).or_default() += 1;
            trace.last = now;
        }
    });
}
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        TRACE.with_borrow_mut(|trace| *trace = None);
    }
}
fn capture(f: impl FnOnce() -> bool) -> (bool, Trace) {
    let _reset = Reset;
    TRACE.with_borrow_mut(|trace| {
        *trace = Some(Trace {
            last: Instant::now(),
            stages: BTreeMap::new(),
            counts: BTreeMap::new(),
        })
    });
    let changed = f();
    mark("result");
    (
        changed,
        TRACE.with_borrow_mut(|trace| trace.take().unwrap()),
    )
}

#[test]
#[ignore = "opt-in CPU stage diagnosis; no wall-clock assertion"]
fn profile_stamp_stages() {
    let mut geometry = Geometry::new(Mesh::starting_sphere());
    for _ in 0..3 {
        geometry.subdivide().unwrap();
    }
    let mut rows = Vec::new();
    for hidden in [false, true] {
        let mut base = geometry.mesh.clone();
        if hidden {
            base.hidden_faces = base
                .triangles
                .iter()
                .map(|t| t.iter().all(|&i| base.vertices[i].0 < -0.06))
                .collect();
        }
        for kind in [Kind::Standard, Kind::Clay, Kind::Grab] {
            let mut brush = Brush::new(kind);
            if kind == Kind::Grab {
                brush.strength = 2.;
            }
            for radius in [0.18, 0.65] {
                let dab = Dab {
                    center: Vec3(0.03, 0.03, 1.),
                    normal: Vec3(0., 0., 1.),
                    right: Vec3(1., 0., 0.),
                    up: Vec3(0., 1., 0.),
                    radius,
                    pressure: 0.75,
                    delta: Vec3(0., 0., -3.),
                    invert: false,
                };
                for trial in 0..6 {
                    let mut mesh = base.clone();
                    let start = Instant::now();
                    let (changed, trace) = capture(|| mesh.stamp_many(&brush, &[dab]));
                    let total_ms = start.elapsed().as_secs_f64() * 1000.;
                    if trial > 0 {
                        rows.push(
                            serde_json::json!({"hidden":hidden,"kind":format!("{kind:?}"),
                            "radius":radius,"trial":trial,"changed":changed,"total_ms":total_ms,
                            "stages_ms":trace.stages,"stage_counts":trace.counts,
                            "output_hash_fnv1a":super::brush_loop_profile::fingerprint(&mesh)}),
                        );
                    }
                }
            }
        }
    }
    let out = serde_json::json!({"schema":1,"vertices":geometry.mesh.vertices.len(),
        "triangles":geometry.mesh.triangles.len(),"rows":rows});
    std::fs::write(
        std::env::var("CLAY_STAMP_PROFILE").expect("output path"),
        serde_json::to_vec_pretty(&out).unwrap(),
    )
    .unwrap();
}

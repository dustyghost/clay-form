//! Opt-in CPU brush workload: broader footprints and non-default pressure/alpha settings.
use super::*;
use serde::Serialize;
use std::{hint::black_box, io::Write, time::Instant};

pub(super) fn fingerprint(value: &impl Serialize) -> String {
    struct Hash(u64);
    impl Write for Hash {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            for b in bytes {
                self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x100000001b3);
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hash = Hash(0xcbf29ce484222325);
    serde_json::to_writer(&mut hash, value).unwrap();
    format!("{:016x}", hash.0)
}

#[test]
#[ignore = "opt-in CPU brush timing; no wall-clock assertion"]
fn profile_brush_loops() {
    let mut geometry = Geometry::new(Mesh::starting_sphere());
    for _ in 0..2 {
        geometry.subdivide().unwrap();
    }
    let base = &geometry.mesh;
    let mut rows = Vec::new();
    for kind in [Kind::Standard, Kind::Clay, Kind::Smooth, Kind::Paint] {
        for (alpha_name, alpha) in [
            ("soft", Alpha::Soft),
            ("hard", Alpha::Hard),
            (
                "image",
                Alpha::Image {
                    width: 3,
                    height: 2,
                    pixels: vec![0., 0.4, 1., 0.7, 0.1, 0.8].into(),
                },
            ),
        ] {
            let mut brush = Brush::new(kind);
            brush.alpha = alpha;
            brush.rotation = 37.;
            brush.inner_radius = 0.35;
            brush.falloff = 1.7;
            brush.pressure_curve = 1.6;
            brush.set_paint_colour(false, [0.8, 0.15, 0.05]);
            for radius in [0.18, 0.65] {
                let dabs = [(-0.08, 0.37), (0.08, 0.83)].map(|(x, pressure)| Dab {
                    center: Vec3(x, 0.03, 1.),
                    normal: Vec3(0., 0., 1.),
                    right: Vec3(1., 0., 0.),
                    up: Vec3(0., 1., 0.),
                    radius,
                    pressure,
                    delta: Vec3::default(),
                    invert: false,
                });
                for trial in 0..6 {
                    let mut mesh = base.clone();
                    let start = Instant::now();
                    let changed = mesh.stamp_many(black_box(&brush), black_box(&dabs));
                    let ms = start.elapsed().as_secs_f64() * 1000.;
                    assert!(changed);
                    if trial > 0 {
                        rows.push(serde_json::json!({"kind": format!("{kind:?}"),
                            "alpha": alpha_name, "radius": radius, "trial": trial,
                            "ms": ms, "output_hash_fnv1a": fingerprint(&mesh)}));
                    }
                }
            }
        }
    }
    let result = serde_json::json!({"schema": 1, "fixture_hash_fnv1a": fingerprint(base),
        "vertices": base.vertices.len(), "triangles": base.triangles.len(),
        "rotation": 37., "inner_radius": 0.35, "falloff": 1.7, "pressure_curve": 1.6,
        "dab_pressures": [0.37, 0.83], "rows": rows});
    let path = std::env::var("CLAY_BRUSH_LOOP_PROFILE").expect("profile output path");
    std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
}

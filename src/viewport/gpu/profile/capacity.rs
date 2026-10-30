//! Reproducible, bounded workloads. Timings are opt-in; correctness is not timed.
use super::*;
use crate::sculpt::{Material, Project};
use serde::Serialize;
use std::{collections::BTreeMap, io::Write};

const DABS: usize = 12;
const SCHEMA: u32 = 1;

fn fixture(name: &str, level: usize) -> Geometry {
    assert!(
        level <= 3,
        "higher levels require a separate capacity investigation"
    );
    assert!(matches!(name, "sphere" | "organic" | "hidden"));
    let mut geometry = Geometry::new(Mesh::starting_sphere());
    for _ in 0..level {
        geometry.subdivide().unwrap();
    }
    if name != "sphere" {
        // A synthetic closed organic form with fine relief, not an artist fixture.
        // Keep every stored level's topology; only the active level gains detail.
        for (i, p) in geometry.mesh.vertices.iter_mut().enumerate() {
            let relief = 0.018 * (p.0 * 23.).sin() * (p.1 * 19.).cos();
            *p = Vec3(p.0 * (0.85 + 0.12 * p.1), p.1 * 1.15, p.2 * (1. + relief));
            geometry.mesh.mask[i] = if p.0 < -0.06 { 1. } else { 0. };
        }
        let material = Material::from_srgb([0.25, 0.45, 0.6], 0.65, 0.1);
        geometry.mesh.materials = vec![material; geometry.mesh.vertices.len()];
    }
    if name == "hidden" {
        geometry.mesh.hidden_faces = geometry
            .mesh
            .triangles
            .iter()
            .map(|t| t.iter().all(|&i| geometry.mesh.vertices[i].2 < -0.2))
            .collect();
    }
    geometry
}

fn document(geometry: Geometry) -> Document {
    let mut doc = Document::default();
    let mut project = doc.project();
    project.mesh = geometry.mesh;
    project.mesh_settings = geometry.settings;
    doc.replace_project(project, "capacity-fixture.clay")
        .unwrap();
    doc
}

fn brush(kind: Kind) -> Brush {
    let mut brush = Brush::new(kind);
    brush.strength = 0.25;
    brush.set_paint_colour(false, [0.8, 0.15, 0.05]);
    brush
}

fn dab(step: usize) -> Dab {
    Dab {
        center: Vec3(-0.04 + step as f32 * 0.008, 0.03, 1.),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 0.18,
        pressure: 0.65 + (step % 3) as f32 * 0.1,
        delta: Vec3::default(),
        invert: false,
    }
}

#[derive(Default, Serialize)]
struct Measurements(BTreeMap<String, Vec<f64>>);
impl Measurements {
    fn time<T>(&mut self, name: &str, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let result = f();
        self.0
            .entry(name.into())
            .or_default()
            .push(start.elapsed().as_secs_f64() * 1000.);
        result
    }
    fn summary(&self) -> serde_json::Value {
        self.0
            .iter()
            .map(|(name, values)| {
                let mut sorted = values.clone();
                sorted.sort_by(f64::total_cmp);
                (
                    name.clone(),
                    serde_json::json!({
                        "samples_ms": values, "n": values.len(),
                        "p50_ms": sorted[(sorted.len() * 50).div_ceil(100) - 1],
                        "p95_ms": sorted[(sorted.len() * 95).div_ceil(100) - 1],
                        "max_ms": sorted[sorted.len()-1],
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>()
            .into()
    }
}

// Stable streaming checksum: no second serialized copy of a large mesh in RAM.
fn fingerprint(value: &impl Serialize) -> String {
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

fn memory() -> serde_json::Value {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let get = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u64>().ok())
            .map(|kib| kib * 1024)
    };
    serde_json::json!({"rss_bytes": get("VmRSS:"), "process_peak_rss_bytes": get("VmHWM:")})
}

fn refresh(previous: &DisplayMesh, doc: &Document) -> (DisplayMesh, bool) {
    let live = previous.during_stroke(doc);
    let temporary = live.is_some();
    (live.unwrap_or_else(|| DisplayMesh::new(doc)), temporary)
}

fn finish_display(
    previous: &Arc<DisplayMesh>,
    doc: &Document,
    temporary: bool,
) -> Arc<DisplayMesh> {
    if temporary {
        Arc::new(DisplayMesh::new(doc))
    } else {
        previous.clone()
    }
}

fn assert_restored(doc: &Document, before: &Geometry) {
    assert!(
        doc.geometry_matches(before),
        "undo must restore all geometry, attributes and levels"
    );
    assert_eq!(doc.history_counts(), (0, 1));
}

/// A single fixture/level per process, orchestrated by scripts/benchmark-capacity.py.
/// The gpu_load_profile prefix keeps this out of the existing GPU CI selection.
#[test]
#[ignore = "manual optimized capacity benchmark; no timing thresholds in CI"]
fn gpu_load_profile_capacity() {
    assert!(!cfg!(debug_assertions), "run benchmarks with --release");
    let output = std::env::var("CLAY_CAPACITY_OUTPUT").expect("set CLAY_CAPACITY_OUTPUT");
    let name = std::env::var("CLAY_CAPACITY_FIXTURE").unwrap_or_else(|_| "sphere".into());
    let level: usize = std::env::var("CLAY_CAPACITY_LEVEL")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .unwrap();
    let repeats: usize = std::env::var("CLAY_CAPACITY_REPEATS")
        .unwrap_or_else(|_| "3".into())
        .parse()
        .unwrap();
    assert!((1..=10).contains(&repeats));
    let use_gpu = std::env::var_os("CLAY_CAPACITY_GPU").is_some();
    let mut metrics = Measurements::default();
    let geometry = fixture(&name, level);
    let initial_hash = fingerprint(&(&geometry.mesh, &geometry.settings));
    let mut doc = metrics.time("document_open", || document(geometry));
    let document_idle = memory();
    let before = metrics.time("geometry_snapshot", || doc.geometry_snapshot());
    let idle = memory();
    let mesh = doc.mesh();
    let counts = serde_json::json!({
        "active_vertices": mesh.vertices.len(), "triangles": mesh.triangles.len(),
        "quads": mesh.quads.len(), "visible_triangles": mesh.triangles.len() - mesh.hidden_faces.iter().filter(|&&v| v).count(),
        "stored_levels": doc.mesh_settings().levels(),
        "stored_vertices": mesh.vertices.len() + doc.mesh_settings().lower.iter().map(|m| m.vertices.len()).sum::<usize>(),
        "material_vertices": mesh.materials.len(), "vertex_groups": mesh.vertex_groups.len(),
        "masked_vertices": mesh.mask.iter().filter(|&&m| m == 1.).count(),
        "models": 1, "textures": 0,
    });
    let headless = use_gpu.then(Headless::new);
    let mut renderer = headless.as_ref().map(|h| Renderer::new(&h.state).unwrap());
    if let Some(h) = &headless {
        h.state.queue.submit([]);
        h.wait();
    }
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let mut memory_stages = vec![
        serde_json::json!({"stage": "document_idle", "memory": document_idle}),
        serde_json::json!({"stage": "document_and_baseline_snapshot", "memory": idle}),
    ];
    let mut outputs = BTreeMap::new();
    for trial in 0..repeats {
        eprintln!("capacity {name} level={level} trial={trial}");
        for kind in [Kind::Standard, Kind::Clay, Kind::Paint] {
            let prefix = format!("{kind:?}");
            let brush = brush(kind);
            let mut display = metrics.time("display_initial", || {
                renderer
                    .as_mut()
                    .map_or_else(|| Arc::new(DisplayMesh::new(&doc)), |r| r.geometry(&doc))
            });
            let mut temporary_display = false;
            if let (Some(h), Some(r)) = (&headless, &renderer) {
                metrics.time("gpu_initial_upload_wait", || {
                    r.gpu.lock().unwrap().upload(&display);
                    h.state.queue.submit([]);
                    h.wait();
                });
            }
            for step in 0..DABS {
                doc.begin_sculpt_frame();
                let phase = if step == 0 { "first" } else { "sustained" };
                let changed = metrics.time(&format!("{prefix}.{phase}_dab"), || {
                    doc.apply_dab(&brush, dab(step))
                });
                assert!(
                    changed,
                    "benchmark dab must edit the surface: {prefix}/{step}"
                );
                display = metrics.time(&format!("{prefix}.{phase}_display"), || {
                    renderer.as_mut().map_or_else(
                        || {
                            let (next, temporary) = refresh(&display, &doc);
                            temporary_display = temporary;
                            Arc::new(next)
                        },
                        |r| r.geometry(&doc),
                    )
                });
                if let (Some(h), Some(r)) = (&headless, &renderer) {
                    metrics.time(&format!("{prefix}.{phase}_upload_wait"), || {
                        r.gpu.lock().unwrap().upload(&display);
                        h.state.queue.submit([]);
                        h.wait();
                    });
                }
            }
            let history_bytes = doc
                .diagnostic_manifest(Default::default())
                .retained_estimated_bytes;
            memory_stages.push(serde_json::json!({"stage": format!("{prefix}.stroke.{trial}"), "memory": memory(), "history_estimated_bytes": history_bytes}));
            metrics.time(&format!("{prefix}.end_stroke"), || doc.end_stroke());
            let final_display = metrics.time(&format!("{prefix}.end_display"), || {
                renderer.as_mut().map_or_else(
                    || finish_display(&display, &doc, temporary_display),
                    |r| r.geometry(&doc),
                )
            });
            assert_eq!(doc.history_counts(), (1, 0), "one stroke, one undo entry");
            let after_hash = fingerprint(doc.mesh());
            assert_ne!(after_hash, fingerprint(&before.mesh));
            if let Some(previous) = outputs.insert(prefix.clone(), after_hash.clone()) {
                assert_eq!(
                    previous, after_hash,
                    "repeated strokes must produce identical results"
                );
            }
            // Use the same CPU projection/picking path as the GPU viewport, without software draw.
            for frame in 0..12 {
                let camera = Camera {
                    yaw: frame as f32 * 0.015,
                    ..Default::default()
                };
                let scene = metrics.time("orbit_cpu_projection", || {
                    Scene::from_geometry(rect, &camera, final_display.clone(), false)
                });
                let hit = metrics.time("pick", || {
                    scene.pick(rect.center() + egui::vec2(frame as f32 - 6., 4.))
                });
                assert!(hit.is_some(), "pick samples must hit the model");
                if let (Some(h), Some(r)) = (&headless, &renderer) {
                    let (right, up, view) = camera.basis();
                    let params = Params::new(
                        rect,
                        right,
                        up,
                        view,
                        scene.scale,
                        rect.center(),
                        final_display.depth_extent,
                    );
                    let mut gpu = r.gpu.lock().unwrap();
                    // Include final canonical data, but keep upload separate from frame timing.
                    if frame == 0 {
                        metrics.time(&format!("{prefix}.end_upload_wait"), || {
                            gpu.upload(&final_display);
                            h.state.queue.submit([]);
                            h.wait();
                        });
                        gpu.paint(
                            &params,
                            [800, 600],
                            false,
                            WireMode::Visible,
                            false,
                            &Preview::default(),
                        );
                        h.wait();
                    }
                    metrics.time("orbit_gpu_draw_wait", || {
                        gpu.paint(
                            &params,
                            [800, 600],
                            false,
                            WireMode::Visible,
                            false,
                            &Preview::default(),
                        );
                        h.wait();
                    });
                }
            }
            metrics.time(&format!("{prefix}.undo"), || doc.undo());
            assert_restored(&doc, &before);
            metrics.time(&format!("{prefix}.redo"), || doc.redo());
            assert_eq!(fingerprint(doc.mesh()), after_hash);
            doc.undo();
            assert_restored(&doc, &before);
        }
        let project = metrics.time("project_snapshot", || doc.project());
        let path = std::path::Path::new(&output).with_extension("scratch.clay");
        let path_str = path.to_str().unwrap();
        metrics.time("save_validate_serialize_sync", || {
            crate::persistence::save_project(path_str, &project).unwrap()
        });
        let bytes = std::fs::metadata(&path).unwrap().len();
        let loaded: Project = metrics.time("load_read_parse_validate", || {
            crate::persistence::load_project(path_str).unwrap()
        });
        assert_eq!(loaded.mesh, project.mesh);
        assert_eq!(loaded.mesh_settings, project.mesh_settings);
        memory_stages.push(serde_json::json!({"stage": format!("save_load.{trial}"), "file_bytes": bytes, "memory": memory()}));
        std::fs::remove_file(path).unwrap();
    }
    // Subdivision is measured only where the result fits existing limits. Never raise caps.
    let subdivision = if level < 3 {
        let mut g = before.clone();
        metrics.time("subdivide", || g.subdivide().unwrap());
        serde_json::json!({"status": "measured", "output_vertices": g.mesh.vertices.len(), "output_triangles": g.mesh.triangles.len()})
    } else {
        serde_json::json!({"status": "skipped", "reason": "next level exceeds current per-mesh caps"})
    };
    let result = serde_json::json!({
        "schema": SCHEMA, "fixture": name, "level": level, "repeats": repeats, "dabs_per_stroke": DABS,
        "fixture_hash_fnv1a": initial_hash, "output_hashes_fnv1a": outputs,
        "counts": counts, "gpu": headless.as_ref().map(|h| format!("{:?}", h.state.adapter.get_info())),
        "viewport": [800, 600], "metrics": metrics.summary(), "memory_stages": memory_stages,
        "memory_final": memory(), "subdivision": subdivision,
        "not_measured": ["multi-million single mesh (hard caps)", "multi-model scenes", "remeshing", "texture painting", "input-to-present latency", "allocation counts", "GPU timestamps", "cold disk reads"],
    });
    if let Some(r) = renderer {
        r.destroy();
    }
    std::fs::write(output, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
}

#[test]
fn capacity_workloads_preserve_mask_topology_and_exact_history() {
    for name in ["sphere", "organic", "hidden"] {
        let before = fixture(name, 0);
        let mut doc = document(before.clone());
        for kind in [Kind::Standard, Kind::Clay, Kind::Paint] {
            for step in 0..DABS {
                assert!(doc.apply_dab(&brush(kind), dab(step)));
            }
            doc.end_stroke();
            assert_eq!(doc.history_counts(), (1, 0));
            let after = doc.geometry_snapshot();
            assert_ne!(before.mesh, after.mesh);
            assert_eq!(before.mesh.triangles, after.mesh.triangles);
            assert_eq!(before.mesh.quads, after.mesh.quads);
            assert_eq!(before.mesh.mask, after.mesh.mask);
            assert_eq!(before.mesh.hidden_faces, after.mesh.hidden_faces);
            assert_eq!(before.mesh.vertex_groups, after.mesh.vertex_groups);
            for (i, &mask) in before.mesh.mask.iter().enumerate() {
                if mask == 1. {
                    assert_eq!(before.mesh.vertices[i], after.mesh.vertices[i]);
                    assert_eq!(before.mesh.materials[i], after.mesh.materials[i]);
                }
            }
            if kind == Kind::Paint {
                assert_eq!(before.mesh.vertices, after.mesh.vertices);
            } else {
                assert_eq!(before.mesh.materials, after.mesh.materials);
            }
            doc.undo();
            assert_restored(&doc, &before);
            doc.redo();
            assert!(doc.geometry_matches(&after));
            doc.undo();
            assert_restored(&doc, &before);
        }
    }
}

#[test]
fn capacity_dense_live_display_preserves_edited_surface() {
    // Smallest sphere subdivision that actually takes the >=20k live-display path.
    let mut doc = document(fixture("organic", 2));
    let mut display = DisplayMesh::new(&doc);
    for kind in [Kind::Standard, Kind::Paint] {
        for step in 0..2 {
            assert!(doc.apply_dab(&brush(kind), dab(step)));
            display = display
                .during_stroke(&doc)
                .expect("exercise the dense cached path");
            assert_eq!(display.mesh.vertices, doc.mesh().vertices);
            assert_eq!(display.mesh.triangles, doc.mesh().triangles);
            assert_eq!(display.mesh.materials, doc.mesh().materials);
            assert!(
                display
                    .shading
                    .vertices
                    .iter()
                    .all(|(_, n)| n.0.is_finite() && n.1.is_finite() && n.2.is_finite())
            );
        }
        doc.end_stroke();
        assert!(
            display.during_stroke(&doc).is_none(),
            "canonical shading is required on release"
        );
        display = DisplayMesh::new(&doc);
    }
}

#[test]
fn capacity_canonical_display_is_reused_on_release() {
    // Below the live-cache threshold and with hidden faces, the last dab already
    // built canonical shading. Match Renderer::geometry: no second rebuild.
    for name in ["sphere", "hidden"] {
        let mut doc = document(fixture(name, 0));
        let display = DisplayMesh::new(&doc);
        assert!(doc.apply_dab(&brush(Kind::Standard), dab(0)));
        let (updated, temporary) = refresh(&display, &doc);
        assert!(!temporary);
        let updated = Arc::new(updated);
        doc.end_stroke();
        assert!(Arc::ptr_eq(
            &updated,
            &finish_display(&updated, &doc, temporary)
        ));
    }
}

//! Reproducible, bounded workloads. Timings are opt-in; correctness is not timed.
use super::*;
use crate::sculpt::{Material, Project};
use serde::Serialize;
use std::{collections::BTreeMap, io::Write};

const DABS: usize = 12;
const SCHEMA: u32 = 2;

fn fixture(name: &str, level: usize) -> Geometry {
    assert!(level <= 4, "capacity fixtures are bounded to level 4");
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
struct Measurements {
    values: BTreeMap<String, Vec<f64>>,
    progress: Option<(std::path::PathBuf, serde_json::Value)>,
}
impl Measurements {
    fn time<T>(&mut self, name: &str, f: impl FnOnce() -> T) -> T {
        self.checkpoint(name, "running");
        let start = Instant::now();
        let result = f();
        self.values
            .entry(name.into())
            .or_default()
            .push(start.elapsed().as_secs_f64() * 1000.);
        self.checkpoint(name, "completed");
        result
    }
    /// Atomic checkpoints are outside timing windows and survive a guard stop.
    fn checkpoint(&self, stage: &str, status: &str) {
        if let Some((path, context)) = &self.progress {
            let value = serde_json::json!({"schema": SCHEMA, "stage": stage,
                "status": status, "context": context, "metrics": self.summary(),
                "memory": memory()});
            let temporary = path.with_extension("tmp");
            std::fs::write(&temporary, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
            std::fs::rename(temporary, path).unwrap();
        }
    }
    fn summary(&self) -> serde_json::Value {
        self.values
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
        previous
            .after_stroke(doc)
            .unwrap_or_else(|| Arc::new(DisplayMesh::new(doc)))
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
fn gpu_load_profile_capacity() -> Result<(), &'static str> {
    if cfg!(debug_assertions) {
        return Err("run benchmarks with --release");
    }
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
    let workload = std::env::var("CLAY_CAPACITY_WORKLOAD").unwrap_or_else(|_| "full".into());
    assert!(matches!(workload.as_str(), "full" | "interactive"));
    let capacity = benchmark_capacity(
        level,
        env_limit("CLAY_CAPACITY_VERTICES", 500_000),
        env_limit("CLAY_CAPACITY_TRIANGLES", 1_000_000),
    )
    .unwrap();
    crate::sculpt::capacity::initialize(capacity).unwrap();
    assert_eq!(crate::sculpt::capacity::current(), capacity);
    let mut metrics = Measurements {
        progress: Some((
            std::path::Path::new(&output).with_extension("progress.json"),
            serde_json::json!({"fixture": name, "level": level, "repeats": repeats,
                "workload": workload, "mesh_capacity": capacity, "gpu_requested": use_gpu}),
        )),
        ..Default::default()
    };
    let geometry = metrics.time("fixture_build", || fixture(&name, level));
    metrics.checkpoint("fixture_fingerprint", "running");
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
    metrics.progress.as_mut().unwrap().1["counts"] = counts.clone();
    metrics.progress.as_mut().unwrap().1["fixture_hash_fnv1a"] = initial_hash.clone().into();
    metrics.checkpoint("gpu_setup", "running");
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
            metrics.progress.as_mut().unwrap().1["trial"] = trial.into();
            metrics.progress.as_mut().unwrap().1["brush"] = prefix.clone().into();
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
                metrics.progress.as_mut().unwrap().1["step"] = step.into();
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
            metrics.checkpoint("history_diagnostics", "running");
            let history_bytes = doc
                .diagnostic_manifest(Default::default())
                .retained_estimated_bytes;
            memory_stages.push(serde_json::json!({"stage": format!("{prefix}.stroke.{trial}"), "memory": memory(), "history_estimated_bytes": history_bytes}));
            metrics.progress.as_mut().unwrap().1["memory_stages"] = memory_stages.clone().into();
            metrics.time(&format!("{prefix}.end_stroke"), || doc.end_stroke());
            let final_display = metrics.time(&format!("{prefix}.end_display"), || {
                renderer.as_mut().map_or_else(
                    || finish_display(&display, &doc, temporary_display),
                    |r| r.geometry(&doc),
                )
            });
            assert_eq!(doc.history_counts(), (1, 0), "one stroke, one undo entry");
            metrics.checkpoint("stroke_verification", "running");
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
                        metrics.checkpoint("orbit_gpu_warmup", "running");
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
            metrics.checkpoint("undo_verification", "running");
            assert_restored(&doc, &before);
            metrics.time(&format!("{prefix}.redo"), || doc.redo());
            metrics.checkpoint("redo_verification", "running");
            assert_eq!(fingerprint(doc.mesh()), after_hash);
            doc.undo();
            assert_restored(&doc, &before);
            metrics.progress.as_mut().unwrap().1["output_hashes_fnv1a"] =
                serde_json::to_value(&outputs).unwrap();
        }
        if workload == "interactive" {
            continue;
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
    // Additional growth remains confined to the original small baseline cases.
    let subdivision = if workload == "full" && level < 3 {
        let mut g = before.clone();
        metrics.time("subdivide", || g.subdivide().unwrap());
        serde_json::json!({"status": "measured", "output_vertices": g.mesh.vertices.len(), "output_triangles": g.mesh.triangles.len()})
    } else {
        serde_json::json!({"status": "skipped", "reason": "additional subdivision is outside this workload"})
    };
    let result = serde_json::json!({
        "schema": SCHEMA, "fixture": name, "level": level, "repeats": repeats, "dabs_per_stroke": DABS,
        "workload": workload, "mesh_capacity": capacity,
        "fixture_hash_fnv1a": initial_hash, "output_hashes_fnv1a": outputs,
        "counts": counts, "gpu": headless.as_ref().map(|h| format!("{:?}", h.state.adapter.get_info())),
        "viewport": [800, 600], "metrics": metrics.summary(), "memory_stages": memory_stages,
        "memory_final": memory(), "subdivision": subdivision,
        "not_measured": ["levels above 4", "multi-model scenes", "remeshing", "texture painting", "input-to-present latency", "allocation counts", "GPU timestamps", "cold disk reads"],
    });
    if let Some(r) = renderer {
        r.destroy();
    }
    std::fs::write(output, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    metrics.checkpoint("complete", "completed");
    Ok(())
}

fn env_limit(name: &str, default: usize) -> usize {
    std::env::var(name).map_or(default, |s| s.parse().expect("numeric mesh limit"))
}

fn benchmark_capacity(
    level: usize,
    vertices: usize,
    triangles: usize,
) -> Result<crate::sculpt::capacity::MeshCapacity, String> {
    let capacity = crate::sculpt::capacity::MeshCapacity {
        vertices,
        triangles,
    };
    capacity.validate()?;
    if level > 4 {
        return Err("capacity fixtures are bounded to level 4".into());
    }
    let quads = 4704 * 4_usize.pow(level as u32);
    if vertices < quads + 2 || triangles < quads * 2 {
        return Err("fixture exceeds the requested mesh capacity".into());
    }
    Ok(capacity)
}

#[test]
fn capacity_large_fixture_requires_explicit_valid_budgets() {
    assert!(benchmark_capacity(3, 500_000, 1_000_000).is_ok());
    assert!(benchmark_capacity(4, 500_000, 1_000_000).is_err());
    assert!(benchmark_capacity(4, 1_204_226, 2_408_448).is_ok());
    assert!(benchmark_capacity(4, 1_204_225, 2_408_448).is_err());
    assert!(benchmark_capacity(4, 1_204_226, 2_408_447).is_err());
    assert!(benchmark_capacity(5, 16_000_000, 32_000_000).is_err());
    assert!(benchmark_capacity(4, 16_000_001, 32_000_000).is_err());
}

#[test]
fn capacity_checkpoints_preserve_completed_samples_when_next_phase_fails() {
    let path = std::env::temp_dir().join(format!(
        "clay-capacity-checkpoint-{}.json",
        std::process::id()
    ));
    let mut metrics = Measurements {
        progress: Some((path.clone(), serde_json::json!({"fixture": "test"}))),
        ..Default::default()
    };
    assert_eq!(metrics.time("first", || 7), 7);
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        metrics.time("next", || panic!("simulated failed phase"));
    }));
    assert!(failed.is_err());
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["status"], "running");
    assert_eq!(saved["stage"], "next");
    assert_eq!(saved["context"]["fixture"], "test");
    assert_eq!(saved["metrics"]["first"]["n"], 1);
    assert!(saved["metrics"].get("next").is_none());
    std::fs::remove_file(path).unwrap();
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
            assert_eq!(*display.mesh.vertices, doc.mesh().vertices);
            assert_eq!(*display.mesh.triangles, doc.mesh().triangles);
            assert_eq!(*display.mesh.materials, doc.mesh().materials);
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

/// Isolate CPU upload preparation without changing the capacity workload schema.
#[test]
#[ignore = "manual CPU packing diagnosis; excluded from routine GPU CI"]
fn gpu_load_profile_vertex_packing() {
    let output =
        std::env::var("CLAY_PACKING_PROFILE").expect("set CLAY_PACKING_PROFILE output path");
    let mode = std::env::var("CLAY_PACKING_MODE").unwrap_or_else(|_| "incremental".into());
    assert!(matches!(mode.as_str(), "full" | "incremental"));
    let mut doc = document(fixture("hidden", 3));
    let before = fingerprint(doc.mesh());
    let mut metrics = Measurements::default();
    let mut writes = Vec::new();
    for trial in 0..3 {
        for kind in [Kind::Standard, Kind::Paint] {
            let mut display = DisplayMesh::new(&doc);
            let mut packed = display.vertices(display.vertex_stride());
            for step in 0..DABS {
                assert!(doc.apply_dab(&brush(kind), dab(step)));
                let next = display.during_stroke(&doc).unwrap();
                let phase = if step == 0 { "first" } else { "sustained" };
                let prefix = format!("{kind:?}.{phase}");
                let stride = metrics.time(&format!("{prefix}.stride"), || next.vertex_stride());
                let (ranges, repacked_vertices) = if mode == "full" {
                    let vertices = metrics.time(&format!("{prefix}.full_pack"), || {
                        next.reference_vertices(stride)
                    });
                    let ranges = metrics.time(&format!("{prefix}.dirty_blocks"), || {
                        super::super::upload::changed_ranges(
                            bytemuck::cast_slice(&packed),
                            bytemuck::cast_slice(&vertices),
                        )
                    });
                    packed = vertices;
                    (ranges, next.shading.vertices.len())
                } else {
                    let update = metrics.time(&format!("{prefix}.pack_and_dirty_blocks"), || {
                        next.update_vertices(stride, &mut packed)
                    });
                    (update.ranges, update.changed_vertices)
                };
                let compatible = metrics.time(&format!("{prefix}.face_comparison"), || {
                    display.shading.triangles == next.shading.triangles
                        && display.poly_colors == next.poly_colors
                        && display.uv_corners == next.uv_corners
                });
                assert!(std::hint::black_box(compatible));
                writes.push(
                    serde_json::json!({"trial":trial,"brush":format!("{kind:?}"),"step":step,
                    "packed_bytes":packed.len()*4,"repacked_vertices":repacked_vertices,
                    "write_bytes":ranges.iter().map(|r|r.len()).sum::<usize>()}),
                );
                display = next;
            }
            doc.end_stroke();
            doc.undo();
            assert_eq!(fingerprint(doc.mesh()), before);
        }
    }
    let report = serde_json::json!({"mode":mode,"fixture":"hidden","level":3,"repeats":3,"dabs_per_stroke":DABS,
        "fixture_hash_fnv1a":before,"vertices":doc.mesh().vertices.len(),
        "metrics":metrics.summary(),"frames":writes,"memory_final":memory(),
        "scope":"Isolated CPU preparation; excludes display refresh, allocation destruction, queue staging and GPU completion. Not an end-to-end upload timing."});
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

/// Break down hidden-surface release preparation without timing reference checks.
#[test]
#[ignore = "manual release diagnosis; excluded from routine GPU CI"]
fn gpu_load_profile_release_display() {
    let output =
        std::env::var("CLAY_RELEASE_PROFILE").expect("set CLAY_RELEASE_PROFILE output path");
    let mode = std::env::var("CLAY_RELEASE_MODE").unwrap_or_else(|_| "cached".into());
    assert!(matches!(mode.as_str(), "full" | "cached"));
    let mut frames = Vec::new();
    let mut doc = document(fixture("hidden", 3));
    let before = fingerprint(doc.mesh());
    let mut metrics = Measurements::default();
    for trial in 0..3 {
        for kind in [Kind::Standard, Kind::Clay, Kind::Paint] {
            let mut display = DisplayMesh::new(&doc);
            for step in 0..DABS {
                assert!(doc.apply_dab(&brush(kind), dab(step)));
                display = display.during_stroke(&doc).unwrap();
            }
            doc.end_stroke();
            for (phase, elapsed) in display.profile_after_stroke(&doc, mode == "cached") {
                metrics
                    .values
                    .entry(format!("{kind:?}.{phase}"))
                    .or_default()
                    .push(elapsed);
            }
            let (bytes, unchanged) = display.release_cache_stats(&doc);
            frames.push(serde_json::json!({"trial":trial,"brush":format!("{kind:?}"),"angle_cache_payload_bytes":bytes,"unchanged_faces":unchanged,"faces":doc.mesh().triangles.len()}));
            doc.undo();
            assert_eq!(fingerprint(doc.mesh()), before);
        }
    }
    let report = serde_json::json!({"mode":mode,"frames":frames,"fixture":"hidden","level":3,"repeats":3,"dabs_per_stroke":DABS,
        "fixture_hash_fnv1a":before,"vertices":doc.mesh().vertices.len(),"metrics":metrics.summary(),"memory_final":memory(),
        "scope":"Isolated CPU release stages; excludes renderer bookkeeping, allocation destruction and GPU work. Reference checks are outside timers."});
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

#[test]
#[ignore = "manual hardware release-upload diagnosis; excluded from routine GPU CI"]
fn gpu_load_profile_release_upload() {
    let output = std::env::var("CLAY_UPLOAD_PROFILE").expect("set CLAY_UPLOAD_PROFILE output path");
    let headless = Headless::new();
    let mut renderer = Renderer::new(&headless.state).unwrap();
    let mut doc = document(fixture("hidden", 3));
    let before = fingerprint(doc.mesh());
    let mut metrics = Measurements::default();
    let mut frames = Vec::new();
    for trial in 0..3 {
        for kind in [Kind::Standard, Kind::Clay, Kind::Paint] {
            let mut live = renderer.geometry(&doc);
            renderer.gpu.lock().unwrap().upload(&live);
            headless.state.queue.submit([]);
            headless.wait();
            for step in 0..DABS {
                doc.begin_sculpt_frame();
                assert!(doc.apply_dab(&brush(kind), dab(step)));
                live = renderer.geometry(&doc);
                renderer.gpu.lock().unwrap().upload(&live);
                headless.state.queue.submit([]);
                headless.wait();
            }
            doc.end_stroke();
            let display = renderer.geometry(&doc);
            let mut gpu = renderer.gpu.lock().unwrap();
            let before_vertices = gpu.packed_vertices.len() / gpu.vertex_stride;
            gpu.upload_trace.start();
            metrics.time(&format!("{kind:?}.cpu_upload"), || gpu.upload(&display));
            for (stage, elapsed) in gpu.upload_trace.stop() {
                metrics
                    .values
                    .entry(format!("{kind:?}.{stage}"))
                    .or_default()
                    .push(elapsed);
            }
            metrics.time(&format!("{kind:?}.submit_and_wait"), || {
                headless.state.queue.submit([]);
                headless.wait();
            });
            frames.push(serde_json::json!({"trial":trial,"brush":format!("{kind:?}"),"before_vertices":before_vertices,"after_vertices":display.shading.vertices.len(),"stride":gpu.vertex_stride,"vertex_write_bytes":gpu.vertex_write_bytes,"face_write_bytes":gpu.face_write_bytes,"packed_vertices":gpu.vertex_pack_count}));
            drop(gpu);
            std::hint::black_box(&live);
            doc.undo();
            assert_eq!(fingerprint(doc.mesh()), before);
        }
    }
    let report = serde_json::json!({"fixture":"hidden","level":3,"repeats":3,"dabs_per_stroke":DABS,
        "fixture_hash_fnv1a":before,"vertices":doc.mesh().vertices.len(),"metrics":metrics.summary(),"frames":frames,
        "memory_final":memory(),"gpu":format!("{:?}",headless.state.adapter.get_info()),
        "scope":"Actual upload path with opt-in CPU checkpoints and separate queue submit/completion. Excludes display construction, drawing and orbit workload."});
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

#[test]
#[ignore = "manual paint startup diagnosis; excluded from routine GPU CI"]
fn gpu_load_profile_paint_startup() {
    let output = std::env::var("CLAY_PAINT_PROFILE").expect("output path");
    let level = env_limit("CLAY_PAINT_LEVEL", 3);
    crate::sculpt::capacity::initialize(benchmark_capacity(level, 2_000_000, 4_000_000).unwrap())
        .unwrap();
    let mut rows = Vec::new();
    for name in ["sphere", "organic", "hidden"] {
        let mut doc = document(fixture(name, level));
        let before = fingerprint(doc.mesh());
        let mut expected = None;
        for trial in 0..3 {
            for step in 0..4 {
                let start = Instant::now();
                let (changed, stages) = crate::sculpt::texture::profile::capture(|| {
                    doc.apply_dab(&brush(Kind::Paint), dab(step))
                });
                let total = start.elapsed().as_secs_f64() * 1000.;
                assert!(changed);
                rows.push(serde_json::json!({"fixture":name,"trial":trial,"step":step,"source_triangles":doc.mesh().triangles.len(),"total_ms":total,"stages_ms":stages}));
            }
            doc.end_stroke();
            let after = fingerprint(doc.mesh());
            if let Some(expected) = &expected {
                assert_eq!(&after, expected);
            }
            expected = Some(after.clone());
            doc.undo();
            assert_eq!(fingerprint(doc.mesh()), before);
            doc.redo();
            assert_eq!(fingerprint(doc.mesh()), after);
            doc.undo();
            rows.push(serde_json::json!({"fixture":name,"trial":trial,"phase":"verified","output_hash_fnv1a":after}));
        }
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"level":level,"rows":rows,"memory_final":memory()}),
        )
        .unwrap(),
    )
    .unwrap();
}

/// Actual display code, with phase tracing disabled outside this probe.
#[test]
#[ignore = "manual display phase diagnosis; excluded from routine GPU CI"]
fn gpu_load_profile_display_stages() {
    let output = std::env::var("CLAY_DISPLAY_PROFILE").expect("output path");
    let level = env_limit("CLAY_DISPLAY_LEVEL", 3);
    let capacity = benchmark_capacity(level, 2_000_000, 4_000_000).unwrap();
    crate::sculpt::capacity::initialize(capacity).unwrap();
    let mut rows = Vec::new();
    for name in ["sphere", "organic", "hidden"] {
        let mut doc = document(fixture(name, level));
        for trial in 0..3 {
            for kind in [Kind::Standard, Kind::Paint] {
                let mut display = Arc::new(DisplayMesh::new(&doc));
                for step in 0..4 {
                    assert!(doc.apply_dab(&brush(kind), dab(step)));
                    let start = Instant::now();
                    let (next, stages) = super::super::display::stroke_profile::capture(|| {
                        display.during_stroke(&doc).unwrap()
                    });
                    let total = start.elapsed().as_secs_f64() * 1000.;
                    display = Arc::new(next);
                    rows.push(serde_json::json!({"fixture":name,"trial":trial,"brush":format!("{kind:?}"),"phase":"live","step":step,"total_ms":total,"stages_ms":stages}));
                }
                doc.end_stroke();
                let start = Instant::now();
                let (next, stages) = super::super::display::stroke_profile::capture(|| {
                    finish_display(&display, &doc, true)
                });
                let total = start.elapsed().as_secs_f64() * 1000.;
                // Reference work is outside the timers and after the live snapshot is dropped.
                drop(display);
                let canonical = DisplayMesh::new(&doc);
                assert_eq!(
                    next.vertices(next.vertex_stride()),
                    canonical.vertices(canonical.vertex_stride())
                );
                assert_eq!(next.shading.triangles, canonical.shading.triangles);
                rows.push(serde_json::json!({"fixture":name,"trial":trial,"brush":format!("{kind:?}"),"phase":"release","total_ms":total,"stages_ms":stages,"output_hash_fnv1a":fingerprint(doc.mesh())}));
                drop(canonical);
                drop(next);
                doc.undo();
            }
        }
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"level":level,"rows":rows,"memory_final":memory()}),
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "manual camera/picking diagnosis; excluded from routine GPU CI"]
fn gpu_load_profile_camera_picking() {
    let output = std::env::var("CLAY_CAMERA_PROFILE").expect("output path");
    let level = env_limit("CLAY_CAMERA_LEVEL", 3);
    let stages = std::env::var_os("CLAY_SCENE_STAGES").is_some();
    let selected = std::env::var("CLAY_CAMERA_FIXTURE").ok();
    assert!(
        selected
            .as_deref()
            .is_none_or(|s| matches!(s, "sphere" | "organic" | "hidden"))
    );
    let cursor = std::env::var("CLAY_CAMERA_CURSOR").unwrap_or_else(|_| "footprint".into());
    assert!(matches!(cursor.as_str(), "footprint" | "rings" | "none"));
    let capacity = benchmark_capacity(level, 2_000_000, 4_000_000).unwrap();
    crate::sculpt::capacity::initialize(capacity).unwrap();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let mut rows = Vec::new();
    let context = egui::Context::default();
    let brush = brush(Kind::Standard);
    for name in ["sphere", "organic", "hidden"] {
        if selected.as_deref().is_some_and(|s| s != name) {
            continue;
        }
        let doc = document(fixture(name, level));
        let display = Arc::new(DisplayMesh::new(&doc));
        for perspective in [false, true] {
            let mut metrics = Measurements::default();
            let mut face_counts = Vec::new();
            for frame in 0..12 {
                let camera = Camera {
                    yaw: frame as f32 * 0.025,
                    perspective,
                    ..Default::default()
                };
                let mut build = || {
                    metrics.time("scene_projection", || {
                        Scene::from_geometry(rect, &camera, display.clone(), false)
                    })
                };
                let scene = if stages {
                    let (scene, times) = crate::viewport::scene::profile::capture(build);
                    for (stage, ms) in times {
                        metrics
                            .values
                            .entry(format!("scene.{stage}"))
                            .or_default()
                            .push(ms);
                    }
                    scene
                } else {
                    build()
                };
                face_counts.push(scene.active_faces().len());
                // Measure actual production picking. Build geometry outside the
                // timers, vary sample locations, and keep assertions outside.
                for offset in [-15., 0., 15.] {
                    let point = rect.center() + egui::vec2(offset, 4.);
                    let hit = metrics.time("pick_hit", || std::hint::black_box(scene.pick(point)));
                    assert!(hit.is_some());
                    assert!(scene.project(hit.unwrap().0).distance(point) < 0.002);
                    let miss = metrics.time("pick_miss", || {
                        std::hint::black_box(scene.pick(point + egui::vec2(2000., 2000.)))
                    });
                    assert!(miss.is_none());
                }
                if cursor == "none" {
                    continue;
                }
                let mut hit = false;
                let painted = metrics.time(
                    if cursor == "footprint" {
                        "cursor_footprint"
                    } else {
                        "cursor_rings"
                    },
                    || {
                        context.run(
                            egui::RawInput {
                                screen_rect: Some(rect),
                                ..Default::default()
                            },
                            |ctx| {
                                hit = scene.brush_cursor_with_footprint(
                                    &ctx.layer_painter(egui::LayerId::background()),
                                    rect.center(),
                                    50.,
                                    &brush,
                                    egui::Color32::WHITE,
                                    cursor == "footprint",
                                );
                            },
                        )
                    },
                );
                assert!(hit && !painted.shapes.is_empty());
            }
            rows.push(serde_json::json!({
                "fixture":name, "perspective":perspective,
                "source_triangles":doc.mesh().triangles.len(),
                "display_triangles":display.mesh.triangles.len(),
                "candidate_faces":face_counts, "metrics":metrics.summary(),
            }));
        }
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&serde_json::json!({
            "level":level, "viewport":[800,600], "rows":rows, "memory_final":memory(),
            "scene_stages":stages, "cursor":cursor, "fixture":selected,
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "manual prepared-scene reuse diagnosis; excluded from routine GPU CI"]
fn gpu_load_profile_scene_reuse() {
    use crate::viewport::scene::cache::Cache;
    let output = std::env::var("CLAY_REUSE_PROFILE").expect("output path");
    let trace_stages = std::env::var_os("CLAY_SCENE_STAGES").is_some();
    let level = env_limit("CLAY_CAMERA_LEVEL", 3);
    crate::sculpt::capacity::initialize(benchmark_capacity(level, 2_000_000, 4_000_000).unwrap())
        .unwrap();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let bits = |hit: Option<(Vec3, Vec3)>| {
        hit.map(|(p, n)| [p.0, p.1, p.2, n.0, n.1, n.2].map(f32::to_bits))
    };
    let mut rows = Vec::new();
    for name in ["sphere", "organic", "hidden"] {
        let doc = document(fixture(name, level));
        let display = Arc::new(DisplayMesh::new(&doc));
        for perspective in [false, true] {
            for moving in [false, true] {
                let mut cache = Cache::default();
                let mut previous: Option<Scene> = None;
                let mut metrics = Measurements::default();
                for frame in 0..12 {
                    let camera = Camera {
                        perspective,
                        yaw: if moving { frame as f32 * 0.025 } else { 0. },
                        ..Default::default()
                    };
                    let fresh = metrics.time("fresh_scene", || {
                        Scene::from_geometry(rect, &camera, display.clone(), false)
                    });
                    let cached = if trace_stages {
                        let (scene, stages) = crate::viewport::scene::profile::capture(|| {
                            metrics.time("cached_scene", || {
                                cache.get(rect, &camera, display.clone(), false)
                            })
                        });
                        for (name, elapsed) in stages {
                            metrics
                                .values
                                .entry(format!("cached.{name}"))
                                .or_default()
                                .push(elapsed);
                        }
                        scene
                    } else {
                        metrics.time("cached_scene", || {
                            cache.get(rect, &camera, display.clone(), false)
                        })
                    };
                    if let Some(previous) = &previous {
                        assert_eq!(cached.shares_preparation(previous), !moving);
                    }
                    let point = rect.center() + egui::vec2((frame % 3) as f32 * 5., 4.);
                    let expected = metrics.time("fresh_pick", || fresh.pick(point));
                    let actual = metrics.time("cached_pick", || cached.pick(point));
                    assert!(actual.is_some());
                    assert_eq!(bits(actual), bits(expected));
                    let repeated = metrics.time("repeat_pick", || cached.pick(point));
                    assert_eq!(bits(repeated), bits(expected));
                    previous = Some(cached);
                }
                rows.push(serde_json::json!({"fixture":name,"perspective":perspective,"moving":moving,"source_triangles":doc.mesh().triangles.len(),"display_triangles":display.mesh.triangles.len(),"metrics":metrics.summary()}));
            }
        }
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"level":level,"rows":rows,"memory_final":memory()}),
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "manual dense drawing/preparation profile; excluded from routine GPU CI"]
fn gpu_load_profile_draw() {
    let output = std::path::PathBuf::from(std::env::var("CLAY_DRAW_PROFILE").expect("output path"));
    let level = env_limit("CLAY_CAMERA_LEVEL", 3);
    crate::sculpt::capacity::initialize(benchmark_capacity(level, 2_000_000, 4_000_000).unwrap())
        .unwrap();
    let headless = Headless::new();
    let driver = format!("{:?}", headless.state.adapter.get_info());
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let context = egui::Context::default();
    let mut rows = Vec::new();
    for name in ["sphere", "organic", "hidden"] {
        let doc = document(fixture(name, level));
        let mut renderer = Renderer::new(&headless.state).unwrap();
        let display = renderer.geometry(&doc);
        renderer.gpu.lock().unwrap().upload(&display);
        headless.state.queue.submit([]);
        headless.wait();
        for perspective in [false, true] {
            let mut metrics = Measurements::default();
            let mut last_params = None;
            for frame in 0..15 {
                let camera = Camera {
                    perspective,
                    yaw: frame as f32 * 0.025,
                    ..Default::default()
                };
                // Measure the UI preparation with the current view already prepared.
                // Cold/orbit scene construction has a separate CPU diagnostic.
                let scene = renderer.prepare_scene(rect, &camera, display.clone(), false);
                let mut prepare = || {
                    context.run(
                        egui::RawInput {
                            screen_rect: Some(rect),
                            ..Default::default()
                        },
                        |ctx| {
                            Scene::paint_with_renderer(
                                &ctx.layer_painter(egui::LayerId::background()),
                                rect,
                                &doc,
                                &camera,
                                false,
                                WireMode::Visible,
                                false,
                                Some(&mut renderer),
                            );
                        },
                    )
                };
                let prepared = if frame < 3 {
                    prepare()
                } else {
                    metrics.time("paint_prepare", prepare)
                };
                assert!(!prepared.shapes.is_empty());
                drop(prepared);
                let mut params = Params::new(
                    rect,
                    scene.right,
                    scene.up,
                    scene.view,
                    scene.scale,
                    scene.center,
                    display.depth_extent,
                );
                params.set_perspective(scene.eye_distance, display.depth_extent);
                let mut gpu = renderer.gpu.lock().unwrap();
                let mut draw = || {
                    gpu.paint(
                        &params,
                        [800, 600],
                        false,
                        WireMode::Visible,
                        false,
                        &Preview::default(),
                    );
                    headless.wait();
                };
                if frame < 3 {
                    draw();
                } else {
                    metrics.time("draw_and_wait", draw);
                }
                last_params = Some(params);
            }
            let mut gpu = renderer.gpu.lock().unwrap();
            let pixels = gpu.read();
            // Compare both pipelines in the same process, outside timers.
            // Repeated renders must be stable; dense triangles can differ at
            // an 8-bit rounding boundary after shared-vertex interpolation.
            let params = last_params.unwrap();
            let mut render = |general| {
                gpu.force_general_surface = general;
                gpu.paint(
                    &params,
                    [800, 600],
                    false,
                    WireMode::Visible,
                    false,
                    &Preview::default(),
                );
                gpu.read()
            };
            assert_eq!(pixels, render(false), "indexed renders must be stable");
            let reference = render(true);
            assert_eq!(reference, render(true), "reference renders must be stable");
            gpu.force_general_surface = false;
            let mut differing_pixels = 0;
            let mut max_channel_delta = 0;
            for (actual, expected) in pixels
                .as_chunks::<4>()
                .0
                .iter()
                .zip(reference.as_chunks::<4>().0)
            {
                assert_eq!(actual[3], expected[3], "alpha must match");
                assert_eq!(
                    actual[..3] == params.background,
                    expected[..3] == params.background,
                    "surface coverage must match"
                );
                differing_pixels += usize::from(actual != expected);
                for (&a, &b) in actual.iter().zip(expected) {
                    max_channel_delta = max_channel_delta.max(a.abs_diff(b));
                }
            }
            assert!(
                max_channel_delta <= 1,
                "shading differs beyond 8-bit rounding"
            );
            let capture = format!(
                "{name}-{}.png",
                if perspective {
                    "perspective"
                } else {
                    "orthographic"
                }
            );
            image::save_buffer(
                output.parent().unwrap().join(&capture),
                &pixels,
                800,
                600,
                image::ColorType::Rgba8,
            )
            .unwrap();
            let reference_capture = capture.replace(".png", "-general.png");
            image::save_buffer(
                output.parent().unwrap().join(&reference_capture),
                &reference,
                800,
                600,
                image::ColorType::Rgba8,
            )
            .unwrap();
            rows.push(serde_json::json!({"fixture":name,"perspective":perspective,"source_triangles":doc.mesh().triangles.len(),"display_triangles":display.mesh.triangles.len(),"capture":capture,"reference_capture":reference_capture,"reference_differing_pixels":differing_pixels,"reference_max_channel_delta":max_channel_delta,"metrics":metrics.summary()}));
        }
        renderer.destroy();
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"level":level,"driver":driver,"rows":rows,"memory_final":memory()}),
        )
        .unwrap(),
    )
    .unwrap();
}

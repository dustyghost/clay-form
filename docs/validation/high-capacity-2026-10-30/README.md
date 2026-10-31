# Editing at 2.4 million triangles

Follow-up to [local brush normals](../local-brush-normals-2026-10-30/README.md), under #103.

## Result and scope

**All six GPU workloads completed**: sphere, attributed organic and partially hidden organic fixtures at 602,112 and 2,408,448 triangles, with three repetitions each. Each repetition includes Standard, Clay and vertex Paint strokes, live display preparation/uploads, orbit/picking, and exact single-entry Undo/Redo restoration. The larger fixture has 1,204,226 vertices and 1,204,224 quad polygons, with 1,604,074 vertices across five retained subdivision levels.

The runs demonstrate successful editing at this size on the AC-powered i7-7500U / Intel HD 620 laptop. Display preparation, release and drawing still impose substantial delays. These are synthetic, single-model, fixed-topology workloads at **800×600 offscreen**. They establish measured operation costs; they do not establish comfortable desktop interaction, full input-to-present latency, or multi-million capacity on other machines.

Changes in this batch are confined to benchmark and test code. Benchmark processes initialize the same supported capacity mechanism used by Settings, with explicit budgets of 2,000,000 vertices / 4,000,000 triangles. Installed preferences are not written. The workload schema is now **2**, recording fixture construction, explicit budgets/workload mode, and atomic progress checkpoints outside timing windows. Previous schema-1 timing sets are kept as historical evidence; the automatic comparator rejects mismatched schemas and modes.

## Sustained sculpting

Each row uses 33 sustained samples. Brush, display and upload columns are p50 milliseconds; the combined p50 / p95 sums matching samples before calculating percentiles. Upload includes CPU packing, submission and completion wait. These sums exclude drawing, event handling, OS presentation and checkpoint I/O.

| Fixture | Brush | 602k combined p50 / p95 | 2.4m brush | 2.4m display | 2.4m upload | 2.4m combined p50 / p95 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Sphere | Standard | 34.6 / 36.0 | 28.5 | 110.9 | 30.3 | 169.3 / 199.7 |
| Sphere | Clay | 39.7 / 47.3 | 47.3 | 105.4 | 28.7 | 184.1 / 205.1 |
| Sphere | Paint | 31.3 / 32.6 | 18.6 | 140.0 | 63.6 | 222.0 / 239.3 |
| Organic | Standard | 46.9 / 52.4 | 28.0 | 151.8 | 59.7 | 242.5 / 291.1 |
| Organic | Clay | 50.5 / 62.5 | 46.6 | 137.3 | 54.0 | 243.6 / 276.7 |
| Organic | Paint | 30.4 / 31.7 | 15.8 | 120.6 | 62.7 | 199.6 / 233.5 |
| Hidden | Standard | 45.0 / 47.3 | 31.3 | 119.9 | 51.2 | 202.3 / 246.4 |
| Hidden | Clay | 48.4 / 58.1 | 45.8 | 118.5 | 50.2 | 215.7 / 238.4 |
| Hidden | Paint | 27.4 / 29.2 | 10.3 | 93.7 | 54.6 | 160.7 / 181.7 |

## Stroke release and initial work

Release preparation and upload medians, milliseconds. Each has only three samples, so their p95 is the maximum observed sample rather than a stable tail estimate.

| Fixture | Brush | 602k release display | 2.4m release display | 2.4m release upload |
| --- | --- | ---: | ---: | ---: |
| Sphere | Standard | 187.3 | 908.1 | 200.6 |
| Sphere | Clay | 184.6 | 903.8 | 197.8 |
| Sphere | Paint | 198.3 | 991.4 | 67.7 |
| Organic | Standard | 195.5 | 1031.6 | 145.3 |
| Organic | Clay | 189.8 | 963.3 | 145.0 |
| Organic | Paint | 191.9 | 1029.1 | 70.2 |
| Hidden | Standard | 53.9 | 304.3 | 169.6 |
| Hidden | Clay | 54.8 | 277.0 | 167.2 |
| Hidden | Paint | 8.2 | 31.9 | 0.4 |

| Fixture | 2.4m construction (ms) | Document install/validation (ms) | First Standard dab p50 (ms) | First Paint dab p50 (ms) |
| --- | ---: | ---: | ---: | ---: |
| Sphere | 3998 | 9336 | 65.6 | 1459.5 |
| Organic | 4047 | 10043 | 214.8 | 1391.2 |
| Hidden | 3968 | 9996 | 142.2 | 855.2 |

Construction is one sample per process and includes all subdivisions and fixture attributes. Document installation excludes file reading and fixture generation. First dabs include the history snapshot; the unpainted sphere additionally exercises initial material creation when Paint first touches it.

## Orbit, picking and memory

There are 108 orbit samples per case, pooled across post-stroke shapes. Projection prepares CPU positions and pick faces. GPU drawing includes encoding, submission and completion after warm-up. Display resolution is 800×600.

| Fixture | Triangles | Projection p50 / p95 (ms) | Pick p50 / p95 (ms) | GPU draw p50 / p95 (ms) | Peak RSS (MiB) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Sphere | 602,112 | 20.3 / 35.7 | 2.7 / 3.9 | 36.4 / 40.5 | 682.1 |
| Sphere | 2,408,448 | 77.1 / 86.5 | 6.7 / 7.8 | 80.4 / 83.3 | 2258.1 |
| Organic | 602,112 | 19.8 / 33.3 | 2.7 / 3.9 | 35.5 / 39.9 | 756.7 |
| Organic | 2,408,448 | 77.2 / 88.7 | 6.7 / 7.7 | 80.1 / 81.6 | 2388.4 |
| Hidden | 602,112 | 14.8 / 29.7 | 2.7 / 4.1 | 29.6 / 33.5 | 827.7 |
| Hidden | 2,408,448 | 58.9 / 69.1 | 6.7 / 7.8 | 55.0 / 56.5 | 2654.1 |

| 2.4m fixture | Visible triangles | Estimated retained history, one stroke (MiB) | Lowest available system RAM (MiB) |
| --- | ---: | ---: | ---: |
| Sphere | 2,408,448 | 177.7 | 1409 |
| Organic | 2,408,448 | 269.6 | 1274 |
| Hidden | 1,448,235 | 271.9 | 1211 |

Every case stayed within the **3072 MiB process RSS / 600 second guard**, with a **768 MiB system reserve**, and the monitor observed no swap use by the benchmark processes. Whole-process RSS includes an extra baseline geometry snapshot used for correctness checks, history, renderer data, verification and allocator retention. Driver allocations may not all appear in RSS. Single-entry Undo/Redo is verified; a long sculpting session with many retained edits is outside this workload.

The hidden fixture retains about 60% of the organic fixture’s triangles for display. At 2.4m total triangles, this reduces median GPU draw time from 80.1 to 55.0 ms and Standard release preparation from 1031.6 to 304.3 ms. Its whole-process peak is higher (2654 versus 2388 MiB), so the retained visibility/topology cache has a memory tradeoff.

## Persistence smoke check

A separate full CPU workload completed at 2.4 million triangles, including exact native-project save/reload equality for mesh and subdivision levels. The single saved file was 187.7 MiB; save took 10.01 s and load/parse/validation took 10.91 s. Peak RSS was 1996.9 MiB. These are one-off, warm-filesystem measurements.

## Code analysis and next targets

1. **Live display preparation.** `DisplayMesh::during_stroke` copies mesh/topology/material arrays, shading, ownership and source-face data for each update, and deformation recalculates temporary display normals across the mesh. The new brush-local normals optimization applies to sculpt proposals; these display loops remain a separate measured cost. Split copying, topology checks and display-normal work in a focused trace before choosing the next change.
2. **Release shading.** Visible meshes fall through to canonical display rebuilding, while the hidden-surface path retains a topology/shading cache. Investigate extending that reuse to visible fixed-topology strokes, preserving creases, fans, visibility and final shading. Measure the additional retained RAM as well as time when extending that cache. The release table shows why this is worth measuring.
3. **Orbit and picking.** `Scene::from_geometry` projects all vertices and scans face orientation per camera update; picking scans candidate faces. A spatial query structure and selective projection work are candidates after measuring construction/invalidation costs. GPU draw time is also substantial on this integrated adapter; CPU loop improvements alone cannot remove that cost.

No production optimization is claimed by this batch. [Scaling data](scaling.csv), [paired per-dab timings](paired-timings.csv), and [raw GPU results](gpu-matrix/) retain all measured phases.

## Validation and provenance

**Five focused Rust tests and seven Python tests passed.** They cover the original workload/history/display contracts, level-4 budget boundaries, durable checkpoints after a failed phase, comparison compatibility, and guard termination with retained memory telemetry. [Logs](checks/) are retained. Every large workload asserts real edits, deterministic repeated outputs and exact Undo/Redo. The 602k input/output hashes match the previous normal-optimization workload, and the large CPU/GPU/full probes match the large matrix hashes.

Production parent: `62d2648`. Combined benchmark source SHA-256: `024617c3c5395997a6a013d2bc37ac9fea7147d180ae11ed8d88f3e1cb25bbad`. Final combined source SHA-256: `accfb741d35470f188ebfe9b1538a1e49c8fc594682ff42d84fcd61601dffce2`. Final review hardened the Python guard so a recorded budget violation remains a failure even if the child exits successfully just as it is being stopped; a mocked race regression passes. The measured runs never reached that branch. [Measurement-source patch](measurement-source.patch) reconstructs the driver used for the timings from final source; Rust benchmark/application code is identical. Optimized library tests were built once, with subsequent runs reusing the matching Cargo artifact. All workloads ran sequentially on the docked, AC-powered i7-7500U with Intel HD 620 / Vulkan, about 7.63 GiB physical RAM and `powersave` / `balance_performance`. [Power settings](power.json) and per-run environment/adapter metadata are retained.

The runner checks that the requested RSS budget plus the system reserve is available before starting each process, stops the batch on a failure, and records sampled peak RSS/swap, minimum available memory, elapsed time and the last durable checkpoint. Checkpoint JSON is atomically replaced outside timing windows; it preserves completed samples if the next phase is killed or fails. The final JSON is required before a case is reported as measured.

Formatting and diff whitespace checks passed. Full-suite/lint/CI remains for the eventual PR; no PR was opened.

## Reproduce

```sh
python3 scripts/test-benchmark-capacity.py
cargo test --release --locked --lib viewport::gpu::profile::capacity::capacity_ -- --test-threads=1
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/capacity-large-gpu --fixtures sphere organic hidden --levels 3 4 --repeats 3 --gpu --workload interactive --mesh-vertices 2000000 --mesh-triangles 4000000 --max-rss-mib 3072 --min-available-mib 768
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/capacity-large-full --fixtures sphere --levels 4 --repeats 1 --workload full --mesh-vertices 2000000 --mesh-triangles 4000000 --max-rss-mib 3072 --min-available-mib 768
```

`interactive` omits file I/O and extra growth after the requested fixture is built. `full` includes save/reload and retains the original extra-subdivision measurement at levels 0–2. Default runs still select levels 0–3 and default mesh budgets; level 4 requires explicit larger budgets. Use fresh output directories. For future before/after optimization comparisons, use identical schema, budgets, modes, fixtures, levels, sample counts and hardware with the existing `compare` command.

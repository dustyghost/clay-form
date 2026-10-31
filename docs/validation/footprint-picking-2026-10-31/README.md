# Brush-footprint sample candidates

Follow-up to [shared display arrays and camera diagnosis](../display-arrays-2026-10-30/README.md), under #103.

The follow-up [scene-allocation investigation and fix](../scene-preparation-2026-10-31/README.md) resolves the hidden orthographic preparation increase recorded below.

## Change

The optional brush footprint samples the surface at 17×17 screen points. Previously, every point scanned the same coarse list of triangles intersecting the whole footprint. A temporary lookup now assigns each triangle only to samples within its projected bounds. Each sample uses the existing exact triangle-intersection routine and keeps the original traversal order, closest-hit rules, normals and brush weights. The centre pick still uses the ordinary picker.

The lookup is built only when the footprint is enabled and its centre hits the active model. It is discarded after drawing preparation, so there is no persistent cache to invalidate on geometry, visibility, model or camera changes. Perspective bounds allow for projection rounding; triangles crossing the near plane remain conservative candidates. Grid comparisons use the actual sample coordinates, including repeated coordinates at tiny radii.

The index is limited to 65,536 face references: 512 KiB of live IDs on this 64-bit machine, plus vector capacity and headers. Overlapping sheets or wide triangles can exhaust that budget; those cases use a coarse candidate scan. The cap bounds index duplication, not total scene or fallback storage. Hidden faces are excluded by the display surface, while visible inactive models still occlude the active model.

## Measurements

The footprint took **62–73% less time** across these paired fixtures and camera modes, including lookup construction.

### 602,112 source triangles

| Fixture | Camera | Before p50 / p95 (ms) | After p50 / p95 (ms) | p50 reduction |
| --- | --- | --- | --- | --- |
| Sphere | Orthographic | 19.4 / 20.5 | 7.2 / 9.4 | 63% |
| Sphere | Perspective | 23.5 / 24.2 | 6.7 / 7.8 | 71% |
| Organic | Orthographic | 19.4 / 21.9 | 6.9 / 7.6 | 64% |
| Organic | Perspective | 23.7 / 24.7 | 6.3 / 7.2 | 73% |
| Hidden | Orthographic | 19.3 / 20.5 | 7.0 / 11.1 | 64% |
| Hidden | Perspective | 23.6 / 24.2 | 6.7 / 11.9 | 72% |

### 2,408,448 source triangles

| Fixture | Camera | Before p50 / p95 (ms) | After p50 / p95 (ms) | p50 reduction |
| --- | --- | --- | --- | --- |
| Sphere | Orthographic | 75.6 / 78.6 | 28.2 / 38.3 | 63% |
| Sphere | Perspective | 88.5 / 90.3 | 25.8 / 34.1 | 71% |
| Organic | Orthographic | 77.1 / 102.8 | 28.2 / 29.2 | 63% |
| Organic | Perspective | 91.4 / 93.5 | 26.0 / 27.4 | 72% |
| Hidden | Orthographic | 75.5 / 79.0 | 28.8 / 29.2 | 62% |
| Hidden | Perspective | 93.0 / 128.1 | 25.4 / 27.3 | 73% |

Timings cover the complete optional footprint operation, including the centre pick, lookup construction, all 289 sample picks, overlay/ring construction and egui frame preparation. Display geometry and scene construction are outside this timer. GPU drawing and desktop presentation are excluded: these are operation timings, not application frame rates.

Both versions run the same existing CPU diagnostic at 800×600 with a 50-pixel footprint, 12 camera orbit angles per camera mode, 36 hit picks and 36 off-model misses per row. Source/display/candidate face counts and sample counts match across all 12 before/after rows. The 602k and 2.4m labels refer to 602,112 and 2,408,448 source triangles; the hidden 2.4m fixture displays 1,448,235 triangles. All samples and controls are retained in the profiles and [comparison CSV](comparison.csv). With only 12 footprint samples per row, p95 is the observed maximum, not a precise tail-latency estimate.

Ordinary hit/miss picking p50 stayed within about −3% to +3% at 2.4m source triangles. Most scene-projection controls also stayed close. **Hidden orthographic scene preparation regressed in this interleaved workload:** 37.7 → 47.9 ms initially and 38.5 → 44.9 ms in a fresh paired repeat. The scene constructor was not edited, but the cause of this repeatable 6–10 ms increase has not been isolated; it is retained as a limitation rather than dismissed as noise.

The repeat confirms footprint p50 at 25.1–28.9 ms versus 76.3–93.0 ms before. For the affected hidden orthographic row, adding each frame's scene and footprint samples before calculating percentiles still gives a **33–36% reduction**:

| Hidden orthographic, 2.4m source | Combined before p50 / p95 (ms) | Combined after p50 / p95 (ms) |
| --- | --- | --- |
| Initial pair | 113.9 / 122.9 | 76.1 / 80.3 |
| Repeat pair | 114.4 / 140.5 | 72.9 / 80.9 |

The combined table uses the profiler’s nearest-rank percentile convention (the sixth sorted sample for p50 of 12); its p50 values were corrected from an upper-middle sample calculation. The 33–36% conclusion is unchanged.

The primary tables retain the original pair. [Repeat comparison](repeat-comparison.csv), [baseline repeat](before-l4-repeat/) and [optimized repeat](after-l4-repeat/) retain all controls and samples. No gain is claimed for footprint-disabled interaction. These measurements also do not establish the performance of fallback-heavy overlapping geometry.

| Probe | Whole-process peak RSS before → after (MiB) |
| --- | --- |
| 602k | 358.9 → 359.8 |
| 2.4m | 1123.2 → 1122.4 |
| 2.4m repeat | 1124.8 → 1123.2 |

Peak memory was effectively unchanged. All primary and repeated probes completed within the 3072 MiB RSS / 768 MiB system reserve / 600-second guards, with no sampled process swap use. The lowest sampled available system RAM was 3038 MiB. Process high-water RSS and periodically sampled guard RSS are distinct measurements, both preserved in the raw files. These short runs do not establish long-session memory behavior.

## Correctness and visual checks

**21 focused CPU tests and four Vulkan tests pass** on the final optimized binary. [Logs](checks/) include the five new candidate tests, eight scene tests, four projection tests, four stroke-preview tests, the new footprint pixel comparison, the existing perspective/alpha/clipping regression, and visible/hidden stroke-cache pixel, picking and history regressions. Vulkan checks used Intel HD Graphics 620.

The new CPU comparisons require identical hit position and normal bits and face IDs against exhaustive picking for every sample. Cases cover orthographic/perspective cameras, pan/zoom/roll, hidden faces, exact projected vertices, tiny radii, near-plane crossings, depth ties, inactive-model occlusion, changed scenes and the real storage fallback. A dense local query also verifies that all sample bins together contain fewer references than one exhaustive face scan.

The new Vulkan regression compares accelerated and exhaustive footprint pixels byte for byte across eight cases: both cameras, whole/hidden spheres and soft/image alphas. Each case must visibly differ from rings alone. The [capture strip](checks/footprint.png) shows these cases left to right in that order, with camera then alpha varying within each visibility group. The cut-surface cases were visually inspected as well as compared automatically.

Visual inspection caught an initial test-fixture error: separate documents with equal revision counters reused the renderer's previous geometry. The test now edits one document and asserts displayed face counts, ensuring the hidden cases render the cut surface. No renderer behavior was changed. An initial 602k run before those test-only changes and a comment correction is retained in [after-l3-initial](after-l3-initial/); the primary tables use the final test binary.

## Provenance and reproduction

Baseline: `3455473`, source SHA-256 `4d262a6d2fe57ec6685aa4f73d9d32c0744fd1305d55eb95a3bdfe3c546ceb59`. Final measured source SHA-256: `59819d8fb5f8ce950aa33d6d1687bf89d00c487efbd26aa5631e9562e4f35bd9`. Final optimized test executable SHA-256: `bd886bba8da9ba23f0d92f14cc7b26cd95a4cbe4ba60a52e335191e302e5f3f2`. The saved baseline executable was verified against the preceding report before edits; [its original provenance](baseline-provenance.json) records its build before that source was committed. Per-run metadata identifies the source used even when the working tree was uncommitted.

Runs were sequential on the same docked, AC-powered i7-7500U with about 7.63 GiB RAM and powersave / balance_performance policy. [Power readings](power.json), executable hashes and environment records are retained. Optimized library tests were used for realistic measurements; no release application build was run. Full-suite/lint/CI remain for the eventual PR.

Build each revision separately, retain its executable and run the diagnostic with a fresh output path (repeat with level 3 for the smaller fixture):

```sh
CLAY_CAMERA_LEVEL=4 CLAY_CAMERA_PROFILE=/tmp/camera-profile.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_camera_picking -- --exact --ignored --test-threads=1
cargo test --release --locked --lib viewport::scene::footprint::tests:: -- --test-threads=1
CLAY_FOOTPRINT_PREVIEW=/tmp/footprint.png cargo test --release --locked --lib gpu_footprint_bins_match_exhaustive_overlay_pixels -- --ignored --test-threads=1
```

Recorded probes used `scripts/benchmark-capacity.py`'s bounded-process helper with the camera diagnostic as its test target, a 3072 MiB RSS limit, 768 MiB minimum available memory and 600-second timeout. Each run includes its guard summary. Ordinary picking and scene projection still scan the mesh and remain separate optimization targets.

# Shared display mesh arrays and camera/picking diagnosis

Follow-up to [local display normals](../local-display-normals-2026-10-30/README.md), under #103.

## Change

Display snapshots now use a dedicated immutable surface payload instead of cloning an editable `Mesh`. Validated triangle and quad indices survive live updates and release. Positions, masks and materials are reused only when their actual values are unchanged. This covers combined edits and skipped display frames without relying on brush kind. Release retains all five arrays while restoring canonical shading.

Position, mask and material comparisons preserve floating-point bits, including signed zero. Material comparisons include every base and layer channel, blend weights and material links; shared immutable layer/link allocations avoid repeated value checks. Changed arrays still receive independent owned copies. Document meshes, history, persistence and sculpt arithmetic are unchanged. Builder vectors are trimmed before being retained in shared snapshots.

At 2,408,448 visible triangles / 1,204,224 quads on this 64-bit machine, indices alone occupy **91.9 MiB**. Eligible display updates no longer allocate/copy that payload. Mask, material and Paint position reuse remove further copies, but comparison scans remain. Allocator retention and initial display/history/GPU allocations mean whole-process peak RAM does not equal the bytes removed per update.

## Paired GPU measurements

Three trials per fixture, 12 dabs per stroke, **1,204,226 vertices / 2,408,448 source triangles**, Intel HD 620 Vulkan, 800×600 offscreen. Each sustained row has 33 samples. Combined figures sum matching brush, display and upload/wait samples before calculating p50/p95. Drawing, camera/input handling, OS presentation and checkpoint I/O are excluded; these are operation timings, not desktop frame rates.

Across these fixtures and brushes, display p50 fell **23–68%** and combined sustained p50 fell **14–50%**. All workload input/output hashes and sample counts match.

| Fixture | Brush | Display p50 before → after (ms) | Combined before p50 / p95 (ms) | Combined after p50 / p95 (ms) |
| --- | --- | --- | --- | --- |
| Sphere | Standard | 56.0 → 22.1 | 104.1 / 138.2 | 69.0 / 96.8 |
| Sphere | Clay | 38.7 → 22.8 | 103.9 / 158.9 | 89.7 / 112.5 |
| Sphere | Paint | 121.4 → 75.3 | 194.6 / 217.4 | 146.9 / 169.1 |
| Organic | Standard | 121.9 → 38.6 | 193.0 / 221.7 | 95.6 / 133.5 |
| Organic | Clay | 65.7 → 38.3 | 163.5 / 212.8 | 114.8 / 137.8 |
| Organic | Paint | 99.9 → 63.0 | 166.2 / 191.1 | 127.5 / 143.2 |
| Hidden | Standard | 62.0 → 37.8 | 135.9 / 183.0 | 99.9 / 127.1 |
| Hidden | Clay | 56.1 → 39.1 | 145.2 / 180.4 | 115.7 / 140.9 |
| Hidden | Paint | 73.6 → 56.4 | 135.1 / 167.6 | 116.1 / 133.7 |

First display and release timings have three samples each. Release still computes canonical shading after deformation; Paint already reused its completed geometry before this change.

| Fixture | Brush | First display before → after (ms) | Release display before → after (ms) |
| --- | --- | --- | --- |
| Sphere | Standard | 57.8 → 41.1 | 260.9 → 216.8 |
| Sphere | Clay | 53.8 → 41.3 | 239.0 → 214.0 |
| Sphere | Paint | 124.2 → 73.9 | 33.4 → 31.7 |
| Organic | Standard | 81.2 → 55.7 | 264.4 → 246.6 |
| Organic | Clay | 80.4 → 56.8 | 253.3 → 188.5 |
| Organic | Paint | 115.3 → 72.8 | 32.3 → 32.0 |
| Hidden | Standard | 76.5 → 56.9 | 271.5 → 264.0 |
| Hidden | Clay | 74.1 → 58.5 | 275.4 → 215.0 |
| Hidden | Paint | 96.1 → 69.1 | 33.3 → 32.2 |

| Fixture | Whole-process peak RSS before → after (MiB) | Initial display before → after (ms) |
| --- | --- | --- |
| Sphere | 2420.4 → 2424.2 | 1048.8 → 1057.8 |
| Organic | 2740.4 → 2787.3 | 1112.7 → 1101.9 |
| Hidden | 2618.1 → 2605.0 | 1498.3 → 1517.1 |

Peak RSS changed by **−13 to +47 MiB**, with a highest observed peak of **2787 MiB**. No general peak-memory saving is claimed. The cause of the small increases is not isolated. Initial preparation varied by about −1% to +1%; this batch targets repeated updates rather than initial mesh construction.

All six primary GPU cases completed within **3072 MiB process RSS / 768 MiB system reserve / 600 seconds per case**, with no sampled process swap use. Lowest observed available system RAM was **997 MiB**. These short, single-model workloads do not establish long-session memory behavior. Raw measurements retain all p95, brush/upload and draw controls.

## Display phase check at 602k triangles

These medians cover 12 live display samples per brush and fixture. The existing `live.mesh_copy` checkpoint now covers payload comparison/reuse and copying changed arrays. Position equality moved into that phase from the normals/bounds phase, so it is not an isolated memory-copy microbenchmark. End-to-end display timings above remain the comparison that matters.

| Fixture | Brush | Payload phase before → after (ms) |
| --- | --- | --- |
| Sphere | Standard | 2.6 → 0.4 |
| Sphere | Paint | 8.8 → 6.7 |
| Organic | Standard | 7.8 → 4.6 |
| Organic | Paint | 7.9 → 6.0 |
| Hidden | Standard | 6.8 → 5.0 |
| Hidden | Paint | 6.9 → 5.9 |

All 18 released document hashes match in the before/after phase probes. Each probe also compares packed release vertices and shading indices against a fresh canonical display, outside the timers.

## Camera and picking: next target

A separate ignored CPU diagnostic measures the production scene constructor, surface picks and optional brush-footprint overlay. Geometry construction is outside the timers. Each camera mode uses 12 orbit angles, 36 hit samples and 36 off-model misses. Hits are checked by projecting them back onto the query point. The footprint uses a 50-pixel radius and includes egui frame/shape preparation; it does not measure GPU drawing. Source/display/candidate face counts and p95 values are retained in the raw profiles. These are current costs, not before/after gains.

| Triangles | Fixture | Camera | Scene p50 (ms) | Hit p50 (ms) | Miss p50 (ms) | Footprint p50 (ms) |
| --- | --- | --- | --- | --- | --- | --- |
| 602k | Sphere | Orthographic | 11.6 | 1.6 | 1.8 | 18.8 |
| 602k | Sphere | Perspective | 9.9 | 2.4 | 2.1 | 23.1 |
| 602k | Organic | Orthographic | 11.1 | 1.6 | 1.7 | 19.7 |
| 602k | Organic | Perspective | 10.2 | 2.4 | 2.1 | 23.5 |
| 602k | Hidden | Orthographic | 10.1 | 1.6 | 1.7 | 19.2 |
| 602k | Hidden | Perspective | 9.4 | 2.4 | 2.1 | 23.0 |
| 2.4m | Sphere | Orthographic | 46.8 | 6.7 | 7.4 | 77.0 |
| 2.4m | Sphere | Perspective | 39.9 | 9.9 | 8.5 | 89.3 |
| 2.4m | Organic | Orthographic | 45.4 | 6.6 | 7.3 | 75.3 |
| 2.4m | Organic | Perspective | 40.9 | 9.8 | 8.4 | 89.3 |
| 2.4m | Hidden | Orthographic | 38.6 | 6.7 | 7.2 | 77.7 |
| 2.4m | Hidden | Perspective | 34.0 | 9.8 | 8.4 | 90.5 |

The isolated CPU diagnostic uses undeformed fixtures without GPU drawing interleaved. Its timings should not be added to or substituted for timings from the GPU workload. In the integrated GPU workload, camera preparation remains about **61–78 ms** and GPU drawing about **55–80 ms** after the change, close to their before values. Those paths still need separate optimization.

Code analysis identifies these remaining loops:

- `Scene::from_geometry` projects every vertex, scans visible display triangles for facing, calculates face depth, derives active bounds and copies the active face list on each construction.
- `pick_surface` scans the front-facing face list for each query. Orthographic picks calculate projected triangle bounds repeatedly; perspective picks perform ray/triangle work without a spatial candidate index.
- The optional brush footprint filters candidates once, then calls the picking routine for a 17×17 grid. A spatial candidate index could serve both individual picks and those repeated samples.

The next implementation candidate is a bounded spatial index for picking, with measurements including its build/update cost. It needs explicit correctness coverage for near-plane crossings, occluding inactive models, hidden faces, perspective/orthographic cameras, stale geometry and closest-hit ordering. Camera reuse can use immutable position/topology identity, but must also account for viewport and camera state. No camera or picking algorithm was changed in this batch.

## Validation and provenance

**55 focused CPU tests and 7 Vulkan tests pass.** The added regressions cover reuse across mixed edits, immutable older snapshots, release sharing, exact float bits, material presence and all material channels/layers/links. Existing topology/visibility invalidation, hidden-boundary normals, packing, camera picking, stamp previews, history, UV textures and GPU cache checks pass. Visible and hidden captures were inspected and are byte-identical to the earlier captures; [capture hashes](checks/capture-comparison.json) and [test logs](checks/) retain the evidence.

Baseline: `e8858e9`, source SHA-256 `370910aba5bd461c1c47842eedef84ca450d79725ad78290d86e1f7eaf70c03e`. Measured optimized source SHA-256: `4d262a6d2fe57ec6685aa4f73d9d32c0744fd1305d55eb95a3bdfe3c546ceb59`. Environment records retain executable hashes. The baseline executable was saved only after verifying its source and binary hashes against the preceding validated report. Optimized library test builds were used; no release application build was run.

An initial after-run was interrupted with exit 143 after its sphere test completed, before its enclosing runner wrote a guard summary. No orphan benchmark process remained. That incomplete run is retained in [gpu-interrupted](gpu-interrupted/) and excluded from the primary comparison. The complete fresh rerun supplies every after value above.

Runs were sequential on the same docked, AC-powered i7-7500U, about 7.63 GiB RAM, powersave / balance_performance. [Power readings](power.json) and per-run metadata are retained. Full-suite/lint/CI remain for the eventual PR; no PR was opened.

## Reproduce

From the baseline and optimized commits, in separate fresh output directories:

```sh
python3 scripts/benchmark-capacity.py run target/display-arrays-run --fixtures sphere organic hidden --levels 4 --repeats 3 --gpu --workload interactive --mesh-vertices 2000000 --mesh-triangles 4000000 --max-rss-mib 3072 --min-available-mib 768
python3 scripts/benchmark-capacity.py compare path/to/before path/to/after
CLAY_DISPLAY_LEVEL=3 CLAY_DISPLAY_PROFILE=/tmp/display-phases.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_display_stages -- --exact --ignored --test-threads=1
CLAY_CAMERA_LEVEL=4 CLAY_CAMERA_PROFILE=/tmp/camera-profile.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_camera_picking -- --exact --ignored --test-threads=1
```

Recorded phase/camera probes used the capacity runner's bounded-process helper with the same RSS/reserve/time limits; see each `guard.json`. Camera level 3 repeats the diagnostic at 602,112 triangles. [Comparison CSV](comparison.csv), [before GPU data](gpu-before/), [after GPU data](gpu-after/), [display phase data](phases-after/profile.json) and camera profiles at [602k](camera-l3/profile.json) / [2.4m](camera-l4/profile.json) retain the samples.

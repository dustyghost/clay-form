# Picking groups and prepared-scene reuse

Follow-up to [scene face-list allocation](../scene-preparation-2026-10-31/README.md), under #103.

## Changes

Ordinary picking keeps the exact triangle intersection routine and face traversal order. A first query uses the existing scan, and identical queries reuse its result. A later distinct query, or a footprint candidate request, builds conservative screen bounds for groups of 128 consecutive front-facing faces. Queries skip groups outside their point/rectangle. Near-plane crossings and nonfinite projections remain conservative; small scenes keep the scan. The index contains one rectangle per group, without duplicating face IDs. Locality affects its effectiveness; scrambled or overlapping faces can still require a full scan.

Footprint candidate construction now uses the same group bounds before its per-sample bins. This removes most of its repeated whole-mesh filtering once the group lookup exists.

The renderer retains one prepared base view and one independent preview view. Reuse requires exact camera/viewport bits, face-order mode, immutable position/triangle/owner identities, active-model counts and depth bounds. Changed inputs rebuild the view. Rebound display payloads supply current materials, UVs, textures and normals; a changed display snapshot resets the remembered hit, while geometric bounds can remain shared. Leaving preview mode releases its cached view. Retained older scenes remain immutable.

## Camera/picking probe at 2,408,448 source triangles

The final before/after probe uses the same 12 camera angles, three hits, three misses and one 50-pixel footprint per angle. The first query remains a scan. In this sequence, the first miss is the distinct follow-up that builds the index. The table separates that cost from warmed queries; a warm-query median alone would hide it.

| Fixture | Camera | First hit after (ms) | Build + first miss (ms) | Warm hit (ms) | Footprint before → after (ms) |
| --- | --- | --- | --- | --- | --- |
| Sphere | Orthographic | 6.70 | 19.99 | 0.020 | 28.1 → 2.1 |
| Sphere | Perspective | 9.89 | 15.36 | 0.028 | 26.3 → 1.4 |
| Organic | Orthographic | 6.65 | 21.37 | 0.025 | 27.9 → 2.3 |
| Organic | Perspective | 9.88 | 16.55 | 0.031 | 26.3 → 1.4 |
| Hidden | Orthographic | 6.55 | 20.01 | 0.024 | 28.7 → 2.3 |
| Hidden | Perspective | 9.84 | 15.31 | 0.027 | 26.1 → 1.4 |

The baseline first-hit medians were 6.4–10.0 ms; final first hits remain 6.5–9.9 ms. Warm hits are 0.020–0.031 ms and warm misses 0.016–0.022 ms, but the first distinct follow-up costs 15–22 ms to build the lookup. This is a tradeoff for reuse, not a claim that every individual query became faster.

Adding scene preparation, all six queries and the footprint per angle gives **39–52% less time**, including index construction: 94.6–116.0 ms before versus 50.0–62.0 ms after. The source/display/candidate counts and sample counts match. Hidden geometry displays 1,448,235 of the 2,408,448 source triangles. [Comparison CSV](comparison.csv), [baseline](picking-before/) and [final measurements](cpu-after/) retain all samples and p95 values.

## Reusing unchanged views

A separate same-binary probe compares fresh scene construction with the production cache. It checks both an unchanged camera and a camera that orbits every frame, and compares hit position/normal bits. The first view is cold and remains in the raw samples. Each subsequent static-frame query moves the pointer, so these gains do not rely solely on returning the identical previous hit.

| Fixture | Camera | Fresh scene p50 (ms) | Reused scene p50 (ms) | Reused pick p50 (ms) |
| --- | --- | --- | --- | --- |
| Sphere | Orthographic | 32.3 | 0.0012 | 0.025 |
| Sphere | Perspective | 32.3 | 0.0012 | 0.028 |
| Organic | Orthographic | 32.1 | 0.0013 | 0.024 |
| Organic | Perspective | 33.0 | 0.0016 | 0.031 |
| Hidden | Orthographic | 23.4 | 0.0012 | 0.024 |
| Hidden | Perspective | 23.7 | 0.0011 | 0.031 |

Static prepared-view reuse is approximately 0.001–0.002 ms. The first lookup construction is still paid on the second distinct query. Orbiting correctly invalidates preparation: cold scene p50 remains about 23–33 ms and first picks about 6–10 ms. Identical repeated picks in the reuse probe take less than 0.001 ms. [Reuse/orbit samples](scene-reuse/profile.json) retain cold frames, p95 values and correctness assertions. These CPU operation timings exclude GPU drawing and desktop presentation.

## Validation and limits

**28 focused CPU tests and five Vulkan tests pass.** Tests cover conservative group bounds, exact exhaustive-pick agreement, equal-depth traversal, near-plane crossings, hidden/inactive models, camera and viewport changes, topology/ownership changes, refreshed shading and materials, immutable older scenes, and separate preview/base roles. [Logs](cpu-batch-checks/) retain the results. The [eight-case footprint capture](cpu-batch-checks/footprint.png) is byte-identical to the preceding commit; [comparison hash](cpu-batch-checks/capture-comparison.json) records it.

The primary process peak RSS was 1124.6 → 1081.2 MiB; the separate reuse probe reached 1122.2 MiB. No general peak-memory saving is claimed: caching retains prepared arrays, and replacement/externally retained views can coexist. The two renderer cache slots are bounded, not an unbounded history. All three probes completed within 3072 MiB RSS / 768 MiB reserve / 600 seconds with no sampled process swap. Long sessions and arbitrary imported topology are not established by these fixtures.

## Provenance and reproduction

Baseline `cf6826f` source SHA-256: `34b9a939367007cf54f5d16031143a806d95560a006bdb26b38b48ca24112618`. Final measured source SHA-256: `e936bbdb71d665b5445ca35d242caea73631d6c9e28fc9a47143e5e196342627`. Final test executable SHA-256: `7547ce33ac6f2f5fd93dfeccba58884d93c2f30c53de3db6e1a93ddcdd4baf90`. The baseline binary was verified before edits. Optimized library tests were used; no release application build was run. Full-suite/lint/CI remain for the eventual PR.

Runs were sequential on the docked, AC-powered i7-7500U; environment records and per-run memory guards are retained. GPU regressions used Intel HD Graphics 620 Vulkan. The guarded runner targets the following diagnostics with fresh output paths:

```sh
CLAY_CAMERA_LEVEL=4 CLAY_CAMERA_PROFILE=/tmp/picking.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_camera_picking -- --exact --ignored --test-threads=1
CLAY_CAMERA_LEVEL=4 CLAY_REUSE_PROFILE=/tmp/reuse.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_scene_reuse -- --exact --ignored --test-threads=1
```

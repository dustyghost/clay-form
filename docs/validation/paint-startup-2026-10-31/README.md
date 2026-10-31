# Paint stroke startup

Follow-up to [indexed surface drawing](../indexed-surface-draw-2026-10-31/README.md), under #103.

The startup probe identified visibility-hierarchy construction as 82–92% of the first Paint dab on the 602k-triangle fixtures. Each new stroke previously rebuilt the same tree, even when only materials or image pixels had changed.

The builder now calculates triangle bounds and centroid sums once, using temporary construction data, while preserving the original partition comparisons, traversal order and node bounds. It retains the hierarchy between strokes and validates every triangle's coordinate bits and face visibility, including visible other-model occluders, before reuse. Changed geometry or visibility rebuilds it. Normals and sculptable-vertex eligibility still refresh at stroke start; projected brush bins refresh per dab. Cache validity does not rely on pointer addresses or vertex counts alone.

## Measured startup

Same docked i7-7500U, optimized library tests, sequential runs. Each fixture runs three four-dab Paint strokes, with exact undo/redo checks between them. Trial zero has a cold hierarchy; trials one and two can reuse geometry. The table keeps that distinction: cold entries are one initial sample per fixture, and repeated entries show both later-start samples as a range. They are not p95 estimates or end-to-end presentation timings.

| Source triangles | Fixture | Cold first dab before → after (ms) | Later stroke starts before → after (ms) |
| --- | --- | --- | --- |
| 602,112 | Sphere | 326.5 → 160.2 | 318–321 → 28–31 |
| 602,112 | Organic | 315.8 → 167.0 | 313–324 → 24–32 |
| 602,112 | Hidden | 190.1 → 102.0 | 179–191 → 24–32 |
| 2,408,448 | Sphere | 1425.7 → 683.4 | 1410–1431 → 124–132 |
| 2,408,448 | Organic | 1432.6 → 724.0 | 1394–1428 → 111–144 |
| 2,408,448 | Hidden | 857.8 → 448.4 | 852–859 → 120–137 |

Cold startup takes approximately **46–52% less time** in these samples. Subsequent strokes on unchanged geometry take **83–92% less time**. Geometry edits still pay for rebuilding the hierarchy, and reuse still includes a linear validation scan. At 2.4 million triangles, later starts remain 111–144 ms because the undo snapshot, normals, validation and paint application still have costs. No gain is claimed for every individual dab. [All paired samples](comparison.csv) retain both startup and sustained dabs; raw profiles separate document setup, normals, visibility construction/query and material application.

## Correctness and memory

**17 focused CPU tests and two Vulkan tests pass.** The new builder is compared against the frozen original for exact partitions and node bounds, including reordered and flat geometry. Cache tests cover coordinate edits, topology/order changes, hidden faces, group visibility, masks/materials, and changed occluders. Existing regressions cover distant occluders, hidden/masked surfaces, pressure, spray, float textures, material channels, cancellation, history and stored-channel GPU pixels. [Check logs](paint-checks/) retain the executable/source hashes.

All **18 before/after stroke-result hashes match** across both densities, all fixtures and all trials. Each diagnostic checks exact undo/redo restoration. [Result hashes](output-hashes.json) record the comparison.

The 2.4m probe peaked at 1111.1 → 1119.8 MiB process RSS; all four probes completed within 3072 MiB RSS / 768 MiB available-memory reserve / 600 seconds, with no sampled process swap. The new construction array temporarily uses 36 bytes per source/occluder triangle and is released after building. Cached visibility adds one boolean per triangle. The existing world/projected-face arrays and tree storage were already retained allocations between strokes. This is a latency improvement, not a general memory-saving claim.

## Provenance and reproduction

Baseline `ff5574b` plus opt-in test instrumentation: source SHA-256 `456116c367d66bfdbd79987205a4bd49c33d8c0ef4b67e6271bc8ea896cb01a1`. Measured implementation: `a3c47d2964b36a530c6bfb4810685d5481c2a539e064d85d5f84642c08fee119`; test executable: `02952abf8b459df59411d330a1ebc4e6ce2ffa68c09999b17ae43520ad5c25cd`. Environment and memory-guard records accompany each profile. No production shader changed, and no release application build was run. Full-suite/lint/CI remain for the eventual PR.

```sh
CLAY_PAINT_LEVEL=3 CLAY_PAINT_PROFILE=/tmp/paint-startup.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_paint_startup -- --exact --ignored --test-threads=1
```

Use level 4 for 2.4 million triangles. The recorded runs wrapped the executable with the existing capacity runner's `bounded_run` memory guard; the direct Cargo command does not impose that guard. This manual diagnostic is excluded from routine GPU CI. The builder/cache correctness tests remain ordinary CPU tests.

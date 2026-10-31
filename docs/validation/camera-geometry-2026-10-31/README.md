# Dense camera geometry reuse

Follow-up to [Paint startup](../paint-startup-2026-10-31/README.md), under #103.

Camera changes previously recalculated each triangle's cross product and repeatedly looked up its vertices. Dense unchanged surfaces now retain an unnormalized face normal and origin, then traverse that contiguous array for subsequent views. Facing arithmetic and traversal order are unchanged. GPU scenes also skip the unused depth-sort key; the CPU painter still calculates and sorts those keys.

The cache activates only on a second view of the same immutable positions and triangles, for at least 20,000 displayed triangles. The first view and continuously deforming frames do not build it. Position or triangle allocation changes discard it; explicit renderer clearing frees it. Existing scene-key validation and separate base/preview roles remain in place.

## Measurement

Docked i7-7500U, optimized library tests, 2,408,448 source triangles. Hidden geometry displays fewer triangles. Each fixture/camera has twelve sequential views. Warm medians below use frames 2–11 (ten samples) in both versions, separating the first view and the cache-building second view. These are CPU scene-preparation timings, not whole-frame FPS or GPU draw timings.

| Fixture | Camera | Warm median before → after (ms) | Less time | First view after (ms) | Cache-building view after (ms) |
| --- | --- | --- | --- | --- | --- |
| Sphere | Orthographic | 32.78 → 14.20 | 57% | 24.60 | 30.15 |
| Sphere | Perspective | 33.11 → 15.61 | 53% | 28.33 | 33.07 |
| Organic | Orthographic | 33.68 → 17.17 | 49% | 23.49 | 52.64 |
| Organic | Perspective | 32.15 → 15.67 | 51% | 26.56 | 48.38 |
| Hidden | Orthographic | 23.97 → 10.40 | 57% | 15.77 | 27.98 |
| Hidden | Perspective | 23.83 → 12.28 | 48% | 18.84 | 26.02 |

Static unchanged-view cache hits remain around one microsecond. Picks are checked bit for bit against freshly constructed scenes throughout the probe. [All view timings](comparison.csv) and raw profiles preserve cold samples and individual frames; twelve frames are not a long-tail latency study.

## Validation and costs

29 focused CPU tests and five Vulkan tests pass. The new regression compares facing decisions and sorted order with frozen original arithmetic, and checks exact picks and projection through perspective, orthographic, rolled and near-plane-crossing views. It verifies geometry/visibility invalidation, reuse, clearing and immutable snapshots. Existing tests cover preview roles, stroke history, footprint pixels, clipping and picking.

The dense draw diagnostic additionally exercises the renderer's camera cache at 2.4 million source triangles. Its twelve indexed/general captures and the footprint capture match the earlier saved images byte for byte; [capture hashes](capture-hashes.json) retain that comparison. Pixel correctness and CPU geometry checks are both covered.

Retained face data costs 24 bytes per displayed triangle: about 55.1 MiB at 2.4 million displayed triangles, per populated rendering role. It trades bounded per-surface memory for repeated camera work and is invalidated after deformation. The CPU probe's sampled peak RSS was 1122.4 → 1178.1 MiB; these peaks also include display construction and do not isolate cache memory. Both probes and dense GPU validation completed within the 3072 MiB RSS / 768 MiB reserve / 600-second guards, with no sampled process swap.

## Provenance and reproduction

Baseline: saved `b7cd928` implementation. Measured source SHA-256: `6b90b6eee0982d3e5cf71e7683c51669d03cb6d8475849dc603f58dbaa8373ef`; optimized test executable: `c4eefdbaa340fa930dc4579cbd22229fd7dde55c39b7ab34d6a4d6745399b81d`. Environment records and check logs accompany the profiles. The separate [release-shading experiment](../local-release-shading-2026-10-31/README.md) was rejected; its production changes are absent from this build. No production shader changed. No release application build or full local suite was run; CI remains for the eventual PR.

```sh
CLAY_CAMERA_LEVEL=4 CLAY_REUSE_PROFILE=/tmp/camera-reuse.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_scene_reuse -- --exact --ignored --test-threads=1
```

`CLAY_SCENE_STAGES=1` optionally separates projection, facing and bounds stages; recorded primary measurements leave tracing off. Recorded runs use the existing capacity runner's memory guard around the test executable. The direct Cargo command above does not impose that guard. The diagnostic is excluded from routine GPU CI.

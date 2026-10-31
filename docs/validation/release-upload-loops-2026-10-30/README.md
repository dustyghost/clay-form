# Release upload and loop audit

Follow-up to [cached corner angles](../release-corner-angles-2026-10-30/README.md), under #103. The [code audit](CODE_AUDIT.md) records implemented fixes, rejected experiments and ranked follow-up candidates across display, upload, shading, material and brush loops.

The capacity comparison baseline is `38dcc46`. The upload diagnostic was preserved separately in commit `7be63d3`, with no production behaviour changes. Before diagnosis source SHA-256: `32803646345232acc33a682c42111ee4f46fdfd1872a4ea1f9d2c116009cb0f7`; final measured source SHA-256: `ecc95ccc39d66bbf87839472a60c4ce1b9cab66a98547f9d097d5f73536fe80e`. Both are verified against their source snapshots. Metadata's `revision` can identify a parent commit with a dirty working tree; the digest identifies the measured source.

## Accepted changes

Full vertex packing now borrows effective material values and creates default/muted materials once per call. The previous loop obtained an owned material for every shading vertex, including shared layer/link reference operations and repeated default construction. The original implementation remains a test-only byte-exact reference. The existing incremental packer already borrowed materials and keeps its previous path.

Canonical shading now flattens parents and counts fans during normal accumulation. Emission reuses the parent table for output indices, removes the second root traversal and temporary corner-index array, and reserves output vectors using exact counts. At 602,112 source triangles on this 64-bit machine, the removed array alone is **14,450,688 bytes (13.78 MiB)**. Root ordering, corner weights, accumulation order and resulting normals remain identical. This works because joining chooses the lowest corner as root, which is encountered before its other fan corners.

No additional retained cache or GPU capacity is introduced by these changes. The production buffer allocation policy remains the preceding exact-size policy. The 10.34 MiB corner-angle cache from the previous batch is unchanged.

## Diagnosis and rejected buffer experiments

The new ignored `gpu_load_profile_release_upload` test runs the actual upload path with opt-in CPU checkpoints and separate queue submission/completion timings. It uses the hidden level-3 fixture, three repetitions and twelve dabs per brush, retaining the last live display through release as the capacity workload does. It excludes display construction, drawing and the orbit workload. Instrumentation is omitted from production builds; ordinary test uploads do not enable the timestamps. The existing `gpu_load_profile` filter excludes it from routine GPU CI.

| CPU upload stage | Standard before → after p50 | Clay before → after p50 |
| --- | ---: | ---: |
| Full vertex packing | 10.02 → 4.96 ms | 10.54 → 4.10 ms |
| Vertex storage update | 34.26 → 34.27 ms | 1.83 → 1.83 ms |
| Index comparison, packing and write | 8.83 → 8.35 ms | 8.33 → 8.29 ms |
| Face-colour/UV comparisons and writes | 4.99 → 4.81 ms | 4.71 → 4.77 ms |

Paired sums of each trial's CPU upload and submit/completion samples give **67.86 → 61.60 ms** for Standard and **35.54 → 28.63 ms** for Clay. These are not sums of independent medians. Raw [before](upload-before/profile.json) and [after](upload-after/profile.json) files retain every stage, sample and write count. Fan counts and transferred payload sizes match; required updates were not skipped.

The roughly 34 ms Standard storage stage remained after two experiments:

- Queue-based replacement initialization measured 34.02 ms, with completed upload around 61 ms once the packing improvement was included. It did not improve the storage stage.
- Rounding single-buffer allocations to 4 KiB passed tests proving reuse for small fan changes, bounded padding and shrinkage. Storage update still measured 33.97 ms. Reusing the destination did not remove the dominant measured cost.

Both policies were removed. The stage covers allocation/initialization or queued writes plus replacement bookkeeping; these results do not isolate a single cause. Staging, copying and driver behaviour need finer profiling. [Queued experiment](queued-experiment/profile.json) and [bounded experiment](bounded-experiment/profile.json) results are retained, each with a `source.patch` against `7be63d3`. Applying each patch to that baseline reproduces its recorded source digest. Neither is shipped as a production improvement.

## Shading-loop measurements

The existing release-display probe ran in cached-angle mode before and after. It uses the same three repeated hidden strokes, with reference checks outside timers.

| Stage | Standard before → after p50 | Clay before → after p50 |
| --- | ---: | ---: |
| Angle accumulation, including flattening/counting after | 11.99 → 12.80 ms | 11.71 → 13.22 ms |
| Fan emission | 15.67 → 7.52 ms | 17.65 → 6.67 ms |
| Paired accumulation + emission totals | 27.66 → 20.26 ms | 30.77 → 19.87 ms |

Flattening moves some work into accumulation, but the paired combined cost falls. Raw [before](shading-before/profile.json) and [after](shading-after/profile.json) results include all validation and copying stages; the full release is measured below.

## Matched hardware results

Same i7-7500U, 7.63 GiB usable RAM, Intel HD Graphics 620, Mesa 26.2.4/Vulkan and Rust 1.99.0 release toolchain as the baseline, at 800×600. All twelve cases ran sequentially without overlapping builds or benchmarks, with three repetitions of twelve dabs per brush. Environment, fixture, count, metric coverage, sample counts and output hashes passed strict comparison. Workload and metric schema were unchanged. See the [raw matrix](gpu/index.json), [all-phase comparison](gpu-comparison.csv), [hidden repeats](repeat/index.json) and [repeat comparison](repeat-comparison.csv).

| Hidden vertices | Brush | Before release display p50 / p95 | After p50 / p95 | Before upload p50 / p95 | After p50 / p95 |
| --- | --- | ---: | ---: | ---: | ---: |
| 75,266 | Standard | 20.20 / 20.84 ms | 18.25 / 22.75 ms | 8.22 / 14.42 ms | 7.53 / 13.05 ms |
| 75,266 | Clay | 20.95 / 20.98 ms | 18.50 / 18.86 ms | 11.47 / 18.53 ms | 9.12 / 16.34 ms |
| 301,058 | Standard | 81.70 / 85.87 ms | 73.27 / 75.12 ms | 68.71 / 69.06 ms | 61.56 / 62.65 ms |
| 301,058 | Clay | 79.30 / 79.82 ms | 74.20 / 76.52 ms | 36.07 / 37.21 ms | 29.75 / 30.16 ms |

Combined release work is the median of per-trial `end_stroke` + `end_display` + `end_upload_wait` sums:

| Hidden vertices | Brush | Before matrix | New matrix | New repeat |
| --- | --- | ---: | ---: | ---: |
| 75,266 | Standard | 29.06 ms | 30.29 ms | 29.16 ms |
| 75,266 | Clay | 32.45 ms | 27.89 ms | 28.77 ms |
| 301,058 | Standard | 150.41 ms | 135.55 ms | 137.49 ms |
| 301,058 | Clay | 115.89 ms | 103.45 ms | 103.41 ms |
| 301,058 | Paint | 40.11 ms | 41.37 ms | 40.85 ms |

Largest hidden deformation releases improve by roughly **9–11% against the previous matrix**, persisting on repeat. The preceding report also recorded 150.17 ms Standard and 118.12 ms Clay in its repeat, consistent with the gain. Smaller Standard's combined result does not improve: lower separate phase medians are not evidence of a lower paired total. Its repeat is effectively back at baseline. Paint is approximately unchanged.

These totals exclude final drawing, event scheduling and presentation. Release samples number three per brush, so p95 is the maximum here. No statistical confidence interval, general frame-rate increase or higher maximum model capacity is claimed.

Largest organic Standard combined release changes from 275.94 to 258.03 ms, and Clay from 239.83 to 229.39 ms. Sphere Standard release display remains close, 198.17 → 200.32 ms, and upload 22.87 → 22.98 ms. Largest hidden sustained Standard upload is 17.02 → 16.81 ms p50. Improvements depend on the path used; already-incremental uploads are not the target of the full-packer change.

## Memory

| Workload | Previous matrix peak RSS | New matrix | New repeat |
| --- | ---: | ---: | ---: |
| Sphere, 75,266 vertices | 378.1 MiB | 380.7 MiB | — |
| Organic, 75,266 vertices | 428.2 MiB | 448.8 MiB | — |
| Hidden, 75,266 vertices | 431.1 MiB | 427.8 MiB | 441.7 MiB |
| Sphere, 301,058 vertices | 822.2 MiB | 805.1 MiB | — |
| Organic, 301,058 vertices | 956.7 MiB | 895.5 MiB | — |
| Hidden, 301,058 vertices | 899.8 MiB | 907.9 MiB | 907.8 MiB |

The preceding report's largest organic repeat was 885.2 MiB and hidden repeat 939.2 MiB. These ranges demonstrate why removing a known temporary allocation is not equivalent to a guaranteed reduction in whole-process peak. RSS includes allocator retention, staging, snapshots, history and save/load verification. The exact temporary-index removal is established by the code; process peaks remain mixed. All runs completed within 1536 MiB RSS and 600-second guards. Geometry limits were not raised.

## Correctness and reproduction

Sixteen focused CPU tests passed: seven shading, three vertex-packing and six hidden-stroke cache regressions. The new shading case covers reordered, disconnected, degenerate and empty input against the frozen original implementation, checking normal bits and fan order. Full and incremental packing both compare with the frozen owned-material packer. Existing coverage retains sharpness transitions, caps, creases, masks, material channels, signed zero, NaNs, cache invalidation and unseen final edits.

Ten focused hardware GPU tests passed on the final implementation: upload/layout/release regressions, visibility/picking/history, face colours/depth batching, UV/checker batching, material channels, depth/wireframe caching and incremental texture-atlas updates. The hidden preview was visually inspected and matches the [previous capture](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png) byte-for-byte, SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`. Formatting and diff whitespace checks passed. Full-suite CI remains pending; no PR was opened.

Run sequentially with fresh output directories and graphics-device access for hardware checks:

```sh
cargo test --release --locked --lib sculpt::shading::tests:: -- --test-threads=1
cargo test --release --locked --lib vertex_packing_ -- --test-threads=1
cargo test --release --locked --lib viewport::gpu::display::stroke_tests:: -- --test-threads=1
cargo test --release --locked --lib gpu_vertex_upload -- --ignored --test-threads=1
CLAY_HIDDEN_STROKE_PREVIEW=/tmp/clay-upload-loops.png cargo test --release --locked --lib viewport::gpu::visibility_test:: -- --ignored --test-threads=1

CLAY_UPLOAD_PROFILE=/tmp/clay-upload-profile.json cargo test --release --locked --lib gpu_load_profile_release_upload -- --ignored --test-threads=1
CLAY_RELEASE_MODE=cached CLAY_RELEASE_PROFILE=/tmp/clay-shading-profile.json cargo test --release --locked --lib gpu_load_profile_release_display -- --ignored --test-threads=1

GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/upload-loops-repeat/gpu --gpu
python3 scripts/benchmark-capacity.py compare docs/validation/release-corner-angles-2026-10-30/gpu target/upload-loops-repeat/gpu
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/upload-loops-repeat/repeat --gpu --fixtures hidden --levels 2 3
```

The five additional GPU filters are listed in the [earlier buffer-reuse report](../release-buffer-reuse-2026-10-30/README.md#reproduce). To reproduce before diagnosis, use `7be63d3` in an isolated checkout and the same profile commands. Experimental patches apply to that commit. Retained diagnostic runs used the capacity runner's process guards; their metadata records the limits. The standalone commands above run the ignored tests directly.

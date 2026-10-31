# Hidden-surface release refresh

Follow-up to the [hidden-surface stroke cache](../hidden-stroke-cache-2026-10-30/README.md), under #103. The comparison baseline is commit `1a44de8`. Final measurements use source SHA-256 `6257c4da586fdd690bb30e85d858dc5734cb2acf1aaea24252ad8f19998c344e`; hardware, compiler, timestamps, individual samples and memory stages are retained in the [CPU results](cpu/index.json) and [GPU results](gpu/index.json).

The upload-phase follow-up is recorded in [Reuse mesh buffers and upload changed vertex blocks](../vertex-upload-cache-2026-10-30/README.md).

## Change and scope

Cached hidden-surface strokes now retain position-independent edge adjacency alongside the source topology. On release, deformation recomputes canonical angle-weighted normals and smoothing fans from that adjacency, while reusing filtered display faces, source-face IDs and attributes. It rechecks the sixty-degree smoothing threshold, so edges can become either sharp or smooth. Hidden faces still contribute to boundary normals, and the updated fan mapping is retained for the next stroke.

A flag records whether deformation introduced temporary normals. Paint and mask-only strokes can reuse their exact display geometry at release when no temporary normals remain. Release checks source topology, visibility, model identity, positions, masks and materials; an unseen final dab or an incompatible edit takes the full rebuild path. These linear checks still cost time even when the display is reused.

Eligibility remains the existing dense single-model hidden-surface path: at least 20,000 vertices, no UV data, textures, explicit creases, primitive preview or temporary thickness. Uncached surfaces join edges directly without allocating the stored adjacency list. Geometry limits, sculpt operations, project format and history semantics are unchanged.

The CPU benchmark now calls the production release helper before its full-rebuild fallback, matching the renderer. Fixtures, pressure sequence, twelve dabs per stroke, three repetitions, measured phases and output requirements are unchanged. All twelve CPU and twelve GPU cases passed the strict input/output comparison against the previous cache implementation.

## Release display work

Same i7-7500U, 7.63 GiB usable RAM and release toolchain as the previous run. The browser remained closed, and CPU/GPU matrices ran sequentially without concurrent builds. These timings measure CPU display preparation at release, not complete pointer-up latency: a later upload and draw are separate. Each release metric has three samples; p95 is the maximum of those three here.

| Hidden fixture | Brush | Before release p50 / p95 | After release p50 / p95 |
| --- | --- | ---: | ---: |
| 75,266 vertices | Standard | 67.67 / 68.84 ms | 30.48 / 32.10 ms |
| 75,266 vertices | Clay | 66.50 / 67.92 ms | 30.81 / 32.27 ms |
| 75,266 vertices | Paint | 66.20 / 72.78 ms | 6.14 / 6.15 ms |
| 301,058 vertices | Standard | 273.10 / 290.06 ms | 123.07 / 133.15 ms |
| 301,058 vertices | Clay | 285.79 / 286.95 ms | 123.84 / 125.01 ms |
| 301,058 vertices | Paint | 279.27 / 290.74 ms | 25.47 / 25.63 ms |

The largest CPU Standard release preparation is about 55% shorter; Paint is about 91% shorter. Sustained Standard display preparation remains close to the previous cache result: 25.51 ms p50 versus 25.83 ms. Its p95 rose from 32.05 to 34.85 ms in this run. Largest hidden initial display preparation is 280.78 ms versus 275.50 ms; creating the cache still has an upfront cost.

For the complete measured display work of a twelve-dab stroke, sum each trial's first display, eleven sustained displays and release display, then take the median of the three trial sums:

| 301,058-vertex hidden fixture | Before | After |
| --- | ---: | ---: |
| Standard | 584.76 ms | 431.73 ms |
| Clay | 595.18 ms | 434.25 ms |
| Paint | 462.62 ms | 213.36 ms |

These are sums of samples from the same trial, not sums of independent percentiles. They exclude initial viewport construction, brush evaluation, picking, GPU upload, drawing and OS presentation, and do not imply a frame rate.

## Hardware GPU and controls

The same matrix ran on Intel HD Graphics 620, Mesa 26.2.4/Vulkan, at 800×600. For the largest hidden fixture:

| Phase | Before p50 / p95 | After p50 / p95 |
| --- | ---: | ---: |
| Standard release display | 285.08 / 286.11 ms | 125.20 / 143.63 ms |
| Clay release display | 277.75 / 283.28 ms | 125.16 / 125.29 ms |
| Paint release display | 280.02 / 285.79 ms | 24.74 / 25.20 ms |
| Standard sustained display | 26.22 / 30.85 ms | 25.81 / 27.10 ms |
| Standard buffer preparation, upload and completion wait | 71.77 / 90.19 ms | 71.77 / 90.18 ms |
| Completed orbit draw | 29.45 / 33.68 ms | 29.50 / 33.55 ms |

Upload remains a separate bottleneck. Deformed geometry still needs a new upload after release. The smaller hidden GPU workload drops from 66.65 to 30.30 ms for Standard release, and from 62.44 to 6.18 ms for Paint release.

Fully visible Standard CPU display medians are 23.76 ms versus 23.53 ms for the largest sphere, and 29.00 ms versus 28.52 ms for the organic form. Controls are not uniformly faster: sphere Paint sustained display rose from 17.21 to 18.36 ms in the CPU run, and sphere Standard upload rose from 44.41 to 49.10 ms in the GPU run. These short runs do not establish statistical significance; the retained comparisons report every measured phase rather than claiming an across-the-board improvement.

## Memory and remaining limits

The largest cached mesh stores 903,168 eligible edge pairs, about 14 MiB of pair data before vector spare capacity (about 16 MiB allocated). This is additional retained adjacency; release shares the source topology instead of rebuilding a second copy. Uncached surfaces do not retain or allocate this list.

| Hidden workload | Before process peak RSS | After process peak RSS |
| --- | ---: | ---: |
| CPU, 75,266 vertices | 336.6 MiB | 330.7 MiB |
| CPU, 301,058 vertices | 691.8 MiB | 715.0 MiB |
| GPU, 75,266 vertices | 429.0 MiB | 425.1 MiB |
| GPU, 301,058 vertices | 876.6 MiB | 850.4 MiB |

The largest CPU hidden workload adds about 23 MiB of peak memory. GPU peak fell in this run, but that does not mean the retained cache is free. Peaks include allocator retention, benchmark reference snapshots, history and save/load verification; they are not isolated cache sizes. Largest sphere CPU peak is 593.8 MiB versus 600.2 MiB; its GPU peak is 813.8 MiB versus 801.1 MiB. All cases stayed within the 1536 MiB RSS and 600-second per-process guards.

Release is still synchronous and can remain noticeable. Exact normals are recomputed over all source faces, guard checks scan source arrays, and deformation release still copies display arrays. Incremental upload and regional normal updates remain future work. These results cover the existing 301k-vertex maximum benchmark fixture, not multi-million-polygon capacity.

## Correctness and visual validation

Fourteen focused CPU tests passed: six hidden-cache tests, five shading tests and three capacity-workload tests. Added regressions compare both cached and uncached shading exactly with the pre-change implementation, including open boundaries, explicit creases, degenerate triangles and non-manifold input. They exercise sharp-to-smooth and smooth-to-sharp transitions, propagation of rebuilt fans into the next stroke, persistent temporary-normal state, and an unseen final Standard/Paint/Mask dab. Existing checks retain visibility and topology invalidation, hidden boundary normals, protected vertices, empty displays and exact history restoration.

Both hardware visibility regressions passed. They check solid/wire pixels, picking, polygroup lookup, release, Undo/Redo and Show all. The Paint release now explicitly asserts that the renderer reuses the same display allocation. Deformation release matches a fresh canonical render exactly.

The final capture was visually inspected and is byte-for-byte identical to the [previous capture](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png), SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`. Rows are Standard and Paint; columns are before, live stroke, release and Undo. Hidden faces remain absent, the mask stays protected, and Undo restores the original view.

Formatting and diff whitespace checks passed. Full-suite CI remains pending; this branch has no PR yet.

## Reproduce

Run sequentially with other intensive applications closed. Hardware tests need access to the graphics device. Use fresh benchmark output directories on a local disk:

```sh
cargo test --release --locked --lib hidden_stroke_cache -- --test-threads=1
cargo test --release --locked --lib sculpt::shading::tests:: -- --test-threads=1
cargo test --release --locked --lib capacity_ -- --test-threads=1
CLAY_HIDDEN_STROKE_PREVIEW=/tmp/clay-hidden-release.png cargo test --release --locked --lib viewport::gpu::visibility_test:: -- --ignored --nocapture --test-threads=1

python3 scripts/benchmark-capacity.py run target/hidden-release-repeat/cpu
python3 scripts/benchmark-capacity.py run target/hidden-release-repeat/gpu --gpu
python3 scripts/benchmark-capacity.py compare docs/validation/hidden-stroke-cache-2026-10-30/cpu target/hidden-release-repeat/cpu
python3 scripts/benchmark-capacity.py compare docs/validation/hidden-stroke-cache-2026-10-30/gpu target/hidden-release-repeat/gpu
```

The retained [CPU comparison](cpu-comparison.csv) and [GPU comparison](gpu-comparison.csv) include all cases and phases. Raw results in this directory are from the final implementation; exploratory runs are not mixed into the comparison.

# Update retained CPU vertex records in place

Follow-up: [Reuse unchanged corner angles at hidden-surface release](../release-corner-angles-2026-10-30/README.md) measures the next reduction in canonical release preparation.

Follow-up to [independent buffer reuse at stroke release](../release-buffer-reuse-2026-10-30/README.md), under #103. The GPU comparison baseline is commit `48baac4`. Final measurements use source SHA-256 `ed1ce295fd72f6ecbfbab3e6f0ca5fa193649380e591e11905445a952104cbf1`. The [raw GPU results](gpu/index.json) retain all samples and memory stages; the [comparison](gpu-comparison.csv) includes every measured phase.

## Change and boundaries

Compatible single-buffer uploads now update the retained packed vertex array in place. The pass borrows effective material values, compares each record byte-for-byte, rewrites changed records and accumulates dirty GPU blocks. It avoids copying an owned material for every shading vertex, allocating a second packed array and then scanning both arrays for differences.

This still scans every shading vertex. Compact comparison values are computed for every record; extended material channels are also expanded when the extended stride is active. The work is not proportional only to brush size. Face-data comparisons and material-stride detection remain full scans. No additional persistent payload cache was added.

Changed records conservatively mark their complete 4 KiB block coverage. Adjacent blocks merge, with the existing limit of 64 writes and a whole-buffer write when more than half the payload is dirty. Whole-record coverage can include an unchanged neighbouring block, although the retained CPU probe produced identical write-byte counts in both modes for all 72 frames. Reaching the whole-buffer threshold does not stop later CPU record updates.

Fan remapping at an unchanged count is checked through effective GPU values. Count, stride, binding-limit or batching changes retain the original full-packing fallback. Bitwise comparisons preserve signed zero and NaN payloads. Sculpting, canonical shading calculation, shaders, history and file formats are unchanged.

## Isolated CPU diagnosis

The ignored `gpu_load_profile_vertex_packing` test runs the same hidden level-3 fixture with Standard and Paint, three repetitions of twelve dabs. Its `full` mode uses the original full packer and separate dirty-block detector; `incremental` uses the new production pass. Both modes ran sequentially from the same final executable and source. They are excluded from routine GPU CI by the existing `gpu_load_profile` filter.

These timings exclude display refresh, allocation destruction, queue staging and GPU completion. There are 33 sustained samples per brush. Before values below are percentiles of paired per-dab packing-plus-detection sums, not sums of independent percentiles.

| Brush | Full pack + detection p50 / p95 | In-place pass p50 / p95 | Median records rewritten, before → after |
| --- | ---: | ---: | ---: |
| Standard | 11.49 / 13.63 ms | 5.13 / 6.07 ms | 301,058 → 2,111 |
| Paint | 11.93 / 13.34 ms | 5.15 / 7.54 ms | 301,058 → 1,935 |

The isolated pass is about 55–57% shorter. Unchanged records are still visited. The first dab can change nearly every record as stroke shading is established. Face comparisons remain about 5.5 ms and stride detection about 1.4–1.5 ms in this probe. Raw [full](packing-full/profile.json) and [incremental](packing-incremental/profile.json) results include per-frame write counts and environment files alongside them.

## Matched hardware results

Same i7-7500U, 7.63 GiB usable RAM, Intel HD Graphics 620, Mesa 26.2.4/Vulkan and Rust 1.99.0 release toolchain as the baseline, at 800×600. All twelve cases ran sequentially without concurrent builds. Each brush has three repetitions of twelve deterministic dabs. The strict comparison passed environment, fixture, count, sample coverage and output checksum checks. The capacity workload and metric schema were unchanged.

The upload phase includes CPU preparation, queued transfers and GPU completion. These are not complete frame timings.

| Fixture, 301,058 vertices | Brush | Before sustained upload p50 / p95 | After p50 / p95 |
| --- | --- | ---: | ---: |
| Sphere | Standard | 25.99 / 28.81 ms | 14.79 / 16.38 ms |
| Sphere | Clay | 25.92 / 27.82 ms | 14.36 / 15.46 ms |
| Sphere | Paint | 28.12 / 29.40 ms | 20.62 / 22.09 ms |
| Organic | Standard | 26.55 / 28.29 ms | 20.57 / 22.39 ms |
| Organic | Clay | 26.59 / 27.72 ms | 20.01 / 22.47 ms |
| Organic | Paint | 27.90 / 29.35 ms | 20.54 / 21.90 ms |
| Hidden organic | Standard | 22.97 / 24.92 ms | 16.55 / 18.23 ms |
| Hidden organic | Clay | 22.81 / 24.26 ms | 16.69 / 18.10 ms |
| Hidden organic | Paint | 23.46 / 24.50 ms | 17.13 / 17.96 ms |

Largest hidden sustained upload is about 27% shorter across these brushes. At 75,266 hidden vertices, Standard falls from 6.15 to 4.65 ms p50. For the largest hidden fixture, paired sums of sustained dab, display and upload work fall from 66.51 to 59.82 ms p50 for Standard, 69.64 to 63.66 ms for Clay, and 41.43 to 34.21 ms for Paint. These totals exclude drawing and event/presentation scheduling. Standard's corresponding p95 is almost unchanged, 68.88 to 68.69 ms.

## Release timing investigation and other phases

The first matrix showed a slower largest-hidden Standard release display: 125.13 → 135.51 ms p50. Its three samples were 135.51, 122.54 and 140.66 ms. Paired `end_stroke` + `end_display` + `end_upload_wait` totals rose from 195.02 to 206.30 ms p50. This was investigated before committing, despite the canonical display algorithm being unchanged.

A separate bounded repeat of the same case, source and three repetitions passed the same workload/output checks. Release display samples were 132.41, 125.22 and 123.29 ms, giving a 125.22 ms median, close to the original 125.13 ms baseline. Combined release work returned to 192.89 ms. Sustained Standard upload remained faster at 16.65 ms. The [repeat is retained separately](hidden-repeat/hidden-l3.json); it does not replace the slower first run. These results suggest run-to-run variation rather than a persistent release slowdown, but do not establish a release improvement or rule out smaller effects.

Release results remain mixed elsewhere. Largest organic Clay release upload rises from 34.99 to 40.26 ms p50, while its p95 is nearly unchanged, 49.17 to 48.92 ms. Largest sphere Standard release upload falls from 31.42 to 22.74 ms. Resized vertex storage still takes the full-packing fallback. Hidden Paint retains its exact display and stays around 0.41 ms for release upload/completion.

The first matrix's largest hidden Standard sustained display p50 rises slightly, 25.77 → 26.13 ms, with a noisier p95 of 27.54 → 35.28 ms. Organic Standard's paired sustained work improves at p50, 72.33 → 66.20 ms, but its p95 worsens, 75.37 → 79.39 ms. Hidden orbit draw remains close, 29.65 → 29.58 ms p50. These short runs do not support an across-the-board latency or frame-rate claim. Canonical release display preparation and unchanged face-data comparisons remain separate substantial costs.

## Memory

| Workload | Before process peak RSS | After |
| --- | ---: | ---: |
| Sphere, 75,266 vertices | 379.5 MiB | 392.4 MiB |
| Organic, 75,266 vertices | 429.3 MiB | 427.6 MiB |
| Hidden organic, 75,266 vertices | 437.3 MiB | 423.4 MiB |
| Sphere, 301,058 vertices | 791.1 MiB | 818.9 MiB |
| Organic, 301,058 vertices | 901.0 MiB | 902.7 MiB |
| Hidden organic, 301,058 vertices | 909.2 MiB | 881.9 MiB |

The hidden repeat peaks at 884.9 MiB. Removing a transient array does not guarantee a lower whole-process peak: these values also include queue staging, allocator retention, reference snapshots, history and save/load verification. Memory results are mixed, with no new retained payload array. Every run completed within the 1536 MiB RSS and 600-second per-process guards. Geometry limits were not raised.

## Correctness and visual checks

Five focused CPU tests passed. Three compare incremental packing and simulated dirty-range uploads byte-for-byte with the independent full packer, covering compact/extended materials, positions, masks, default/linked/muted materials, inactive models, fan remapping, signed zero, NaNs, full-write fallback and empty data. Two retain dirty-block coverage, merging, tail alignment and bounded-write checks.

Ten focused hardware GPU tests passed. Sparse visible/hidden Standard, Clay, Paint and Mask strokes now assert that only a minority of records are rewritten. Identical GPU values rewrite none. Existing checks cover fresh-buffer pixel equality, extended material edits, release fan splitting/merging, layout changes, independent face buffers, empty data, batching transitions, visibility/picking, Undo/Redo, depth, wireframe, UV/checker output and texture-atlas updates.

The hidden-stroke capture was visually inspected and matches the [previous capture](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png) byte-for-byte, SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`. Hidden faces remain absent and Paint remains visible. Formatting and diff whitespace checks passed. Full-suite CI remains pending; no PR was opened.

## Reproduce

Run sequentially with other intensive applications closed. Hardware checks require graphics-device access. Use fresh output directories:

```sh
cargo test --release --locked --lib vertex_pack -- --test-threads=1
cargo test --release --locked --lib vertex_upload -- --test-threads=1
cargo test --release --locked --lib gpu_vertex_upload -- --ignored --nocapture --test-threads=1
CLAY_HIDDEN_STROKE_PREVIEW=/tmp/clay-vertex-packing.png cargo test --release --locked --lib viewport::gpu::visibility_test:: -- --ignored --test-threads=1

CLAY_PACKING_MODE=full CLAY_PACKING_PROFILE=/tmp/clay-packing-full.json cargo test --release --locked --lib gpu_load_profile_vertex_packing -- --ignored --test-threads=1
CLAY_PACKING_MODE=incremental CLAY_PACKING_PROFILE=/tmp/clay-packing-incremental.json cargo test --release --locked --lib gpu_load_profile_vertex_packing -- --ignored --test-threads=1

python3 scripts/benchmark-capacity.py run target/vertex-packing-repeat/gpu --gpu
python3 scripts/benchmark-capacity.py compare docs/validation/release-buffer-reuse-2026-10-30/gpu target/vertex-packing-repeat/gpu
python3 scripts/benchmark-capacity.py run target/vertex-packing-repeat/hidden-repeat --gpu --fixtures hidden --levels 3
```

The five additional hardware tests are listed in the [preceding report](../release-buffer-reuse-2026-10-30/README.md#reproduce). The retained CPU diagnosis runs also used the capacity runner's process guards; their environment metadata records those limits. Their `revision` identifies the parent commit with a dirty working tree; the shared source digest identifies the measured implementation.

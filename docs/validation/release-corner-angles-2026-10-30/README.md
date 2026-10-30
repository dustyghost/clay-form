# Reuse unchanged corner angles at hidden-surface release

Follow-up to [in-place CPU vertex packing](../vertex-packing-2026-10-30/README.md), under #103. Comparison baseline: commit `ee281d9`. Measured source SHA-256: `3bc6563d3900a4f31ce88319263cff86f1f915f3c2c03ffd3191c4b35227df42`. The [raw GPU matrix](gpu/index.json), [complete comparison](gpu-comparison.csv), [targeted repeats](repeat/index.json) and [repeat comparison](repeat-comparison.csv) retain the evidence, including less favourable timings and memory peaks.

## Measured target and change

The release diagnostic identified corner-angle accumulation as the largest stage, about 54 ms on the largest hidden fixture. Material validation cost about 22 ms, display cloning 11–12 ms, fan emission 14–16 ms and fan joining about 10 ms. These stages are measured separately from renderer bookkeeping, allocation destruction and GPU completion.

The existing shared hidden-surface topology now retains a reference position array and three exact corner angles per source triangle. When shading is rebuilt, a triangle reuses its reference angles only if all three positions match bit-for-bit. Otherwise it computes all three angles using the existing formula. Face normals, sharpness decisions, smoothing fans and angle-weighted accumulation are still rebuilt, in the same order. Hidden faces continue to contribute to boundary normals.

The reference snapshot is immutable and shared across stroke frames and releases. It does not grow with stroke count. As editing changes more of the original surface, fewer faces can reuse weights; a globally changed surface recomputes them all. Restoration can make reference weights reusable again. Existing topology, visibility, model and crease guards still determine whether the hidden display cache is valid. The uncached path continues to calculate every angle.

This adds retained memory: `12 × (vertex count + triangle count)` bytes of payload. For 301,058 vertices and 602,112 triangles, that is 10,838,040 bytes, or **10.34 MiB**. At 75,266 vertices it is about 2.58 MiB. Production use remains limited to the existing eligible hidden-surface cache: a single active model, at least 20,000 vertices, and no unsupported UV, crease, primitive or thickness state. Visible controls do not allocate this cache. No geometry limits were raised.

## Reproducible CPU diagnosis

The ignored `gpu_load_profile_release_display` test runs Standard, Clay and Paint on the same hidden level-3 fixture, three repetitions of twelve deterministic dabs. Both `full` and `cached` modes ran sequentially from the same final executable. Full mode calculates every angle; cached mode uses the production cache. Both retain the same reference cache, so this comparison isolates release calculation rather than initial cache construction. Reference shading checks are outside timers. The existing `gpu_load_profile` exclusion keeps this manual test out of routine GPU CI.

| Brush | Full angle accumulation p50 / p95 | Cached p50 / p95 |
| --- | ---: | ---: |
| Standard | 52.48 / 53.85 ms | 12.33 / 13.55 ms |
| Clay | 52.53 / 60.97 ms | 12.60 / 15.02 ms |

That is about 40 ms less work in this stage. Each deformation stroke leaves 597,287 of 602,112 reference faces unchanged, about 99.2%. The remaining 4,825 faces recompute their angles. All faces still participate in normal accumulation; this is not brush-size-only work. Paint does not need canonical shading reconstruction at release and skips these shading stages. Raw [full](angles-full/profile.json) and [cached](angles-cached/profile.json) results include cache payload and unchanged-face counts.

## Hardware release results

Same i7-7500U, 7.63 GiB usable RAM, Intel HD Graphics 620, Mesa 26.2.4/Vulkan and Rust 1.99.0 release toolchain as the preceding report, at 800×600. The twelve cases ran sequentially without overlapping builds or benchmarks. Each brush has three repetitions of twelve dabs. All environment, fixture, count, sample coverage and output checksum comparisons passed. The capacity workload and metric schema were unchanged.

| Hidden vertices | Brush | Before release display p50 / p95 | After p50 / p95 |
| --- | --- | ---: | ---: |
| 75,266 | Standard | 32.10 / 33.29 ms | 20.20 / 20.84 ms |
| 75,266 | Clay | 31.51 / 31.65 ms | 20.95 / 20.98 ms |
| 301,058 | Standard | 135.51 / 140.66 ms | 81.70 / 85.87 ms |
| 301,058 | Clay | 121.98 / 127.35 ms | 79.30 / 79.82 ms |

The previous matrix's slower Standard result was already investigated in its report: a baseline repeat measured 125.22 ms. The new repeat measures **80.87 ms**, with p95 84.16 ms. Clay repeats at 80.73 ms. The gain therefore persists against the faster baseline repeat, rather than depending on the earlier slow run. With three release samples, p95 is the maximum; these are short runs, not a statistical confidence interval.

For combined release work, sum each trial's `end_stroke`, `end_display` and `end_upload_wait`, then take the median of those paired totals:

| Largest hidden brush | Previous matrix | Previous repeat | New matrix | New repeat |
| --- | ---: | ---: | ---: | ---: |
| Standard | 206.30 ms | 192.89 ms | 150.41 ms | 150.17 ms |
| Clay | 158.45 ms | 159.57 ms | 115.89 ms | 118.12 ms |
| Paint | 41.10 ms | 40.88 ms | 40.11 ms | 41.31 ms |

Standard saves about **43 ms, or 22%**, against the previous repeat. Clay saves about **41 ms, or 26%**, comparing repeats. Paint is effectively unchanged. At 75,266 hidden vertices, Standard's matrix total falls from 42.57 to 29.06 ms and Clay's from 42.14 to 32.45 ms. These totals exclude final drawing, event scheduling and presentation; they are not complete pointer-up latency or frame-rate measurements.

## Initial display and other phases

Largest hidden initial display preparation is 286.24 → 287.79 ms p50; p95 is 303.14 → 316.45 ms. At 75,266 vertices, its p50 is 64.62 → 67.00 ms. Reference weights are computed during cache construction and reused by initial shading. These results show no large initial-display penalty, but do not establish that initial construction is free.

Largest hidden Standard sustained upload is 16.55 → 17.02 ms p50, and Clay is 16.69 → 16.68 ms. Standard sustained display is 26.13 → 26.83 ms. Hidden orbit draw is 29.58 → 29.60 ms. Earlier during-stroke gains remain, with no claim of a new sustained-stroke improvement. Clay's sustained upload p95 worsens from 18.10 to 21.73 ms in the matrix; the complete comparison retains such variation.

Visible controls remain broadly similar: largest sphere Standard release display is 193.86 → 198.17 ms, organic is 206.77 → 207.52 ms, and organic repeats at 198.41 ms. The cache does not target those paths. Largest hidden Standard release upload remains about 69 ms, and material validation still costs about 21 ms in the CPU probe; those remain separate costs.

## Memory and repeat investigation

| Workload | Previous matrix peak RSS | New matrix | New repeat |
| --- | ---: | ---: | ---: |
| Sphere, 75,266 vertices | 392.4 MiB | 378.1 MiB | — |
| Organic, 75,266 vertices | 427.6 MiB | 428.2 MiB | — |
| Hidden, 75,266 vertices | 423.4 MiB | 431.1 MiB | — |
| Sphere, 301,058 vertices | 818.9 MiB | 822.2 MiB | — |
| Organic, 301,058 vertices | 902.7 MiB | 956.7 MiB | 885.2 MiB |
| Hidden, 301,058 vertices | 881.9 MiB | 899.8 MiB | 939.2 MiB |

The organic increase prompted a targeted repeat because that fixture does not allocate the new cache. Its peak did not remain elevated. Both results are retained. The hidden repeat also confirms whole-process variation: measured peaks are **900–939 MiB**, compared with **882–885 MiB** across the prior matrix and repeat. The exact additional cache payload is 10.34 MiB; it does not explain every change in process peak. RSS also includes allocator retention, queue staging, reference snapshots, history and save/load verification. This is a speed/memory tradeoff, not a memory reduction or demonstrated increase in maximum model capacity. Every case completed within the 1536 MiB RSS and 600-second process guards.

## Correctness and visual checks

Twelve focused CPU tests passed: six shading regressions and six hidden-stroke cache regressions. The new test checks unchanged-face reuse and compares fan indices and normal bits with the frozen original algorithm after successive local edits, whole-mesh movement, restoration and a signed-zero change. Existing coverage includes hard edges, creases, open and non-manifold input, degenerate faces, thin cut triangles, caps, sharpness splitting/merging, visibility invalidation, successive strokes and unseen final dabs.

Five focused hardware GPU regressions passed. They compare fresh-buffer pixels across sparse visible/hidden strokes, material/layout changes, canonical release fan splitting/merging, visibility, picking, Undo/Redo and wireframe output. The hidden preview was visually inspected and matches the [previous capture](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png) byte-for-byte, SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`.

Formatting and diff whitespace checks passed. Full-suite CI remains pending; no PR was opened.

## Reproduce

Run sequentially, with other intensive applications closed. Hardware checks need graphics-device access. Use fresh output directories:

```sh
cargo test --release --locked --lib sculpt::shading::tests:: -- --test-threads=1
cargo test --release --locked --lib viewport::gpu::display::stroke_tests:: -- --test-threads=1
cargo test --release --locked --lib gpu_vertex_upload -- --ignored --test-threads=1
CLAY_HIDDEN_STROKE_PREVIEW=/tmp/clay-release-angles.png cargo test --release --locked --lib viewport::gpu::visibility_test:: -- --ignored --test-threads=1

CLAY_RELEASE_MODE=full CLAY_RELEASE_PROFILE=/tmp/clay-release-full.json cargo test --release --locked --lib gpu_load_profile_release_display -- --ignored --test-threads=1
CLAY_RELEASE_MODE=cached CLAY_RELEASE_PROFILE=/tmp/clay-release-cached.json cargo test --release --locked --lib gpu_load_profile_release_display -- --ignored --test-threads=1

python3 scripts/benchmark-capacity.py run target/release-angles-repeat/gpu --gpu
python3 scripts/benchmark-capacity.py compare docs/validation/vertex-packing-2026-10-30/gpu target/release-angles-repeat/gpu
python3 scripts/benchmark-capacity.py run target/release-angles-repeat/repeat --gpu --fixtures organic hidden --levels 3
```

The retained CPU probes also used the capacity runner's process guards. Their metadata records those limits. `revision` identifies the parent commit with a dirty working tree; the shared source digest identifies the measured implementation. Only the final implementation's reproducible full/cached diagnosis and hardware results are retained here.

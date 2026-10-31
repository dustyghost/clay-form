# Display reuse during and after sculpting

Follow-up to [2.4-million-triangle capacity measurements](../high-capacity-2026-10-30/README.md), under #103.

## Change and scope

Display snapshots now share immutable ownership, visibility-related metadata, edge overlays, fan indices and normals. Paint retains exact normal data; deformation allocates replacement temporary normals without first copying old values. Dense visible meshes now retain the validated topology/angle cache already used for hidden meshes, allowing exact shading restoration without rebuilding all display metadata at release. Visible release fans also share their allocation with display triangle indices. Builder allocations are trimmed before sharing.

The cache applies to a single visible model with at least 20,000 vertices, fixed topology, no explicit creases or UV data, and no primitive or temporary thickness. Topology, visibility, model identity and final displayed attributes are checked before reuse. Hidden faces still contribute to normals. Sharpness is recomputed at release, including fan splits and merges. Other configurations retain their existing paths. Document storage, capacity settings and shading mathematics are unchanged.

## Large-mesh comparison

Three repetitions per case at **1,204,226 vertices / 2,408,448 triangles**, 800×600 offscreen, Intel HD 620 Vulkan. These are measured operation costs, not desktop frame rates. The primary paired runs used the same fixtures, budgets and schema; every input/output hash matched. All nine brush/fixture combinations improved combined sustained p50 in this run (5–30%). Visible Standard/Clay release preparation improved by 72–74%; visible Paint by about 97%.

Each sustained row contains 33 samples. Combined time sums matching brush, display and upload/wait samples before calculating percentiles. It excludes drawing, input handling, OS presentation and checkpoint I/O. Times are milliseconds.

| Fixture | Brush | Display p50 before → after | Combined before p50 / p95 | Combined after p50 / p95 |
| --- | --- | --- | --- | --- |
| Sphere | Standard | 127.9 → 80.5 | 183.5 / 200.2 | 129.2 / 171.7 |
| Sphere | Clay | 109.0 → 80.7 | 187.4 / 210.6 | 147.0 / 163.7 |
| Sphere | Paint | 141.1 → 116.9 | 222.4 / 244.2 | 191.7 / 208.4 |
| Organic | Standard | 148.8 → 125.2 | 238.8 / 274.7 | 200.7 / 244.9 |
| Organic | Clay | 129.9 → 108.7 | 229.9 / 245.9 | 202.9 / 243.3 |
| Organic | Paint | 97.0 → 105.1 | 182.2 / 229.8 | 170.8 / 186.8 |
| Hidden | Standard | 124.1 → 116.3 | 204.7 / 244.7 | 194.6 / 222.4 |
| Hidden | Clay | 112.0 → 100.2 | 210.7 / 239.7 | 195.8 / 221.1 |
| Hidden | Paint | 92.2 → 77.6 | 156.5 / 177.9 | 137.5 / 161.1 |

Release display and upload/wait medians, with three samples each. Paint release on visible meshes now reuses the exact displayed snapshot, also allowing the existing upload path to skip unnecessary work. Hidden release results are mixed; no consistent hidden-release speedup is claimed.

| Fixture | Brush | Release display before → after | Release upload before → after |
| --- | --- | --- | --- |
| Sphere | Standard | 936.6 → 242.9 | 198.1 → 166.9 |
| Sphere | Clay | 907.6 → 245.5 | 199.7 → 167.3 |
| Sphere | Paint | 976.5 → 32.5 | 68.3 → 0.4 |
| Organic | Standard | 1040.9 → 293.0 | 146.4 → 116.2 |
| Organic | Clay | 964.9 → 264.5 | 147.9 → 124.7 |
| Organic | Paint | 992.7 → 32.3 | 68.8 → 0.3 |
| Hidden | Standard | 280.4 → 308.9 | 166.4 → 166.5 |
| Hidden | Clay | 268.6 → 282.6 | 166.9 → 166.7 |
| Hidden | Paint | 32.7 → 32.5 | 0.4 → 0.4 |

## Initial cost and memory

| Fixture | Initial display p50 before → after (ms) | Whole-process peak RSS before → after (MiB) |
| --- | --- | --- |
| Sphere | 815.5 → 1016.0 | 2305.4 → 2508.1 |
| Organic | 840.4 → 1113.6 | 2469.0 → 2725.3 |
| Hidden | 1526.4 → 1511.9 | 2704.7 → 2606.9 |

Visible meshes pay an additional 201–273 ms of initial display preparation and retain more cache data. Whole-process peaks rose by 203–256 MiB in the primary visible cases; hidden peak RSS fell by 98 MiB. RSS includes benchmark snapshots, history, rendering and allocator retention, so these deltas are not isolated cache allocation measurements. The repeated workloads rebuild a display before each brush; an unchanged interactive display can retain its cache across strokes.

All primary and repeat cases completed within **3072 MiB process RSS / 768 MiB available-system reserve / 600 seconds per case**, with no sampled benchmark swap use. Across these runs the highest reported peak was 2785 MiB and the lowest available system RAM was 1020 MiB. A long editing session with many history entries is outside this workload. GPU drawing remained around 80 ms for visible and 55 ms for hidden meshes; this change does not remove that cost.

## Timing variation and focused repeats

The first organic Paint display median increased from 97.0 to 105.1 ms, despite its combined p50 improving from 182.2 to 170.8 ms. Hidden deformation release also increased in the primary comparison. Both cases were repeated with the saved baseline and current optimized executables, three repetitions each, under the same guards. All input/output hashes matched again. These repeats are retained separately, rather than replacing the original measurements.

| Fixture | Brush | Live display before → after | Release display before → after | Combined p50 before → after |
| --- | --- | --- | --- | --- |
| Organic | Standard | 171.9 → 123.8 | 1005.9 → 286.8 | 260.7 → 203.5 |
| Organic | Clay | 142.3 → 104.8 | 964.5 → 250.0 | 242.8 → 197.2 |
| Organic | Paint | 131.0 → 109.5 | 1038.3 → 31.7 | 215.4 → 179.3 |
| Hidden | Standard | 120.1 → 101.9 | 291.6 → 283.5 | 204.4 → 183.1 |
| Hidden | Clay | 105.2 → 99.1 | 268.2 → 245.2 | 198.7 → 191.8 |
| Hidden | Paint | 89.0 → 55.6 | 32.9 → 31.9 | 154.6 → 114.2 |

The organic Paint and hidden-release regressions did not recur in the repeat. Display-copy timings vary substantially between processes, while the large visible-release improvement holds in both pairs. The evidence supports reduced copying and much faster visible release; it does not establish a consistent hidden-release improvement or a fixed per-dab speedup.

Memory also varies: the organic repeat rose from 2405 to 2785 MiB (+380 MiB), while hidden rose from 2643 to 2714 MiB (+71 MiB). Visible caching has a clear memory cost; a reliable hidden-mesh peak-memory saving is not established by these runs.

## Phase diagnosis at 602k triangles

An opt-in test-only trace measures checkpoints inside the actual display methods; it adds no application-build instrumentation. Each row has 12 live samples (three trials of four dabs). Both versions verify released packed vertices and shading triangles against a freshly built canonical display outside the timers. All 18 released mesh hashes match across versions.

| Fixture | Brush | Live total p50 before → after (ms) | Display metadata/shading copy p50 before → after (ms) |
| --- | --- | --- | --- |
| Sphere | Standard | 22.32 → 18.44 | 3.424 → 0.002 |
| Sphere | Paint | 16.80 → 12.28 | 4.271 → 0.002 |
| Organic | Standard | 27.59 → 23.94 | 3.634 → 0.002 |
| Organic | Paint | 17.94 → 11.98 | 4.791 → 0.002 |
| Hidden | Standard | 25.00 → 22.76 | 2.308 → 0.002 |
| Hidden | Paint | 13.73 → 10.99 | 2.201 → 0.002 |

Sharing removes nearly all work from the former metadata/shading-copy phase. Mesh arrays still copy, and deformation still scans all source triangles for temporary display normals. At 602k triangles those normals/bounds loops remain around 13 ms. These are the next display targets; correctness-preserving local updates need explicit dirty-region and invalidation tracking. Initial cache construction and retained RAM also warrant further work.

## Correctness and provenance

**38 focused CPU tests and five Vulkan tests passed.** Coverage includes immutable snapshots, hidden boundary normals, exact canonical release, topology/visibility/material invalidation, unseen final dabs, sharpness split/merge, packing layouts, sparse uploads, picking and Undo/Redo. Pixel checks cover visible and hidden surfaces, solid and wire display, exact Paint/release images and bounded temporary-normal differences. [Visible capture](checks/visible.png) and [hidden capture](checks/hidden.png) were inspected: rows are Standard/Paint; columns are before/live/release/undo. The captures show retained silhouettes and masks, a localized paint mark, and exact visual restoration on undo. [Check logs](checks/) retain test results.

Production baseline is `81d8c2e`, plus the diagnostic-only [baseline patch](baseline-probe.patch). Reconstructed baseline source SHA-256: `bf1e3f026ef8588e2d0e9f5de863bfc40a4f888917fc16d033f65bf91b178f8f`. Current measured source SHA-256: `5718385845609a2f3d44db0c9541a0a53e5f1b8ac0b3dffe8b2aa29405343159`. Both sets used the same probe; the patch adds only test tracing/probe code. Baseline reconstruction and current source digests were verified. [Phase-probe arguments](probe-arguments.json) clarify its fixed internal budgets, which are not configured by the generic metadata defaults.

Runs were sequential on the docked, AC-powered i7-7500U (2 cores / 4 threads), about 7.63 GiB RAM, `powersave` / `balance_performance`; [power readings](power.json) and per-run environment/adapter metadata are retained. Source hashes cover Rust sources, Cargo files, build script and capacity runner. Saved-executable repeats additionally record executable hashes and distinguish the checked-out source from the executable's source. No release application build was needed. Formatting and diff whitespace checks passed; full-suite/lint/CI remain for the eventual PR. No PR was opened.

[Primary comparison CSV](comparison.csv), [paired sample totals](paired-timings.csv), [organic repeat comparison](repeat-comparison.csv), [hidden repeat comparison](hidden-repeat-comparison.csv), [before GPU data](gpu-before/), [after GPU data](gpu-after/), and [phase data](after/profile.json) retain the measured evidence.

## Reproduce

Reconstruct the baseline by applying `baseline-probe.patch` to `81d8c2e`; use the final branch for the optimized version. Run from each checkout's repository root, with a fresh output directory:

```sh
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/display-run --fixtures sphere organic hidden --levels 4 --repeats 3 --gpu --workload interactive --mesh-vertices 2000000 --mesh-triangles 4000000 --max-rss-mib 3072 --min-available-mib 768
python3 scripts/benchmark-capacity.py compare path/to/before path/to/after
GIT_OPTIONAL_LOCKS=0 python3 docs/validation/display-reuse-2026-10-30/run-phase-probe.py target/display-phases
cargo test --release --locked --lib viewport::gpu::display:: -- --test-threads=1
cargo test --release --locked --lib stroke_cache_pixels_picking_and_history -- --ignored --test-threads=1
cargo test --release --locked --lib viewport::gpu::upload::gpu_tests:: -- --ignored --test-threads=1
```

The phase runner uses the same bounded-process helper. `run-repeat.py` records the saved-executable method used for the follow-up checks; its executable and metadata paths refer to this session's ignored `target/display-reuse` artifacts. Reproduction can instead run the ordinary capacity runner from each source checkout with `--fixtures organic hidden`.

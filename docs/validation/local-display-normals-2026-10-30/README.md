# Local display-normal updates

Follow-up to [display reuse](../display-reuse-2026-10-30/README.md), under #103.

## Change and scope

After the first deformation frame establishes the existing area-weighted preview normals, later frames compare current positions with the displayed snapshot and recompute only affected smoothing fans. Each affected fan includes every incident source face, including hidden faces, in the original triangle/corner summation order. Old contributions are not subtracted, so repeated updates cannot accumulate subtraction drift. Unaffected fans retain their exact previous values.

A compact vertex-to-source-corner lookup is created lazily on the first eligible local update and shared across snapshots and stroke releases. It stores source corners rather than fan IDs, so release can change sharpness and split or merge fans without invalidating the lookup. Existing model/topology/visibility/UV/crease checks still gate reuse. Paint does not request this lookup. More than one eighth of vertices changing selects the full pass before lookup allocation; estimated corner work above half the face count also falls back to the full pass, potentially after lookup creation. This bounds work on scattered changes and high-valence vertices.

The first deformation still performs a full normal pass. Its arithmetic is unchanged and has been extracted into a helper. The global bounds calculation, position scan, mesh-array copying, topology validation and canonical release shading remain. This change does not make the entire display update proportional to the brush area.

## Live editing at 2.4 million triangles

Three repetitions per fixture, **1,204,226 vertices / 2,408,448 triangles**, Intel HD 620 Vulkan, 800×600 offscreen. Each sustained row has 33 samples. Combined values sum matching brush, display and upload/wait samples before calculating p50/p95; drawing, input handling, OS presentation and checkpoint I/O are excluded. These measurements are operation timings, not desktop frame rates.

Standard/Clay display preparation takes **31–62% less time**, with their combined sustained p50 improving **19–43%** in this paired run. Every workload input and stroke-output hash matches. Paint is a control: its normal-update path is unchanged, and its timing movements are not claimed as a local-normal improvement.

| Fixture | Brush | Display p50 before → after (ms) | Combined before p50 / p95 (ms) | Combined after p50 / p95 (ms) |
| --- | --- | --- | --- | --- |
| Sphere | Standard | 97.3 → 37.4 | 146.0 / 171.7 | 83.3 / 122.8 |
| Sphere | Clay | 81.4 → 37.7 | 148.5 / 175.3 | 103.6 / 134.5 |
| Sphere | Paint | 122.0 → 112.7 | 196.1 / 210.2 | 190.6 / 201.7 |
| Organic | Standard | 117.3 → 79.4 | 196.8 / 249.3 | 158.5 / 230.8 |
| Organic | Clay | 107.6 → 66.8 | 203.5 / 260.8 | 159.8 / 213.6 |
| Organic | Paint | 94.4 → 97.3 | 166.2 / 187.2 | 167.3 / 189.3 |
| Hidden | Standard | 101.5 → 70.1 | 180.0 / 226.8 | 143.4 / 184.9 |
| Hidden | Clay | 104.0 → 57.8 | 201.8 / 225.9 | 148.1 / 184.0 |
| Hidden | Paint | 63.7 → 76.4 | 127.0 / 151.5 | 134.9 / 154.6 |

## Focused hidden-mesh repeat

The primary hidden Paint control had a 20% slower display median and a 6% slower combined median, while hidden Standard release was also slower. A second before/after pair used the saved baseline and current executables, with three repetitions each and the same guards. Its input/output hashes and sample counts match. Results are retained alongside the original pair.

| Brush | Display p50 before → after (ms) | Release display before → after (ms) | Combined before p50 / p95 (ms) | Combined after p50 / p95 (ms) |
| --- | --- | --- | --- | --- |
| Standard | 98.7 → 53.8 | 305.2 → 287.1 | 178.2 / 215.3 | 132.0 / 179.7 |
| Clay | 99.7 → 54.9 | 259.2 → 276.4 | 192.0 / 209.3 | 145.2 / 176.0 |
| Paint | 67.3 → 74.4 | 33.6 → 33.0 | 135.8 / 160.8 | 134.0 / 167.0 |

The deformation gain held. Paint display preparation was slower in both pairs, by 10–20%, while combined Paint timing was mixed: 6% slower in the primary run and about 1% faster in the repeat, with a 4% higher repeat p95. The cause of this control variation is not isolated; no Paint performance improvement or unchanged Paint performance is claimed. Release timings also vary in direction, and no release improvement is claimed. These results leave copying/allocation behavior as a separate investigation.

Repeat peak RSS rose from 2646 to 2708 MiB (+62 MiB); both cases stayed within the guard without sampled swap use. [Repeat comparison](hidden-repeat-comparison.csv) and [before](hidden-repeat-before/)/[after](hidden-repeat-after/) data retain the measurements and executable hashes. Across the primary pairs and this repeat, observed peak increases range from 4 to 62 MiB.

## First update and release

Medians in milliseconds, three samples each. First deformation and sustained p95 both matter when assessing whether lookup creation introduces a hitch. Canonical release is outside the optimized path; its timing variation is retained without claiming a release improvement.

| Fixture | Brush | First display before → after | Release display before → after |
| --- | --- | --- | --- |
| Sphere | Standard | 78.6 → 55.5 | 245.2 → 224.0 |
| Sphere | Clay | 79.1 → 55.5 | 242.2 → 258.4 |
| Sphere | Paint | 124.9 → 129.4 | 32.7 → 33.5 |
| Organic | Standard | 110.2 → 93.4 | 273.0 → 300.7 |
| Organic | Clay | 103.5 → 82.0 | 262.6 → 265.4 |
| Organic | Paint | 121.3 → 124.9 | 31.7 → 32.7 |
| Hidden | Standard | 99.4 → 91.8 | 274.6 → 303.5 |
| Hidden | Clay | 104.8 → 74.9 | 285.7 → 270.8 |
| Hidden | Paint | 92.7 → 97.9 | 33.1 → 32.5 |

## Phase diagnosis at 602k triangles

The opt-in trace runs inside the actual display methods. These are Standard normals-and-bounds phase medians, with three first-update samples, three lookup-construction samples and six later-update samples per fixture. The second displayed deformation builds the lookup in this workload; the later samples reuse it. Reference display checks run outside the timers.

| Fixture | Phase | Before (ms) | After (ms) |
| --- | --- | --- | --- |
| Sphere | First deformation | 12.86 | 6.81 |
| Sphere | First local update | 12.81 | 9.21 |
| Sphere | Later local updates | 12.79 | 1.94 |
| Organic | First deformation | 12.90 | 6.91 |
| Organic | First local update | 13.08 | 10.06 |
| Organic | Later local updates | 12.76 | 1.84 |
| Hidden | First deformation | 12.78 | 6.82 |
| Hidden | First local update | 12.97 | 9.25 |
| Hidden | Later local updates | 12.76 | 1.88 |

The later local updates fall from about 13 ms to about 2 ms. Lookup creation remains below the old phase cost in this probe. The extracted full pass also measured faster on the first deformation; that observation is recorded separately from the locality improvement. No timing thresholds are asserted in correctness tests.

## Memory and remaining costs

The lookup contains `(vertices + 1 + 3 × triangles)` 32-bit entries: **8.04 MiB at 602k triangles**, **32.16 MiB at 2.4m**. It is allocated only when requested, survives fixed-topology releases, and is discarded with its source topology. Temporary selection flags/lists and a replacement normal snapshot also allocate. The old snapshot remains immutable.

| Fixture | Whole-process peak RSS before → after (MiB) | Initial display p50 before → after (ms) |
| --- | --- | --- |
| Sphere | 2439.0 → 2467.3 | 1022.9 → 1025.3 |
| Organic | 2747.2 → 2751.3 | 1092.6 → 1099.8 |
| Hidden | 2599.2 → 2639.1 | 1493.2 → 1496.2 |

All six GPU cases completed within the **3072 MiB process RSS / 768 MiB system reserve / 600 seconds per case** guards, with no sampled process swap use. Highest reported peak: **2751 MiB**; lowest available system RAM: **1057 MiB**. Whole-process RSS includes document/history snapshots, GPU resources and allocator retention; its change does not isolate lookup storage. These are short, single-model workloads, not long-session memory tests.

The next display costs are unchanged mesh/attribute copying and validation scans. The position scan, replacement-normal copy and full bounds pass also remain. Camera projection and GPU drawing need separate work; this batch does not optimize them.

## Correctness and provenance

**45 focused CPU tests and five Vulkan tests passed.** Seven new regressions cover compact corner ordering, repeated/degenerate corners, slivers, open boundaries, nearby disconnected sheets, signed zero, scattered/high-valence fallbacks, skipped dabs, immutable snapshots, hidden-source contributions, fan split/merge across releases and broad deformations without lookup allocation. Local updates match the full pass bit for bit through 80 repeated edits. Existing packing, shading, display invalidation, picking, upload and history checks also pass.

The visible/hidden GPU captures were inspected and are byte-identical to the [parent visible capture](../display-reuse-2026-10-30/checks/visible.png) and [parent hidden capture](../display-reuse-2026-10-30/checks/hidden.png). [Capture comparison](checks/capture-comparison.json) and [test logs](checks/) retain the evidence. The phase probes also verify exact released packed vertices and fan triangles against a fresh canonical display; all 18 released hashes match between versions.

Baseline: `da4d943`, source SHA-256 `5718385845609a2f3d44db0c9541a0a53e5f1b8ac0b3dffe8b2aa29405343159`. Measured optimized source SHA-256: `370910aba5bd461c1c47842eedef84ca450d79725ad78290d86e1f7eaf70c03e`. The baseline is the unmodified parent commit; source reconstruction and final-source hashes were verified. The saved baseline executable was used for the CPU phase probe and its binary hash is retained. Measurements used the optimized library test executable, without a release application build.

All workloads ran sequentially on the docked, AC-powered i7-7500U (2 cores / 4 threads), about 7.63 GiB RAM, `powersave` / `balance_performance`. Per-run environment/adapter details and [power readings](power.json) are retained. Formatting and diff whitespace checks passed. Full-suite/lint/CI remain for the eventual PR; no PR was opened.

[Comparison CSV](comparison.csv), [paired totals](paired-timings.csv), [before GPU data](gpu-before/), [after GPU data](gpu-after/), and [before](phases-before/profile.json)/[after](phases-after/profile.json) phase traces contain the measured samples.

## Reproduce

Run the same commands from `da4d943` and the optimized commit, using fresh output directories:

```sh
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/local-normal-run --fixtures sphere organic hidden --levels 4 --repeats 3 --gpu --workload interactive --mesh-vertices 2000000 --mesh-triangles 4000000 --max-rss-mib 3072 --min-available-mib 768
python3 scripts/benchmark-capacity.py compare path/to/before path/to/after
CLAY_DISPLAY_LEVEL=3 CLAY_DISPLAY_PROFILE=/tmp/display-phases.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_display_stages -- --exact --ignored --test-threads=1
cargo test --release --locked --lib viewport::gpu::display:: -- --test-threads=1
cargo test --release --locked --lib stroke_cache_pixels_picking_and_history -- --ignored --test-threads=1
cargo test --release --locked --lib viewport::gpu::upload::gpu_tests:: -- --ignored --test-threads=1
```

Recorded phase runs used the capacity runner's bounded-process helper with the same RSS/reserve/time guards; their `guard.json` files retain the outcomes. The diagnostic itself fixes capacity at 2m vertices / 4m triangles and runs sphere/organic/hidden, Standard/Paint, three trials of four dabs.

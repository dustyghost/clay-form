# Reduce repeated whole-mesh deformation scans

Follow-up to [brush-loop preparation](../brush-loop-preparation-2026-10-30/README.md), under #103.

## Diagnosis and change

A new opt-in, test-only trace measures phases inside actual sculpt proposals and deformation guards. The baseline found approximately 6 ms in normal calculation, 0.01 ms in fully visible eligibility (1.5–1.6 ms with hidden faces), and **7.5–8.7 ms discovering active faces**. Small-footprint vertex loops took roughly 2–3.6 ms. Large unsafe Grab proposals spent much more in repeated safety checks, with another 2–3 ms scanning reduction flags.

The largest ordinary-stroke opportunity was active-face discovery, so this batch leaves normal calculation and eligibility unchanged:

- Compare old/current positions once per vertex into a temporary Boolean array. Reuse those flags to discover incident faces and, for projection, affected neighbouring-face pairs. Previously each face compared its vertices again.
- Release that array after discovery. It is never retained across calls, so visibility, topology, Undo and brush edits need no cache invalidation.
- When a face/pair fails a safety check, add each flagged vertex once to a pending list. Reduce only those vertices and clear their flags. Each reduction reads only that vertex's old/current position, so processing order does not affect the result.

All active faces and projection pairs still receive the existing safety tests. Winding, quality, directional stamping, fold prevention, thresholds, 12/20-pass limits, local pinning and final rejection are unchanged. The active-face scan remains linear in face count, but its repeated floating-point comparisons are removed. The pending list can grow with the number of rejected vertices; no memory-footprint reduction is claimed.

## Actual phase measurements

The diagnostic uses 301,058 vertices / 602,112 triangles, visible and partially hidden spheres, radii 0.18 and 0.65, Standard/Clay, and an oversized Grab proposal. Each case has one warm-up and five measured operations. Mesh cloning for the fixture and hashing are outside the timer; the stroke's own position snapshot remains inside it. Grab exercises all twelve reduction passes. The test-only trace is absent from application builds, and disabled during the existing capacity workload.

Selected stage medians, milliseconds:

| Mesh | Brush | Radius | Stage | Before | After |
| --- | --- | ---: | --- | ---: | ---: |
| Visible | Standard | 0.18 | guard_active_faces | 7.691 | 2.656 |
| Visible | Standard | 0.65 | guard_active_faces | 8.716 | 3.454 |
| Visible | Clay | 0.18 | guard_active_faces | 8.444 | 2.573 |
| Visible | Clay | 0.65 | guard_active_faces | 8.009 | 3.485 |
| Visible | Grab | 0.18 | guard_active_faces | 7.699 | 2.584 |
| Visible | Grab | 0.18 | guard_reduce | 2.220 | 0.022 |
| Visible | Grab | 0.65 | guard_active_faces | 8.102 | 3.444 |
| Visible | Grab | 0.65 | guard_reduce | 2.831 | 0.292 |
| Hidden | Standard | 0.18 | guard_active_faces | 7.564 | 2.560 |
| Hidden | Standard | 0.65 | guard_active_faces | 8.055 | 3.060 |
| Hidden | Clay | 0.18 | guard_active_faces | 7.743 | 2.659 |
| Hidden | Clay | 0.65 | guard_active_faces | 7.824 | 3.058 |
| Hidden | Grab | 0.18 | guard_active_faces | 7.721 | 2.568 |
| Hidden | Grab | 0.18 | guard_reduce | 2.133 | 0.019 |
| Hidden | Grab | 0.65 | guard_active_faces | 7.899 | 3.638 |
| Hidden | Grab | 0.65 | guard_reduce | 2.599 | 0.163 |

The complete [stage comparison](stage-comparison.csv) retains other stages and both repeat runs. Total direct brush-operation medians, milliseconds:

| Mesh | Brush | Radius | Before | After | Before repeat | After repeat |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Visible | Standard | 0.18 | 19.49 | 14.03 | 18.79 | 14.18 |
| Visible | Standard | 0.65 | 25.40 | 20.29 | 24.51 | 20.38 |
| Visible | Clay | 0.18 | 24.45 | 18.26 | 23.78 | 18.80 |
| Visible | Clay | 0.65 | 30.97 | 26.24 | 30.66 | 26.04 |
| Visible | Grab | 0.18 | 25.25 | 17.02 | 24.84 | 16.94 |
| Visible | Grab | 0.65 | 68.96 | 61.55 | 68.17 | 62.39 |
| Hidden | Standard | 0.18 | 18.61 | 13.24 | 19.37 | 13.84 |
| Hidden | Standard | 0.65 | 22.86 | 16.92 | 22.13 | 16.96 |
| Hidden | Clay | 0.18 | 23.07 | 17.01 | 22.31 | 16.45 |
| Hidden | Clay | 0.65 | 26.76 | 21.27 | 25.67 | 21.68 |
| Hidden | Grab | 0.18 | 24.84 | 15.93 | 23.84 | 17.01 |
| Hidden | Grab | 0.65 | 49.88 | 42.04 | 48.46 | 41.71 |

Repeating the original and changed executables confirmed roughly 15–29% lower Standard/Clay calculation times and 9–32% lower Grab times in these fixtures. All 60 output hashes, returned change flags and safety-loop pass counts match across all four runs. These totals exclude document display/upload/drawing work and are not desktop frame rates. Phase medians are not added together to derive total medians; totals are timed directly.

## Existing sustained strokes

Fresh CPU-only capacity runs cover the largest sphere, organic and hidden fixtures, three 12-dab strokes per brush, with 33 sustained samples. All workload output hashes and sample counts match. Table entries are brush calculation p50 / p95 milliseconds:

| Fixture | Brush | Before | After | Median reduction |
| --- | --- | ---: | ---: | ---: |
| Sphere | Standard | 17.23 / 18.20 | 12.24 / 13.03 | 29.0% |
| Sphere | Clay | 21.90 / 22.88 | 16.89 / 17.96 | 22.9% |
| Sphere | Paint | 3.56 / 4.32 | 3.57 / 4.78 | -0.4% |
| Organic | Standard | 17.56 / 21.26 | 12.25 / 13.21 | 30.3% |
| Organic | Clay | 21.62 / 23.57 | 16.93 / 17.96 | 21.7% |
| Organic | Paint | 3.11 / 3.84 | 3.14 / 3.80 | -1.0% |
| Hidden | Standard | 17.74 / 18.99 | 12.03 / 12.99 | 32.2% |
| Hidden | Clay | 21.26 / 22.22 | 15.66 / 16.64 | 26.4% |
| Hidden | Paint | 2.18 / 2.58 | 2.19 / 2.48 | -0.3% |

Standard improves about 29–32% and Clay about 22–26%. Paint does not use this deformation guard and remains broadly unchanged; sphere Paint's p95 increased from 4.32 to 4.78 ms. Other timing variations are retained in the [full comparison](capacity-comparison.csv), including display, release, history, loading and saving. No improvement is claimed for every phase.

Whole-process peak RSS, MiB:

| Fixture | Before | After |
| --- | ---: | ---: |
| Sphere | 607.1 | 611.8 |
| Organic | 671.3 | 702.1 |
| Hidden | 706.1 | 706.7 |

These peaks include history, reference fixtures, verification and allocator retention. The organic peak is higher; this batch establishes a CPU-time benefit, not lower RAM use. Every run stayed under the 1536 MiB process guard. GPU timings were not repeated for this CPU-only change.

## Correctness and provenance

**45 focused tests passed** on the final measured source. A new frozen-reference regression compares return values and every final position bit in 120 cases: empty/isolated/ordinary/degenerate meshes, six motion patterns, and all five combinations used by sculpt, directional stamping, projection and local smoothing/projection guards. It explicitly checks that reductions/rejections were exercised. Existing tests cover unsafe motion, folded pairs, slivers, steep surfaces, Snake, all smoothing and relaxation regressions, projection, Clay, Polish, symmetry, repeated/adaptive strokes, stencils, masks, hidden geometry and history. [Logs](checks/) are retained.

Baseline production code is commit `25a697c`. The baseline diagnostic build adds only test instrumentation and its probe; [source.patch](before/source.patch) reproduces it. Baseline source SHA-256: `c6f6947986a9a8de7ab3cc405b0bf25f1c35f4b3ba9bc49a6bca3f50e02b82db`. Final measured source: `aa2ed447ce35e423f6a10dcb3617e8498ca2af4c77c5c9c2292e19a37ec90914`.

The baseline executable was saved immediately after its build and reused for the original-code repeat. Repeat metadata identifies the build being reused and the executable SHA-256 rather than mislabelling it as the current working-tree source. Runs were sequential on the docked, AC-powered i7-7500U with `powersave` / `balance_performance`, using the same optimized library build settings and memory/timeout guards. Source digests, workload metadata, samples and output hashes are retained in each dataset. Focused formatting and whitespace checks passed. Full-suite/lint/CI checks remain for the eventual PR; no PR was opened.

## Capacity limits and architecture

The local application and benchmark binaries were verified as x86-64 ELF executables. This batch does not change the fixed limits of 500,000 vertices and 1,000,000 triangles per mesh, or expose them in Settings. Those are application policy limits, not a 32-bit address-space restriction. Multi-million-polygon capacity is still unmeasured. Normal calculation, at roughly 6 ms here, remains a clear candidate for the next investigation.

## Reproduce

Use matching power conditions, sequential runs and fresh paths:

```sh
CLAY_STAMP_PROFILE=/tmp/stamp-stages.json cargo test --release --locked --lib profile_stamp_stages -- --ignored --nocapture --test-threads=1
cargo test --release --locked --lib sculpt::deformation:: -- --test-threads=1
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/deformation-capacity --fixtures sphere organic hidden --levels 3
python3 scripts/benchmark-capacity.py compare docs/validation/deformation-scan-2026-10-30/capacity-before target/deformation-capacity
```

Recorded diagnostic runs used the capacity driver's `bounded_run` helper with the `sculpt::stamp_profile::profile_stamp_stages` test name and `CLAY_STAMP_PROFILE` output path. Timing probes are ignored in routine tests and have no wall-clock assertions.

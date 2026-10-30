# Skip discarded shading copies at release

Follow-up: [shared face payloads](../shared-face-payload-2026-10-30/README.md) removes repeated colour/UV copies and upload comparisons, with timing and memory trade-offs recorded.

Follow-up to [release material validation](../release-material-validation-2026-10-30/README.md), under #103.

## Change

Hidden sculpt release previously cloned all of `DisplayGeometry`, including the old shading vertices and triangle indices, then immediately replaced those shading vectors. `clone_without_shading` now copies the retained fields and starts with empty shading vectors. The existing canonical shading calculation and visible-face filtering then populate them as before. The production path and release diagnostic share this helper. Every field is explicit in the initializer, so adding a display field requires updating this copy operation.

There is no new shared payload cache or change to the shading calculation. The previous live display remains immutable and valid while the renderer prepares release. Paint's already canonical display still returns through the existing reuse path.

At the largest fixture the old live shading has 301,058 vertex/normal records and 362,494 visible triangle records. At 24 bytes per record on this 64-bit build, skipping both vectors avoids about **15.19 MiB of redundant temporary payload copying/allocation** per eligible release. This is distinct from whole-process peak RSS, which did not improve in these runs.

## Measurement scope

Baseline commit: `616f456`. Baseline source SHA-256: `c3ef1011745b1253932efb8a1d5cae6afb7fead1a2e521828805d7dfc8966251`, verified against that commit. The fresh baseline reused the previously tested optimized library executable for that identical source; only documentation and Git metadata had changed since its build. Final measured source SHA-256: `fa3fd45a88334e6514bd957a3a5fa27994f82b28a4f3c1a4100268361da1db9a`. Environment files identify the parent revision and source digest, including dirty-tree measurements.

Fresh before/after measurements cover the two affected hidden-geometry sizes, with Paint as an unchanged release-path control, followed by a largest-case repeat. Each has three repetitions of twelve dabs per brush. The CPU release probe ran before and after. Builds, tests and benchmarks were sequential, with the existing 1,536 MiB RSS and 600-second case guards. This focused batch did not rerun the full twelve-case matrix.

Both sides used the docked i7-7500U, Intel HD Graphics 620, Mesa 26.2.4/Vulkan and Rust 1.99.0 at an 800×600 benchmark viewport. Mains power, governor/preference and both monitor modes matched in the [before](dock-before.json) and [after](dock-after.json) snapshots. All strict workload, environment, count, output-hash and sample-coverage comparisons passed. Probe fixture/frame metadata and sample counts also match. Workload definitions are unchanged.

## Stage measurements

Times below are p50 / p95 milliseconds. With three samples, p95 is the observed maximum, not a robust tail estimate. Combined values use paired samples before taking percentiles.

| Brush | Geometry copy before → after | Copy + filter before → after |
| --- | ---: | ---: |
| Standard | 10.73 / 14.92 → 9.63 / 10.26 | 12.56 / 16.38 → 11.02 / 12.69 |
| Clay | 11.83 / 14.43 → 10.46 / 10.92 | 13.17 / 16.36 → 11.81 / 12.32 |

The copy stage is about 10–12% shorter, saving about 1–1.4 ms at the median. Most of its former 11–12 ms belongs to other data that is still copied. [Before probe](probe-before/profile.json) and [after probe](probe-after/profile.json) retain all stages and samples.

## Complete release

Complete release includes stroke finalisation, display preparation and upload/completion. These totals are paired per trial, not sums of independent medians. Entries remain p50 / p95 milliseconds.

| Vertices | Brush | Display before → after | Complete release before → after | Complete release repeat |
| --- | --- | ---: | ---: | ---: |
| 75,266 | Standard | 15.22 / 17.09 → 13.43 / 13.91 | 24.67 / 28.20 → 21.61 / 26.15 | — |
| 75,266 | Clay | 14.10 / 14.24 → 13.58 / 14.64 | 23.38 / 30.63 → 24.25 / 29.57 | — |
| 75,266 | Paint | 2.07 / 2.07 → 2.24 / 2.51 | 6.38 / 6.46 → 6.72 / 9.15 | — |
| 301,058 | Standard | 57.63 / 62.53 → 54.67 / 58.67 | 120.57 / 125.17 → 116.52 / 121.09 | 117.00 / 118.78 |
| 301,058 | Clay | 56.21 / 56.72 → 54.46 / 55.24 | 86.17 / 87.08 → 83.80 / 84.64 | 84.92 / 85.95 |
| 301,058 | Paint | 8.26 / 9.07 → 8.26 / 8.31 | 23.33 / 23.70 → 23.11 / 23.22 | 23.36 / 24.15 |

The larger Standard result is about 121 → 117 ms, and Clay about 86 → 84–85 ms across the after run and repeat. These are small observed whole-release improvements; variation in the unchanged stages means the full difference cannot be attributed precisely to the removed copy. Paint stays around 23 ms. Smaller Clay totals are mixed despite lower display time. No broad frame-rate improvement is claimed. See the [full phase comparison](comparison.csv) and [largest-case repeat](repeat-comparison.csv).

Largest-case process peak RSS was **894.1 MiB before, 940.2 MiB after and 911.3 MiB on repeat**. Removing the transient shading copies does not establish lower process memory use: the benchmark also retains history/reference snapshots and measures allocator and driver behaviour. All cases stayed within the memory guard; no retained structure was added. The higher observed peaks remain recorded as a limitation.

## Validation

Twelve focused tests passed:

- Seven hidden-stroke regressions check canonical shading, filtered topology, Paint, unseen final edits, layer/link changes and history. Existing assertions now also check active/muted metadata and verify that a retained live snapshot keeps its original shading and temporary-normal state after release.
- Two hardware visibility regressions cover pixels, picking and Undo/Redo.
- Three hardware upload regressions compare fresh-buffer rendering and exercise fan splitting/merging, layouts and sparse updates.

[Test logs](checks/) are retained. The hidden-stroke capture was inspected and is byte-for-byte identical to the [existing reference](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png), SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`. Focused formatting and diff whitespace checks passed. Full-suite and lint checks remain for the eventual PR; no PR was opened.

## Reproduce

Use a fresh output directory and run sequentially under matching dock/power conditions:

```sh
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/shading-copy-run --gpu --fixtures hidden --levels 2 3
python3 scripts/benchmark-capacity.py compare docs/validation/release-shading-copy-2026-10-30/before target/shading-copy-run
```

The existing ignored `viewport::gpu::profile::capacity::gpu_load_profile_release_display` optimized library test supplies the stage breakdown with `CLAY_RELEASE_MODE=cached` and `CLAY_RELEASE_PROFILE` set to a new JSON path. Hardware GPU access is required for the capacity and GPU regression runs.

The remaining geometry-copy cost is about 9.6–10.5 ms in this probe. Sharing immutable display arrays is a larger separate change; unchanged face-colour/UV comparisons during upload also remain a measured candidate from the [code audit](../release-upload-loops-2026-10-30/CODE_AUDIT.md).

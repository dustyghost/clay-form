# Docked release material validation

Follow-up: [skip discarded shading copies at release](../release-shading-copy-2026-10-30/README.md) measures the next targeted copy reduction.

Follow-up to the [loop audit](../release-upload-loops-2026-10-30/CODE_AUDIT.md), under #103. A specialised material comparison removes repeated comparisons of shared layer and link payloads during hidden-geometry release validation.

## Change and correctness contract

`Material::slices_equal` compares every base surface value, then the layer and link contents. It remembers only the last successfully compared pair of layer storages and the last successfully compared pair of link storages, for the duration of that scan. Repeated pairs can reuse a successful comparison. Distinct pairs still receive full value comparisons; separately allocated equal values remain equal.

Pointer identity alone never establishes equality. In particular, shared NaNs still compare unequal and signed zeros compare equal. The borrowed slices keep the payloads alive and immutable throughout the call; no cached identity survives it. This uses constant stack storage, with no allocation, persistent cache, document revision contract or change to general `Material::PartialEq`. Both production release validation and its diagnostic use the new comparison. Benefits depend on repeated shared payloads, rather than arbitrary material diversity.

## Matched docked baseline

Baseline commit: `950f5a742e0cc2107a85d002a00b970596dc92a2`. Its source SHA-256, verified against the commit, is `ecc95ccc39d66bbf87839472a60c4ce1b9cab66a98547f9d097d5f73536fe80e`. Final measured source SHA-256: `c3ef1011745b1253932efb8a1d5cae6afb7fead1a2e521828805d7dfc8966251`. Metadata records the parent revision and a dirty working tree; the digest identifies the measured source. The baseline's dirty flag comes from untracked user session directories, which were not included or changed.

Both sides were measured after docking, on the i7-7500U, 7.63 GiB usable RAM and Intel HD Graphics 620, using Mesa 26.2.4/Vulkan and Rust 1.99.0 optimized library tests. Mains power was online, with `powersave` governor and `balance_performance` preference. The internal 1920×1080 display ran at 60.049 Hz and the external 1920×1080 display at 74.973 Hz, both at scale 1. The benchmark viewport stayed 800×600. [Before](dock-before.json) and [after](dock-after.json) snapshots confirm these settings matched. Earlier undocked timings are not used as the comparison baseline.

All twelve capacity workloads ran sequentially, with three repetitions and twelve dabs per brush, a 1,536 MiB process RSS limit and a 600-second case timeout. Builds, tests and benchmarks did not overlap. The strict comparison verified environment, fixture hashes, counts, output hashes, metric coverage and sample counts. No workload or metric definitions changed. The largest fixture has 301,058 vertices and 602,112 source triangles; these measurements do not establish multi-million-polygon capacity.

## Isolated material comparison

The existing release-display probe uses the hidden level-3 fixture and checks reference shading and restored history outside timers. All fixture, frame and sample metadata matched. Times below are p50 / p95 in milliseconds; with three samples, p95 is the observed maximum, not a robust tail estimate.

| Brush | Before material validation | After material validation |
| --- | ---: | ---: |
| Standard | 21.22 / 21.35 | 3.88 / 4.80 |
| Clay | 20.96 / 21.72 | 3.97 / 4.46 |
| Paint | 21.47 / 27.00 | 3.93 / 4.79 |

This is an 81–82% reduction in the measured comparison stage. Raw [before](probe-before/profile.json) and [after](probe-after/profile.json) files retain the other stages as well.

## Complete release measurements

Complete release includes stroke finalisation, display preparation and upload/completion. Totals are computed from paired samples before taking percentiles, not by adding independent medians. All entries below are p50 / p95 in milliseconds.

| Vertices | Brush | Display before → after | Complete release before → after | Complete release repeat |
| --- | --- | ---: | ---: | ---: |
| 75,266 | Standard | 21.34 / 24.84 → 14.13 / 21.15 | 28.81 / 38.95 → 27.44 / 28.66 | 21.54 / 27.50 |
| 75,266 | Clay | 18.54 / 18.55 → 14.23 / 15.46 | 27.66 / 34.93 → 25.48 / 30.81 | 23.22 / 30.16 |
| 75,266 | Paint | 6.37 / 7.78 → 2.11 / 2.69 | 11.04 / 12.20 → 6.54 / 7.03 | 6.53 / 7.67 |
| 301,058 | Standard | 73.85 / 76.75 → 56.67 / 58.44 | 136.96 / 137.55 → 117.69 / 120.18 | 120.31 / 120.75 |
| 301,058 | Clay | 73.15 / 73.95 → 56.61 / 57.88 | 102.99 / 103.06 → 84.92 / 88.06 | 85.08 / 87.05 |
| 301,058 | Paint | 26.46 / 26.64 → 8.27 / 8.31 | 41.25 / 41.33 → 23.19 / 24.00 | 23.51 / 32.94 |

At the largest fixture, the matched run saves roughly 17–19 ms per release; the repeat confirms the improvement. Smaller Standard totals vary with upload samples, so the first smaller-fixture result alone is not treated as a stable whole-release percentage gain. The upload implementation is unchanged.

The [full comparison](comparison.csv) preserves all phases, including noisy controls. For example, the organic level-3 Standard upload median was 61.40 ms before, 74.63 ms in the first after run, and 60.22 ms in the [control repeat](control-repeat/organic-l3.json). That fixture does not enter this hidden-release comparison. Other small load, projection and Undo timings also fluctuate; no speedup is claimed for those paths. The [hidden repeat comparison](repeat-comparison.csv) retains the repeated measurements against the same docked baseline.

Largest-case peak process RSS was 907.9 MiB before, 898.1 MiB after and 889.3 MiB on repeat. No retained cache was added; these RSS differences are not presented as a memory optimisation.

## Focused validation

Sixteen focused tests passed on the final measured source:

- Three comparison regressions check all 70 surface/layer values, link IDs and all four weights, length differences, separately allocated equal payloads, alternating shared pairs, copy-on-write changes, signed zeros, NaNs and infinity against ordinary equality.
- Seven hidden-stroke display regressions cover filtered topology, shading, paint, visibility, unseen final dabs and new material-only layer/link edits through Undo/Redo.
- One material-history regression covers stroke history, cancellation, save, fill and level switching.
- Two hardware visibility regressions and three hardware upload regressions cover pixels, picking, history, fan splitting/merging and fresh-buffer equivalence.

Logs are in [checks](checks/). The hidden-stroke capture was inspected and is byte-for-byte identical to the [existing reference](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png), SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`. Focused formatting and whitespace checks passed. Full-suite, lint and routine CI checks are deferred to the eventual PR; no PR was opened for this batch.

## Reproduction and next candidate

From the repository root, use the capacity driver with a new output directory:

```sh
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/material-validation-run --gpu
python3 scripts/benchmark-capacity.py compare docs/validation/release-material-validation-2026-10-30/before target/material-validation-run
```

For stage-level diagnosis, run the ignored `viewport::gpu::profile::capacity::gpu_load_profile_release_display` optimized library test with `CLAY_RELEASE_PROFILE` pointing to a new output JSON file and `CLAY_RELEASE_MODE=cached`. Hardware GPU access is required for the capacity driver, visibility and upload tests. Keep runs sequential and retain matching dock/power settings.

The next measured candidate remains display copying: release clones old shading arrays immediately before replacing them with canonical shading. The probe still measures roughly 11–12 ms for the entire geometry clone; only part of that is avoidable. It needs an isolated change and measurement before any improvement claim.

# Local release-shading experiment: rejected

Investigated under #103 alongside [Paint startup](../paint-startup-2026-10-31/README.md) and [camera preparation](../camera-geometry-2026-10-31/README.md). **The experiment is not enabled or retained in production source. The existing release shader remains in place.** Its patch and measurements are retained to make the investigation reproducible.

## Outcome

A local edit that preserves smoothing connectivity can reuse canonical fans and recompute complete normal sums in the original face/corner order. This worked well on the sphere. The masked organic and hidden fixtures changed smooth/hard edge decisions, requiring full reconstruction. Detecting those changes added enough work to regress their release latency. Earlier versions that built full reference-normal arrays were worse; the final candidate stores edge-decision flags and classifies faces lazily, but still does not justify shipping.

The table reports medians of three four-dab strokes per fixture/brush/density on the docked i7-7500U, in optimized library tests. Each trial starts with a new display cache. Release timing includes validation, local attempt, fallback when needed and hidden-face filtering. Fresh canonical comparison is outside the timer. These are small samples, not p95 estimates or complete interactive-frame timings.

| Source triangles | Fixture | Standard baseline → candidate (ms) | Candidate time used | Paint baseline → candidate (ms) |
| --- | --- | --- | --- | --- |
| 602,112 | Sphere | 44.19 → 14.25 | 68% less | 8.30 → 7.80 |
| 602,112 | Organic | 44.88 → 57.26 | 28% more | 7.57 → 8.17 |
| 602,112 | Hidden | 46.40 → 54.90 | 18% more | 7.71 → 7.91 |
| 2,408,448 | Sphere | 224.68 → 59.01 | 74% less | 32.37 → 31.54 |
| 2,408,448 | Organic | 257.13 → 310.44 | 21% more | 32.03 → 32.14 |
| 2,408,448 | Hidden | 292.85 → 316.65 | 8% more | 31.57 → 30.94 |

Paint retains its shading and acts as a control. Raw profiles distinguish `release.local`, `release.fallback` and the subsequent full `release.shade` time. [All paired timings](comparison.csv) preserve cold variation and live-dab samples as well as release. The fast path still performs linear scans, and its reference is fixed: accumulated edits can also force fallback. A future attempt needs cheaper affected-edge discovery or a way to update split/merged fans directly, with benchmarks covering mask boundaries from the start.

## Correctness and memory evidence

The candidate passes 25 focused shading/display CPU tests, including exact normal bits and fan ordering against the frozen original shader after local edits, restoration, signed-zero changes, open edges, explicit creases, degenerate and non-manifold faces. Broad/high-valence edits and sharpness splits/merges exercise fallback. Integration tests verify hidden-face contributions, repeated release and immutable snapshots. [Candidate check logs](shading-experiment-checks/) retain source and binary hashes. It was rejected on CPU performance before additional candidate GPU validation.

All 36 before/candidate release-result hashes match across both densities, brushes, fixtures and trials. Every timed release diagnostic also asserts exact packed display vertices and triangle indices against a fresh canonical display, beyond checking the mesh hashes. [Result comparison](output-hashes.json) retains the matching hashes.

The candidate retains one boolean per paired edge and shares the original canonical vertices/fans, keeping those arrays alive across changed snapshots. It avoids a whole-mesh normal cache. The 2.4m probe's sampled peak RSS was 2357.8 → 2487.8 MiB. All four recorded probes completed within 3072 MiB RSS / 768 MiB available-memory reserve / 600-second guards, with no sampled process swap. None of these additional release-cache costs are present in the shipped source.

## Reproduction

Baseline: saved `b7cd928` executable. Candidate source SHA-256: `ad155c5c8dd70c7322fb97d627a453d769d79766d882d38e5578a08e5afedf0e`; executable: `7d571e2e37cc3ae76114ce46df678bd502ca4471c39dad68970d13ffaf551814`. The [experiment patch](experiment.patch) applies to the measured camera implementation at commit `1f781ab8a03773ff74b8376ecb5200252b930ef3` and restores only the three experimental shading/display source files. It is a recorded prototype, not an active feature. Camera-loop changes were also present in the measured candidate; this display-stage diagnostic does not construct camera scenes.

In an isolated checkout of `1f781ab8a03773ff74b8376ecb5200252b930ef3`, apply the zero-context archive with `git apply --unidiff-zero docs/validation/local-release-shading-2026-10-31/experiment.patch`, then run:

```sh
CLAY_DISPLAY_LEVEL=3 CLAY_DISPLAY_PROFILE=/tmp/release-stages.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_display_stages -- --exact --ignored --test-threads=1
```

Use level 4 for 2.4 million triangles. Recorded runs use the existing capacity runner's memory guard around the executable; the direct Cargo command does not impose that guard. The diagnostic remains excluded from routine GPU CI. No release application build was run.

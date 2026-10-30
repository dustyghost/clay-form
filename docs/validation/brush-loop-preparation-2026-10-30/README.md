# Prepare repeated brush calculations once per operation

Follow-up to [shared display face attributes](../shared-face-payload-2026-10-30/README.md), under #103. This batch improves weight sampling and broader brush calculations. The default small-footprint capacity workloads show much smaller, mixed changes; no desktop frame-rate or capacity increase is claimed.

## Implementation and compiler evidence

The optimized original executable still called `sincosf` in every `Brush::weight` evaluation, evaluated falloff `powf` before selecting the alpha, and evaluated pressure `powf` inside the sculpt vertex loop. These were actual compiled calls, not just apparent repetition in source. A [disassembly excerpt](compiler-evidence.txt) retains that evidence.

- `PreparedWeight` borrows the brush and computes rotation and the inner-radius denominator once per operation. The main sculpt, smooth, material-paint and texture-paint loops reuse it. The borrow prevents brush edits during its lifetime; nothing is retained between operations.
- Pressure is prepared once per dab. Sculpting stores the original `strength * pressure` result; smoothing keeps its original `weight * pressure * strength` order. Material and texture paint also preserve their existing multiplication order.
- Hard and Square alphas skip falloff powers whose results they never use. Soft and Image alphas keep the original formula, including division, rotation, interpolation and falloff order within each calculation. No approximations, reciprocal substitutions or zero-rotation shortcuts were introduced.
- The general `Brush::weight` entry point remains available. Secondary Snake, symmetry-overlap, polish sampling and adaptive callers still use that scalar path; this batch does not claim to remove every repeated brush calculation.

The new storage is one short-lived weight descriptor and, for sculpt/smooth, one scalar per dab. Mesh-sized caching, topology, deformation guards, history formats and GPU code are unchanged.

## Sampling benchmark and exactness

The new regression compares **1,344,800 point/settings combinations bit-for-bit** against the frozen previous formula: all alpha types, singleton and rectangular images, eight rotations, four inner radii, five falloff values, signed zero and samples around brush/square boundaries.

The ignored sampling probe measures 65,536 evaluations per sample, including preparation, with ten recorded pairs after warm-up and alternating old/new order. Rotation is 37°, inner radius 0.35 and falloff 1.7. It checks identical result sums as well. Median milliseconds:

| Alpha | Original | Prepared | Reduction |
| --- | ---: | ---: | ---: |
| Soft | 1.581 | 1.042 | 34.1% |
| Hard | 1.561 | 0.264 | 83.1% |
| Square | 1.623 | 0.308 | 81.0% |
| Image 1×1 | 2.391 | 1.536 | 35.8% |
| Image 3×2 | 2.483 | 1.631 | 34.3% |

These are weight-sampling results, not complete strokes. Raw samples are in [weights.json](weights.json); there is no wall-clock assertion in routine tests.

## Actual brush operations with broader footprints

The new ignored CPU probe runs actual `Mesh::stamp_many` operations on 75,266 vertices / 150,528 triangles. It uses two overlapping dabs with pressures 0.37 and 0.83, pressure curve 1.6, rotation 37°, inner radius 0.35 and falloff 1.7. Each case starts from the same fresh mesh. Mesh cloning and output hashing are outside the timed region; normals, eligibility, deformation guards, smoothing and paint work are inside it. One warm-up plus five measured operations are run for each brush/alpha/radius combination.

All **120 output hashes** match the original in both changed-code runs. The large-footprint radius is 0.65 on the unit sphere; medians below are milliseconds:

| Brush | Alpha | Original | Prepared | Prepared repeat |
| --- | --- | ---: | ---: | ---: |
| Standard | Soft | 9.42 | 8.41 | 8.22 |
| Standard | Hard | 7.56 | 6.14 | 5.86 |
| Standard | Image | 7.45 | 6.67 | 6.57 |
| Clay | Soft | 11.69 | 9.15 | 8.65 |
| Clay | Hard | 9.76 | 9.17 | 8.30 |
| Clay | Image | 9.97 | 9.43 | 8.96 |
| Smooth | Soft | 26.41 | 26.21 | 24.63 |
| Smooth | Hard | 27.67 | 27.36 | 26.00 |
| Smooth | Image | 22.14 | 20.53 | 20.18 |
| Paint | Soft | 106.73 | 100.48 | 97.06 |
| Paint | Hard | 105.56 | 103.66 | 100.78 |
| Paint | Image | 105.59 | 101.30 | 99.27 |

Broader Standard operations improved by about 11–19% in the first comparison and 12–22% on repeat. Clay and Paint also improved in these broader cases; Smooth's first Soft/Hard measurements were nearly unchanged. The [complete comparison](probe-comparison.csv) includes the smaller 0.18-radius cases. The three raw datasets retain every sample and output hash: [original](probe-before/profile.json), [prepared](probe-after/profile.json), [repeat](probe-repeat/profile.json).

Runs were sequential under the same power settings, not a randomized interleaving of executable versions. The repeat was generally faster than the first changed-code run, so these observations should not be treated as precise universal speedup factors. The alternating same-process sampling probe provides the narrower evidence for the eliminated work.

## Existing capacity workloads

A fresh CPU-only baseline and two changed-code runs cover the largest sphere, organic and hidden fixtures (301,058 vertices / 602,112 source triangles), with three 12-dab strokes per brush. All output hashes and sample counts match. Table entries are sustained **brush calculation** p50 / p95 milliseconds, with 33 samples per brush. They exclude display preparation, GPU work, drawing and presentation:

| Fixture | Brush | Original | First prepared run | Final repeat |
| --- | --- | ---: | ---: | ---: |
| Sphere | Standard | 17.66 / 18.66 | 17.42 / 19.10 | 16.97 / 17.87 |
| Sphere | Clay | 21.95 / 23.23 | 22.22 / 23.40 | 21.65 / 23.80 |
| Sphere | Paint | 3.57 / 3.93 | 3.66 / 4.49 | 3.57 / 4.52 |
| Organic | Standard | 17.47 / 19.14 | 17.11 / 19.73 | 17.10 / 18.21 |
| Organic | Clay | 22.06 / 22.69 | 22.43 / 27.75 | 21.72 / 23.72 |
| Organic | Paint | 3.06 / 4.47 | 3.06 / 3.51 | 3.10 / 3.91 |
| Hidden | Standard | 17.25 / 18.66 | 17.49 / 20.78 | 16.95 / 18.08 |
| Hidden | Clay | 20.99 / 22.70 | 21.13 / 22.39 | 20.35 / 21.74 |
| Hidden | Paint | 2.36 / 3.97 | 2.21 / 2.79 | 2.25 / 3.04 |

The small default footprints are broadly steady. Slower observations remain in the record: sphere Paint's p95 rose from 3.93 to 4.52 ms on the final repeat; organic Clay's p95 was 27.75 ms in the first run versus 22.69 ms before, then 23.72 ms on repeat. No uniform latency improvement is claimed. The [final comparison](comparison.csv) and [initial comparison](initial-comparison.csv) retain every measured phase, including history, release, load and save.

Process peak RSS, in MiB, includes fixture copies, history, verification and allocator retention:

| Fixture | Original | First prepared run | Final repeat |
| --- | ---: | ---: | ---: |
| Sphere | 611.4 | 631.3 | 608.1 |
| Organic | 727.0 | 689.8 | 671.7 |
| Hidden | 691.8 | 706.0 | 724.8 |

Memory is mixed: the hidden fixture's final peak is higher. No overall RAM reduction is claimed. Every run stayed within the 1536 MiB process guard; limits were not raised.

## Provenance and checks

Measurements used the docked, AC-powered i7-7500U, `powersave` governor and `balance_performance` energy preference. Environment metadata records the compiler, OS, memory and source digests. No GPU benchmarks or visual changes are claimed in this batch.

- Original capacity baseline: commit `2c82211`, source `8b1e3e3a3b8b76970917e8056ed878e1a38698ddb712e23a8860398607fc0794`. The existing validated optimized executable was reused only after its source digest matched.
- Original code plus the new diagnostic: source `ff90cd8691d7d29c1776ccfba8fb0cc453d46cc73861dec289808ec057f985cc`. The [diagnostic patch](probe-before/source.patch) against `2c82211` reproduces this source snapshot. The future weight-test file is included in the digest but is not compiled in that baseline. This digest was independently reconstructed from the commit and additions.
- First prepared run: source `937fafc93b5b5dc9db3c846b31cee9245c450e885b2dcf228f54cc7b8b7e7c31`.
- Final source: `888fe442438c6464b774ce4cc61c9e7eb704b983608f3cc4ef2d09f9c68061e8`. Production code is identical to the first prepared run; the later addition is the standalone brush-operation diagnostic and its test-module declaration. The final capacity repeat, both prepared operation probes and final focused tests use this source.

**38 focused correctness tests passed** on the final build. These cover exact weight bits, inner radius, all smoothing regressions, symmetry/overlap, texture and material paint, spray/airbrush deposition, zero alpha, pressure, Snake, Clay, Flatten, Polish, masks, hidden faces, stencils and stencil Undo. [Logs](checks/) are retained. Both ignored performance probes also passed their correctness assertions. Focused formatting and whitespace checks passed; full-suite/lint/CI checks remain for the eventual PR. No PR was opened.

## Reproduce

Use sequential runs, fresh output paths and matching power conditions:

```sh
CLAY_BRUSH_WEIGHT_PROFILE=/tmp/brush-weights.json cargo test --release --locked --lib profile_prepared_brush_weights -- --ignored --nocapture --test-threads=1
CLAY_BRUSH_LOOP_PROFILE=/tmp/brush-loops.json cargo test --release --locked --lib profile_brush_loops -- --ignored --nocapture --test-threads=1
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/brush-capacity --fixtures sphere organic hidden --levels 3
python3 scripts/benchmark-capacity.py compare docs/validation/brush-loop-preparation-2026-10-30/before target/brush-capacity
```

The brush-operation probe uses the same Linux process/RSS timeout guard when launched through the capacity driver's `bounded_run` helper; the direct Cargo commands above are for the small diagnostic workloads. Default capacity comparisons cannot establish gains for multi-million-polygon meshes.

The next [deformation-scan batch](../deformation-scan-2026-10-30/README.md) profiles whole-mesh phases and removes repeated position comparisons and full reduction-flag scans.

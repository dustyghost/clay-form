# Sculpting and display code audit

This is a focused first pass over release uploads, display construction, shading, material handling, brush weighting and deformation guards. It is not an exhaustive audit of every repository loop. Stage timings are evidence for prioritisation, not estimates of how much of each stage can be eliminated. See the [matched measurements](README.md) for the implemented changes and their limits.

## Implemented in this batch

- **Full vertex packing:** `DisplayMesh::vertices` called the owned `material(i)` accessor inside the shading-vertex loop. That copied material values and incremented/decremented shared layer/link references per record; absent or muted materials also recreated defaults. It now borrows existing materials and creates default/muted values once per packing call. The original packer remains a test-only byte-exact reference. Compact and extended layouts, masks, muted models, signed zero and NaN payloads are checked. [Source](../../../src/viewport/gpu/display.rs)
- **Repeated shading root traversal and temporary indexing:** `emit_shading` traversed roots again after normal accumulation, allocated one `usize` index per corner, and grew output vectors. Accumulation now flattens each parent once and counts fans. Emission reuses that table for output indices and reserves the exact output counts. At 602,112 triangles this removes a 14,450,688-byte temporary array, plus output growth overhead. This relies on `join` choosing the smallest corner as root, so roots are emitted before their other corners. Frozen-reference tests check fan ordering and normal bits, including reordered, disconnected, degenerate and empty input. [Source](../../../src/sculpt/shading.rs)

## Experiments not retained

The upload trace measured about 34 ms in Standard's vertex-storage stage, despite release adding only 27 vertices. That stage includes more than destination allocation. Two tested experiments did not materially change its timing:

- Queue-based initialization instead of mapped replacement: about 34 ms remained.
- Bounded 4 KiB allocation rounding: hardware regressions confirmed reuse for small fan changes and resize/shrink across block boundaries, but the stage still took about 34 ms.

Both policies were removed from the final implementation. The retained profiler can investigate staging, copying and driver behaviour more closely. The [measurement report](README.md) retains the experimental results and patches against the diagnostic baseline; neither experiment is presented as a performance improvement.

## Remaining candidates, ranked by current evidence

The release material-validation candidate is now completed in the [docked comparison follow-up](../release-material-validation-2026-10-30/README.md). Reusing successful comparisons of shared immutable payloads reduced that stage from about 21 ms to 4 ms while preserving ordinary floating-point equality and unseen material-edit rejection.

| Area | Evidence | Next investigation |
| --- | --- | --- |
| Display copying | [Discarded release shading copies are now removed](../release-shading-copy-2026-10-30/README.md), saving about 1–1.4 ms in that stage. Remaining copying is about 9.6–10.5 ms. During-stroke snapshots also clone several topology-derived arrays. | Separately measure shared immutable arrays. Whole-process RSS did not improve in the copy-removal measurements; retain memory checks alongside timing comparisons. |
| Face-data comparisons during upload | About 4.7–5 ms for colour/UV comparisons in the upload probe, even when their payloads are unchanged. | Investigate shared payload identity or channel revisions. Index/fan changes must still be detected independently. |
| Brush-wide and dab-wide calculations inside vertex loops | `Brush::weight` evaluates rotation `sin_cos`; pressure `powf` appears inside sculpt, smooth and paint vertex loops. Hard/Square alpha branches do not consume the source-level fade calculation. | Measure a prepared brush/dab representation, and inspect whether the compiler already hoists or sinks this work. Preserve operation order, alpha rotation, pressure response and symmetry/stencil behaviour. No speedup is claimed yet. |
| Deformation guards | Active-face discovery scans triangles; rejected proposals can repeat guard calculations and full reduction-flag scans for up to 12 or 20 passes. | Profile ordinary and difficult deformation cases separately before caching invariants or tracking sparse reductions. Keep winding, sliver, nearby-sheet and local-rejection protections. |
| GPU batching | A fresh mapping table and growing packed vector are built for each batch. The current capacity fixtures do not exercise large real batches. | Measure a bounded forced-batching workload before reserving additional capacity or reusing mappings. Preserve face order, UV/colour alignment and binding limits. |

Relevant follow-up locations: [release/display copying](../../../src/viewport/gpu/display.rs), [upload comparisons and batching](../../../src/viewport/gpu.rs), [material layers](../../../src/sculpt/material/layers.rs), [brush weights](../../../src/sculpt/brush.rs), [sculpt loops](../../../src/sculpt/mesh.rs), [smoothing](../../../src/sculpt/smooth.rs), [paint weighting](../../../src/sculpt/material.rs), and [deformation guards](../../../src/sculpt/deformation.rs).

The smaller material-stride scan measured roughly 1.5 ms and currently has lower priority. Source-level repetition alone is not sufficient evidence to change numerical formulas, add retained caches or claim an improvement. Future candidates should use focused geometry/pixel checks and matched timings, as this batch does.

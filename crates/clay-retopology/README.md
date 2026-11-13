# Native surface-field retopology

Experimental Rust implementation for the automatic-retopology replacement in
PR #119. It now powers the app's experimental guided-retopology preview.
It is not a quality-equivalent release of QuadriFlow.

The pipeline follows Huang et al., [QuadriFlow (2018)](https://stanford.edu/~jingweih/papers/quadriflow/):

1. Conforming surface sampling and an area-weighted hierarchy.
2. Fourfold tangent-orientation and position fields.
3. Original integer seeding, frame/seam balancing and ordered maximum flow on
   closed surfaces; constraint-aware minimum-cost correction for hard boundaries/features.
4. Exact subdivision of long integer offsets.
5. Upstream coarse connected-fan inversion repair, bounded grid-distance SAT
   patches, and fine-level repair as a fallback.
6. A shared-vertex tangent-plane solve, upstream edge-graph coarsening, quad
   triangle-fan separation before quad pairing, angle-based hole filling, valence
   cleanup, and dynamic optimization of the quad graph.
7. Projection to the source, outward render-diagonal selection, sampled shape-error
   checks, and topology validation.

The default hierarchy and field solver now ports the original serial CPU
implementation, including aggregate edge weights, compatible running averages,
six sweeps per level and propagation in both directions. On two whole-character
inputs, every uniform unconstrained direction and origin matches pinned C++
exactly. Soft guide strengths and scalar painted spacing remain supported Rust
extensions; their focused regressions pass. Hard boundaries remain hard during
coarsening. Hierarchy depth and aggregate samples are bounded, and no process
global random state or libc random calls are used. See the
[field port](../../docs/validation/guided-retopology/native-field-port.md) and the
[fine-neighbour-order correction](../../docs/validation/guided-retopology/native-fan-order.md).
Fine neighbours now follow the original surface-fan traversal rather than vertex
number order. The regularized whole character passes the existing checks against
the original sculpt (9,157 quads); the original irregular input still fails shape
coverage without preparation. Native input preparation now redistributes the
working triangles while preserving sampled two-way surface coverage. The new
integer mapper matches pinned C++ on three whole-character inputs. The public
`reconstruct_regularized_with_progress` composes five-pass preparation and the
normal solver, then independently checks against the untouched original using
its paint and vertex indices. The original character produces 10,122 accepted
quads entirely in Rust. Prepared-input release reconstruction takes 8.32 s;
the complete original-input path takes 25.35 s. See the
[integer port, timings and visual limits](../../docs/validation/guided-retopology/native-integer-port.md).
The [app integration report](../../docs/validation/guided-retopology/native-app-integration.md) records the controls, attribute transport, coordinate boundary and remaining failures.

The local solver encodes regularity and nonnegative-area constraints for
[Batsat](https://github.com/c-cube/batsat), an MIT-licensed Rust SAT solver.
Coarse repair uses the scalar-coordinate formulation adapted from upstream
`localsat.cpp`; the fine fallback retains its nine-state edge/table formulation.
Both preserve fixed coordinates and validate decoded assignments. A controlled
fine-scalar trial left more body inversions, so it was not retained. See the
[frozen-patch comparison](../../docs/validation/guided-retopology/native-junction-repair.md).
It uses cancellation callbacks, bounded patches, and a per-solve work/time budget.
Audit-only surrounding-grid controls include hierarchy rebuilding, conflict-led
boundary expansion and global reflow with temporary coordinate locks. The global
reset trial worsened the body and is rejected; reflow rolls back on an increased
inversion count. These experiments have not replaced the normal repair pipeline.
It is not the authors' C++ SAT implementation. The flow
implementation currently operates at one resolution. Neither C++, Blender, nor
an external solver process is a runtime dependency.

`Options::guides` accepts any number of surface samples from independent paths.
Compatible fourfold directions are combined. `Options::detail` is a separate
per-vertex signed field: -1 means less detail, 0 automatic, and +1 more detail.
Its density multiplier ranges from 1/4 to 4, normalized by surface area to keep
the requested global budget. Empty detail is automatic everywhere. These core
inputs still require app painting, persistence, and path-selection integration.
The optional adaptive term is an experimental one-ring normal heuristic,
disabled by default. Only uniform base sizing has input-resampling coverage;
shape-adaptive allocation still needs a sampling-independent curvature estimate.

Cancellation, explicit sample and local-search budgets, and output manifold
validation return errors instead of publishing partial meshes. Final output must
preserve component count, boundary count, and Euler characteristic. Surface-fit
queries sample vertices, face centroids, and edge midpoints in both directions; they do not
certify absence of intersections or a continuous Hausdorff bound. Character
quality, same-surface resampling, broader boundary/feature cases, and
supported-platform builds remain acceptance requirements.

The focused checks cover closed-sphere reconstruction, an open planar surface,
two independent direction guides, final polygon-size changes from signed density,
integer-flow optimality on small exhaustive cases, SAT assignments against brute
force, exact long-edge subdivision, fan separation, render-diagonal selection,
and cancellation. `Layout::triangles` supplies a manifold triangulation of the
quads; callers must use it rather than choosing every diagonal independently.
The biped now passes through the complete native path from the original sculpt.
Its coarse fingers, pole placement and app integration still require work before
feature acceptance. See the [validation record](../../docs/validation/guided-retopology/README.md#native-rust-implementation-in-progress).

Focused checks (with the repository's memory and build-lock rules):

```sh
cargo test --manifest-path crates/clay-retopology/Cargo.toml -j 1 regularized_ -- --test-threads=1
cargo run --manifest-path crates/clay-retopology/Cargo.toml -j 1 --example compare -- source.clay output.obj 5000 --regularize
```

The comparison executable refuses to overwrite an existing output. It reports
stage timings and Linux resident memory when available; OBJ output is for visual
review, not an app-save or attribute-transfer implementation.

`symmetry.rs` adapts the extrinsic tangent-lattice helpers from
[quadrs](https://github.com/vidstige/quadrs), under the included
[`LICENSE-quadrs`](LICENSE-quadrs). That portion implements the field math of
[Instant Field-Aligned Meshes](https://igl.ethz.ch/projects/instant-meshes/).
The regularity, flow and inversion code is implemented in Rust under this
repository's GPL-3.0-only license. `field/reference.rs`, `geometry.rs`, `coarse.rs`, `extraction.rs` and `quad.rs` adapt
selected upstream QuadriFlow routines; their original copyright and full terms
are retained in [`LICENSE-QuadriFlow`](LICENSE-QuadriFlow). See the
[stage audit](../../docs/validation/guided-retopology/upstream-audit.md) for the
pinned source, exact comparisons, intentional differences and remaining gaps.
No upstream C++ library is bundled or used by the app.

`resample.rs` uses split/collapse/swap/relax/project working triangles, with
local guards informed by VCGLib; its source attribution is retained in
[`LICENSE-VCGLib`](LICENSE-VCGLib). Original-surface witnesses reject local
changes that would lose reverse coverage. The audit command
`prepare-surface source.clay working.clay report.json target [iterations]`
exports a checked preparation without changing the source. It supports signed
detail transfer internally; the CLI comparison uses uniform detail. Preparation
does not change the default solver and is not a standalone quad output tool.

The public `resample::prepare` API exposes the preparation report and transferred
detail. `reconstruct_regularized_with_progress` composes it with reconstruction;
plain `reconstruct_with_progress` skips this extra preparation for controlled
already-prepared inputs. The `compare` example opts into it with `--regularize`.

`integer/maxflow.rs` adapts the ordered two-tree implementation from Boost Graph,
under [`LICENSE-Boost-MaxFlow`](LICENSE-Boost-MaxFlow). The port includes local
MT19937/shuffle arithmetic and has no process-global random state. Independently
generated C++ mapping fixtures and their provenance are in `tests/data/`.

The `audit` feature also supports a controlled field swap before integer
correction: `capture-field`, `export-field`, `import-field`, then
`integerize-field`. The exported fine geometry, normals, areas, adjacency and
uniform spacing are shared with a pinned C++ field-only harness; only directions
and origins are imported. Input identity, finite/unit/tangent fields and atomic
imports are checked. Audit JSON uses exact floating-point round trips so saved
comparisons do not silently perturb the mesh or fields. These commands exclude
guides, painted density and hard constraints. See the
[field comparison](../../docs/validation/guided-retopology/native-field-comparison.md)
for the runner, results and limits; this is developer tooling, not an app backend.

Integer-stage controls use `export-field-grid`, `export-corrected-grid` and
`import-integer-grid`, with `make-integer-reference.py` and `compare-integers.py`.
They compare correction before subdivision, validate input identity and edge
transports, then retain native subdivision, repair and geometric acceptance.

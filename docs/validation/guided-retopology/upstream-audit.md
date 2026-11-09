# QuadriFlow implementation audit

Reference: [hjwdzh/QuadriFlow at 810b7a0](https://github.com/hjwdzh/QuadriFlow/tree/810b7a0967c35b0dc85b4464e3835e26a756c967).
This is a comparison of individual stages on identical inputs, not a claim that
Clay Form reproduces the complete upstream pipeline. The app backend remains
unchanged. Source code was studied and selected routines were adapted into Rust;
the upstream notice and full terms are retained in
[LICENSE-QuadriFlow](../../../crates/clay-retopology/LICENSE-QuadriFlow).

## Corrections to the earlier implementation

**Latest on 6 November:** the [Rust integer mapper](native-integer-port.md)
now matches every C++ edge and face frame on three whole-character fields,
including the prepared input. It ports seeding, frame union, both balancing
passes and the ordered small/Boykov–Kolmogorov flow branches. This mapper is
the normal choice for closed surfaces without hard constraints; boundaries and
hard features retain the constraint-aware solver. The new public
`reconstruct_regularized_with_progress` composes native preparation, the normal
solver and immutable-original validation. It produces 10,122 accepted quads
from the original sculpt with no C++ stage import. App integration remains open.

**Earlier on 6 November:** a new audit-only Rust preparation replaces MeshLab for
the [same-input mapping experiment](native-input-preparation.md). Native fields
again match pinned C++ exactly. Importing the original default integer mapping
on those fields yields 10,122 quads passing the unchanged original-source gates;
native correction yields 9,521 quads and fails source coverage. This isolates
the integer stage as a whole, including seeding, seam construction, balancing
and flow. It does not identify the maximum-flow routine alone as the cause.
The default algorithm selects Boost Boykov–Kolmogorov at this supply size;
the optional minimum-cost branch uses LEMON. Neither preparation nor this C++
control has been promoted into the public/default solve path.

**6 November:** the fine field input now follows upstream's directed-edge fan
ordering. The previous common-input field proof supplied vertex-sorted adjacency
to both solvers and therefore did not cover the upstream adjacency builder.
Stable hierarchy pairing and running compatible averages depend on neighbour
order. The controlled order-only change and subsequent Rust port preserve exact
C++ field parity and produce a regularized whole-character result that passes
the unchanged checks against the original sculpt. The original irregular input
still fails coverage. See [the experiment and integer controls](native-fan-order.md).

The pinned upstream `ComputeMaxFlow` uses one edge-graph level. Its main flow
correction is not a multiresolution solve. `FixFlipHierarchy` and
`AdvancedExtractQuad` use the graph hierarchy. The earlier suggestion that a
missing multiresolution main-flow pass explained the character failure was
incorrect.

The continuous fitter previously optimized individual source samples. Upstream
collapses zero-offset groups, selects a representative tangent plane for each,
averages ideal displacements per ordered group pair, and solves their positions
together. The Rust implementation now follows that construction, using a
cancellable conjugate-gradient solve with explicit residual validation.

Extraction now uses the upstream zero-edge graph coarsening order, retaining
original face/corner identities. Frame rotations must be canonicalized modulo
four at the comparison boundary: Rust's lattice accepts arbitrary signed quarter
turns, whereas upstream's `rshift90` assumes values from zero through three.
An initial comparison that omitted this conversion was invalid and was replaced.

Quad cleanup now removes valence-two vertices, including collapsed two-quad
components, and splits high-valence closed fans. The previous port skipped the
collapsed case and left three detached four-vertex/two-face bubbles on the biped.
They are impossible enclosed geometric surfaces and cause singular tangent-plane
systems. Component, boundary and Euler checks now also run before the dynamic
optimizer, in addition to the final validation.

Coarse greedy repair now follows upstream's connected-fan transport and
propagates the corrected offsets through the saved graph maps. An explicit
stack replaces recursive calls. The pinned function's length-two retry uses an
already-drained work queue; the port matches its effective length-one behaviour.
The same-source comparison verifies every resulting integer offset and face
rotation, rather than only counting remaining flips.

A separate bounded Batsat pass marks complete fans and crosses edges by their
integer L1 lengths, operating from graph level five down to the fine level.
It retains per-axis locks and regularity/nonnegative-area checks, with explicit
patch and time limits. Coarse SAT now uses scalar coordinates with forbidden
closure/negative-area assignments, adapted from pinned `localsat.cpp`. The
native engine, propagation, clause order, budgets and hierarchy traversal still
differ from the upstream external executable. The existing fine-level table
encoding remains the fallback. The [frozen junction comparison](native-junction-repair.md)
checks the exact upstream CNF in both engines and separately validates the Rust
encoding against exhaustive small problems.

The post-extraction optimizer retains source membership through vertex splits,
walks source neighbours for tangent-plane updates, interpolates missing edge
directions around poles and solves the quad graph together. It normalizes the
transport rotation axis (upstream discards the return of `axis.normalized()`),
handles parallel interpolation directions without NaNs and checks numerical
convergence. Closed-fan cleanup has focused tests; upstream boundary/sharp corner
behaviour is not yet fully ported.

An outward render-diagonal selection handles concave quads while retaining the
global uniqueness of diagonals around poles. `Layout::triangles` is the validated
triangulation and must be retained by callers. This is not a relaxation of any
normal, topology or sampled-distance gate.

## Identical-input comparisons

The unmasked biped's corrected 5,000-target lattice has 67,764 samples after
integer subdivision. Capturing it once avoids rerunning the field/flow/SAT
stages for every geometry change. All comparison executables ran sequentially
under the shared build lock, a 500 MiB memory cap and no process swap.

| Stage | Result |
| --- | --- |
| `optimize_positions_fixed` | Maximum per-sample position difference 2.6334e-10; RMS 5.0455e-11 across 67,764 samples. |
| `FixFlip` / `PropagateEdge` | Integer corrections and face rotations byte-identical on the pre-repair biped lattice; both reduce 1,308 inverted cells to 18. |
| `DownsampleEdgeGraph` | All twelve graph levels byte-identical after canonicalizing quarter turns, including offsets, frame rotations, permissions and edge/face incidence. |
| Dynamic optimizer fed the earlier Rust bubble-containing intermediate | First three iterations agree within 2.1e-10. Upstream Cholesky reports a numerical failure at iteration 3, then continues with invalid solutions. This is a failed reference solve, not a valid parity comparison. |
| Dynamic optimizer after cleanup | Against upstream with the explicitly labelled axis-normalization correction: maximum position difference 1.0371e-10; RMS 4.2099e-11 across 4,579 output vertices. |

The position-stage fixture includes curved frames, unequal spacing and collapsed
sample groups. The graph fixture is a refined periodic grid. Expected results
were generated from the pinned C++ functions; the Rust tests compare against
those independent results. Separate tests cover valence-two cleanup, detached
bubble removal, high-valence splitting, fixed-coordinate elimination,
cancellation and concave-quad render diagonals.

## Reproducing the comparisons

The optional `audit` Cargo feature enables an offline snapshot/replay example;
it adds no runtime dependencies to the normal crate build. Snapshots are trusted
local developer fixtures tied to this implementation, not app documents or a
public interchange format. The example refuses to overwrite outputs.

```sh
cargo build --locked -j 1 --manifest-path crates/clay-retopology/Cargo.toml \
  --features audit --example audit
# Run each command under the repository's build lock and memory limits.
audit capture source.clay lattice.json 5000
audit capture-raw source.clay before-flips.json 5000
audit repair-coarse before-flips.json repaired-integers.txt
audit export lattice.json fixed.input
audit fit lattice.json fixed-rust.txt
audit export-coarse lattice.json coarse.input
audit trace-coarse lattice.json coarse-rust.trace
audit export-dynamic lattice.json dynamic.input
audit fit-dynamic lattice.json dynamic-rust.txt
audit trace-dynamic lattice.json dynamic-rust.trace
audit resume lattice.json validated.obj
```

The scripts in [scripts/retopology](../../../scripts/retopology) generate small
comparison programs from the pinned upstream checkout and verify the consulted
source files against that commit. Compile them separately with Eigen headers;
there is no C++ build step in the Rust crate or app. `make-dynamic-reference.py`
accepts explicit `normalize-axis` and `trace` variants; the latter writes through
`QF_TRACE` and reports the linear solver status. The graph generator accepts
`repair` to compare `FixFlip` and its downward propagation. The fixed and graph generators
leave the compared function bodies unchanged apart from class wrapping.

## Complete extraction comparison — subsequent pass

`extraction.rs` replaces the previous quad-first fan splitting and centre-fan
hole filling with `AdvancedExtractQuad`, `BuildTriangleManifold` and `FixHoles` /
`QuadEnergy` from the same pinned source. Triangle fans are separated before
pairing diagonals. Split vertices keep their original positions and source
memberships until optimization. Hole filling minimizes the upstream signed
corner-angle objective; memoizing sub-polygons avoids repeated recursive work.

The valence port now begins each reverse fan walk at the same directed edge as
upstream and compacts vertices in original index order. This matters: rotating
the walk changes the candidate high-valence cuts, not just vertex numbering.

`make-extraction-reference.py` copies the original function bodies, verifies the
source against the pin, and links only the original `dedge.cpp` with Eigen. Its
normal mode includes `FixValence`; a `raw` argument skips that final call. It is
an offline harness, not a dependency of the Rust crate or app.

| Target | Before cleanup (vertices / quads) | After cleanup | Comparison |
| ---: | ---: | ---: | --- |
| 5,000 | 4,639 / 4,588 | 4,585 / 4,583 | Positions, memberships and quad indices exactly equal at both stages |
| 10,000 | 10,655 / 10,620 | 10,620 / 10,618 | Positions, memberships and quad indices exactly equal at both stages |

The checked-in `extraction-sphere.input` / `.expected` fixture tests the complete
extraction and cleanup against independently executed C++ on a small curved
surface. Additional regressions exercise disconnected triangle fans, closing a
missing cube face without new vertices, and cancellation in the hole search.
The user biped and its large snapshots remain local.

```sh
audit repair before-flips.json repaired.json
audit export-extraction repaired.json extraction.input
# Feed extraction.input into the separately compiled upstream harness.
audit extract-raw repaired.json rust-before-cleanup.txt
audit extract repaired.json rust-after-cleanup.txt
audit resume-reference repaired.json cpp-extracted.txt validated.obj
audit validate-obj repaired.json original-quadriflow.obj
audit diagnose-obj repaired.json original-quadriflow.obj measurements.json
audit diagnose-candidate repaired.json rejected-candidate.audit.json
```

`resume-reference` uses the C++ positions/connectivity/memberships followed by
the same Rust dynamic fitting and acceptance gates. Both biped inputs still fail:
5,000 at fingertip coverage and 10,000 at outward triangulation near the crotch.
Replacing extraction alone therefore does not fix the character. `diagnose-*`
records failures and geometry for offline investigation; it never returns an
accepted `Layout` or an app document. Diagnostic distances use topology-only
render diagonals and are not a continuous Hausdorff or self-intersection test.

The original Blender 5,000-target output also fails our unchanged strict gates:
3 quads have no diagonal whose two triangle normals agree with their nearest
source triangles; its worst sampled source coverage is 1.749 grid spacings.
Do not claim that an original complete QuadriFlow run passes these particular
gates. It nevertheless retains the fingertip better than the current 5,000
Rust candidate. See [current measurements](native-extraction-progress.json).

The orientation/position field hierarchy, integer regularity correction and SAT
repair still contain independent implementations. Complete extraction parity
on these closed, unconstrained inputs does not prove boundary/sharp-corner
parity: Rust preserves real open boundaries rather than filling them, rejects
oversized holes, budgets traversal and supports cancellation. Field quality,
thin-feature coverage and continuous fitting remain the next investigation.
Painting and app integration remain deferred until the automatic result is useful.

### Fitting diagnosis

The 5,000 candidate already has a worst sampled source-coverage error of 3.202
local spacings before final surface projection/untangling; afterwards it is
3.573. Its four quads without an outward diagonal become zero. At 10,000, the
corresponding counts are 56 before fitting and 55 after; coverage changes from
1.843 to 1.845 spacings. Thus fitting can worsen thin-feature loss, but the main
failures already exist before this final stage.

Fitting now chooses an outward diagonal, where available, **before** attempting
to smooth a quad. A regression verifies that a valid concave interior grid
patch stays stationary. This fixes unnecessary movement on that fixture, but
does not resolve either frozen biped failure or change the reported worst-case
errors. No acceptance threshold was
relaxed. The 30-test suite passes; the final full-crate lint check is recorded in
the measurements.

The diagonal-first fix changes 52 positions in the 5,000 candidate (maximum
movement relative to the previous fitting result 0.02738). The first failing
left-fingertip distance improves from 0.11111 to 0.09024, still above 0.08149.
The worst coverage error on the other hand is unchanged. The 10,000 candidate's
positions and quads are unchanged. This is a local improvement, not acceptance.


## Unmodified positioning control — 5 November 2026

The frozen 10,000-target state was exported through `audit export-dynamic` and
fed into `optimize_positions_dynamic` from the pinned source without the
axis-normalization correction. The generated function body differs only by
removing its class qualifier. Source geometry, field directions/normals,
extracted connectivity, source memberships and ideal-edge observations are
identical inputs. This tests the positioning stage, not the entire upstream
field-generation/remeshing pipeline. No runtime algorithm changed in this pass.

| Before final projection/untangling | Rust | Unmodified upstream |
| --- | ---: | ---: |
| Output vertices / quads | 10,620 / 10,618 | 10,620 / 10,618 |
| Quads without an outward diagonal | 56 | 57 |
| Worst sampled source-to-result distance | 0.106219 | 0.103916 |
| Distance divided by local spacing | 1.84340 | 1.80342 |

Fifty-four failing quad indices are shared. Both worst coverage samples occur
at the same fingertip. An instrumented upstream rerun reports success for all
ten linear solves, with relative residuals between 3.92e-16 and 1.14e-15, and
writes byte-identical final positions to the uninstrumented run. This is a valid
numerical solve, unlike the earlier bubble-containing failed comparison.

The dynamic outputs differ by up to 0.17030 model units. The control with only
upstream's discarded `axis.normalized()` corrected to `axis.normalize()` agrees
with the Rust output to 2.01e-10 maximum / 5.80e-11 RMS. The intended rotation
correction therefore explains the measured dynamic-stage divergence on this
case; it does not cure the shared folded patch.

A separate fixed-position comparison on this same snapshot covers 89,718
samples. Rust and the original `optimize_positions_fixed` agree within
2.73e-10 maximum / 7.32e-11 RMS. Consequently neither positioning port supplies
a plausible explanation for the common failure by itself.

### Locating the bad input

Of the 56 quads failing the Rust post-optimizer check, 39 already have their
quad area vector opposite the average assigned-source normal before dynamic
positioning; one is collapsed. After the first original dynamic solve, 44 of
that set have the same area-vector reversal. This diagnostic uses a quad area
vector and assigned normals, rather than the stricter final per-triangle test.
After the tenth solve, 51 remain opposite their assigned normals. The problem
is therefore not solely the final nearest-surface-normal validation heuristic.

A single-variable experiment replaces each initial output position with the
arithmetic mean of its existing source-member positions. The source arrays,
quad connectivity, membership lists, target vectors and flags are byte/token
identical outside that position block. It improves the worst coverage distance
to 0.08256 (1.43279 spacings), but leaves 59 quads without an outward diagonal.
Initialization alone is not a fix and was not adopted by the runtime solver.

Fifty-two of the 56 failing quads have all four ideal-edge observations; their
initial target area directions agree with the assigned normals (minimum dot
0.9768). Their mean relative target-loop closure error is 0.1443, compared with
0.01992 elsewhere. This is a clue about inconsistent geometric constraints, not
proof of a specific field or integer-correction defect. The next investigation
should compare those earlier stages, rather than adding more final smoothing.

The [machine-readable record](native-positioning-progress.json) includes input
hashes, controls, numerical status and bounded runtime/memory measurements.
The unmodified dynamic solve took 19.11 s at 56.7 MiB peak RSS in the unoptimized
C++ harness. It is not a performance benchmark against optimized Rust. All
heavy runs were serialized under the build lock and 500 MiB cgroup cap.

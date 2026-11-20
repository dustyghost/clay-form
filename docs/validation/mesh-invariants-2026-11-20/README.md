# Mesh invariant and frame-validation follow-up

Implements the approved findings from #168 and #169 on main after #167
(`b559a9a`). The audit first read the adaptive-sculpt research, native retopology
comparisons and the capacity/renderer validation chain. This change preserves
the existing mesh representation, solver and render scheduling.

## Correctness boundaries

The common modeling gate previously accepted two closed tetrahedral shells
welded at one vertex: all edges still had two oppositely oriented faces, but the
shared vertex had two disconnected links. The result gate now checks that every
surface vertex has one connected directed link, either a closed cycle or an
open path. Explicit wire branches and isolated vertices remain allowed. The
check reuses compact source-corner adjacency and costs O(V + F + sum(d log d))
time and O(V + F) storage; it does not introduce a retained topology cache.

Regressions include both closed and open pinched welds, valid boundary welds,
loose geometry, failure atomicity with existing Redo and requested vertex groups,
and exact Undo/Redo/project reopening. A small independent undirected-link oracle
checks 36 deterministic six-edit sequences across closed/open meshes, high
valence, three coordinate scales, triangle order and symmetry. Conservative
rejection is permitted; each fixture must execute successful edits. These tests
do not establish freedom from self-intersection.

Full CI exposed an older directed Move-weld test that expected two separate
quad patches joined at one vertex to succeed. That expectation now explicitly
asserts rejection and exact preservation of the source attributes; its valid
two-endpoint seam, UV, crease and stationary-target checks remain. Invalid Move
snap candidates are skipped, so release can still commit ordinary unwelded
movement. Saved meshes with pinched surface vertices may need topology cleanup
before operations guarded by the stronger manifold check can proceed.

The retopology final gate omitted output vertices while sampling output edge
midpoints and triangle centroids. A flat four-quad source with local spacing h=1
accepted a candidate centre vertex at z=1.5 and reported error 0.75. It now checks
every final output vertex against the same original-surface spacing rule, with
the existing cancellation and nearest-query budget. Public-layout tests reject
that spike and accept a 0.75h control at three scales without moving coordinates.
Malformed indices, repeated corners and nonfinite coordinates now return errors
before geometric indexing. The source/candidate remain unchanged on rejection.

This is a final-validation fix, not evidence that normal solver runs emitted
spikes. Initial fitting already projects vertices. Additional vertex queries can
make a borderline job reach the unchanged work limit earlier. Error remains a
finite sampled distance, not a continuous Hausdorff bound.

## Frame behavior and redundant work

An opt-in, test-only trace follows actual viewport input, prepared scenes,
freehand dabs, immutable GPU callbacks and frame completion. It records frame and
input identities, sample positions, document/callback revisions and elapsed CPU
timestamps. Explicit preview geometry has no base-document callback revision.

The regression sends an initial press, an 80-pixel pointer movement producing
multiple dabs, release without movement and an idle frame. It checks the current
contract: the scene/callback owns the pre-dab snapshot; subsequent updates carry
the changed document; release creates no extra Standard dab; one history entry
restores the exact geometry. The Vulkan variant executes actual callbacks and
compares Undo/Redo framebuffers with identical pointer state, including exact
restoration of the settled stroke frame. It runs in GPU CI.

This trace is not an input-to-present benchmark. The test uses a small synthetic
mesh and offscreen readback; timestamps include test overhead, and no latency or
frame-rate improvement is claimed. Desktop presentation and real-asset sessions
remain under #114; capacity/HD architecture remains under #103.

HUD and action availability now share one effective hidden-face count per
document display invalidation. On unchanged frames the count is O(1), instead
of rescanning F triangles for every reader. Any existing display invalidation
discards it, including geometry, visibility, model switching, history and level
changes. This conservative lifetime also invalidates on unrelated display edits;
it does not claim patch-local sculpt performance. Tests count scans across
repeated reads and exercise hiding, groups, topology, model switches, Undo/Redo,
New/Open and no-op operations.

## Benchmark records

The capacity runner now retains a streamed source/configuration manifest and
executable digest, including embedded shaders and path dependencies. It rejects
source/configuration changes during its build. A shared nonblocking lock covers
build and workloads; host RAM, PSI and competing compiler checks precede builds
and each workload. Compilation explicitly uses at most two jobs, or one with
less than 2 GiB available or observed memory stalls. Exhausted/contended hosts
defer work. These are guards, not hard allocation ceilings. See the updated
[capacity reproduction instructions](../capacity-2026-10-30/README.md#reproduce).

Existing original-reference normal/pixel checks, rejected local release-shading
evidence, native solver parity fixtures and cancellation/history tests remain.
No geometry caps, solver budgets or machine-dependent CI timing thresholds change.
Known retopology feature/quality limits continue under #10.

## Focused reproduction

Local checks passed: 128 modeling regressions (13 optional captures ignored),
six directed Move-weld/snap regressions (one optional capture ignored),
eight solver validation/fitting/cancellation tests, the visibility-cache test,
CPU and Vulkan frame tests, and all 17 Python harness tests. The actual input
trace observed one initial dab, nine dabs for the pointer jump, no release dab
and five matching callbacks. Undo/Redo restored exact geometry; Redo restored
the settled framebuffer byte-for-byte. [Raw trace and executable provenance](frame-trace.json)
retain the sequence. Vulkan used llvmpipe / Mesa 26.2.7, one software-rendering
thread, at 1000×800. This is correctness evidence, not hardware latency data.

The framebuffer fixture disables egui's control fade-in animation: an initial
comparison differed only at the fading viewport controls, outside the sculpt.
The geometry comparison remains exact, with no pixel tolerance. Local builds
used one compiler, the shared lock and disabled debug symbols; full builds were
not repeated beyond the focused test binary needed for these checks.

Run Cargo commands sequentially under the shared build lock with available RAM
checked first; use one compiler on the 8 GiB laptop when resources are tight.
The Python runner acquires the lock itself and must not be wrapped in another
`flock`.

```sh
cargo test --locked -j 1 --manifest-path crates/clay-retopology/Cargo.toml --lib final_layout_validation -- --test-threads=1
cargo test --locked -j 1 --lib modeling_fan_gate -- --test-threads=1
cargo test --locked -j 1 --lib modeling_bounded_edit_sequences -- --test-threads=1
cargo test --locked -j 1 --lib modeling_pinched_weld_rejection -- --test-threads=1
cargo test --locked -j 1 --lib modeling_boundary_edit_sequence -- --test-threads=1
cargo test --locked -j 1 --lib move_snap -- --test-threads=1
cargo test --locked -j 1 --lib hidden_count_cache -- --test-threads=1
cargo test --locked -j 1 --lib sculpt_frame_revision_trace_cpu -- --test-threads=1
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=1 CLAY_SCULPT_FRAME_TRACE=/tmp/clay-frame-trace.json cargo test --locked -j 1 --lib sculpt_frame_revision_trace_gpu -- --ignored --test-threads=1
python3 scripts/test-benchmark-capacity.py
```

Full suite, lint and configured GPU/platform checks run in PR CI. Hardware
pointer-to-present measurement and long-session capacity validation are separate
work; this follow-up neither repeats them nor substitutes offscreen timings.

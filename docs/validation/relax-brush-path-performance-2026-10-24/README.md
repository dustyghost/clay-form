# Relax brush, Detail path performance and CI capture review — 24 October 2026

This batch implements #56 and #45 and moves redundant screenshot-only work out of routine PR CI. Private diagnostic/session exports were not modified or added to the repository.

## Relax brush

Relax paints the same connected-reference relaxation engine as the mesh command, with brush falloff, pressure, symmetry and stencil weights. It caches reference adjacency and correspondence across dabs and consecutive strokes. Another display-affecting edit invalidates that reference; Undo/Redo, model changes and reopening also start a new reference. Brush/camera changes and saving do not. The guide explains this lifetime and the sampled surface tolerance.

Focused regressions cover local influence, zero/partial pressure, falloff, symmetry, masks, hidden faces, creases, unchanged topology/attributes, repeated strokes, Inflate cleanup, groove retention, reference invalidation, save/reopen and pointer-driven Undo/Redo/Escape. Cancellation discards the active stroke without clearing redo. Mesh-command regressions for grooves, redistribution beyond the original fan, boundaries and nearby thin sheets remain intact. Native pen hardware has not been exercised; synthetic pointer events and pressure samples cover the application paths.

Manual matched-camera captures use an aggressively inflated 866-vertex, 1,728-triangle fixture, three strokes of twelve dabs, strength 1 and tolerance 0.01. Wire spacing improves around the bulge; shaded views and the silhouette retain the inflated form. Large tip polygons remain because the existing topology and surface tolerance constrain motion. Controls expose strength and tolerance; the new preset is available to older projects through Add brush.

- [Before wireframe](../../images/relax-brush-wire-before.png) / [after wireframe](../../images/relax-brush-wire-after.png)
- [Before shading](relax-brush-shaded-before.png) / [after shading](relax-brush-shaded-after.png)
- [Brush controls](../../images/relax-brush-controls.png)

```sh
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_GUIDE_PREVIEW=/tmp/clay-relax-brush-review cargo test --locked --lib relax_brush_manual_capture -- --ignored --nocapture
cargo test --locked --lib relax_brush
```

## Focused performance measurements

Only the two diagnostic examples were built optimized for these measurements; no application release build or full local test suite was run. Times are local CPU measurements on an Intel Core i7-7500U host, exclude compilation/rendering and do not establish hardware-tablet latency or a universal performance guarantee.

The Detail reproduction uses a closed cube with 128 × 128 × 2 divisions, a saved surface curve, radius 0.3, default Detail strength/spacing and Adaptive sculpt. The complete path has 62 dabs and grows from 33,794 to 40,890 vertices.

| Work | Baseline main | Changed | Result |
| --- | ---: | ---: | --- |
| Live replay, dabs only | 3,311.6 ms | 2,294.1 ms | 31% less time |
| Final path reattachment | 145.4 ms | 146.3 ms | Unchanged work |
| Complete automatic stroke | 3,508.8 ms | 2,460.9 ms | 30% less time |

Baseline and changed output meshes **and saved path attachments match exactly**, including all mesh attributes. Complete automatic and live replay also match within each build. The optimization classifies shared vertices against conservative brush bounds once per adaptive operation, skipping distant faces before the existing exact membership tests. It does not change dab spacing, topology budgets, protection rules or safety checks. The bounds rebuild after each topology/position change. Focused comparisons include symmetry, stencils, masks, hidden faces, stamp reach boundaries and large translated coordinates. Earlier edge-metric/allocation experiments showed negligible optimized benefit and were discarded.

Reproduce with the same source file in both builds:

```sh
cargo build --release --locked --example path_profile --example relax_profile
CLAY_PROFILE_DIVISIONS=128 CLAY_PROFILE_SOURCE=/tmp/clay-path-source.clay CLAY_PROFILE_AUTOMATIC=1 target/release/examples/path_profile --synthetic 0.3 0 /tmp/clay-path-result.clay
CLAY_PROFILE_AUTOMATIC=1 target/release/examples/path_profile /tmp/clay-path-source.clay 0.3 0 /tmp/clay-path-comparison.clay
```

`CLAY_PROFILE_AUTOMATIC=1` requires a full path (`DABS=0`), times the actual automatic job and asserts that its mesh equals live replay. The synthetic fixture requires no local export. Automatic execution performs all samples together; live pointer input spreads equivalent work across frames and can consume fewer topology budgets when multiple dabs share a frame. The comparison deliberately holds samples and budgets equal.

Relax profile (13 repeated dabs; first setup separate from the remaining twelve):

| Vertices | Initial reference + first dab | Cached median | Cached maximum |
| --- | ---: | ---: | ---: |
| 33,794 | 152.4 ms | 26.4 ms | 27.5 ms |
| 98,306 | 386.7 ms | 34.4 ms | 39.1 ms |

```sh
target/release/examples/relax_profile 128
target/release/examples/relax_profile 128 128
```

These figures expose the one-time reference setup cost; adjacency and connected-face patches are reused thereafter. Each profile commits one undo step. Large meshes can still pause briefly while the first reference is built.

## CI capture audit

[GPU validation](../../gpu-rendering.md#preview-images) maps each removed capture to retained ordinary regressions. Clay and Detail image generators remain ignored manual tests. The independent Inflate image sequence is now `inflate_relaxation_manual_capture`; its geometry regression still runs normally.

`mesh_refinement_offscreen_review` still clicks Preview/Apply/Cancel, checks Undo/Redo, point-split controls and compact-window scrolling. All core GPU pixel/depth/clipping checks and other interactive control tests remain configured. The export-review tools and load profiler were already excluded from CI and were preserved. Relax's new documentation capture is manual only. The documentation screenshot script still includes Clay/Detail captures and now includes Relax brush images.

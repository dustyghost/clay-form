# Mesh refinement review — 24 October 2026

Stories #32 and #54 share one branch, `feat/mesh-refinement`.

## Vertex patches

The real application controls enable **Double ring sides (4 to 8)**, preview a drag from a four-quad junction, commit the eight-sided ring, and undo it. The sidebar displays the live radius during a drag. The same controls serve the Mesh panel, Modeling menu and Q popup.

[Preview and controls](../../images/split-point-octagon-preview.png) · [Committed patch](split-point-octagon-result.png)

Focused checks cover exact model-space radius on differently sized neighborhoods, eight connected ring vertices, attributes, symmetry, masked protection, oversized errors, reversible previews, Escape, one-step undo, repeat after switching tools and preset compatibility. The patch center and larger adjacent polygons use triangle fans; this is not an all-quad construction.

## Surface relaxation

The synthetic grooved fixture has 1,538 vertices and 3,072 triangles. It deliberately crowds vertices toward the ends of the groove. With strength 0.5, 10 iterations and tolerance 0.002:

| Measurement | Before | After |
| --- | --- | --- |
| Vertices | 1,538 | 1,538 |
| Triangles | 3,072 | 3,072 |
| Mean front-quad longest/shortest edge ratio | 1.692414 | 1.595014 |
| Signed volume | 8.199427 | 8.198796 |
| Maximum sampled reference deviation | 0 | 0.002000 |

1,188 vertices moved; 223 were protected and 66 were safety-limited. Mean edge aspect improved about 5.8%; volume changed about 0.0077%. These figures characterize this fixture, not every mesh.

[Wireframe before](../../images/relaxation-wire-before.png) · [Wireframe after](../../images/relaxation-wire-after.png) · [Shaded before](relaxation-shaded-before.png) · [Shaded after](relaxation-shaded-after.png) · [Preview controls](../../images/relaxation-controls-preview.png) · [Small window](relaxation-small-bottom.png) · [Raw measurements](relaxation-measurements.txt)

Visual inspection finds more even spacing and retains the groove and silhouette in matched shaded views. Tests also repeat three ten-iteration applications against one reference and check depth, volume, sampled deviation, attributes and topology. Separate cases cover masked transitions, open boundaries, nearby sheets, hidden faces, cancellation, stale drafts, no-ops, undo/redo, save/reopen, inactive models and retained subdivision levels.

Relaxation walks connected reference faces as vertices move, protects creases and bends above 35°, and backtracks unsafe triangle movement before freezing it. This permits travel beyond the original incident fan while avoiding global nearest-surface jumps to nearby sheets. Tangents are refreshed each iteration. Only faces touching backtracked vertices need rechecking. Surface distance is sampled at triangle centers and edge midpoints; this is not an exact Hausdorff bound or a global self-intersection proof. Crowded or sharply curved regions can remain unchanged. Work is cancellable and bounded; extreme-valence junctions are rejected explicitly. Consecutive applications keep a fixed reference until another document edit, Undo or reopening the project. Only the active level is relaxed; ordinary level-switching detail transport still applies.

## Reported weak relaxation and research

The supplied `clay-session-relax-mesh` export has 4,706 vertices, 9,408 triangles and 4,704 quads. Stage 011 is the pre-relaxation mesh; stage 012 records strength 1, six iterations and tolerance 0.002. All vertices were unmasked. The old operation improved mean quad edge ratio only 1.9%, while its 95th percentile slightly worsened. The exported project files and manifest are not committed.

The limiting factors were slow local redistribution, projection confined to each original vertex fan, whole-neighborhood freezing when a proposal exceeded tolerance, and the tight surface tolerance itself. The revised method walks connected reference faces, permits larger guarded steps and backtracks constrained moves. The default iteration count is now 30; strength 0.5 and the conservative 0.002 tolerance remain unchanged. The controls explain why strength alone cannot overcome a tolerance limit.

| Export measurement | Input | Recorded old result (1 / 6 / 0.002) | Revised preview (1 / 30 / 0.01) |
| --- | --- | --- | --- |
| Mean quad longest/shortest edge ratio | 1.521692 | 1.493259 | 1.408204 |
| 95th-percentile quad edge ratio | 4.277884 | 4.313942 | 2.332327 |
| Vertex-area coefficient of variation | 0.752580 | 0.686805 | 0.387340 |

The revised preview uses a **larger tolerance**, so this comparison includes both tuning and algorithm changes. Its maximum sampled deviation is 0.010000, signed volume changes from 6.802943 to 6.794202 (about 0.1285%), and all counts/connectivity/attributes remain unchanged. At identical settings of 1 / 30 / 0.01, the old implementation produced a 95th-percentile ratio of 2.913585; the revised implementation improves that by another 19.9%.

[Export wireframe before](export-wire-before.png) · [Export wireframe after](export-wire-after.png) · [Export shaded before](export-shaded-before.png) · [Export shaded after](export-shaded-after.png)

The captures use the export's camera orientation, with zoom reduced to 0.65 to include the whole silhouette. The wireframe spacing improves visibly; the matched shaded views retain the overall form, with the small permitted shape change quantified above. A 0.01 tolerance is a useful setting for **this model**, not a new universal default.

Research: [Botsch & Kobbelt, *A Remeshing Approach to Multiresolution Modeling*, SGP 2004, section 4](https://cg.cs.tu-dortmund.de/publications/2004-remeshing.pdf) describes tangential redistribution and an area-weighted variant. [CGAL's tangential relaxation documentation](https://doc.cgal.org/latest/PMP_Remeshing/group__PMP__local__remeshing__grp.html) provides a current implementation reference. An area-weighted experiment improved density but worsened quad stretch in our groove regression, so the final change retains uniform polygon-edge weighting. This implements constrained vertex relocation, not the paper's full split/collapse/flip remeshing pipeline.

## Aggressive Inflate check

A separate 866-vertex cube sphere receives eight full-strength Inflate dabs. The masked regression then relaxes the front while fixing the back and an explicit crease. Mean edge ratio in the inflated region improves from 1.264575 to 1.192510 (about 5.7%) at strength 1, 30 iterations and tolerance 0.01. Maximum sampled deviation is 0.007158. The bulge tip, fixed vertices, topology and attributes are checked. This does not flatten the bulge or remove its sharpest transitions.

The visual fixture uses the same Inflate strokes without the back mask: [wireframe before](inflate-relaxation-wire-before.png), [wireframe after](inflate-relaxation-wire-after.png), [shaded before](inflate-relaxation-shaded-before.png), [shaded after](inflate-relaxation-shaded-after.png). Existing sharp features and the coarse cap remain; relaxation cannot remove excess vertices or generate a replacement layout. A paintable Relax brush is tracked separately in [#56](https://github.com/dustyghost/clay-form/issues/56).

Additional regressions require meaningful spacing improvement and vertex travel beyond the original fan on a flat crowded mesh, and preserve the separation of two nearby sheets connected at a distant boundary. Existing groove, mask, cancellation, undo, persistence and subdivision tests remain in place.

## Reproduce

```sh
cargo test --locked --lib -- sculpt::relaxation:: app::tests::projection::relaxation_ modeling_double_ modeling_point_ring modeling_component_construction modeling_construction_presets viewport_menu_
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_GUIDE_PREVIEW=/tmp/clay-mesh-refinement cargo test --locked --lib mesh_refinement_offscreen_review -- --ignored --nocapture
```

For a local export, generate a separate project with measured quality and volume:

```sh
cargo run --locked --example relaxation_review -- clay-session-relax-mesh/stage-011.clay /tmp/clay-relaxation-improved.clay 1 30 0.01
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_REVIEW_ZOOM=0.65 CLAY_REVIEW_PROJECT=/tmp/clay-relaxation-improved.clay CLAY_REVIEW_MANIFEST=clay-session-relax-mesh/manifest.json CLAY_GUIDE_PREVIEW=/tmp/clay-relax-after cargo test --locked --lib project_file_offscreen_review -- --ignored --nocapture
```

Captures use the real egui/wgpu application with synthetic pointer events and Mesa llvmpipe/Vulkan. Preview, apply, Escape, Undo and Redo are exercised; the 640 × 480 window's Cancel control is clicked. Physical tablet feel remains for hands-on review. Full tests, formatting, lint and configured GPU regressions run in PR CI; no local release build was made.

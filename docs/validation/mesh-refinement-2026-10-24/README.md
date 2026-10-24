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
| Mean front-quad longest/shortest edge ratio | 1.692414 | 1.597350 |
| Signed volume | 8.199427 | 8.198874 |
| Maximum sampled reference deviation | 0 | 0.001971 |

1,188 vertices moved; 223 were protected and 60 were safety-limited. Mean edge aspect improved about 5.6%; volume changed about 0.0067%. These figures characterize this fixture, not every mesh.

[Wireframe before](../../images/relaxation-wire-before.png) · [Wireframe after](../../images/relaxation-wire-after.png) · [Shaded before](relaxation-shaded-before.png) · [Shaded after](relaxation-shaded-after.png) · [Preview controls](../../images/relaxation-controls-preview.png) · [Small window](relaxation-small-bottom.png) · [Raw measurements](relaxation-measurements.txt)

Visual inspection finds more even spacing and retains the groove and silhouette in matched shaded views. Tests also repeat three ten-iteration applications against one reference and check depth, volume, sampled deviation, attributes and topology. Separate cases cover masked transitions, open boundaries, nearby sheets, hidden faces, cancellation, stale drafts, no-ops, undo/redo, save/reopen, inactive models and retained subdivision levels.

Relaxation searches only each vertex's original incident triangles, protects creases and bends above 35°, and rejects unsafe triangle movement. Surface distance is sampled at triangle centers and edge midpoints; this is not an exact Hausdorff bound or a global self-intersection proof. Crowded or sharply curved regions can remain unchanged. Work is cancellable and bounded; extreme-valence junctions are rejected explicitly. Consecutive applications keep a fixed reference until another document edit, Undo or reopening the project. Only the active level is relaxed; ordinary level-switching detail transport still applies.

## Reproduce

```sh
cargo test --locked --lib -- sculpt::relaxation:: app::tests::projection::relaxation_ modeling_double_ modeling_point_ring modeling_component_construction modeling_construction_presets viewport_menu_
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_GUIDE_PREVIEW=/tmp/clay-mesh-refinement cargo test --locked --lib mesh_refinement_offscreen_review -- --ignored --nocapture
```

Captures use the real egui/wgpu application with synthetic pointer events and Mesa llvmpipe/Vulkan. Preview, apply, Escape, Undo and Redo are exercised; the 640 × 480 window's Cancel control is clicked. Physical tablet feel remains for hands-on review. Full tests, formatting, lint and configured GPU regressions run in PR CI; no local release build was made.

# Dynamic Mesh quads and protected adaptive topology

Issue #36, 26 October 2026. The primary workflow is Dynamic Mesh remeshing; Adaptive sculpt has its own topology choice and an independent unmasked-only option.

## Implementation and limits

Quad voxel reconstruction uses surface nets over the existing signed-distance/union field: one vertex per unambiguous crossed cell, one editable quad around each crossed grid edge. The existing marching-tetrahedra extraction remains the explicit Triangles mode. The grid determines quad direction; this is not guided retopology or animation topology.

Surface nets and other dual methods can produce nonmanifold connections when a cell contains multiple sheets. See the primary descriptions in [Frisken, SurfaceNets for Multi-Label Segmentations](https://pmc.ncbi.nlm.nih.gov/articles/PMC9623606/) and [Zint et al., Resolving Non-Manifoldness on Meshes from Dual Marching Cubes](https://diglib.eg.org/bitstream/handle/10.2312/egs20221029/045-048.pdf?sequence=1). This implementation conservatively rejects ambiguous cells and folded/degenerate output instead of silently emitting triangles. It does not implement the papers' multi-label or manifold-repair algorithms. A 0.02-voxel cell-interior margin prevents coincident dual vertices at exact grid tangencies.

Local quad refinement splits a vertex's incident edges, replacing each adjacent quad with three quads. The complete star is accepted atomically; no split propagates beyond it. This introduces irregular junctions. Simplification contracts a quad diagonal after checking its local link, visibility, masks, groups, stencil coverage, orientation, triangle quality and sampled surface error. Refinement allows at most 0.15 target-edge lengths of sampled deviation per operation; contraction allows 0.08 collapse thresholds. These are local sampled bounds, not global Hausdorff or cumulative error guarantees. Protected boundaries and sharp folds can prevent further adaptation. Existing all-quad geometry is required; mixed input gives an actionable error.

Unmasked-only adaptation treats every nonzero mask value as protected, and freezes faces touching that region. Brush deformation still uses the original mask strength. Tests compare protected face connectivity by vertex coordinates across compaction, rather than assuming vertex indices remain stable. Triangle operations now retain untouched quad polygons and reject splits that would modify hidden or protected incident faces.

New models default to quad remeshing. Missing topology fields in old saved settings load as Triangles. Adaptive sculpt defaults to its existing triangle behaviour. Settings, real polygon data and undo snapshots persist independently for the two workflows. Voxel remeshing continues to clear masks, require visible closed surfaces, transfer supported vertex materials/groups, and invalidate stale UV correspondence through the existing topology contract.

## Validation

- Focused adaptive checks: repeated refinement/coarsening, manifold vertex links, duplicate/degenerate faces, both topology modes at partial-mask boundaries, masked/hidden stamp exclusion, symmetry/stencil copies and interpolated attributes.
- Focused remesh checks: cube, cylinder and torus; repeated output remeshing; overlapping-volume fusion and disconnected components; ambiguous-cell rejection and cancellation; material/group transfer.
- Persistence and document checks: absent legacy fields, current round trips, rejected mixed input, strokes and mode changes through Undo/Redo, and existing subdivision-history behaviour.
- The real egui/wgpu control test clicks Dynamic mesh, Quads/Triangles, Remesh now, Modeling face selection, Adaptive sculpt and unmasked-only adaptation. It checks failure atomicity, Undo/Redo and project reopening, and captures shaded/wireframe output. Existing automatic-remesh stroke-release/Undo coverage runs with the new quad default.

For a unit-radius sphere at resolution 24, three successive quad remeshes produced:

| Pass | Quads | Mean absolute radial error | Mean `1 − radial normal · mesh normal` |
| --- | ---: | ---: | ---: |
| 1 | 2,630 | 0.002036 | 0.000188 |
| 2 | 2,622 | 0.004336 | 0.000231 |
| 3 | 2,622 | 0.006512 | 0.000292 |

These are fixture measurements, not bounds on arbitrary input. They show the small cumulative shrinkage of repeated reconstruction; retain a projection reference when preserving fine sculpt detail matters. No interactive speed claim is based on these debug-build checks.

The captures use software Vulkan/llvmpipe at 1200 × 1000. Solid and wire views were inspected for gaps, folded regions and protected-region changes. Adaptive junctions are visibly irregular; the shaded Clay stroke remains continuous at the refined patch.

![Editable quad remesh](quad-remesh-wire.png)

![Quad face selection](quad-remesh-face-selection.png)

![Adaptive quads beside a protected mask](adaptive-quads-mask-wire.png)

![Shaded adaptive result](adaptive-quads-mask-solid.png)

Reproduce the control test and captures:

```sh
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_GUIDE_PREVIEW=/tmp/clay-quad-review \
cargo test --locked --lib quad_topology_controls_offscreen_review -- --ignored --nocapture
```

The interaction regression is included in GPU CI. Image inspection remains manual; the test does not claim automated visual comparison. Full test/lint/GPU checks run in PR CI.

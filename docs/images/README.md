# Guide screenshots

Use real egui/wgpu captures of practice scenes. Capture a panel when the controls matter, or the full window for navigation and spatial results. Keep controls readable. Do not paint over screenshots or generate them with AI. Earlier checked-in captures used glow; regeneration now uses wgpu.

## Regenerate

On Linux with Rust and Mesa Vulkan installed, run from the repository root:

```sh
bash scripts/update-doc-screenshots.sh
```

After the script finishes:

1. Review the selected PNGs in this folder and the extra frames in the temporary directory printed by the script.
2. Check that guide steps, labels and captions still match. Keep the theme consistent.
3. Update the inventory below when captures change.

The script uses software rendering and defaults to two Mesa threads. It copies selected PNGs only after **all capture tests pass**, and exports the two sample reference images into `docs/examples/`.

See [Development](../development.md#update-guide-screenshots) for prerequisites and [documentation style](../documentation-style.md#make-every-screenshot-answer-a-question) for composition and review guidance.

## Inventory

| Images | What they explain | Capture test |
| --- | --- | --- |
| `alpha-library.png`, `alpha-editor.png`, `alpha-surface-stencil.png`, `alpha-adaptive-spiral.png`, `alpha-stencil-controls.png` | [Reusable image alphas and a placed surface stencil](../guide/sculpting.md#create-and-reuse-an-image-alpha) | `alpha_library_stencil_offscreen_review` |
| `submodel-delete.png` | [Deleting a selected hidden reference submodel](../guide/modeling.md#delete-a-submodel) | `delete_submodel_offscreen_review` |
| `detail-projection-preview.png` | [Reference-to-target detail transfer with match feedback before Apply](../guide/modeling.md#recover-detail-after-remeshing) | `detail_projection_offscreen_review` |
| `modeling-separate-fuse-result.png` | [Nonadjacent faces fused into separate tunnels in one edit](../guide/modeling.md#construct-a-region) | `modeling_region_fuse_offscreen_review` |
| `modeling-region-fuse-result.png` | [Connected faces fused into one open tunnel](../guide/modeling.md#construct-a-region) | `modeling_region_fuse_offscreen_review` |
| `modeling-boundary-join.png` | [Joining adjacent holes and opening exterior notches across depth loops](../guide/modeling.md#push-through-to-make-a-tunnel) | `modeling_boundary_join_offscreen_review` |
| `modeling-selection.png`, `modeling-extrusion-preview.png`, `modeling-tunnel.png`, `modeling-loop-preview.png`, `modeling-loop-drag.png` | [Component groups, extrusion previews, and matching-face tunnels](../guide/modeling.md#pen-modeling) | `modeling_workflow_offscreen_review` |
| `edge-loops-before.png`, `edge-loops-after.png` | [Support loops around an open hole](../guide/edges.md#add-support-loops-around-a-hole) | `edge_tools_offscreen_preview` |
| `crease-smooth-before.png`, `crease-smooth-after.png` | [Smoothness with and without creases](../guide/edges.md#control-subdivision-smoothness) | `edge_tools_offscreen_preview` |
| `crease-selection.png` | [Select and remove creases](../guide/edges.md#select-edges-and-loops) | `edge_tools_offscreen_preview` |
| `thickness-modifier.png` | [Rim loops and rim creases](../guide/edges.md#add-loops-and-creases-to-thickness) | `edge_tools_offscreen_preview` |
| `first-sculpt-before.png`, `first-sculpt-after.png` | Basic Standard stroke with a consistent camera (retained capture; the [first sculpt tutorial](../guide/getting-started.md) now makes a moon) | `guide_workflows_offscreen_preview` |
| `brush-reset-before.png`, `brush-reset-after.png` | [Reset](../guide/sculpting.md#example-reset-an-experimental-brush) changes settings but keeps the name | `guide_workflows_offscreen_preview` |
| `mirror-before.png`, `mirror-after.png` | [Replace one half from the other](../guide/modeling.md#mirror-one-side) with a consistent camera | `mirror_guide_offscreen_preview` |
| `symmetry-stroke.png` | [One input stroke, two mirrored marks](../guide/sculpting.md#symmetry) | `guide_workflows_offscreen_preview` |
| `adaptive-settings.png` | [Adaptive detail settings](../guide/sculpting.md#snake-brush-and-adaptive-sculpting), with the separate Decimate brush selected | `smooth_decimate_offscreen_review`, `cleanup-decimate-controls` |
| `smooth-pull-before.png`, `smooth-pull-after.png` | A short Snake pull smoothed back into the surface with fixed topology | `smooth_decimate_offscreen_review`, `cleanup-before` and `cleanup-smoothed` |
| `primitive-cube.png` | [Dimensions versus spans](../guide/modeling.md#parametric-primitives) | `guide_workflows_offscreen_preview` |
| `slice-trim-preview.png`, `slice-trim-result.png` | [Keep/remove preview and the resulting cap](../guide/modeling.md#slice); the result is orbited to expose the cap | `guide_workflows_offscreen_preview` |
| `reference-images.png` | [Placement, locking, and opacity](../guide/references.md) using front/side ellipsoid studies | `guide_workflows_offscreen_preview` |
| `obj-import.png` | [Sample pyramid](../guide/files.md#example-import-the-sample-pyramid) with the original sphere hidden | `guide_workflows_offscreen_preview` |
| `recovery-dialog.png` | [Recovery controls](../guide/files.md#recovery-saves) in a focused window | `guide_workflows_offscreen_preview` |
| `menu-placement.png` | [Docked versus pinned controls](../guide/workspace.md#example-keep-frequently-used-controls-open) | `guide_workflows_offscreen_preview` |
| `view-orthographic.png`, `view-perspective.png` | [Projection comparison](../guide/navigation.md#perspective-view) | `perspective_view_offscreen_preview` |
| `detail-groove.png` | [A narrow fold with raised shoulders](../guide/sculpting.md#detail-brush) | `detail_groove_offscreen_preview` |
| `mask-lasso-preview.png`, `mask-lasso-result.png` | [Protection before/after release](../guide/selection.md#example-protect-a-region-with-a-lasso) | `gpu_lasso_preview_and_commit` |
| `hide-lasso-preview.png`, `hide-lasso-result.png` | [Hiding before/after release](../guide/selection.md#hide-with-a-lasso) | `gpu_lasso_preview_and_commit` |
| `rotation-controls.png` | [Rotation rings and the live angle](../guide/modeling.md#example-rotate-a-cube-and-keep-its-recipe) | `gpu_rotation_controls_and_primitive_regeneration` |
| `modeling-paint-selection.png`, `modeling-face-loop.png`, `modeling-edge-ring.png`, `modeling-edge-loop.png` | [Paint selection and loop patterns](../guide/modeling.md#select-faces-edges-or-vertices) | `modeling_selection_offscreen_review` |
| `modeling-symmetry-selection.png`, `modeling-symmetry-extrude.png`, `modeling-dissolve-before.png`, `modeling-dissolve-after.png` | [Symmetry and dissolving loops](../guide/modeling.md#dissolve-selected-loops) | `modeling_symmetry_dissolve_offscreen_review` |
| `modeling-tap-repeat.png`, `modeling-milestone-inset.png`, `modeling-milestone-smooth.png`, `modeling-milestone-preset.png` | [Integrated construction, presets and smooth preview](../guide/modeling.md#keep-useful-tool-setups-and-inspect-the-smooth-result) | `modeling_milestones_offscreen_review` |
| `modeling-face-group-gap.png` | [Exact face selection groups](../guide/modeling.md#save-a-component-selection-as-a-vertex-group) | `modeling_face_group_gap_offscreen_review` |
| `modeling-point-split.png`, `modeling-point-tube.png`, `modeling-edge-construction-preview.png`, `modeling-face-split.png` | [Edge, point and face-center construction](../guide/modeling.md#construct-from-edges-points-and-face-centers) | `modeling_component_construction_offscreen_review` |
| `relax-brush-wire-before.png`, `relax-brush-wire-after.png`, `relax-brush-controls.png` | [Paint local surface-preserving redistribution](../guide/sculpting.md#redistribute-polygons-with-relax) | `relax_brush_manual_capture` (manual only) |
| `modeling-new-mesh-warning.png`, `modeling-move-gizmo.png`, `modeling-scale-gizmo.png`, `modeling-rotate-gizmo.png` | [Modeling initialization and selection-local transforms](../guide/modeling.md#shared-targets-and-component-transforms) | `modeling_transform_gizmo_offscreen_review` |

| `split-point-octagon-preview.png` | [Adjust and repeat eight-sided vertex patches](../guide/modeling.md#construct-from-edges-points-and-face-centers) | `mesh_refinement_offscreen_review` |
| `relaxation-controls-preview.png`, `relaxation-wire-before.png`, `relaxation-wire-after.png` | [Redistribute stretched polygons while preserving the groove](../guide/modeling.md#relax-stretched-polygons) | `mesh_refinement_offscreen_review` |

| `material-copper.png`, `material-painted-stroke.png` | [Example materials and a local paint stroke](../guide/materials.md) | `material_examples_manual_capture` (manual only) |

## Example assets

[pyramid.obj](../examples/pyramid.obj) contains the imported sample. [Front study](../examples/front-study.png) and [Side study](../examples/side-study.png) are original renders of a scaled UV sphere, exported by the same model-icon renderer used in the app. They are input assets, not screenshots of controls.

The recovery screenshot uses an isolated temporary directory whose name may change between captures. Readers' workspace and project names can differ too.

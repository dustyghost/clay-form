# Reusable model validation — 31 October 2026

Implementation batch: shared arrays #16, path assemblies #18 and object brushes #15. Review remains pending; no PR is opened by this batch.

Focused checks use `CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -j 1 --lib reusable_ -- --test-threads=1`, under the shared build lock. Twenty-nine ordinary tests cover private preview/apply/cancel, source edits, independent copies, bounded invalid input, save/reopen, source deletion protection, asset file round trips, path edits, closed-loop spacing and frames, real pointer gestures, sampling at different event rates, closed fusion, open-surface rejection, display picking, occlusion/mask/hidden-face protection, cache invalidation and project-switch tool reset and switching back to sculpt/transform tools. The Vulkan interaction check is ignored in ordinary runs and explicitly included in graphics CI.

The real egui/wgpu walkthrough `reusable_models_offscreen_review` passed on Mesa llvmpipe (LLVM 23.1.4, Mesa 26.2.6), with one software-rendering thread. It clicks Apply for array/path previews, selects and restores a hidden copy through the real controls, brushes a row of objects on a sphere, checks one-step Undo/Redo and validates the saved project. The attached captures were visually inspected for visible Apply/Cancel controls, radial orientation, curved placement and surface contact:

- [Array preview](01-array-preview.png)
- [Path assembly preview](03-path-preview.png)
- [Object brush stroke](05-object-stroke.png)

One bounded debug-build measurement used 128 grid instances of a 108-triangle source: 13,932 display triangles including the original, 498,894 serialized project bytes (including the existing default brush library), 58.1 ms display/scene preparation and 0.118 ms cold picking. This is a single synthetic diagnostic, not a release-performance guarantee. The test verifies shared saved source geometry, selection of the last copy, and source edits reaching the displayed copy.

The renderer expands placed geometry in the display snapshot. Sources and per-copy transforms remain shared in project storage, while normals are prepared once per source. Expanded display and bake budgets are documented separately from source limits. Physical pen feel and performance on large user assets remain hands-on validation.

The recent display/picking savings remain in use: resolved placements share one cached allocation across unchanged frames and texture-only edits; geometry and recipe edits invalidate it. Immutable asset meshes are validated at capture/import/load, rather than on each placement preview update. Thumbnail bounds and wire geometry are cached per asset. Hidden or fully detached copies retain the dense single-model stroke cache; making a copy visible invalidates both live-stroke and release reuse. Focused regressions exercise that transition and preserve the existing scene preparation and hidden-face stroke cache checks.

The additional local compatibility/cache selection passed 27 tests (three Vulkan tests intentionally ignored there), including the exact version-1 JSON round trip, scene cache key/rebinding checks, hidden-face live/release cache checks and shared immutable display payloads.

The optional bend/join extension adds five geometry regressions for actual vertex deformation, distinct caps, a hollow pipe bore, closed-loop welding, end-distance rejection, incompatible subdivisions, cancellation and generation limits. Document checks cover private previews, cached output, save/reopen, baking the exact preview mesh, Undo/Redo and path edits. Display/picking checks compare generated world positions with the visible and picked surface.

`reusable_path_bend_join_offscreen_review` clicks Rigid/Bend and Separate/Join in both directions, applies the preview, clicks Bake joined model, verifies the baked geometry matches, and exercises Undo/Redo. It passed with the same software Vulkan renderer; captures were inspected for continuous bends, reachable controls and an unchanged source. Both control walkthroughs run explicitly in graphics CI.

- [Rigid sections](06-path-rigid.png)
- [Bent and joined preview](08-path-bent-joined.png)
- [Baked joined model](09-path-joined-baked.png)

The thickness extension adds three geometry/profile regressions and a real-pointer application check. Uniform/taper/profile values change width and depth without changing section length, preserve closed joins, reject invalid input, and survive preview/save/reopen/bake/Undo. Inline Start/End, path-point and section-centre drags share the same private profile, leave the saved path unchanged, and reuse cached path samples across thickness and camera edits. Right-drag navigation and one-step Apply/Undo are checked. The focused reusable/legacy selection passed 30 tests; both software Vulkan walkthroughs passed.

The path walkthrough now also drags each inline handle type, switches all three thickness modes, edits a point in the profile graph, adds/removes a point, and bakes the exact displayed geometry. Its first run exposed a crowded profile panel; the compacted controls passed the interaction check. The following captures were inspected:

- [End-to-end taper with inline controls](10-thickness-taper-inline.png)
- [Local thickness with both sets of inline handles](11-thickness-profile-inline.png)
- [Shared profile editor](12-thickness-profile-panel.png)
- [Baked thickness profile](13-thickness-baked.png)

Whole-path transforms add six ordinary regressions covering move/rotation/uniform scale, corner rounding and normals, retained surface routes and closed seams, explicit detachment, real pointer drags, Escape rollback, the start pivot, route-only scaling, linked assembly proportions, invalid-scale rejection, save/reopen and one-step Undo/Redo. Focused checks passed 38 ordinary tests; three Vulkan tests were intentionally ignored in that selection. Existing path-guide cache, corner-rounding, reusable-model and legacy persistence checks were included. A nearby-neighbour join regression reproduced a failure when enlarging an assembly; scaling its join distance with the sections preserves both the weld and the transformed geometry.

`whole_path_transform_offscreen_review` passed on software Vulkan. It opens Editable paths, clicks the whole-path controls, drags each gizmo, saves each change, returns to point editing and verifies Undo/Redo. The captures below were inspected for reachable controls, visible gizmos and the joined assembly following each saved transformation. This interaction test runs explicitly in graphics CI.

- [Whole-path move](14-whole-path-move.png)
- [Whole-path rotation](15-whole-path-rotate.png)
- [Whole-path scale with linked sections](16-whole-path-scale.png)

The multi-object brush checks cover shared membership, project save/reopen, atomic brush import with new object IDs, and switching parts between strokes. A stroke retains its chosen part and settings until release. Undo/Redo restores those settings with the placements. Removing a personal-library copy leaves previously imported project objects and placements intact. The focused selection passed nine ordinary tests.

`object_kit_library_and_strokes_offscreen_review` and the existing `reusable_models_offscreen_review` both passed on software Vulkan. The kit walkthrough captures a hull vent and a sensor dome with different mounting axes, paints a row of each without restarting the brush tool, then adds the saved brush to a fresh project and checks both parts. The captures show the labelled contact and orientation guides, the two painted strokes, and the saved brush with its membership visible:

- [Hull vent capture and orientation](17-object-capture-orientation.png)
- [Two strokes from the Ship fittings brush](18-ship-fittings-strokes.png)
- [Ship fittings in the personal brush library](19-ship-fittings-library.png)

Layered arrays repeat whole patterns, and separately placed arrays can share the same editable source array. Five focused regressions cover independent XYZ transforms, the order of scale and rotation, shading normals on sheared copies, radial repetition about an offset pivot, stable slots for hidden parts, bounds and cycles, legacy array data, source edits, preview cancellation, project round trips, Undo/Redo, bake/detach and picking the displayed surface. The combined local selection passed 36 ordinary tests.

The Vulkan walkthrough `layered_array_controls_offscreen_review` opens Linked copy, adds and removes an array layer, then uses a saved four-part row as the source for six radial repeats. It opens the source through **Edit source array…**, changes the row to three parts and verifies that all six repeats update together, including after Undo/Redo. The existing `reusable_models_offscreen_review` also passed. These shots show the controls and the geometry from the actual UI:

- [Five fittings per row, repeated with XYZ offsets, rotation and scale](20-layered-array-xyz.png)
- [Six radial repeats of one shared, editable row](21-linked-array-of-rows.png)

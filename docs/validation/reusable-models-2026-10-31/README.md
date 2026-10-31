# Reusable model validation — 31 October 2026

Implementation batch: shared arrays #16, path assemblies #18 and object brushes #15. Review remains pending; no PR is opened by this batch.

Focused checks use `CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -j 1 --lib reusable_ -- --test-threads=1`, under the shared build lock. Eighteen ordinary tests cover private preview/apply/cancel, source edits, independent copies, bounded invalid input, save/reopen, source deletion protection, asset file round trips, path edits, closed-loop spacing and frames, real pointer gestures, sampling at different event rates, closed fusion, open-surface rejection, display picking, occlusion/mask/hidden-face protection, cache invalidation and project-switch tool reset and switching back to sculpt/transform tools. The Vulkan interaction check is ignored in ordinary runs and explicitly included in graphics CI.

The real egui/wgpu walkthrough `reusable_models_offscreen_review` passed on Mesa llvmpipe (LLVM 23.1.4, Mesa 26.2.6), with one software-rendering thread. It clicks Apply for array/path previews, selects and restores a hidden copy through the real controls, brushes a row of objects on a sphere, checks one-step Undo/Redo and validates the saved project. The attached captures were visually inspected for visible Apply/Cancel controls, radial orientation, curved placement and surface contact:

- [Array preview](01-array-preview.png)
- [Path assembly preview](03-path-preview.png)
- [Object brush stroke](05-object-stroke.png)

One bounded debug-build measurement used 128 grid instances of a 108-triangle source: 13,932 display triangles including the original, 498,894 serialized project bytes (including the existing default brush library), 58.1 ms display/scene preparation and 0.118 ms cold picking. This is a single synthetic diagnostic, not a release-performance guarantee. The test verifies shared saved source geometry, selection of the last copy, and source edits reaching the displayed copy.

The renderer expands placed geometry in the display snapshot. Sources and per-copy transforms remain shared in project storage, while normals are prepared once per source. Expanded display and bake budgets are documented separately from source limits. Physical pen feel and performance on large user assets remain hands-on validation.

The recent display/picking savings remain in use: resolved placements share one cached allocation across unchanged frames and texture-only edits; geometry and recipe edits invalidate it. Immutable asset meshes are validated at capture/import/load, rather than on each placement preview update. Thumbnail bounds and wire geometry are cached per asset. Hidden or fully detached copies retain the dense single-model stroke cache; making a copy visible invalidates both live-stroke and release reuse. Focused regressions exercise that transition and preserve the existing scene preparation and hidden-face stroke cache checks.

The additional local compatibility/cache selection passed 27 tests (three Vulkan tests intentionally ignored there), including the exact version-1 JSON round trip, scene cache key/rebinding checks, hidden-face live/release cache checks and shared immutable display payloads.

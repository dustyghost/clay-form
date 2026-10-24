# Hide/Isolate selection review — 23 October 2026

The visibility branch was checked on the combined context-menu and hold-to-mask stack.

- Focused visibility checks: 24 passed, including line projection in orthographic/perspective views, both operations and flip states, topology/attribute preservation, project round-trip, undo/redo, editable path application, camera cancellation, remapped Alt hold, release cancellation for every shape, and no stale path when re-entering the hold.
- Shared viewport menu checks: 5 passed, covering Modeling selection preservation, alternate bindings, dismissal, focus loss, and preference persistence.
- `visibility_selection_offscreen_review`: passed with Mesa llvmpipe/Vulkan. Inspected line preview/result, closed-path preview/result, and menus at 1000×850 and 640×480. Red face previews match the resulting whole-face boundaries. Controls and active labels remain readable; all seven visibility menu choices fit the compact window.
- `viewport_context_menu_offscreen_review`: passed and inspected at 1100×800 and 640×480. The final Up/Down and active-state labels render correctly.

Generate the visibility captures with:

```sh
CLAY_GPU_SOFTWARE=1 CLAY_GUIDE_PREVIEW=/tmp/clay-visibility-review cargo test --locked --lib visibility_selection_offscreen_review -- --ignored
```

The path is a closed view-space region with up to 64 editable points. It is intentionally distinct from an open Slice path: no supporting plane cuts, cap generation, or topology changes occur. Face-center classification keeps complete quads together, so visible boundaries follow existing polygon edges. Flip selects the opposite line half-plane or the outside of a closed region. Holding/releasing a mode or opening a menu never commits geometry.

The tests exercise actual egui input routing and the running offscreen application renderer. Physical tablet feel remains a hands-on check; no physical pen session was performed for this change. Full test, formatting, lint, and configured GPU checks run in PR CI.

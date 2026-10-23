# Hold-to-mask validation

Nine focused controller tests cover no-op holds, brush settings restoration, masks without changed geometry, one-step undo/redo and project persistence, release with the pen still down, explicit lasso/line Add/Erase, cancelled previews, Modeling component selection preservation, Escape/focus loss, visibility priority, Ctrl/Shift/Alt hold bindings, cleared settings, and a rebound Alt+F2 menu workflow. The shared menu suite additionally checks pointer dismissal and text-entry suppression.

`temporary_mask_offscreen_review` drives the running egui/wgpu viewport: hold M, open the tool menu, choose through-depth line with keyboard navigation, draw a line preview, apply masking and release back to the original tool. It saves the actual menu, preview and result for visual inspection and asserts that only mask weights change. The PR graphics job runs this focused capture alongside the existing GPU regressions. Native pen hardware is not exercised by this automated review.

```sh
CLAY_GUIDE_PREVIEW=/tmp/clay-temporary-mask-review cargo test --locked --lib temporary_mask_offscreen_review -- --ignored
```

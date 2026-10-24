# Viewport context-menu validation

The focused controller checks cover face/edge/vertex selection retention, keyboard choice, alternative Alt+F2 hold/menu bindings, modifier release, Escape, focus loss, no geometry/history edits, outside-click capture, text-entry suppression and saved/cleared settings. Existing visibility regressions cover cancellation on releasing the temporary hold and preserve the operation selected at gesture start.

The `viewport_context_menu_offscreen_review` test renders the real toolbar, model and shared menu at 1100×800 and 640×480, puts the popup beside the screen edge, and selects Inset through pointer events. It checks that the menu opens/closes and does not modify geometry or history. The screenshots are visually inspected for clipping, readable labels, selected state and target size. Native pen hardware has not been exercised by this automated review.

Run with a Vulkan adapter:

```sh
CLAY_GUIDE_PREVIEW=/tmp/clay-context-menu-review cargo test --locked --lib viewport_context_menu_offscreen_review -- --ignored
```

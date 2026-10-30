# Materials, colours and textures workflow review

Tickets #104 and #105 share selection and assignment conventions. The independent menus reuse Paint preset inputs; browsing an image is independent of assigning it to a model.

## Interaction and data checks

- `app::tests::appearance`: actual controls at 1200×900 and 720×600; create without UVs, generate UVs, assign, duplicate without assigning, unassign, confirmed deletion, undo, shared-use feedback and entering Paint. Independent material assignment works while a sculpting brush is selected. Opening an already docked menu preserves its placement.
- `document::texture_library`: shared edits and history, retained-level image uses, preserving replaced images, float pixels through save/reopen, subsequent painting, and library/history memory limits.
- `document::materials`: shared edits and usage counts, including inactive subdivision levels.
- Existing `app::textures::tests` retain external-editor reload, conflict detection, float precision, capture and custom Paint presets.
- Existing GPU interaction checks: `material_controls_offscreen_review`, `uv_texture_controls_offscreen_review`, `uv_island_controls_offscreen_review`, `paint_target_scope_inspection_offscreen_review`, and `paint_palette_picker_offscreen_review`.

The normal test suite runs the new non-GPU interaction tests. The full suite, formatting, lint and configured GPU checks run in PR CI. The capture below remains manual because it adds visual review, rather than a pixel-comparison oracle.

## Visual review

Captured with the real egui controls, clicking and scrolling the workflow rather than setting up screenshots of inert widgets. Reviewed selection borders, target names, shared-use warnings, scrolling and action reachability. The compact review uses 720×600. Materials can be docked/pinned; the texture library also shares its controls across menu, dock and editor placements.

- [Independent Materials menu](materials-1200x900.png): chosen Paint preset, material swatches, explicit model assignment and separate shared editor.
- [Independent Colours menu, compact](colours-720x600.png): future-stroke scope and palette access without changing an existing surface.
- [Shared image replacement, compact](texture-shared-replace-720x600.png): image identity, named uses and confirmation before replacing shared pixels.
- [Texture browser beside Alpha management](textures-beside-alphas-1200x900.png): thumbnail browsing, selected state and resize controls, with texture-specific assignment separated from the asset.

Reproduce the captures:

```sh
CLAY_APPEARANCE_REVIEW=/tmp/clay-appearance-review \
  cargo test --locked --lib appearance_workflow_offscreen_review -- --ignored --nocapture
```

The guides provide a short [material/colour workflow](../../guide/materials.md#try-the-complete-workflow) and [texture workflow](../../guide/textures.md#paint-an-image). Image painting still exposes base colour only; scalar-map authoring remains #70. Personal palette/library edits remain separate from project Undo, and each Paint preset retains its own inputs.

# Materials, colours and textures workflow review

Tickets #104 and #105 share selection and assignment conventions. The independent menus reuse Paint preset inputs; browsing an image is independent of assigning it to a model.

## Interaction and data checks

The appearance row now lives in the **Tool controls** custom menu (formerly Brush controls), including its S popup. The main toolbar has no separate appearance row. Migration preserves existing placements, custom names, shortcuts, saved workspaces and Paint-workspace backups.

- `app::tests::brush_controls`: narrow sidebar and S popup visibility, swapping without closing the popup or sculpting, and no appearance group in the main toolbar. Preference migration covers saved layouts, custom names and the Paint backup.
- `app::tests::appearance`: actual controls at 1200×900 and 720×600; create without UVs, generate UVs, assign, duplicate without assigning, unassign, confirmed deletion, undo, shared-use feedback and entering Paint. Independent material assignment works while a sculpting brush is selected. Opening an already docked menu preserves its placement.
- `appearance_colour_pair_controls_and_defaults` clicks secondary selection, palette picking, swapping, default material/colour, colour reset and secondary sampling at both sizes, then saves/reopens and uses Swap in the expanded UV editor. It verifies these input changes leave the model untouched. `appearance_colour_pair_legacy_hdr_and_validation` checks older preset defaults, HDR swaps and invalid colour rejection.
- `appearance_default_material_uses_regular_copy_edit_and_reset` selects the ordinary default tile, copies it, edits/resets both definitions and verifies the selection action preserves edited settings. Preference migration checks retain existing names, materials and hidden presets when adding the default tile.
- `document::texture_library`: shared edits and history, retained-level image uses, preserving replaced images, float pixels through save/reopen, subsequent painting, and library/history memory limits.
- `document::materials`: shared edits and usage counts, including inactive subdivision levels.
- Existing `app::textures::tests` retain external-editor reload, conflict detection, float precision, capture and custom Paint presets.
- The compact reference-removal regression still clicks **Remove image** without scrolling, including hidden/locked references. The Paint mode regression scrolls to each spray setting before checking it.
- The universal-transform regression still drags the centre pivot at 640×480; its compact toolbar omits the caption to avoid covering that handle.
- Existing GPU interaction checks: `material_controls_offscreen_review`, `uv_texture_controls_offscreen_review`, `uv_island_controls_offscreen_review`, `paint_target_scope_inspection_offscreen_review`, and `paint_palette_picker_offscreen_review`.

The normal test suite runs the new non-GPU interaction tests. The full suite, formatting, lint and configured GPU checks run in PR CI. The capture below remains manual because it adds visual review, rather than a pixel-comparison oracle.

## Visual review

- [Tool controls sidebar at 640×720](tool-controls-640.png) and [S popup](tool-controls-popup-640.png): material/colour controls and brush settings share one menu. The earlier reference-removal and transform captures below retain the previous top-strip layout.

Captured with the real egui controls, clicking and scrolling the workflow rather than setting up screenshots of inert widgets. Reviewed selection borders, target names, shared-use warnings, scrolling and action reachability. The compact review uses 720×600. Materials can be docked/pinned; the texture library also shares its controls across menu, dock and editor placements.

- [Independent Materials menu](materials-1200x900.png): chosen Paint preset, material swatches, explicit model assignment and separate shared editor.
- [Independent Colours menu, compact](colours-720x600.png): future-stroke scope and palette access without changing an existing surface.
- [Shared image replacement, compact](texture-shared-replace-720x600.png): image identity, named uses and confirmation before replacing shared pixels.
- [Texture browser beside Alpha management](textures-beside-alphas-1200x900.png): thumbnail browsing, selected state and resize controls, with texture-specific assignment separated from the asset.
- [Material and colour controls in Tool controls, compact](appearance-strip-720x600.png): the appearance group is part of the left menu alongside the brush settings.
- [Secondary colour editing, compact](primary-secondary-colours-720x600.png): explicit palette target, default colour and reset controls, with the appearance group in the Tool controls menu.
- [Expanded UV appearance controls](appearance-expanded-uv-720x600.png): material and colour controls remain available in the editor sidebar.

- [Reference removal with the persistent strip, compact](reference-removal-760x560.png): the selected image’s removal control remains above import controls and visible without scrolling.

- [Transform pivot access at 640×480](universal-pivot-640x480.png): compact Reset pivot / Done controls stay clear of the centre handle.

- [Editable copy of the default material](default-material-copy-720x600.png): the same shared editor and reset controls used by other materials. The Default colour label uses dark text against teal for contrast.

Reproduce the captures:

```sh
CLAY_BRUSH_CONTROLS_PREVIEW=/tmp/clay-tool-controls \
  cargo test --locked --lib brush_controls_offscreen_review -- --ignored --nocapture
CLAY_APPEARANCE_REVIEW=/tmp/clay-appearance-review \
  cargo test --locked --lib appearance_workflow_offscreen_review -- --ignored --nocapture
CLAY_APPEARANCE_REVIEW=/tmp/clay-appearance-review \
  cargo test --locked --lib appearance_colour_pair_offscreen_review -- --ignored --nocapture
CLAY_APPEARANCE_REVIEW=/tmp/clay-appearance-review \
  cargo test --locked --lib appearance_default_material_offscreen_review -- --ignored --nocapture
```

The guides provide a short [material/colour workflow](../../guide/materials.md#try-the-complete-workflow) and [texture workflow](../../guide/textures.md#paint-an-image). Image painting still exposes base colour only; scalar-map authoring remains #70. Personal palette/library edits remain separate from project Undo, and each Paint preset retains its own inputs.

# UI design rules

[Documentation home](../README.md#documentation)

Use `src/ui/design.rs` for shared action buttons, menus, icons, brush tiles and palette changes.

- Keep names on primary actions. Icons accompany labels; icon-only controls need a tooltip.
- Use one monochrome line-icon family, displayed at 16 pixels. Icons use the current text colour and support disabled controls.
- Buttons have a 28-pixel minimum height, 8 × 5 pixel padding and 4-pixel rounded corners. Keep 6 pixels between controls.
- Group project/file actions separately from editing and view actions. Wrap toolbar rows when the window is narrow; constrain path fields so they leave room for actions.
- Brush tiles share an 82-pixel height and a 48-pixel square preview. Use as many equal-width columns as fit (one to four). Put names below previews and show the full name on hover.
- Teal fill and outlines identify selected tools and brushes. Neutral dark surfaces and restrained borders keep the model visually prominent. Hover and disabled states follow egui's interaction styling.
- Preserve accessible labels, keyboard interaction and existing shortcut behaviour. Preview image changes before committing them to a brush.

## Controls over the model

Use `src/viewport/controls.rs` for the overlay palette, markers and labels.

- Teal identifies shape parameters. Squares change dimensions or uniform size; circles change radii and anatomical features.
- Axis-colored diamonds identify X/Y/Z span counts; violet diamonds identify non-axis topology parameters such as segments and subdivision levels. Place these in a compact stack beside the model, with persistent names and current values. Drag horizontally; every 12 logical pixels changes one unit. Keep exact-value controls in the single collapsible Primitive section within Submodels; the section disappears after conversion.
- World axes use the same coral X, green Y and blue Z colours in transform controls and the camera gizmo. Include axis names; colour alone must never communicate the axis.
- Amber identifies Action Line controls. Neutral controls apply to all axes. Move and placement use circles; scale and stretch use squares.
- Markers have dark interiors and a two-pixel coloured outline for contrast against both clay and wireframe. A white outer ring marks hover or dragging without replacing the semantic colour.
- Labels sit on dark backing plates, stay inside the viewport and show direction or operation when needed. Shape labels appear on hover or during dragging; topology labels remain visible to distinguish resolution from size.
- Use a 12-pixel hit radius and preserve one Undo step per drag. Topology changes affect the recipe, never the primitive's size or placement.

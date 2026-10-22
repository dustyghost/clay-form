# UI design rules

[Documentation home](../README.md#documentation)

Make sculpting comfortable and the next action clear. Use other sculpting apps as workflow references; choose gestures and terminology that work for Clay Form.

## Interaction principles

- **Show common actions.** Shortcuts and modifier gestures accelerate visible controls; they must not be required knowledge.
- **Show what will happen.** Keep gestures predictable, make the active mode and likely result clear, and provide a way to cancel or finish.
- **Keep input comfortable.** Reuse existing controls and actions before adding another route. Minimise steps and interruptions for mouse and pen users.
- **Try the whole task.** Judge completion through hands-on feedback and the end-to-end workflow as well as automated checks. Revisit awkward interactions even if another app uses them.

## Shared interface controls

Use `src/ui/design.rs` for shared action buttons, menus, icons, brush tiles and palette changes.

### Labels, modes and help

- Name actions with plain verbs. Keep names on primary actions; icons accompany labels. Icon-only controls need a tooltip.
- Separate persistent modes from immediate actions. Use a checkbox or clearly selected mode control. Keep the active mode visible when its settings are collapsed.
- Show the affected selection or region beside its controls. Separate model-wide clearing actions from targeted edits.
- Group controls by task. Put optional detail in clearly named collapsible sections.
- Use `design::help` for short help on hover **and keyboard focus**: explain the result, important limits and the user's configured shortcut. Disabled controls must explain how to enable them.
- Keep active modes, scope and subdivision-level replacement visible without a tooltip.
- Preserve accessible labels, keyboard interaction and existing shortcuts. Preview image changes before committing them to a brush.

### Layout and appearance

| Element | Requirement |
| --- | --- |
| Icons | One monochrome line-icon family at 16 pixels; use the current text colour and support disabled controls. |
| Buttons | Minimum height 32 pixels; padding 8 × 5 pixels; corner radius 4 pixels; spacing 6 pixels. |
| Checkboxes | Native keyboard and accessibility behavior; 20-pixel indicators, 32-pixel targets, full clickable rows in vertical settings panels. |
| Toolbar | Separate project/file actions from editing and view actions. Wrap rows in narrow windows; constrain path fields to leave room for actions. |
| Brush tiles | Height 82 pixels; square preview 48 pixels. Use one to four equal-width columns, as space allows. Put names below previews and show full names on hover. |
| Selection | Teal fill and outlines identify selected tools and brushes. Do not use the selected-state colour to decorate immediate actions. |
| Surfaces | Use neutral dark surfaces and restrained borders so the model stays prominent. Follow egui styling for hover and disabled states. |

## Controls over the model

Use `src/viewport/controls.rs` for the overlay palette, markers and labels.

### Colour and shape

| Control | Appearance and behavior |
| --- | --- |
| Shape parameters | Teal. Squares change dimensions or uniform size; circles change radii. |
| Topology | Axis-coloured diamonds for X/Y/Z span counts; violet diamonds for non-axis parameters such as segments and subdivision levels. |
| World axes | Coral X, green Y and blue Z in both transform controls and the camera gizmo. Always include axis names; colour alone is insufficient. |
| Action Line | Amber. Neutral controls apply to all axes. Move and placement use circles; scale and stretch use squares. |
| Markers | Dark interiors and a two-pixel coloured outline for contrast against clay and wireframe. Add a white outer ring on hover or drag, keeping the semantic colour. |

### Placement and dragging

- Stack topology controls compactly beside the model, with persistent names and current values. Drag horizontally: **12 logical pixels changes one unit**.
- Keep exact primitive values in one collapsible **Primitive** section within **Submodels**. Hide that section after conversion.
- Put labels on dark backing plates and keep them inside the viewport. Show direction or operation where needed.
- Show shape labels on hover or drag. Keep topology labels visible so resolution is distinguishable from size.
- Use a **12-pixel hit radius** and **one Undo step per drag**.
- Topology changes affect the recipe, never the primitive's size or placement.

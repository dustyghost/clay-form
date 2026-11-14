# Workspace and shortcuts

[Documentation home](../../README.md#documentation)

Keep the controls you reach for visible and give the rest of the screen to your sculpt. Start by pinning one menu; build a custom shelf when you know which tools you use most.

- [Pin or dock a menu](#key-settings-and-workspace)
- [Create a custom shelf](#create-a-custom-menu-or-shelf)
- [Save a workspace](#save-and-switch-workspaces)
- [Recover a layout](#recover-a-layout)
- [Change mesh capacity](#change-mesh-capacity)
- [Change shortcuts](#shortcuts)
- [Find default keys](#default-shortcuts)

## Finding controls

The first toolbar row keeps **Project**, **New**, **Open**, **Save**, the project name
and save state, **Search**, **Workspace** and **Zen** together. Edit the project path
inside **Project**. The next row holds editing tools, with appearance and view
controls in the final row. **Show wireframe** toggles the overlay;
**Wire settings** opens its options. Symmetry has one menu for its enabled state and settings.

The compact toolbar retains **Project**, **Save**, **Undo**, **Redo**, **Search**,
**Workspace** and **Zen**, alongside menus. **More tools** keeps the remaining
appearance, modelling and view menus available. Your current workspace remains
visible. A selected texture target and Boolean result toggle stay visible when
relevant, including in compact mode.

Below 960 logical pixels wide, the toolbar becomes compact. **Panels** offers
**Sculpting panels**, **Scene panels**, or **Hide panels**. This temporary choice
keeps saved placement and widths intact; enlarging the window restores both
sidebars. Search reveals the required side, and **Return to workspace** restores
the previous choice.

The fresh workspace puts **Brush settings** before the brush library, with size
and strength visible first. The **Tool controls** menu and **S** popup remain
available for a compact collection of controls; you can dock this menu again.
Existing saved arrangements keep their placements.

Checkbox settings use larger indicators and 32-pixel click targets. In vertical
panels you can click anywhere in the checkbox row, including beside its label.
Menus scroll when their contents exceed the available window height.

**Search** (default **Ctrl+F**) finds commands, settings, brushes, menus, panels,
shortcuts and named items in the current project and libraries. Try **cut**, **roll**,
**Mesh capacity**, a model name or a configured shortcut. Choose **Run** to use
an action, or **Show in UI** to reveal its control. When an action needs preparation,
the result explains why and offers **Show prerequisite** where a direct route exists.

Search opens the current placement and highlights the requested control or section.
**Return to workspace** restores temporary reveals, including the previous Zen
layout; **Keep layout** saves the revealed placement. Editing a setting or shelf
contents during a reveal remains an explicit change. Mode-specific settings can
still require the relevant tool or selection. Browsing saved paths, reusable models
and personal object-brush names keeps the active sculpting tool unchanged.

The earlier default **Ctrl+K** moves to **Ctrl+F** when that key is free. Custom
bindings, an unbound Search action, and any existing use of Ctrl+F are preserved.

Built-in menus share the same controls whether opened from the toolbar, floated,
or docked in a sidebar. The **Panel** strip offers pin, dock left and dock right
controls in each location. Hover an icon for its name. Existing saved workspace
placements and shortcuts continue to apply.

### Zen mode

Choose **Zen mode**, or press **Tab** with the pointer over the viewport and no
text field or dialog active. Panels and shelves disappear while view controls
follow the expanded viewport edges, keeping their configured relative placement.
**Exit Zen mode** or another Tab restores the normal arrangement. Search
remains available, and **Show in UI** can temporarily reveal a hidden control.
Tab continues to move keyboard focus when a field or dialog is active. Rebind
Zen in **Workspace → Shortcuts** if you prefer another key.

<a id="brush-controls"></a>

### Tool controls

In a fresh workspace, **Brush settings** comes before **Brush library** on the
left, and the active model appears first on the right. **Tool controls** is a
toolbar menu and **S** popup. Existing workspaces retain their placements.

Tool controls contains the selected material preview, editable **Primary** and **Secondary** colours, and **Swap colours** (**Z**), followed by the active brush icon, draggable **Size** (radius in pixels), **Inner radius** (percent of
the outer radius), and **Strength** (percent),
**Surface footprint and normal**, **Stroke mode**, and **Stroke stabilisation**
with its trailing distance. You can move it to a top shelf, where it wraps to fit narrower windows.

Press **S** to open the same menu near the pointer. Drag a size or strength value
left/right, or double-click it to type. Adjusting **Size** or **Inner radius** keeps
the brush outline visible at its last viewport position (or the model centre if you
have not hovered it yet), with both the outer and inner boundaries updating live.
Adjustments keep the popup open; press
**S** again, **Escape**, or click outside to dismiss it. Brush settings follow the active
brush; material and colour controls use the Paint input preset selected in Materials or Colours. Unsupported stroke options are disabled.

This is an ordinary custom menu: right-click its header or an item to edit it,
or open **Workspace** to rename, reorder, remove, or move its
controls. The menu remains visible with the compact toolbar. Existing preferences
gain the appearance controls once. The stock Brush controls name becomes Tool controls; custom names, placements, collapsed state and the S shortcut are preserved. Later customisations are respected. An existing custom **S**
binding is retained rather than overwritten.

## Change mesh capacity

Raise the mesh budget before importing or building a model that exceeds the default limits.

1. Open **Settings → Mesh capacity**.
2. Drag **Maximum vertices** and **Maximum triangles**, or click a number to type an exact value.
3. Save your project, close Clay Form, and start it again. The settings save automatically.
4. Reopen **Mesh capacity** and check the **Active** values before continuing.

| Limit per mesh | Default | Largest allowed setting |
| --- | ---: | ---: |
| Vertices | 500,000 | 16,000,000 |
| Triangles | 1,000,000 | 32,000,000 |

Quads count as two triangles. Both limits apply independently, so raise both when increasing mesh density. **Reset mesh limits** restores the defaults for the next restart. Lowering a limit does not simplify a model; raise it again if an existing project exceeds the active budget.

![Mesh capacity settings with higher saved limits and the current active limits](../images/mesh-capacity.png)

The **Active** line shows this session's budget. The edited values in this example take effect after restarting.

Higher limits allow more geometry; they do not reserve RAM or guarantee smooth editing at that size. File-size, texture, subdivision-level and operation work limits still apply. These are application preferences, independent of saved workspace layouts.

## Key settings and workspace

### Pin a menu open

1. Open a menu, such as **Mask**.
2. Click its pushpin icon, **Pin menu open**.
3. Drag the floating window's title bar to place it.

The crossed-pin icon returns it to a drop-down. Closing a built-in window also returns it to the toolbar; closing a custom window hides it.

### Dock a menu in a sidebar

Click **Make collapsible on left/right** in an open or pinned menu. Click the resulting section header to collapse or expand it.

Right-click a header to move it up/down, switch sides, or return the menu to the toolbar. Drag the sidebar edge to resize it. Placement and expanded state save automatically.

### Example: keep frequently used controls open

1. Dock **Transform** on the left.
2. Pin **Mask** and move it to the upper right.
3. Collapse Transform when you need more room; expand it to resume adjusting the model.

![Transform docked on the left and Mask pinned in a floating window on the right](../images/menu-placement.png)

*Docking keeps controls at the edge; pinning makes a movable window. Several menus can stay open together.*

## Create a custom menu or shelf

1. Open **Workspace**, then choose **+ New menu / shelf**.
2. Give it a name and choose its location. The destination is shown above its contents.
3. Search the catalogue for your favourite brush or action and choose **Add**.

Right-clicking an action or brush tile still offers **Add to custom menu / shelf…**.
You can also choose this option from a search result. Both routes use the same editor.

| Location | Best for |
| --- | --- |
| Left/right sidebar | Collapsible groups of controls |
| Top/bottom/left/right shelf | Frequently used buttons |
| Menu bar | Commands used occasionally |
| Floating | A movable tool group |
| Hidden | Keeping a menu for later |

Right-click an empty sidebar area and choose **New collapsible menu here…** to start directly in that sidebar.

### Edit the contents

Rename buttons, add separators, and reorder with the `::` grip or **Up / Down**. Set the shelf's column count; narrow shelves wrap automatically. **Remove item from this menu** removes the copy, not the underlying command.

Actions use the current model or brush and retain normal undo/availability rules. A brush button is disabled if its brush is missing. Dialog decisions and per-object numeric editors remain in their original panels.

## Save and switch workspaces

Open **Workspace** in the toolbar. **Settings → Customize workspace** opens the same window.
The heading identifies the active arrangement and the workspace selector switches saved layouts.

| Control | Use |
| --- | --- |
| Built-in panels and toolbar | Move, hide, or reorder tool panels |
| Compact toolbar | Leave more room for custom shelves |
| Save as workspace | Name the current arrangement |
| Workspace selector in the toolbar | Switch saved arrangements |
| Shortcuts | Edit bindings shared by every workspace |
| Configure viewport controls | Add, style, remove and place individual controls |
| Find Roll view | Find the camera roll control and add a copy to the viewport |
| Workspace colours and style | Choose Dark/Light/Warm or customize colors, spacing, and rounding |
| Manage saved workspaces | Switch or delete saved arrangements |
| Recover your layout | Restore hidden panels, recover floating panels or start from defaults |

Create a named workspace once, then arrange panels as you work: later layout and style edits save automatically. Try a compact sculpting layout and a second layout with modeling controls open. Starting from defaults preserves your saved workspaces.

Keyboard bindings are shared across workspaces, so switching layouts keeps your shortcuts.
Named workspaces store panels, menus and shelves, viewport controls, positions, widths,
collapsed sections, compact toolbar choice, and colours. Your sculpt remains in its project file.

Default view buttons adapt to a short canvas. Placing, aligning or editing them
keeps your chosen arrangement across restarts. **Restore default controls**
returns them to automatic fitting.

### A short customisation walkthrough

1. Open **Workspace**, enter **My sculpting**, and choose **Save as workspace**.
2. Create a shelf named **Favourites**, select **Top shelf**, search for a brush, and choose **Add**. The confirmation names the destination.
3. Choose **Find Roll view**, then **Add** beside Roll view. Choose **Place controls**, drag the control to a comfortable spot, and choose **Done placing controls**.
4. Open **Workspace → Shortcuts**, search for the brush, click **Unbound** and press a key. If it conflicts, **Find binding** takes you to the existing owner so you can clear or change it.
5. Move or hide a panel in **Built-in panels and toolbar**. Use **Restore hidden panels** to bring hidden tool panels and custom shelves back to the sidebars.
6. Switch to another named workspace, then back. Restart Clay Form to check that your layout and shared keys have been retained.

## Recover a layout

The recovery controls explain their scope before changing anything:

| Control | Effect |
| --- | --- |
| Restore hidden panels | Brings hidden built-in panels and custom shelves into the sidebars, preserving their contents |
| Recover floating panels | Brings displaced floating title bars inside the current window after a display change |
| Start a default workspace | Asks before replacing the current panels, shelves, viewport controls and colours; keeps shortcuts and saved workspaces |
| Restore default controls, in the viewport editor | Asks before replacing only this workspace's viewport controls |
| Reset keys to defaults, in Key settings | Asks before resetting shared bindings and held modifiers; keeps layouts |

A layout reset saves the previous arrangement first. Named workspaces retain their name;
an unnamed layout becomes **Before layout reset** (with a number if necessary).
Choose **Return to [workspace]** immediately, or use the workspace selector after restarting.
Restoring hidden or floating panels is a normal layout edit, so it saves to the active workspace.

### Where preferences are stored

Preferences save separately from projects, after a short delay and on exit:

- `$XDG_CONFIG_HOME/clay-form/preferences.json`, if set.
- Otherwise `~/.config/clay-form/preferences.json`.

Invalid preferences fall back to defaults with a status message. Key and layout reset controls are independent.

## Shortcuts

1. Open **Settings → Key settings**, or right-click a button and choose **Assign shortcut…**.
2. Select a binding and press its replacement. Press **Escape** to cancel.
3. Clear a binding if you want to leave that action unassigned.

Conflicting shortcuts are rejected. Shortcuts pause during text entry and settings editing. Smooth, reverse, pan, and zoom hold modifiers can also be changed.

### Use number keys for favorites

**1–0** and **Shift+1–0** give 20 initially unbound combinations. For example, bind `1` to Detail, `Shift+1` to Invert mask, and `0` to **Open [your menu]**.

Shifted numbers use the physical number-row key, so `!` can bind as Shift+1. Other keys follow your keyboard layout.

**Toggle perspective view**, **Reset brush settings**, **Rotate**, and pivot actions are available without default keys. New actions that conflict with your existing bindings start unbound.

### Default shortcuts

| Action | Key |
| --- | --- |
| Save / Open / New sphere | Ctrl+S / Ctrl+O / Ctrl+N |
| Undo / Redo | Ctrl+Z / Ctrl+Shift+Z |
| Search Clay Form | Ctrl+F |
| Zen mode (viewport) | Tab |
| Reset view | F |
| Toggle symmetry / guides | Shift+X / G |
| Toggle wireframe | W |
| Modeling quick menu | Q |
| Mask / visibility quick menu | X |
| Transform quick menu | E |
| Universal transform / return to sculpting | T |
| Smaller / larger brush | [ / ] |
| Tool controls popup | S |
| Swap Primary / Secondary colours | Z |
| Previous / next brush | , / . |
| Subdivide | Ctrl+D |
| Lower / higher subdivision level | Shift+D / D |
| Remesh in dynamic mode | Ctrl+R |

The mask/visibility menu now defaults to X; its previous stock W binding upgrades to X and stock symmetry moves to Shift+X. Wireframe returns to W when that key is free. Custom assignments and explicit unbindings are preserved. If a custom action already uses X, E or T, a newly introduced action for that key starts unbound; an existing menu binding is kept. All bindings can be changed in Key settings.

## Toolbar and brush previews

The toolbar wraps as the window narrows. Brush tiles show the selected brush with a teal outline; hover for its full name. Restore hidden panels in **Customize workspace**.

## Related guides

[Navigation](navigation.md) · [Brush settings](sculpting.md) · [Recovery saves](files.md#recovery-saves)

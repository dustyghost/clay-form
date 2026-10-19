# Workspace and shortcuts

[Documentation home](../../README.md#documentation)

Keep the controls you reach for visible and give the rest of the screen to your sculpt. Start by pinning one menu; build a custom shelf when you know which tools you use most.

- [Pin or dock a menu](#key-settings-and-workspace)
- [Create a custom shelf](#create-a-custom-menu-or-shelf)
- [Save a workspace](#save-and-switch-workspaces)
- [Change shortcuts](#shortcuts)
- [Find default keys](#default-shortcuts)

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

1. Right-click an action or brush tile and choose **Add to custom menu / shelf…**.
2. Create a menu, name it, and add actions from the searchable catalog.
3. Choose a location in **Settings → Customize workspace**.

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

Open **Settings → Customize workspace**.

| Control | Use |
| --- | --- |
| Built-in panels and toolbar | Move, hide, or reorder tool panels |
| Compact toolbar | Leave more room for custom shelves |
| Save as workspace | Name the current arrangement |
| Workspace selector in the toolbar | Switch saved arrangements |
| Workspace colours and style | Choose Dark/Light/Warm or customize colors, spacing, and rounding |
| Manage saved workspaces | Delete arrangements or start from defaults |

Create a named workspace once, then arrange panels as you work: later layout and style edits save automatically. Try a compact sculpting layout and a second layout with modeling controls open. Starting from defaults preserves your saved workspaces.

Keyboard bindings are shared across workspaces, so switching layouts keeps your shortcuts.

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
| Reset view | F |
| Toggle symmetry / guides | X / G |
| Toggle wireframe | W |
| Smaller / larger brush | [ / ] |
| Previous / next brush | , / . |
| Subdivide | Ctrl+D |
| Lower / higher subdivision level | Shift+D / D |
| Remesh in dynamic mode | Ctrl+R |

## Toolbar and brush previews

The toolbar wraps as the window narrows. Brush tiles show the selected brush with a teal outline; hover for its full name. Restore hidden panels in **Customize workspace**.

## Related guides

[Navigation](navigation.md) · [Brush settings](sculpting.md) · [Recovery saves](files.md#recovery-saves)


# Workspace and shortcuts

[Documentation home](../../README.md#documentation)

- [Key settings and workspace](#key-settings-and-workspace)
- [Toolbar and brush previews](#toolbar-and-brush-previews)

## Key settings and workspace

**Keep menus open:** open a built-in or custom menu, then click its **pushpin icon** (tooltip: **Pin menu open**). Transform, Slice, Primitive, and Add brush use the same placement controls as Mask and Symmetry settings. Its controls stay available in a floating window while you sculpt or use other menus. Drag the title bar to move it. The **crossed-pin icon** (Unpin menu) returns it to a normal drop-down. Built-in windows also return to their toolbar menu when closed with **×** or by clicking their highlighted toolbar button; closing a custom floating menu hides it. Multiple menus can stay open together, including with the compact toolbar. Pin state and positions save automatically with your workspace and are restored when you reopen the app.

**Turn a whole menu into a collapsible section:** open a built-in or custom menu and click its **left-sidebar** or **right-sidebar icon** (tooltips: **Make collapsible on left/right**). Pinned menu windows offer the same choices. The whole menu, including sliders and numeric controls, becomes a sidebar section; click its header to collapse or expand it. Right-click any sidebar section header and choose **Move section up / down** to reorder it among built-in tools, custom sections, and converted menus. The same order is editable under **Customize workspace → Built-in panels and toolbar**. Header options also move sections to the other sidebar, return menus to the toolbar, or float a converted built-in menu. Placement, order, and expanded state save with the workspace.

**Custom menus and button shelves:** right-click an action button (including a brush tile) and choose **Add to custom menu / shelf…**, or open **Settings → Customize workspace**. Create a menu, choose actions from the searchable catalog, and give it a name. Placement can be either sidebar, the menu bar, a top/bottom/left/right shelf, floating, or hidden. Right-click empty space or a section header in either sidebar and choose **New collapsible menu here…** to create a section in that sidebar. Click its header to collapse or expand it; right-click its header to edit or delete it. Right-click a copied button and choose **Remove item from this menu**, or use the editor to remove buttons and separators. Removing custom controls leaves their underlying actions available. Set the shelf's column count; narrow shelves wrap to fewer columns. Rename individual buttons, add separators, and drag the `::` grip or use **Up / Down** to reorder them. Floating shelves move by their title bar; closing one hides it until restored in the editor.

The catalog includes project commands, undo/redo, masking, visibility, mesh operations, transforms, brush settings, individual brushes, panel visibility, and opening your custom menus. Actions operate on the current model/brush and keep the usual availability checks and undo behavior. Buttons requiring a dialog decision and per-object numeric editors remain in their original panels. Brush selections use persistent brush IDs; a missing brush is disabled until that brush is available again.

Use **Built-in panels and toolbar** to move, hide, or reorder the existing panels. **Compact toolbar** leaves Settings, Edit, custom menus, and mesh-job cancellation accessible while freeing space for your shelves. Use **Save as workspace** in the workspace editor to name the current arrangement, then switch with the toolbar’s **Workspace** selector. Named workspaces automatically remember later edits, including collapsed menus, panel sizes, and colours. **Workspace colours and style** offers Dark, Light, and Warm presets plus panel, button, accent, text, and model-view colours, spacing, and corner rounding. **Manage saved workspaces** lets you delete workspaces or start from defaults; starting from defaults preserves saved workspaces. Key bindings are shared across all workspaces.

**Number keys:** right-click a button and choose **Assign shortcut…**, then press **1–0** or **Shift+1–0**. These are 20 distinct combinations and are initially unbound. For example, assign `1` to a brush, `Shift+1` to Invert mask, and `0` to **Open [your menu]** in Key settings. Shifted number-row keys are normalized using the physical key, so punctuation such as `!` still binds as `Shift+1`. Other keys continue to follow the keyboard layout. Modifier holds still apply when you use pointer gestures.

Open **Settings → Key settings** to rebind or clear shortcuts. Click a binding and press its replacement; Escape cancels. Conflicting shortcuts are rejected. Smooth, reverse, zoom and pan hold modifiers are configurable too. Shortcuts pause during text entry and settings editing.

Defaults: Ctrl+S saves, Ctrl+O opens the project path, Ctrl+N creates a sphere, Ctrl+Z undoes, Ctrl+Shift+Z redoes, F resets the view, X toggles symmetry, W toggles wireframe, G toggles symmetry guides, brackets change brush radius, and comma/period select brushes. Ctrl+D subdivides, Shift+D selects a lower level, D selects a higher level, and Ctrl+R remeshes in dynamic mode. Existing preferences upgrade without losing custom bindings; new shortcuts that conflict with yours start unbound.

Open **Settings → Customize workspace** to place brush library, brush settings, alpha, presets, tablet/navigation, and mesh management groups on the left, right, in floating windows, or hidden. Arrows reorder groups, headers collapse them, panel edges resize them, and floating titles move them. Closing a floating tool hides it; restore it in the workspace editor. Reset buttons restore key or layout defaults independently.

Preferences save automatically after a short delay and on exit, separately from sculpt projects, to `$XDG_CONFIG_HOME/clay-form/preferences.json` or `~/.config/clay-form/preferences.json`. Invalid preferences fall back to defaults with a status message. Tool groups can be rearranged, and reusable action buttons can also be copied into your custom menus.

## Toolbar and brush previews

Toolbar actions share 16-pixel line icons and 28-pixel minimum heights. Project actions and modeling/view actions occupy separate rows and wrap on narrower windows. Brush previews use equal-sized responsive tiles, with names beneath the images, full-name tooltips and teal selection outlines.

## Related guides

[Navigation and tablet input](navigation.md) · [Undo and redo](sculpting.md#undo-and-redo) · [Recovery saves](files.md#recovery-saves)

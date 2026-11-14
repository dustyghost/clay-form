# Editable 3D paths

[Documentation home](../../README.md#documentation) · [Sculpting and brushes](sculpting.md)

Choose **Path Brush** in the **Brush Library** to draw and edit paths in the viewport. Paths belong to a submodel and save with the project. You can use saved paths as sculpt guides or as routes for [assemblies of reusable models](instances.md#assemble-pieces-along-a-path).

## Draw and edit a path

1. Select **Surface** or **Free** in the brush's **Mode** menu. Tap to place points or drag to sketch. Lift the pen between strokes to continue the same path. **Smooth** curves the entire path and replaces individual corner rounding. Use **Close loop** after placing at least three points, or **Open loop** to reopen it.
2. Drag **Orbit**, **Pan**, or **Zoom** beside the viewport gizmo. During the drag the viewport shows which camera action is active. Release to resume the same draft. Right-drag, middle-drag, and the wheel also navigate.
3. Tap **Complete path** in the top-left tool strip or press **Enter**. This saves the path as one undo step. Tap **Cancel** to discard the draft. Press **Escape** during a drag to cancel that gesture.
4. Tap an existing guide or a visible handle to edit it. Grab a point to move it. **Insert point** lets you tap the selected guide to add a point. If a draft is already underway, use **Resume draft** after editing the saved guide. Press **Delete** or tap **Delete…** to open the confirmation for the selected path.

**Free** points lie on a view plane through the previous point, or through the origin for a new path. Rotate the view to place points at different depths. A selected point has X, Y and Z coordinate controls and draggable handles for the axes visible from the current view.

**Surface** points follow the active model. Drag **Offset** to move the route along the surface normals in model units. Positive values lift it off the surface and negative values sink it. The path retains its surface anchors for sculpting and retopology. Assemblies and the displayed route use the offset.

Open **Paths…** or **Mesh → Editable paths** to rename, hide or manage saved guides and access advanced settings. Each saved path has a thumbnail showing its route on the model. Other saved guides appear faintly in the viewport. Use the checkbox beside a path to hide its guide.

## Move, rotate and scale the whole path

Choose **Transform whole path** in Editable paths. **Move path** provides a centre handle for movement in the view plane and coloured world-axis handles. **Rotate path** uses the same projected rotation rings as mesh transforms, including an outer ring for rotation around the view. **Scale path** has a square handle for uniform scaling. Choose **Centre**, **Start** or **Origin** as the pivot. Numeric values and **Apply transform to draft** provide precise offsets, rotation angles and scale factors.

The route remains a private draft: **Save path** commits it and updates its linked assemblies in one undo step. **Escape** during a drag restores the previous draft; **Discard edits** restores the saved path. Camera navigation remains available. **Edit points** returns to the ordinary point editor.

**Scale linked sections** is enabled by default. Saving a scaled path also scales the linked assemblies' cross-sections, active fixed spacing, join distance and individual offsets, preserving the assembly's overall proportions and nearby joins. Disable it to resize only the route while keeping section sizes and spacing unchanged. Source meshes stay unchanged. Thickness profiles retain their relative positions along the route.

Surface paths must first use **Detach from surface** for free transforms. Detaching a saved path preserves its current world-space route and is independently undoable. After transforming, **Attach saved path to surface** can reproject it explicitly. Free paths can be edited and transformed while their original target model is hidden.

## Round individual corners

Choose **Move points** and select a corner. Use **Round** and adjust **Rounding (model units)**, or drag the diamond handle toward the inside of the bend. The two circular handles adjust how far the rounding extends along the adjoining segments. Choose **Sharp** to remove that corner’s explicit rounding.

Rounding changes the guide around the corner without moving its control point. The amount uses model units and is limited by the adjoining segment lengths. Open endpoints stay sharp; closed loops can round every corner, including the seam. Explicit corner rounding takes precedence over the whole-path **Smoothing** setting. When all corners are sharp, that smoothing setting applies again; set it to zero for a completely angular guide.

Edits remain a draft until **Save path** or **Save & sculpt**. Press **Escape** during a handle drag to restore the preceding draft. Rounded surface guides stay attached to the model and work with manual or automatic brush strokes.

## Draw a ring with one pen drag

1. Choose **Ring** in the Path Brush **Mode** menu. **New ring · Auto** fits a planar section to the local shape. Choose **X**, **Y** or **Z** for a fixed axis.
2. Touch the side of the model. Drag to position the loop and release to keep it. Tap elsewhere on the model to place another ring.
3. Tap an existing ring to select it. Drag its start handle, or choose **Move ring** and tap a new location. An Auto ring refits to its new location. A fixed-axis ring keeps its chosen axis.
4. Drag **Tilt** to set a manual angle. Select **Auto** again to refit it. If a section cannot produce a closed loop, the last valid ring is kept.

Ring orientation settings save with the project. To give an older closed surface guide these controls, select it and choose **Fit as ring**.

## Sculpt along a saved guide

1. Choose **Save & sculpt**, or close the editor after saving. Keep **Mesh → Guide brush strokes with paths** enabled.
2. Choose your usual brush and start a stroke on the model near a saved surface path. Starting farther away gives you an ordinary stroke.
3. Draw along the guide. Only the section you traverse receives the brush; a short drag does not run around the whole path. You can reverse direction and continue across a loop's seam.
4. Release to finish the stroke, or press **Escape** to cancel it. Use **Undo** to restore the stroke and its guide attachment together.

The stroke stays on its captured path and follows nearby visible sections, so a crossing or a rear-facing section does not steal the stroke. Orbit to reach another side of the model, then start a new stroke there. Enable **Mesh → Only start strokes on paths** to ignore strokes that begin away from a guide, including a drag that later crosses one. Disable it to allow ordinary freehand strokes away from guides. Turning **Guide brush strokes with paths** off also restores freehand sculpting everywhere. Both toggles can be assigned to shortcuts or custom controls.

Live guidance uses the active brush's normal radius, spacing, strength, pressure, alpha, masks, and symmetry. It supports **Detail**, **Grab**, and **Snake**, including their movement along the route. Existing stencil restrictions still apply: disable the stencil for Grab, Snake, or Detail. The guide used by a stroke reattaches to that stroke's changed surface so you can keep sculpting along it. Unrelated surface edits can make saved paths stale and require explicit reprojection.

## Apply a brush to the entire path

Choose **Stroke entire path** in the Mesh controls to automatically apply the active brush to the last selected or manually used guide. The same button appears near the top of Editable paths for its selected saved path.

**Use active brush size** is enabled by default. The automatic stroke uses the selected brush’s radius, strength, spacing, alpha, falloff, pressure curve, masks, and symmetry. Radius is converted from pixels using the current viewport and zoom, then held constant in model units for the operation. In perspective view, the nearest side of the path sets that size. Disable **Use active brush size** to choose a fixed **Custom radius (model units)** instead.

Expand **Automatic stroke settings** to adjust brush size, strength, or spacing; these edits also change that brush for manual sculpting. **Automatic pressure** is separate: it defaults to 100% and stays constant. Lower it to approximate a lighter pen stroke. Manual strokes still use your changing pen pressure. Direction and inverted effect are also separate automatic-stroke options.

Adaptive sculpt uses the current mesh settings and refines along the complete route. The dialog shows whether adaptive is on. It does not enable adaptive when you have it turned off.

The result commits as one undo step when it finishes. **Undo** and **Redo** work from the buttons at the top of the dialog or your shortcuts while the dialog stays open. Save or discard draft edits first; cancel a running stroke or wait for it to finish. Undo discards an unapplied preview before restoring committed history. **Cancel stroke** discards unfinished work. If you want to inspect a result before committing, choose **Preview stroke**, then **Apply stroke** or **Cancel preview**.

The route is sampled on the target surface before the automatic stroke runs. Closed routes divide the entire perimeter into equal intervals without an extra relief dab at the seam; drag brushes finish the closing movement. Brush alpha, masks, and supported symmetry settings affect the result. Disable an enabled stencil before using this automatic preview.

The saved path supplies the trajectory, so the preset's pointer stroke mode (Freehand, Line, Drag Rect, or Drag Dot) does not change path application. Its pressure curve and pressure-controlled radius or strength settings still use the chosen constant pressure.

Brushes still need enough mesh resolution to show their footprint. Start with a moderate radius and strength, and inspect the preview before committing a fine groove or repeated alpha.

## Reproject after changing the mesh

Edits made outside a live stroke's captured guide, transforms, and surface rebuilding can make a saved surface path stale. Save or discard any draft edits, set **Distance**, and choose **Reproject saved path**. Reprojection commits immediately as one undo step; inspect the updated guide before sculpting along it. **Attach saved path to surface** uses the same workflow for a free path.

Reprojection searches the visible target surface within that distance. It reports unreachable points, opposing surfaces, and ambiguous or disconnected routes rather than silently jumping between shells. Add intermediate guide points or correct their positions if the route is unclear. A failed reprojection or preview leaves the saved path and mesh unchanged.

## Saving, undo, and limits

Save the project to preserve paths, their points, corner rounding, smoothing, closure, direction, and seam. Older projects open with an empty path library. Path saves and applied strokes each use one Undo/Redo step; unfinished authoring edits and stroke previews can be cancelled.

A project can contain up to 128 paths, each with up to 2,048 control points. Brush application permits up to 4,096 dabs. Whole-path previews also bound total work for dense meshes; live guidance bounds that work per frame. Use shorter strokes or increase spacing if a route exceeds a limit. These authoring guides do not select mesh edges or create topology automatically.

### Return to a sculpt brush

Choose a brush from the library, a custom control or its shortcut to leave Path Brush or Editable paths. An untouched editor closes immediately. If it contains a draft or stroke preview, choose **Apply and switch**, **Discard and switch**, or **Keep editing**. Apply saves the path or commits the preview with its usual Undo step. Discard cancels private work and keeps the saved scene. Keep editing returns to the same draft.

The Editable paths window’s close control, **Done**, and **Finish paths** use the same decision. While a stroke preview is running, you can wait for it before applying, or discard it and switch immediately. A running whole-path stroke waits for your decision before committing. Other settings windows can stay open while you choose a brush or navigate; typing and shortcut capture keep their keyboard focus.

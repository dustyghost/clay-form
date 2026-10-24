# Editable 3D paths

[Documentation home](../../README.md#documentation) · [Sculpting and brushes](sculpting.md)

Use **Mesh → Editable paths** to draw a reusable guide, edit it while orbiting, and guide your brush as you sculpt on the model. Paths belong to a submodel and save with the project. They are separate from [mesh edge-loop selections](edges.md).

## Draw and edit a path

1. Select the submodel, open **Mesh → Editable paths**, and choose **New surface path** or **New free path**.
2. Use **Add / draw** and click to place points, or drag to draw a sequence. Surface points attach to the visible target surface. Free points lie on a plane parallel to the current view, through the selected or last point; the first point uses a plane through the origin.
3. Right-drag to orbit, middle-drag to pan, and use the wheel to zoom. Continue placing points from the new view, including around the back of the model.
4. Choose **Move points** to select and drag a point, or **Insert point** to add detail between existing points. Use the numeric **X**, **Y**, and **Z** controls for precise positions and adjust **Smoothing** to soften the whole guide, or round individual corners as described below.
5. Choose **Save & sculpt** to save a surface path and return to sculpting, with path guidance enabled. **Save path** commits the edit while keeping the editor open. Either save is one undo step. **Discard edits** restores the saved path; **Done** closes the editor and discards unsaved edits. **Escape** cancels an active point drag.

A free path can leave the surface and is useful as a spatial guide. Save it, then choose **Attach saved path to surface** before using it as a brush route. Surface paths retain their world-space positions as you move the camera. Rear-facing surface segments appear dim and dashed; orbit until their points face the camera before selecting or moving them.

**Add / draw** keeps dense guides readable by showing only the start, open endpoint, selected point, and the point under the pointer. Choose **Move points** or **Insert point** to show every editable control-point handle. These handles are not brush dabs or mesh vertices.

## Round individual corners

Choose **Move points** and select a corner. Use **Round** and adjust **Rounding (model units)**, or drag the diamond handle toward the inside of the bend. The two circular handles adjust how far the rounding extends along the adjoining segments. Choose **Sharp** to remove that corner’s explicit rounding.

Rounding changes the guide around the corner without moving its control point. The amount uses model units and is limited by the adjoining segment lengths. Open endpoints stay sharp; closed loops can round every corner, including the seam. Explicit corner rounding takes precedence over the whole-path **Smoothing** setting. When all corners are sharp, that smoothing setting applies again; set it to zero for a completely angular guide.

Edits remain a draft until **Save path** or **Save & sculpt**. Press **Escape** during a handle drag to restore the preceding draft. Rounded surface guides stay attached to the model and work with manual or automatic brush strokes.

## Draw a ring with one pen drag

1. Select **New ring** in Editable paths. Choose the cylinder's lengthwise **Axis**: Y for an upright cylinder, or X/Z for one lying along those axes.
2. Choose **Level ring** for a flat cross-section, or **Shortest surface route** to let the loop travel along the surface where a shorter route is available.
3. Touch the side of the model. A complete loop appears with its starting point at the pen. Keep the pen down and slide along the surface to reposition the loop.
4. Release to keep the draft, then choose **Save & sculpt**. **Escape** during a drag restores the preceding draft.

Level rings stay perpendicular to the chosen axis. Shortest routes use the mesh edges and the level section to find a shorter loop that winds once around that axis; mesh resolution affects the result. They can rise or fall along an uneven surface. Neither mode jumps between disconnected shells or cuts through a cap. A section that is open, branched, or cannot surround the selected axis reports an error and keeps the last valid draft.

You can also draw a loop point by point with **New surface path**, orbit around the model, then choose **Close loop**. Do not duplicate the first point. Move or insert points to refine either type of loop.

Select a point and choose **Use as seam** to change where traversal starts. **Reverse direction** changes travel direction while preserving the seam's position. **Open loop** breaks the loop at its seam.

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

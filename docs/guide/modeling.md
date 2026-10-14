# Models and mesh tools

[Documentation home](../../README.md#documentation)

Build a base shape, position it, and choose how to add detail.

- [Manage submodels](#submodels)
- [Create a primitive](#parametric-primitives)
- [Move, scale, and rotate](#model-transforms)
- [Mirror one side onto the other](#mirror-one-side)
- [Add mesh detail](#mesh-management)
- [Thicken an open surface](#temporary-thickness)
- [Cut the model](#slice)
- [Combine submodels](#merge-down)

## Submodels

### Select and organize parts

Open **Submodels**. Click a row or click an inactive surface in Sculpt mode to select it; sculpting starts on the next drag.

| Control | Result |
| --- | --- |
| Eye | Hide/show a model |
| Up / down | Reorder models |
| Duplicate | Copy the active surface, mask, and subdivision levels |
| Extract masked | Copy fully selected faces into a new unmasked model |
| Group selector | Assign the model to an organizational group |
| Ungroup | Remove the folder while keeping its models |

The active model is clay-colored and inactive models blue-gray unless vertex-group colors override them. Hidden models cannot be sculpted or transformed. Inactive models display their lowest stored level without losing higher detail.

Extracted boundaries stay open and use a fresh subdivision stack. The selection threshold defaults to 50%. Add an existing mesh through [OBJ import](files.md#obj-import).

## Parametric primitives

### Create a base shape

1. Choose **Primitive → Create primitive…**.
2. Confirm replacement of the active submodel's geometry, masks, and levels. **Undo** can restore them.
3. Choose a shape and adjust its controls.
4. Choose **Convert to sculpting mesh** when ready to use brushes or mesh operations.

Available shapes: Plane, Cube, Circle, UV sphere, Icosphere, Cylinder, Cone, Torus, Grid, and Human head.

### Adjust dimensions and resolution

Use the **Primitive** menu or **Submodels → Primitive**. Pin/dock the menu to keep its controls visible.

![Cube with width 1.4, height 0.8, depth 1.0, two spans per axis, and colored viewport handles](../images/primitive-cube.png)

*Dimensions change the shape; spans change its resolution. This cube has two spans on each axis.*

| Handle | Controls |
| --- | --- |
| Axis-colored square | Size along X (red), Y (green), or Z (blue) |
| Teal circle | Radius or anatomical dimension |
| Diamond | Resolution; drag left/right |
| White hover ring | The highlighted handle |

Hover for a label, or type precise values in the panel. Wireframe shows the axis-colored span lines. Plane/Grid and Circle lie in XY; cylinders, cones, and tori use Y as their main axis.

The recipe saves with the model and survives duplication, transforms, and reopening. Converting removes it but keeps the surface; Undo restores it. An unfilled Circle must be filled before conversion. Open surfaces need closing before adaptive sculpting or voxel remeshing.

### Shape a human-head base

The Human head controls adjust jaw, chin, cheeks, nose, eyes, brow, ears, mouth, lips, and neck. Facial settings are symmetric and provide a starting form for sculpting.

| Control | Effect |
| --- | --- |
| Open mouth | Rotate the lower jaw, lower teeth, and tongue |
| Upper / lower eyelid closure | Adjust each lid independently |
| Open eyes / Close eyes | Set both lids together |
| Include neck | Add/remove the welded neck |
| Neck length, width, muscle, throat | Adjust the neck profile |
| Reset head proportions | Apply current base proportions to an older recipe; undoable |

Eyeballs, teeth, and tongue are separate closed surfaces within the same submodel. Drag a depth handle horizontally when facing it directly, or orbit to expose its direction. Choose **Done** after a transform to return to shape handles.

## Model transforms

Open **Transform** and select **World axes**.

| Operation | Gesture |
| --- | --- |
| Move | Drag an axis, or the center for view-plane movement |
| Scale | Drag an axis, or the center for uniform scale |
| Rotate | Drag a colored ring; watch the signed angle |
| Place pivot | Click the active surface, then drag to adjust |
| Reset pivot | Return to the model's bounding-box center |

Orbit to expose an axis facing the camera. Edge-on rotation rings accept a straight drag along the projected ring. Drag outside handles to orbit; choose **Done** to sculpt again.

### Example: rotate a cube and keep its recipe

1. Create the cube shown above: **1.4 × 0.8 × 1.0**, with **2** spans per axis.
2. Choose **Transform → Rotate** and drag the blue Z ring.
3. Release, choose **Done**, then change a cube dimension.

![Cube rotated around Z with colored axis rings and a live angle of about 46 degrees](../images/rotation-controls.png)

*Changing a primitive's dimensions keeps its rotated placement. Each changed drag is one undo step.*

### Use an Action Line

Choose **Action Line → Set line** and drag. The start is the pivot; the line supplies the axis.

Move with the middle handle. Scale with the end for axial stretch or the middle for uniform scaling. Rotate with the amber ring. **Reset pivot** recenters the start while keeping direction and length. Extend a zero-length line before rotating.

### Protect a masked region

Enable **Respect mask**. Full masks stay fixed; soft masks reduce the transformation. All stored subdivision levels follow.

Pivot/line placement does not change geometry or consume undo. These controls are session-only and reset after actions such as changing models, opening projects, and undo/redo.

## Mirror one side

### Replace the opposite half

1. Open **Symmetry settings → Mirror one side**.
2. Choose **X**, **Y**, or **Z**. The displayed plane uses that axis of the symmetry center below.
3. Choose the source direction: **-X to +X**, for example, keeps the negative-X half and replaces the positive-X half.

This works with brush symmetry turned off. Directions use object coordinates; rotating the camera does not change them.

The active submodel becomes symmetric across the plane. Source masks, vertex-group weights, and face colors are copied too; masks do **not** protect the destination. Crossing faces are cut and joined at a shared center seam. Open surfaces can remain open.

### Example: repeat an existing sculpted fold

![A sphere with a fold only on its negative-X side and the Mirror one side controls open](../images/mirror-before.png)

*Before: the left half contains the fold we want to keep.*

![The same sphere after negative X was mirrored to positive X, with matching folds on both halves](../images/mirror-after.png)

*After **-X to +X**: the opposite half matches, including the fold. The camera has not moved.*

### Mesh levels and undo

Mirror keeps the current mesh level and discards its other subdivision levels. It also turns off dynamic/adaptive mesh modes. Other submodels are unchanged. **Undo** restores the previous geometry, masks, groups, and levels in one step.

Convert parametric primitives first, show all faces of the active model, and apply or remove temporary thickness. A source half with no surface produces an error and leaves the model unchanged. Large operations can be cancelled from the status controls.

## Mesh management

Choose the method that fits your work:

| Method | Best for | Changes topology |
| --- | --- | --- |
| Subdivision | Keeping several editable detail levels | Adds resolution to the whole surface |
| [Adaptive sculpt](sculpting.md#snake-brush-and-adaptive-sculpting) | Detail near the brush | Refines/simplifies locally |
| Dynamic mesh / voxel remesh | Redistributing geometry or fusing overlapping closed parts | Rebuilds the whole surface |

### Add subdivision levels

Choose **Mesh → Subdivide** from the highest level. Use **Lower / Higher** or the level slider to switch levels.

Lower-level edits carry upward while retaining finer detail. Subdivision is linear: it adds resolution without automatically smoothing the shape. Masks interpolate between levels.

**Delete lower** removes levels below the current one; **Delete higher** removes those above it. Both are undoable. Projects store all levels; OBJ exports only the active one.

### Remesh a whole surface

1. Switch to **Dynamic mesh**. This keeps the active surface and discards stored levels.
2. Choose a resolution, **16–96 voxels** along the longest bounding-box axis.
3. Select **Remesh now**, or enable **Remesh after each stroke**.

**Remeshing clears masks and can lose details thinner than a voxel.** Use moderate resolution first. It requires closed, consistently oriented surfaces and can fuse overlapping outward-facing shells; separate volumes remain separate.

Mesh operations run in the background. Cancel keeps the original surface; Undo restores prior geometry and settings. A failed automatic remesh disables the automatic option so you can adjust it.

## Temporary thickness

### Preview a shell

In **Submodels → Thickness · open meshes**, enable **Temporary thickness** and set its total thickness in model units. Half is added to each side; walls close boundary loops.

Orbit to inspect tight folds for intersections. The original surface, masks, levels, and picking remain unchanged. OBJ export uses the original surface while the shell is temporary.

### Apply the shell

Choose **Apply thickness** to make the preview real geometry. This converts primitives, preserves masks/quads, and resets levels and topology modes. Undo restores the original surface and settings.

Thickness requires consistently oriented open surfaces with nonbranching boundaries. Closed meshes and outlines without faces are not eligible. Front/back/rim groups and masks can be generated in the same controls.

## Slice

### Trim away one side

Before slicing, convert primitives, apply/remove temporary thickness, and show all faces of the active model.

1. Open **Slice**, select **Line**, and choose **Trim**.
2. Leave **Cap cuts** on for a closed result.
3. Drag across the model and inspect **Keep / Remove**. Use **Flip kept side** if needed.
4. Release to apply, then choose **Done slicing** and orbit to inspect the cap.

![Vertical Slice line with Keep and Remove labels and Cap cuts enabled](../images/slice-trim-preview.png)

*Before release: check which side will remain.*

![Trimmed sphere viewed at an angle to expose the new flat cap](../images/slice-trim-result.png)

*After release and orbit: the cut is closed with a flat cap in a new Cut vertex group.*

### Choose a cut result

| Result | What remains |
| --- | --- |
| Vertex groups | One connected surface, divided into face groups |
| Trim | The chosen side only |
| Separate | Both sides as separate submodels, initially in place |

Cuts extend through all depths of the active model. Masks and symmetry do not restrict them. Hold **Space** during a line drag to move it; **Esc** cancels and exits Slice. Right-drag orbits between cuts.

### Make a bent or rounded path

Choose **Path**. Click to add up to **24 points**, click an edge to insert one, and drag points to reshape the cut.

| Control | Action |
| --- | --- |
| Interior point's blue diamond | Drag away to round, toward the point to sharpen |
| Tangent handles | Adjust the rounding |
| Sharp / Rounded | Set corner style |
| Turn / Round (px) | Set precise corner values |
| Shift-drag | Snap to 15° |
| Space-drag | Move the entire path |
| Delete point / Delete / Backspace | Remove the selected point |
| Apply cut / Enter | Commit |
| Clear / Esc | Discard; Esc also exits Slice |

The camera stays fixed while editing a path. Clear/apply it before orbiting. Paths and their extensions must not cross; failed cuts retain the draft for correction.

### Caps, projection, and undo

Caps require closed, consistently oriented input. Turn **Cap cuts** off for open surfaces, or apply thickness first.

Cuts follow the [view projection](navigation.md#perspective-view): parallel planes in orthographic, planes converging at the eye in perspective. Changing projection/FOV discards the draft. A perspective path through geometry crossing the eye is rejected; use orthographic and redraw it.

A cut uses the active level and resets that model's subdivision and adaptive/voxel settings. Uncut quads remain; intersections and caps use triangles. Other submodels stay intact. Each completed cut is one undo step; cancellation keeps the original surface.

## Merge down

Choose **Submodels → Merge down** to combine the active model with the next one in grouped order. The tooltip names the target.

The active model keeps its name/folder. Masks, groups, visibility, and quads survive. Hidden models contribute hidden faces; duplicate group names receive suffixes.

Merge bakes primitives and temporary thickness, uses current mesh levels, and resets the result's level/topology settings. Fill unfilled Circles first. It combines surfaces **without welding or boolean union**; use voxel remeshing for fusion. Undo restores both models.

## Related guides

[Brushes and adaptive detail](sculpting.md) · [Masks and selection](selection.md) · [Import/export](files.md)

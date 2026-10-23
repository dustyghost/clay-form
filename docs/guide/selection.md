# Masks, vertex groups, and selection

[Documentation home](../../README.md#documentation)

**Mask** a region to protect it from brushes. **Hide** it to work without seeing or picking it. **Group** it to recall a selection later.

- [Protect a region with a lasso](#example-protect-a-region-with-a-lasso)
- [Protect one side through the model](#protect-one-side-through-the-model)
- [Edit a mask](#mask-operations)
- [Save a region as a group](#vertex-groups)
- [Hide or isolate faces](#selection-and-visibility)
- [Split off hidden faces](#split-hidden-faces)

## Hold to mask

1. Open **Settings → Key settings** and assign **Hold to mask** (initially unbound), for example the **M** key. Modifier-only chords and key-plus-modifier chords are supported. **Clear** disables it; **Reset** returns it to unbound.
2. Hover over an editable model in Sculpt or Modeling and hold your binding. The viewport shows the temporary mask shape and Add/Erase operation. Your original tool and Modeling selection stay intact.
3. While holding, invoke your **Viewport tool menu** binding to choose **Surface brush**, **Through-depth lasso**, or **Through-depth line**, then **Add** or **Erase**. Line also offers **Flip side**. These choices are remembered between holds and restarts.
4. Paint or draw the selection, release the pen to commit, then release the hold to return to the original tool. Each completed mask gesture is undoable.

The **Mask → Hold to mask** section exposes the same choices and a Mask brush preset selector. Surface brushing uses that preset’s pressure, strength, alpha, spacing and stroke settings independently of the ordinary sculpt brush. Surface brushing respects brush symmetry; all three shapes protect hidden geometry. Through-depth lasso and line do not repeat with brush symmetry. Lasso and line select through every depth on the active model; they also work when the gesture begins in empty space.

If you release the hold midway through a freehand stroke, already-painted mask dabs remain as one Undo step. Unfinished lasso, line and stamp previews are discarded. Escape or focus loss restores the prior tool; a still-down pen cannot leak into a new sculpt or Modeling gesture. Merely holding, changing the mask mode or opening/dismissing the menu makes no edit.

Visibility wins when its hold and the mask hold both match. Otherwise masking takes priority over Smooth, Reverse, primary-drag zoom/pan and Modeling selection subtraction; select Add/Erase explicitly in the mask menu. Right-drag orbit, middle-drag pan and wheel zoom remain available between gestures. Other action shortcuts pause while masking. Key settings reports overlapping assignments; use different keys for the hold and menu invocation. The mode is unavailable during path editing, projection, Slice, transforms, reference arrangement, modal editors or on an unconverted primitive.

## Mask operations

### Example: protect a region with a lasso

1. Choose **Mask → Mask lasso**.
2. Drag an L-shaped outline over the sphere. The green outline closes back to the starting point.
3. Release to apply, then choose **Done**. The dark region is protected.
4. Try a Standard stroke across its boundary. Clay should change on the unmasked side while the dark area stays protected. Undo the trial stroke when finished.

If you protected the wrong side, choose **Invert mask**. To start over, use **Clear mask**; see the controls below.

| While drawing | After release |
| --- | --- |
| ![Green L-shaped mask outline before release](../images/mask-lasso-preview.png) | ![Dark protected region after releasing the mask lasso](../images/mask-lasso-result.png) |

*Mask lasso selects through the model, including its back. Hold Alt to erase masking inside the outline.*

### Protect one side through the model

1. Choose **Mask → Mask through → Line** (also available from **Slice → Mask through · Line**).
2. Choose **Add mask** or **Erase mask** in the tool strip. **Flip side** selects the opposite side of the line.
3. Press in empty viewport space above the model, draw a dividing line across it, and release below it. The tinted side and arrow show the affected half-plane; the line extends beyond both endpoints.
4. Orbit to inspect the back. The same projected side is masked at every depth, in both orthographic and perspective views.

During a drag, **Space** moves the whole line. **Escape** cancels without an edit; **Done** returns to normal sculpting. The existing navigation border and axis gizmo remain reserved for navigation. Away from those controls, an ordinary primary drag belongs to masking even when it starts off the model. Secondary-button orbit, middle-button pan, and your configured zoom/pan holds remain available between gestures. The configured zoom/pan holds take precedence when starting a primary drag.

Line masking changes only mask weights on the active model and all stored subdivision levels. It preserves other masks outside the affected side. Rear-facing surfaces are included; explicitly hidden faces and their boundary vertices stay protected. It never cuts geometry or resets levels. One completed gesture is one Undo step; empty or unchanged selections create no history entry.

**Mask through: Line** and **Flip mask line side** are available in custom menus and **Settings → Key settings**, with no assigned shortcut by default. Assign, remap, or clear them there. The tool strip displays the assigned flip shortcut; it can flip the preview while you hold the drag. Add/Erase and the side checkbox are also available before starting a gesture. **Mask lasso** remains alongside Line for freeform outlines.

### Adjust protection

Use the **Mask** menu:

| Control | Result |
| --- | --- |
| Clear mask | Remove all protection |
| Invert mask | Swap protected and editable regions, including soft values |
| Expand / Contract mask | Grow or shrink by one mesh-edge ring |
| Blur mask | Soften the boundary; repeat for a wider transition |
| Sharpen mask | Increase contrast around 50% |
| Mask intensity | Scale the soft mask's strength |
| Selection threshold | Minimum mask value for grouping, deletion, and extraction; default 50% |

Blur/sharpen affect every stored subdivision level, including hidden faces. They leave geometry and saved group weights unchanged. Each changed operation is undoable.

Setting intensity to zero clears the mask when the drag ends. Undo or recall a group to restore it. Zero-valued vertices remain excluded even at a zero selection threshold.

### Delete masked geometry

Choose **Delete masked** to remove faces whose vertices all meet the threshold. Other submodels stay intact.

**Deletion can leave open boundaries and resets the active subdivision stack and adaptive/voxel settings.** Undo restores the previous state. Deleting the entire surface leaves an empty model; undo, initialize a primitive, or create a new sphere.

## Vertex groups

### Save and recall a region

1. Paint or lasso a mask.
2. Open **Submodels → Vertex groups** and enter a name.
3. Choose **Group masked**.
4. Later, select that group and choose **Mask group** to recall it.

| Group action | Result |
| --- | --- |
| Group all | Replace the active model's groups with one whole-model group |
| Add masked / Remove masked | Move masked membership into the chosen group / remove it from that group |
| Rename / Delete | Manage a saved group |
| Eye icon | Hide/show complete faces assigned to the group |
| Flip visibility | Swap visible and hidden faces |

Blank names become Group 1, Group 2, and so on. New spheres start in a **Base** group. Group changes are undoable.

### Colors and saved weights

Use **View → Show vertex group colors** to check group boundaries, or turn it off to see the clay without group colors.

- Assigning a region to a group replaces its previous membership. Any unassigned remainder stays in its old group; empty groups are removed across stored subdivision levels.
- Neighboring face groups can share boundary vertices. Each face shows one color; a partial face selection can retain weights without coloring the whole face.
- Made the wrong assignment? **Undo** restores the previous groups.

Groups survive saving, duplication, subdivision, extraction, and deletion. Adaptive topology interpolates weights; voxel remeshing approximates boundaries. OBJ exports face groups; `.clay` preserves soft weights.

Thickness can generate front/back/rim groups and masks. These become editable after applying thickness.

## Selection and visibility

### Hide or isolate a region

Choose **Visibility → Select rectangle** or **Select lasso**. Draw the region and release; choose **Done** to return to sculpting.

| During a selection | Effect |
| --- | --- |
| Normal drag | Isolate enclosed faces; green preview |
| Hold Alt | Hide enclosed faces; red preview |
| Hold Space and drag | Move the outline |
| Esc | Cancel the current outline |
| Right drag between selections | Orbit |

Both tools select through all depths on the **active submodel only**. Visibility selects face centers and keeps quads whole. Masks select vertices. Neither repeats with brush symmetry or varies with pressure.

### Hide with a lasso

Use **Select lasso**, hold **Alt**, draw the outline, and release:

| Before release | Hidden region |
| --- | --- |
| ![Red outline previews the region to hide](../images/hide-lasso-preview.png) | ![Faces inside the lasso are hidden, leaving an opening in the displayed sphere](../images/hide-lasso-result.png) |

*Hiding changes visibility, not geometry. Use Show all to restore the view.*

### Temporary rectangle shortcuts

In Sculpt mode, hold **Ctrl+Shift**:

| Gesture | Result |
| --- | --- |
| Drag | Isolate a rectangle; add Alt to hide it |
| Click a group's face | Hide that group; if it was the last visible group, flip visibility |
| Click background | Show all faces |
| Drag entirely in empty background | Invert visibility |

You can release the keys after starting a selection. This temporary gesture remains a rectangle unless an explicit selection tool is active.

### Restore visibility and avoid surprises

Use **Visibility → Show all / Invert visibility**. Hidden faces and their shared boundary vertices are protected from brush edits and excluded from picking and wireframe.

Visibility saves with the project and supports undo across stored subdivision levels. Remeshing pauses while faces are hidden; show all before remeshing or slicing.

An unfinished outline is cancelled by tool/view/model changes or loss of focus. Taps and line-only outlines do nothing. Concave lassos work; crossing outlines alternate inside/outside regions. Overlong outlines are cancelled with a message.

### Split hidden faces

Choose **Visibility → Split hidden** to move hidden faces into a new visible submodel. Both pieces stay in place with masks, groups, and quads intact.

You need both visible and hidden faces. Apply/remove thickness modifier first. Boundaries remain open, and both pieces get fresh subdivision stacks and adaptive/voxel settings. One Undo restores the original model.

## Related guides

[Model tools](modeling.md) · [Undo and redo](sculpting.md#undo-and-redo) · [Thickness modifier](modeling.md#thickness-modifier)


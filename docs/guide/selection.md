# Masks, vertex groups, and selection

[Documentation home](../../README.md#documentation)

**Mask** a region to protect it from brushes. **Hide** it to work without seeing or picking it. **Group** it to recall a selection later.

- [Protect a region with a lasso](#example-protect-a-region-with-a-lasso)
- [Edit a mask](#mask-operations)
- [Save a region as a group](#vertex-groups)
- [Hide or isolate faces](#selection-and-visibility)
- [Split off hidden faces](#split-hidden-faces)

## Mask operations

### Example: protect a region with a lasso

1. Choose **Mask → Mask lasso**.
2. Drag an L-shaped outline over the sphere. The green outline closes back to the starting point.
3. Release to apply, then choose **Done**. The dark region is protected.
4. Try a Standard stroke across its boundary. Undo the trial stroke when finished.

| While drawing | After release |
| --- | --- |
| ![Green L-shaped mask outline before release](../images/mask-lasso-preview.png) | ![Dark protected region after releasing the mask lasso](../images/mask-lasso-result.png) |

*Mask lasso selects through the model, including its back. Hold Alt to erase masking inside the outline.*

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
| Add masked / Remove masked | Change membership using the current mask |
| Rename / Delete | Manage a saved group |
| Eye icon | Hide/show complete faces assigned to the group |
| Flip visibility | Swap visible and hidden faces |

Blank names become Group 1, Group 2, and so on. New spheres start in a **Base** group. Group changes are undoable.

### Colors and saved weights

Groups can overlap as soft vertex selections, but each face displays one group color. Partial face selections retain weights without coloring the whole face. Use **View → Show vertex group colors** to hide the colors without hiding geometry.

Groups survive saving, duplication, subdivision, extraction, and deletion. Adaptive topology interpolates weights; voxel remeshing approximates boundaries. OBJ exports face groups; `.clay` preserves independent soft weights.

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

You need both visible and hidden faces. Apply/remove temporary thickness first. Boundaries remain open, and both pieces get fresh subdivision stacks and adaptive/voxel settings. One Undo restores the original model.

## Related guides

[Model tools](modeling.md) · [Undo and redo](sculpting.md#undo-and-redo) · [Temporary thickness](modeling.md#temporary-thickness)

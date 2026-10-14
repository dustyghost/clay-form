# Masks, vertex groups, and selection

[Documentation home](../../README.md#documentation)

- [Mask operations](#mask-operations)
- [Vertex groups](#vertex-groups)
- [Selection and visibility](#selection-and-visibility)

## Mask operations

The **Mask** menu provides **Clear mask**, **Invert mask**, and **Delete masked**. Invert swaps protected and editable regions using `1 − mask`, including soft values and every stored subdivision level. Delete removes faces whose vertices all meet the selection threshold (50% by default) from the active model, compacts unused vertices, and preserves surviving masks and complete quads. Cutting resets the active subdivision stack and turns off adaptive/voxel modes and automatic remeshing because the result may have open boundaries. Deleting a fully masked model leaves an empty model; you can undo, initialize a primitive, or choose New. Other submodels are kept. Each operation can be undone; deletion restores all previous levels and mesh settings on undo.

## Vertex groups

In **Submodels → Vertex groups**, choose **Group masked** to add a group from the mask, or **Group all** to replace the active model’s existing groups with one group containing every vertex. Group all updates every subdivision level, preserves masks and face visibility, leaves other submodels unchanged, and can be undone. Blank names automatically become **Group 1**, **Group 2**, and so on; enter a name when you want a custom label. Groups belong to each submodel and store independent soft vertex weights; creating, changing or recalling a group is undoable. Select a group to **Mask group**, **Add masked**, **Remove masked**, rename or delete it. Complete selected faces receive that group's color. Vertex groups can overlap, while each face has one category; partial face selections retain vertex membership without coloring the entire face. **View → Show vertex group colors** toggles the colors.

The **Mask** menu adds **Expand mask** and **Contract mask** (one mesh-edge ring per click), **Mask intensity** (scales soft falloff), and **Selection threshold** (minimum vertex mask for grouping, deletion and extraction). Zero mask values are excluded even at a zero threshold. These adjustments leave saved group weights intact. Reducing intensity to zero clears the mask after the adjustment ends; undo or recall a group to restore it.

**Blur mask** softens boundaries by blending each mask value halfway toward its immediate mesh-edge neighbours. Quad diagonals are ignored; repeat for a wider transition. **Sharpen mask** doubles contrast around 50%, pushing softer values toward clear or fully protected while keeping 50% unchanged. This midpoint is independent of Selection threshold. Both commands affect the active submodel, including hidden faces, and apply to every stored subdivision level at that level's mesh density. Geometry, face visibility, saved vertex group weights, and other submodels stay unchanged. Each click that changes the mask creates one undo step; both actions can be added to custom menus or assigned shortcuts. The resulting masks save in ordinary `.clay` projects.

Thickness adds **Group front / back / rim**, **Mask rim vertices**, and **Group masked vertices** options per submodel. Generated groups appear in the preview and become editable when thickness is applied. Grouping the generated mask can replace part face colors where complete faces meet the threshold; the part vertex groups remain available.

Groups survive project save/load, duplication, subdivision, extraction and deletion. Adaptive topology interpolates weights and preserves face categories at boundaries. Voxel remeshing transfers weights from nearest source vertices and categories from nearest source face centers, so boundaries are approximate after remeshing. OBJ exports face groups using `g` records; independent soft vertex weights are saved in `.clay` projects. The weights provide a foundation for future vertex painting; painting itself is not included yet.

New projects start with the sphere assigned to a **Base** vertex group. The eye beside each vertex group hides or shows its assigned faces, including wireframe and picking. Hidden faces and their shared boundary vertices are protected from brush edits. Visibility is saved with the project and supports undo. Groups containing only partial vertex selections have no complete assigned faces to hide. To hide just the colors while keeping the whole surface visible, use **View → Show vertex group colors**.

## Selection and visibility

Open **Visibility → Select rectangle**, or hold **Ctrl+Shift** in Sculpt mode for a temporary selection gesture. The modifier gestures are inspired by external software vendor's external sculpting application partial mesh visibility reference (section 6.9); Clay Form's vertex group face clicks hide the clicked group first:

- **Ctrl+Shift-drag:** isolate the rectangle (green). Release the keys after starting if preferred; the gesture stays a selection.
- **Alt while dragging:** hide the rectangle (red).
- **Space while dragging:** move the rectangle. **Esc:** cancel without changing the model.
- **Ctrl+Shift-click a vertex group’s face:** hide the clicked group. If it is the last visible group, flip visibility: that group becomes hidden and previously hidden faces become visible.
- **Ctrl+Shift-click background:** show all faces. Drag a rectangle entirely in empty background to invert visibility.

The explicit Select rectangle tool uses the same gestures without holding Ctrl+Shift; choose **Done** to return to sculpting. Right drag still navigates. Selection takes priority over Ctrl-drag zoom. This first version selects face centers through all depths and keeps quads whole. Lasso selection is also available as described below.

**Visibility → Select lasso** lets you draw a freehand outline to isolate faces. Hold **Alt** to hide the enclosed faces instead. **Mask → Mask lasso** adds full masking inside the outline; hold **Alt** to erase masking there. Existing mask values outside the outline stay unchanged. Both tools are available in custom menus and configurable shortcuts.

While drawing, a green outline previews isolate/add and a red outline previews hide/erase; a closing line joins the pointer to the starting point. Release to apply. **Space-drag** moves the whole outline, **Esc** cancels the current gesture, and **Done** returns to sculpting. The camera stays fixed during drawing; right drag navigates between selections. Tool changes, loss of focus, and changes to the model or view cancel an unfinished outline. Taps and line-only outlines do nothing. Concave outlines work; crossing an outline uses alternating inside/outside regions. Overlong outlines are cancelled with a message rather than silently simplified.

Both lassos select **through all depths**, including the back of the model, on the **active submodel only**. There is no front-surface-only mode. Visibility uses face centers and keeps quads whole, just like rectangles; isolate keeps only currently visible faces inside the outline, while hide removes those faces. Mask lasso uses vertex positions and protects hidden faces and their shared boundary vertices, just like the Mask brush. Selection acts on the original mesh under temporary thickness, and does not repeat with brush symmetry or vary with pen pressure. The projected region is applied independently to every stored subdivision level. Each changed gesture creates one Undo step; cancelling or an unchanged selection preserves redo history. Results save in ordinary `.clay` projects. The temporary Ctrl+Shift gesture remains a rectangle when no explicit selection tool is active.

**Visibility → Show all / Invert visibility** also work without gestures. **Submodels → Vertex groups → Flip visibility** swaps the visible and hidden faces of the active submodel, including partial selections. A hidden-face count appears in the viewport and menu. Hiding affects only the active submodel, leaves masks and vertex group face assignments intact, and excludes hidden faces from display, wireframe, picking, and brush edits. Shared boundary vertices stay fixed. Region selections are applied to each stored subdivision level; subdividing carries visibility into the new faces. Visibility is saved in `.clay` projects and supports Undo/Redo. Automatic voxel remeshing pauses while faces are hidden; show all before manual remeshing or slicing. Selection acts on the original surface when temporary thickness is enabled. Apply or remove temporary thickness before splitting.

**Visibility → Split hidden** moves hidden faces into a new, visible submodel immediately after the active one, following external sculpting application's Split Hidden behavior. Both pieces stay in place, retaining masks, vertex groups, and complete quads. Boundaries stay open. Both visible and hidden faces are required. Splitting uses the current subdivision level and resets the resulting pieces' subdivision stacks and adaptive/voxel settings; other submodels stay intact. One Undo restores the complete pre-split state.

### Example: protect a region with a lasso

1. On the starting sphere, choose **Mask → Mask lasso**.
2. Drag an L-shaped outline over the right side of the sphere. Before releasing, check the green region; the closing edge joins your pointer to the start.
3. Release to apply the mask, then choose **Done** to return to sculpting. The dark region is protected, including vertices behind it: lasso masking selects through all depths.
4. Try a Standard stroke across the boundary. For a softer transition, undo the trial stroke and choose **Mask → Blur mask** before trying again.

![Green concave lasso preview over the right side of the sphere before release](../images/mask-lasso-preview.png)

*Before release: the green preview includes the concave corner of the outline.*

![Dark L-shaped masked region on the sphere after releasing the lasso](../images/mask-lasso-result.png)

*After release: the masked region is protected. Blur mask would soften this sharp boundary.*

## Related guides

[Models and mesh tools](modeling.md) · [Undo and redo](sculpting.md#undo-and-redo)

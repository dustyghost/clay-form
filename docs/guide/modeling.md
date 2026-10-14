# Models and mesh tools

[Documentation home](../../README.md#documentation)

- [Submodels](#submodels)
- [Parametric primitives](#parametric-primitives)
- [Model transforms](#model-transforms)
- [Mesh management](#mesh-management)
- [Temporary thickness](#temporary-thickness)
- [Slice](#slice)
- [Merge down](#merge-down)

## Submodels

The graphical **Submodels** panel shows shape previews, selection, eye icons for visibility, and up/down reorder controls. Visible models stay in the viewport; inactive models use their lowest stored subdivision level when one exists, without losing higher detail. The active model is clay-colored; inactive models are blue-gray. Click an inactive model in Sculpt mode to select it; sculpting begins on the next drag. Hidden models cannot be sculpted or transformed.

Duplicate copies the active surface, mask, and subdivision stack. Extract masked copies faces whose vertices meet the selection threshold (50% by default) into a new active, unmasked submodel; the source remains intact. Extracted boundaries are open and use a fresh subdivision stack. OBJ export applies to the active model.

Create named groups, assign the active model through its group selector, reorder models within groups, reorder groups, and toggle a group's visibility. Ungroup removes the group while keeping every model. Selection preserves list order. Projects and undo/redo preserve visibility, groups, ordering, and all models; older single-model projects open as Model. Groups and ordering provide organization for future boolean workflows; boolean operations are not implemented yet.

For adding an existing mesh, see [OBJ import](files.md#obj-import).

## Parametric primitives

Open **Primitive → Create primitive…** for the active submodel. A confirmation warns that its current geometry, masks, and subdivision levels will be replaced; Cancel leaves it untouched and Undo can restore it. For an existing primitive, the Primitive menu shows its shared parameter controls and can be pinned or docked. Docking or pinning moves those controls out of Submodels to avoid duplication.

The shape remains an editable recipe: **Plane, Cube, Circle, UV sphere, Icosphere, Cylinder, Cone, Torus, Grid, or Human head**. The collapsible **Submodels → Primitive** section supplies shape-specific dimensions, spans/segments/rings/subdivisions, caps/fill, a live vertex/triangle count, and wireframe display. Cube X/Y/Z spans control face resolution independently; one span per axis yields eight welded corner vertices. Plane/Grid and Circle lie in the front XY plane; cylinders, cones, and tori use the Y axis.

Axis-colored size handles and teal radius handles on the preview let you drag dimensions and radii directly. Hover a handle for its label. The human-head base adds jaw width/length, chin, cheeks, nose projection/width, eye spacing/depth/size/height, brow, ears, mouth width/opening and lip fullness. The revised base uses a shaped cranium, jaw and chin profile, fitted almond-shaped lids, a nose bridge/tip and upper/lower lips. Eyeballs with geometric iris detail, 32 tooth crowns arranged in upper/lower dental arches, and a tongue are separate closed surfaces inside the same submodel; the skin includes a recessed mouth cavity. **Open mouth** rotates the lower jaw, lower teeth and tongue; upper teeth remain fixed. **Upper eyelid closure** and **Lower eyelid closure** have independent sliders and viewport handles, with Open eyes / Close eyes buttons. **Reset head proportions** applies the revised anatomy to an earlier saved head; Undo restores its previous recipe. The head includes a welded neck, with controls for length, width, muscle definition and throat prominence; Include neck can remove it. Neck landmarks follow the sternocleidomastoid path from behind the ears toward the sternum and medial clavicles ([anatomy reference](https://www.ncbi.nlm.nih.gov/books/NBK532881/)). Facial controls range from 0 to 2 for broader adjustments. Dragging a depth control directly facing the camera uses horizontal mouse movement; orbit for a projected depth direction. Facial controls are symmetric and intended to provide a sculpting base rather than finished anatomy. Whole-model placement remains available through World axes or Action Line; recipes retain those transforms when regenerated. Return to Sculpt mode (Done) to show the teal shape controls again.

Choose **Convert to sculpting mesh** to keep the previewed surface and remove its parametric recipe. Brushes, subdivision, and adaptive/voxel workflows become available after conversion; Undo restores the recipe. An unfilled Circle has no faces and must be filled before conversion; planes, grids, disks, and uncapped tubes remain open surfaces, so closed-surface remeshing/adaptive modes require closing them first. Primitive recipes save with each submodel and survive duplication, switching, undo/redo, and reopening projects. Collapsing the Primitive section keeps the parametric model. Its resolution controls support dragging/typing numbers or minus/plus. Shape, resolution, wireframe, and conversion controls share this one section; it disappears after conversion.

Primitive resolution can also be changed with the diamond handles beside the model: drag left or right to change spans, sides, segments, rings or subdivision levels. The live count updates as you drag, and each drag is one Undo step. Teal circles identify radii and anatomical controls, axis-colored squares identify size controls, and the world transform and camera gizmo share labelled X/Y/Z colours. Hovering or dragging adds a white ring while retaining the control colour.

Primitive span cut lines, span controls, and size controls share **X red, Y green, Z blue** colors. Plane/Grid X spans create red vertical cuts; Y spans create green horizontal cuts. Cube cut loops follow their local controlling axis, including after rotation or scaling. Cylinder/Cone height-span rings are green. The span labels and numeric controls use the same colors in the collapsible Primitive section in Submodels. Enable wireframe to see the colored topology; non-axis topology controls remain violet.

## Model transforms

Click **Transform** to open the existing World axes controls. Pin the menu or dock it to either sidebar using its compact icons; its automatic viewport toolbar is suppressed while pinned or docked. The menu, floating toolbar, and Transform panel share **Move**, **Scale**, **Rotate**, and pivot controls. **Done** returns to sculpting.

- **Move / Scale:** drag a colored X/Y/Z handle for one axis, or the center for view-plane movement / uniform scale. Orbit to expose a Move/Scale axis facing the camera.
- **Rotate:** drag a colored ring around its axis. The live signed angle shows the amount of rotation. Rings viewed edge-on use a straight drag along the projected ring, so snapped front/side views remain usable. Drag outside the handles to orbit the camera.
- **Place pivot:** click the active surface to set the World pivot; clicking empty space uses the current pivot depth. Drag to adjust it in the view plane. The pivot stays placed across operation changes and follows Move. **Reset pivot** returns it to the active model's bounding-box center.

**Action Line** reuses its start endpoint as the pivot and its direction as the transform axis. Choose **Set line** and drag to place it; either endpoint can be adjusted. **Move** uses the middle handle. **Scale** uses the end to stretch along the line, or the middle for uniform scale. **Rotate** turns around the line using its amber ring. **Reset pivot** moves the line's start to the model center while retaining the line's direction and length. A zero-length line must be extended before rotation.

**Respect mask** protects fully masked vertices; soft masks reduce the rotation angle without shrinking their radius around the axis. Disable it to transform the whole model. Every stored subdivision level follows, and each changed geometry drag is one undo step. Parametric primitives keep their rotated placement when dimensions or resolution are edited, and when saved/reopened.

Pivot and line placement do not change geometry, dirty the project, or consume undo steps. They are temporary viewport controls retained across transform operation changes, but reset when the viewport is cleared by actions such as model switching, project opening, undo/redo, or mesh/mask operations. They are not stored in projects. Focus loss, a tool switch, or a camera command finishes the current drag and prevents further changes until release. Rotate, Place pivot, and Reset pivot are available in custom menus and Key settings; they have no new default shortcuts.

### Example: rotate a cube and keep its recipe

1. In a practice project, create a **Cube** primitive. Set its X/Y/Z sizes to **1.4 / 0.8 / 1.0** and its spans to **2** on each axis.
2. Right-drag to view the top and sides, then choose **Transform → Rotate**.
3. Drag the blue **Z** ring. Watch the signed angle while dragging, then release to keep the rotation.
4. Choose **Done** and adjust a cube dimension in its Primitive controls. The cube regenerates in its rotated placement. Undo the dimension edit and rotation separately to compare.

![Cube during a World axes rotation with colored rings and a live signed angle](../images/rotation-controls.png)

*The colored rings rotate the model around the pivot. The live angle measures the current drag; it is not a camera orbit.*

## Mesh management

Use the **Mesh** menu or the **Mesh management** panel. New spheres use an all-quad cube-sphere surface with 4,706 vertices and 4,704 quad faces, leaving room for subdivision and avoiding triangle fans at the poles. Opening older projects preserves their geometry.

**Subdivision:** add a level with Subdivide, then select a level with the slider or Lower/Higher buttons. Each face splits into four triangles with shared edge midpoints, preserving the existing shape. This is linear subdivision rather than a smoothing subdivision surface. Lower-level edits propagate upward while retaining stored fine detail; sculpting higher levels updates their parent vertices when you step down. Mask values interpolate between levels. Subdivide from the highest level. **Delete lower** removes every level below the active level, making it the new level 1 while retaining higher detail. **Delete higher** removes every level above it, allowing a fresh subdivision from the current surface. Both preserve the active geometry and masks and can be undone. The active mesh and all stored levels save in `.clay` projects. OBJ export writes the active level.

**Dynamic mesh:** switch modes to keep the active surface as a single mesh, discarding stored subdivision levels. Choose resolution (16–96 voxels along the longest bounding-box axis) and click Remesh now. Voxel reconstruction redistributes geometry, joins overlapping outward-oriented closed shells, and preserves separated volumes. It requires a closed, consistently oriented mesh. Optional Remesh after each stroke runs when a geometry stroke ends; painting masks does not trigger it. This is whole-surface voxel remeshing, rather than local adaptive triangulation while the brush moves.

Remeshing clears masks and can remove features thinner than a voxel. Start with a moderate resolution and raise it for detail. Subdivision and remeshing run in background workers; sculpt input pauses during the operation. Cancel keeps the original mesh. Mode switches, level changes, strokes, and completed topology operations can be undone, including restoring discarded levels. Failed operations leave geometry intact; a failed automatic remesh disables the automatic setting so you can adjust it before retrying.

## Temporary thickness

In **Submodels → Thickness · open meshes**, enable **Temporary thickness** and set the total thickness in model units. The preview offsets half the amount to each side of the surface and adds walls around boundary loops. Orbit to inspect it. Each submodel saves its own value; duplication retains it. The original sculpting vertices, masks, subdivision levels, and surface picking remain unchanged until you apply it. OBJ exports the original surface while thickness is temporary.

**Apply thickness** turns the exact preview into a closed sculpting mesh, preserves masks and complete quads, clears the temporary setting, and resets subdivision levels and topology modes. Applying a primitive preview converts it to a sculpting mesh. Other submodels stay unchanged. Undo restores the original surface, recipe/levels, and thickness setting. Thickness works on consistently oriented open surfaces with nonbranching boundary loops; closed meshes and outlines without faces are not eligible. Large normal offsets can intersect at tight folds or concave areas, so inspect the preview before applying.

## Slice

Choose **Slice** in the toolbar, then choose a result:

- **Vertex groups** cuts faces along the line while keeping one connected surface. Existing face categories stay on one side; categories spanning the line get new **Slice** counterparts on the other. Existing weighted vertex groups remain available.
- **Trim** removes one side. The preview labels **Keep** and **Remove**; **Flip kept side** reverses them.
- **Separate** keeps both sides as independent submodels, placing the new **Slice** model immediately after the active one. Both pieces initially stay in place.

Drag a straight line in the viewport, starting on the model or in empty space. The faint line extension shows the full cut: a plane through all depths of the active model. Hold **Space** while dragging to move the whole line without changing its angle. Release the mouse to apply; **Esc** cancels the gesture and exits Slice mode. Right drag still orbits between cuts. **Done slicing** returns to sculpting. Slice does not mirror with brush symmetry or use masks to restrict the cut; surviving mask values and existing vertex-group weights are preserved and interpolated at new vertices.

Choose **Path** for a cut with editable bends. Click to add points (up to 24), click an edge to insert a point, and drag a point to change the shape. Select an interior point to reveal corner handles: drag the blue diamond away from the point to soften the corner, or back toward it to sharpen it. Rounded corners also show two linked tangent handles where the curve meets the straight edges; dragging either changes the rounding. **Sharp** removes rounding immediately; **Rounded** adds it. The **Turn** angle and **Round (px)** controls remain available; rounding is limited by the neighboring segment lengths. **Shift-drag** snaps to 15°; **Space-drag** moves the whole path. **Delete point** (or Delete/Backspace) removes the selected point. Choose **Apply cut** or press Enter to commit; **Clear** discards the draft; **Esc** discards it and exits Slice mode. The faint end extensions continue through all depths of the model. Paths and their extensions must not cross themselves. The camera stays fixed while editing a path; clear or apply it before orbiting. A failed or cancelled cut keeps the draft available for correction.

Cuts follow the current [view projection](navigation.md#perspective-view). Orthographic cuts extend in parallel through the model; perspective cut planes converge at the camera eye so they match the line or path on screen. Changing projection or field of view discards the draft. A perspective path cut rejects a model crossing the camera eye; move the model fully in front of the camera or switch to orthographic view and redraw the path.

For Trim and Separate, **Cap cuts** is on by default. It closes the cut with triangles in a **Cut** vertex group, including concave boundaries and holes; path caps follow the bends and rounded corners. Caps require a closed, consistently oriented source surface. Turn caps off to slice open surfaces, or apply temporary thickness first. Convert primitives before slicing, apply or remove temporary thickness, and show all vertex group faces of the active model.

A slice uses the active subdivision level and resets that model's subdivision stack, adaptive/voxel modes, and automatic remeshing. Uncut quads are retained; intersected faces and caps use triangles. Other submodels remain unchanged. The operation runs in a cancellable background worker and creates one Undo step; failures and cancellation keep the original model. Slice results save in ordinary `.clay` projects and export through the existing OBJ workflow.

## Merge down

Choose **Submodels → Merge down** to combine the active submodel with the next one in the panel's grouped order, following external sculpting application's MergeDown operation. The tooltip identifies the target; the button is disabled for the last model. The active model keeps its name and folder. Positions, masks, complete quads, weighted vertex groups, and face visibility are preserved; duplicate vertex-group names receive a suffix. Hidden submodels contribute hidden faces.

Merge uses each model's current mesh level, bakes primitive previews and temporary thickness, and resets the merged result's subdivision and adaptive/voxel settings. Fill unfilled Circle primitives first. It combines surfaces without welding vertices or performing a boolean union. Other submodels stay untouched. **Undo** restores both original models, their settings and their order; merged results save normally in `.clay` projects. Mesh and group limits are checked before changing the document.

## Related guides

[Masks, vertex groups, and selection](selection.md) · [Adaptive sculpting](sculpting.md#snake-brush-and-adaptive-sculpting) · [Surface display](navigation.md#surface-display)

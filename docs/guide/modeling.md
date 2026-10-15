# Models and mesh tools

[Documentation home](../../README.md#documentation)

Build a base shape, position it, and choose how to add detail.

- [Manage submodels](#submodels)
- [Create a primitive](#parametric-primitives)
- [Move, scale, and rotate](#model-transforms)
- [Mirror one side onto the other](#mirror-one-side)
- [Model with a pen](#pen-modeling)
- [Edge loops and creases](edges.md)
- [Add mesh detail](#mesh-management)
- [Thicken an open surface](#thickness-modifier)
- [Cut the model](#slice)
- [Combine submodels](#merge-down)

## Pen modeling

Open **Mesh → Pen modeling** to edit the active submodel. For a fresh starting shape, click **New mesh…** beside the toggle. The confirmation dialog defaults to a cube with **3 spans per axis**; adjust X/Y/Z spans before choosing **Replace and start modeling**. Nothing changes until confirmation. Cancel keeps the current model, and one Undo restores its geometry, masks, subdivision levels and temporary Thickness after replacement. Other submodels are preserved. The new cube is immediately editable in Face selection mode.

![The modeling starter warning, with span controls and explicit replacement confirmation](../images/modeling-new-mesh-warning.png)

You can also create another shape through **Primitive**, configure it, and convert it to a sculpting mesh before enabling Pen modeling. Choose the active submodel in **Submodels** before selecting components.

### Select faces, edges, or vertices

1. Choose **Face**, **Edge**, or **Vertex**, then **Select**.
2. Hover to see the gold target; tap or **hold and drag across components** to select them in cyan. Fast strokes collect components between pen samples. Individual selection picks the visible surface, including in perspective view.
3. Choose **Replace**, **Add**, or **Remove** to build the selection without a keyboard. Replace clears the previous selection once when the stroke begins, then collects everything crossed. Shift at pen-down adds; Ctrl removes. Release keeps the selection; Escape cancels the whole stroke and restores the previous selection.
4. Use **Clear selection** to start over. In Edge mode, **Select loop** follows an unambiguous edge loop or border.

Choose the selection pattern below Replace/Add/Remove:

- **Individual:** paint faces, edges, or vertices under the pen.
- **Loop** in Face mode: selects a strip of quads. Hover near a face edge to choose the direction across that edge; the full strip previews in gold. Move near a perpendicular edge to choose the other direction.
- **Loop** in Edge mode: follows connected edges along an unambiguous loop, crease, or border chain.
- **Parallel edges** in Edge mode: selects an edge ring by following opposite edges across connected quads. It follows topology, including around curved surfaces.

Loop selections can wrap around the model, including its far side. Face loops and edge rings stop at hidden faces, triangles, open boundaries, or non-manifold branches. Add/Remove and drag selection also work with these patterns. Each crossed component is visited once per stroke, so retracing does not toggle it.

![A row of faces selected by dragging across the mesh](../images/modeling-paint-selection.png)

![A perpendicular face loop previews in gold beside the existing cyan selection](../images/modeling-face-loop.png)

![Parallel edges selected across a quad strip](../images/modeling-edge-ring.png)

Faces select entire quads rather than their rendering triangles. Changing component mode converts the selection: faces contribute their corners and edges; returning to Face selects only faces whose corners are all selected. **Select region boundary** selects only the outer edges of the chosen faces. Selections belong to the active mesh and clear after topology, visibility, or model changes. Navigation does not clear them. Drag the navigation frame or empty space to orbit; secondary-button navigation also works. Escape exits modeling when no preview is active.

### Shared targets and component transforms

The **Target** choices apply to selection and component edits. **Connected** follows connected visible faces, **Flat island** follows connected faces within the chosen **Island angle** of the starting face, **Face group** selects the visible polygons with the same face label, and **Boundary** follows an open boundary in Edge mode. Loop and parallel-edge targets remain available where applicable. An edit starting on an already selected component uses the current selection; otherwise it uses the hovered target. Hover previews the affected target before pressing.

**Island angle** ranges from 0° (flat faces) to 180°. Each face is compared with the original face normal, so a chain of small bends cannot gradually wrap the selection around a curved surface. Hidden faces stop the flood; disconnected surfaces are excluded. Hover previews the result as you adjust the slider. The angle applies to the next selection or edit target and is saved in tool presets.

Choose **Move**, **Rotate**, or **Scale** in any component mode, then choose an interaction:

| Interaction | Move | Rotate | Scale |
| --- | --- | --- | --- |
| **Free** (default) | Drag a target with the pen in the view plane; works in orthographic and perspective views | Drag with a virtual trackball around the selection center | Drag up/down for uniform scaling |
| **Gizmo** | Drag colored X/Y/Z arrows, XY/YZ/XZ plane squares, or the center for free movement | Drag a colored axis ring, or the center for trackball rotation | Drag an axis box to stretch, or the center up/down to scale uniformly |
| **Axis / amount** | Choose X/Y/Z or Normal, then drag up/down or enter Distance | Choose the axis and enter degrees or drag up/down | Choose an axis or Uniform, then enter the factor or drag up/down |

In Gizmo mode, select components first, or tap a target to make it the selection. Merely tapping a gizmo handle never applies an edit; drag to begin. Axis handles pointing almost directly into the screen are hidden because their direction cannot be dragged reliably; orbit the view or use Axis / amount for that axis. Edge-on rotation rings use a stable tangent drag.

![Selection-local move arrows and plane handles](../images/modeling-move-gizmo.png)

![Axis scaling handles and the uniform center handle](../images/modeling-scale-gizmo.png)

![Rotation rings around the selected region](../images/modeling-rotate-gizmo.png)

Release to apply, or Escape to cancel. Each completed drag is one Undo step and keeps the selection. Rotate and scale use the selection center; with symmetry, the gizmo uses the reference-side center and the other copies follow the same edit. Symmetry keeps shared seam vertices on their planes. Hidden faces and, by default, masked vertices are protected. These edits currently replace stored subdivision levels; Undo restores them.

A free or gizmo drag remembers its direction and amount. **Apply amount** reuses that direction with the displayed value; move and rotation need an initial drag to establish a direction. **Repeat last edit** also retains it. Free scaling stays uniform. Free mode supports **Tap to repeat** after establishing the edit; Gizmo mode reserves taps for selecting targets and handles. Transform modes, directions and amounts save in presets; older presets retain Axis / amount behavior. Right-click Free, Gizmo or Axis / amount to add it to a custom menu or shelf, or assign a shortcut.

### Extrude a face or flat region

1. Choose **Face → Extrude**.
2. Press a face and drag **up to extrude outward** or **down to push inward**. Pressing an already-selected face extrudes the selected flat region together; pressing another face extrudes that face alone.
3. Inspect the live preview and distance displayed at the bottom of the viewport.
4. Release to apply, or press Escape to discard the preview and stay in modeling mode.

Pressure does not change extrusion distance. Each completed drag is one Undo step. Selected faces must lie on the same plane and point in the same direction. Topology edits replace stored subdivision levels and turn off adaptive/dynamic topology; Undo restores the previous mesh settings and levels. Apply or disable temporary Thickness before modeling. Modeling uses the enabled symmetry axes when all sides have matching geometry and topology. **Protect mask** is enabled by default: an edit touching masked geometry is rejected as a whole. Turn it off explicitly to edit through a mask. Hidden geometry stays protected.

![A face extruded outward, with its moving rim highlighted gold and its distance shown below](../images/modeling-extrusion-preview.png)

The highlighted rim stays on the dragged face even as the pen moves across other faces.

### Construct a region

**Extrude** supports connected regions with shared side walls. Nonplanar regions move along averaged vertex normals; **Individual faces** makes separate extrusions. Distance is available numerically through **Apply amount**, as well as by pen drag. Matching-face tunnel joins remain available for aligned caps.

**Inset** adds a border of the chosen Width inside a flat region. **Individual faces** insets each face separately. **Bevel boundary** trims a flat region's exterior corners, creating a single chamfer band; each corner must have one exterior edge descending away from the region. This first bevel does not handle arbitrary edge networks or rounded multi-segment profiles. Widths that collapse narrow faces are rejected. Inset and bevel retain the cap selection for the next operation. New faces inherit their source region's face label; surviving weights and creases are retained.

**Extrude and fuse** adds contact handling while pushing a single flat face inward. It can stop at the first compatible intermediate loop, remove the swept exterior strips and retain the moved cap. **Join matching faces** separately permits opening a tunnel when an opposing cap matches. **Snap range** is a fraction of the local edge length, independent of sculpt brush size. Compatibility still requires aligned corners and complete adjoining side strips; this does not cut arbitrary intersecting surfaces. Symmetry applies only when all copies can complete safely.

### Construct from edges, points and face centers

Choose a component mode to reveal its construction tools. Measured tools support numeric amounts, pen previews, Escape to cancel, tap repeat and one Undo per completed edit.

| Mode | Tool | Controls and result |
| --- | --- | --- |
| Edge | Extrude edge | Drag up/down or set Distance. An open border extends into a quad strip. A surface edge creates a ridge with new side faces; hover near the desired adjacent face, or enable **Other adjacent side**, then press to lock that side. |
| Edge | Split edge | Drag along the edge to place a new vertex. **Position** runs from 0 to 1, starting at the circled endpoint nearest the initial press; insertion stays just inside the endpoints. |
| Edge | Collapse edge | Drag along the edge or choose **Marked end**, **Midpoint**, or **Other end**. This merges both endpoints at the chosen position. |
| Vertex | Split point | Set Radius or drag up/down to replace a vertex with a ring and center patch. **Circularize ring**, **Triangulate center**, and **Crease ring** control the patch. |
| Vertex | Extrude point | Set Distance or drag up/down to create a pointed protrusion along the averaged surface normal. **Base radius** controls its footprint independently; circularization and ring creasing are optional. |
| Face | Insert center point | Tap a face to insert its center and connect it to every corner with triangles. |
| Face | Split face | Tap to connect a new center to shared edge midpoints, producing quads inside each selected face. |

**Insert center point** and **Split face** act on a tap even when Tap to repeat is disabled. Both also offer **Apply to selection**. New construction tools clear the component selection after committing; hover or select the new geometry for the next operation. Their buttons can be added to custom menus, shelves and shortcuts, and their settings save in tool presets.

To turn a point into a tube base, use **Vertex → Split point**, then switch to **Face → Extrude** and drag the center patch outward. A three- or four-sided patch remains one selectable polygon unless Triangulate center is enabled; larger patches use center triangles, which can be selected together for region extrusion.

![A pair of vertices split into creased patches with matching symmetry](../images/modeling-point-split.png)

![The patches extruded into short tubes](../images/modeling-point-tube.png)

![A surface edge extrusion preview showing new geometry before release](../images/modeling-edge-construction-preview.png)

![Matching faces split into quads, with neighboring faces connected to the new edge midpoints](../images/modeling-face-split.png)

Point construction requires closed, visible vertex neighborhoods. Selected points must not share a face, and the radius must be less than half the shortest incident edge. Surface-edge extrusion requires separate edge neighborhoods; open border chains can share endpoints. Interactive Collapse edge requires edges with separate endpoints; the **Topology → Collapse edges** command remains available for collapsing connected sets to their average position. Symmetry rejects conflicting endpoint or adjacent-side choices instead of applying only part of an edit.

New edge points propagate to neighboring faces to keep the surface connected without cracks. Polygons with more than four corners are represented by triangles; neighboring faces can therefore become triangulated. Source masks, group weights, face labels and surviving creases are retained. Hidden faces and protected masks reject the complete operation. Component extrusion creates geometry but does not automatically fuse intersecting surfaces.

### Push through to make a tunnel

1. Create a cube with **3 spans on each axis**, then convert it to a sculpting mesh.
2. In **Face → Extrude**, leave **Join matching faces** enabled.
3. Press the middle face of one side and drag downward until it reaches the opposing face.
4. Look for **Tunnel snapped**, then release. The two caps disappear and the boundaries join, leaving a through-hole.

Joining beside an existing hole removes the shared wall to make one wider opening. At an outside edge or corner, it removes the coincident side surface to leave an open notch, including side surfaces split by depth loops. The remaining walls reconnect to those loops; regular spacing stays in quads, while differing spacing may introduce triangles. Hidden adjoining faces must be shown before joining.

Tunnel joining currently works on one face at a time. Opposing faces must have matching, aligned boundaries and opposite normals; equal area alone is insufficient. The drag snaps near the first matching face and stops there even if you overshoot. It rejects joins that collapse faces or create duplicate topology. This is a matching-face join, not a general intersection or Boolean tool; mismatched surfaces do not automatically cut each other.

![A cube with a square through-hole and visible interior walls](../images/modeling-tunnel.png)

A slight turn exposes the tunnel walls and the opening at the far end.

![Adjacent cuts form one opening, and a boundary cut leaves an open notch without a bottom wall](../images/modeling-boundary-join.png)

The exterior depth loops remain connected to the newly exposed walls.

### Insert a loop cut

Choose **Edge → Loop cut**. Hover an edge to preview the cut across its quad strip. A tap inserts a centered cut; press, hold, and drag along the edge to slide the preview before releasing. The original edge stays targeted throughout the drag. Escape cancels.

Use **Loops** or the **− / +** buttons to choose 1–16 evenly spaced cuts. While holding a cut, scroll the wheel up/down to increase/decrease the count. Dragging moves the set together, stopping before the outer cuts reach an existing edge. Release commits the whole set as one Undo step.

The cuts share vertices across faces and preserve masks, group weights, and creases. It follows a closed quad ring or ends at an open boundary. Triangles, hidden faces, and self-crossing strips produce an explanation instead of a partial cut.

![A gold midpoint loop preview crossing the cube before any geometry is changed](../images/modeling-loop-preview.png)

![Three loop cuts slid together while the pen remains held](../images/modeling-loop-drag.png)

For loops around a hole or selected region, use the existing [support-loop controls](edges.md).

### Dissolve selected loops

1. Choose **Edge → Select → Loop**, then select the loop to remove. Use **Add** to select several separate loops.
2. Click **Dissolve selected loops**. The loop edges and redundant vertices disappear; the surrounding faces merge into quads without opening holes.
3. Use **Undo** to restore the entire operation, including stored subdivision levels.

This requires complete, noncrossing interior quad loops. An open loop must end at the mesh border. Partial loops, selected border edges, hidden adjoining faces, and intersections are rejected without changing the mesh. Merging across different face-group labels is also rejected. Remaining vertex masks/group weights and boundary creases are preserved. Removing a curved loop can simplify the shape.

![Two mirrored edge loops selected for removal](../images/modeling-dissolve-before.png)

![The selected loops removed, with larger connected quads remaining](../images/modeling-dissolve-after.png)

### Slide and repair topology

Use **Edge → Select → Loop**, then **Slide loop**. Drag to preview, or enter a fraction between -1 and 1 and choose **Apply amount**. The chosen axis determines the positive rail on the reference side; symmetry mirrors that movement. Sliding keeps the same topology and selection. It requires a complete interior quad loop with unambiguous rails; selected loops must not share a face.

Expand **Topology** for operations on the current selection:

| Mode | Action | Result and limits |
| --- | --- | --- |
| Face | Delete faces | Removes the selected faces and opens holes |
| Edge | Fill boundary | Fills complete planar boundaries; concave outlines are triangulated |
| Edge | Bridge boundaries | Connects two equal-count open boundaries and their symmetry copies with a strip of quads |
| Edge | Split edges | Adds midpoint vertices; adjoining polygons may become triangles |
| Edge | Collapse edges | Merges each connected set of selected edge endpoints at their average position |
| Vertex | Connect vertices | Splits a quad between two opposite corners |
| Vertex | Weld vertices | Merges two vertices at their midpoint, including corresponding symmetry pairs |

Use **Target → Boundary** to select a complete hole. For two boundaries or vertices, use **Add** to select the second. These edits preserve surviving weights and creases, respect **Protect mask**, and reject invalid or ambiguous results as a whole. Bridge does not detect arbitrary intersections with other surfaces. Delete and collapse change the surface; **Dissolve selected loops** removes redundant loops while keeping it connected. Each completed edit is one Undo step.

### Model with symmetry

Enable the existing **Symmetry** toggle and choose mirror axes or radial settings in **Symmetry settings**. Pen modeling uses the same center and axes for component selection, loop/ring selection, group assignment, extrusion/tunnels, loop cuts, and dissolve.

Every enabled copy must match the active mesh's vertex positions, polygon connectivity, winding, and face visibility. A small automatic tolerance accommodates floating-point rounding; it does not approximate asymmetric shapes or remesh mismatched sides. If correspondence fails, the tool displays an explanation and leaves geometry unchanged. Disable symmetry to work on unmatched geometry.

Selection previews show the matching components. Symmetry copies of an edit commit together as one Undo step. Shared center-plane cuts are deduplicated; sliding a cut off center creates its reflected counterpart. Intersecting symmetric cuts form a quad grid. Adjacent coplanar extrusion copies share walls, while copies meeting at an ambiguous corner are rejected. Changing symmetry settings cancels an active preview.

![A face selection mirrored across the symmetry plane](../images/modeling-symmetry-selection.png)

![Two matching faces extruded together using symmetry](../images/modeling-symmetry-extrude.png)

### Keep useful tool setups and inspect the smooth result

Expand **Tool presets**, enter a name, then **Save tool preset**. Each preset stores component mode, action, target, transform interaction/direction, axis, amount, loop count, flat-island angle, component-construction options, contact options and protection/preview settings. **Use …** restores that setup without changing geometry. Right-click a preset to add it to a custom menu or shelf, or assign a shortcut. Presets save in application preferences, independently of the project. Deleting one leaves any old shelf entry unavailable; its identifier is never reused.

**Tap to repeat** is on by default for Extrude, Extrude and fuse, Inset, Bevel boundary, Move, Rotate, Scale, Slide loop and the measured edge/point construction tools. Drag to establish an amount, then tap another target to apply the displayed amount and current options. Each measured tool remembers its amount during the session, including when switching tools. Tapping a selected component applies to that selection; tapping elsewhere uses the hovered target. You can also enter an amount before the first tap; free move and rotation need an initial drag to establish their direction. Gizmo transforms use drag handles instead of tap repeat. Small pen movements within three screen pixels count as a tap; once you drag farther, returning to the starting point does not trigger repeat. Escape cancels, and each completed tap is one Undo. Turn **Tap to repeat** off for drag-only editing; the toggle can be added to menus/shelves or shortcuts and is saved in tool presets. Select keeps its tap/paint behavior, and a Loop cut tap remains centered.

![An inset repeated by tapping another target, with matching symmetry](../images/modeling-tap-repeat.png)

**Repeat last edit** explicitly applies the previous measured operation to the current selection; its tooltip names the operation and amount. It uses the current mask-protection setting and current document symmetry. Tap selection remains selection. After tunnel/fusion snapping, the remembered distance is the snapped distance. Topology buttons such as Delete and Weld are direct commands and are not recorded as measured repeats.

**Smooth preview** shows one Catmull–Clark level over the editable cage, respecting creases. It changes neither geometry, history nor saved subdivision levels. Picking and the faint cage overlay refer to the original mesh. During a drag the operation preview takes priority; the smooth result returns on release. Show all faces first; this interactive preview supports cages up to 20,000 triangles. Turn it off to inspect the unsmoothed geometry.

![A symmetric inset with the shared modeling controls](../images/modeling-milestone-inset.png)

![The smooth result displayed over the unchanged editable cage](../images/modeling-milestone-smooth.png)

![A named tool preset restored without changing the mesh](../images/modeling-milestone-preset.png)

### Save a component selection as a vertex group

Choose **Select**, build a face, edge, or vertex selection, then expand **Selection to vertex group**. Enter a name and choose **Create group from selection**, or choose an existing group and use **Add to group** / **Remove from group**. Face selections assign group colors and face-group targets to exactly the selected faces, including their symmetry copies. Unselected gaps and neighboring faces keep their existing labels, even when they share every corner with selected faces. This applies to creating, adding and removing. Vertex weights include all selected face corners; edges include both endpoints. Sculpt masks stay unchanged. Use a single subdivision level for component-to-group assignment.

![Two selected face strips assigned to a group while the unselected middle strip stays ungrouped](../images/modeling-face-group-gap.png)

![The selected quad outlined in cyan beside controls for creating and editing vertex groups](../images/modeling-selection.png)

Right-click a modeling button and choose **Add to custom menu / shelf…** or **Assign shortcut…**. Component modes, Select/Extrude/Loop cut, selection modes and patterns, join toggle, loop-count buttons, selection clearing/loop expansion, dissolve, and group actions are available in the action catalog. Extrude and Loop cut on a custom shelf automatically enter the appropriate modeling mode. Group actions use the name and group currently chosen in the Mesh panel. Arbitrary intersecting-face bridging and general Boolean fusion remain outside the supported topology operations.

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

Available shapes: Plane, Cube, Circle, UV sphere, Icosphere, Cylinder, Cone, Torus, and Grid.

### Adjust dimensions and resolution

Use the **Primitive** menu or **Submodels → Primitive**. Pin/dock the menu to keep its controls visible.

![Cube with width 1.4, height 0.8, depth 1.0, two spans per axis, and colored viewport handles](../images/primitive-cube.png)

*Dimensions change the shape; spans change its resolution. This cube has two spans on each axis.*

| Handle | Controls |
| --- | --- |
| Axis-colored square | Size along X (red), Y (green), or Z (blue) |
| Teal circle | Radius |
| Diamond | Resolution; drag left/right |
| White hover ring | The highlighted handle |

Hover for a label, or type precise values in the panel. Wireframe shows the axis-colored span lines. Plane/Grid and Circle lie in XY; cylinders, cones, and tori use Y as their main axis.

The recipe saves with the model and survives duplication, transforms, and reopening. Converting removes it but keeps the surface; Undo restores it. An unfilled Circle must be filled before conversion. Open surfaces need closing before adaptive sculpting or voxel remeshing.

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

Convert parametric primitives first, show all faces of the active model, and apply or remove thickness modifier. A source half with no surface produces an error and leaves the model unchanged. Large operations can be cancelled from the status controls.

## Mesh management

Choose the method that fits your work:

| Method | Best for | Changes topology |
| --- | --- | --- |
| [Smooth subdivision](edges.md#control-subdivision-smoothness) | Rounding shapes while preserving creased edges | Bakes the current level into refined quads |
| Subdivision | Keeping several editable detail levels | Adds resolution to the whole surface |
| [Adaptive sculpt](sculpting.md#snake-brush-and-adaptive-sculpting) | Detail near the brush | Refines/simplifies locally |
| Dynamic mesh / voxel remesh | Redistributing geometry or fusing overlapping closed parts | Rebuilds the whole surface |

### Add subdivision levels

Choose **Mesh → Subdivide** from the highest level. Use **Lower / Higher** or the level slider to switch levels.

Lower-level edits carry upward while retaining finer detail. Subdivision is linear: it adds resolution without automatically smoothing the shape. Masks interpolate between levels. For controlled rounding, use the separate [Smoothness slider and Smooth subdivide](edges.md#control-subdivision-smoothness) controls.

**Delete lower** removes levels below the current one; **Delete higher** removes those above it. Both are undoable. Projects store all levels; OBJ exports only the active one.

### Remesh a whole surface

1. Switch to **Dynamic mesh**. This keeps the active surface and discards stored levels.
2. Choose a resolution, **16–96 voxels** along the longest bounding-box axis.
3. Select **Remesh now**, or enable **Remesh after each stroke**.

**Remeshing clears masks and can lose details thinner than a voxel.** Use moderate resolution first. It requires closed, consistently oriented surfaces and can fuse overlapping outward-facing shells; separate volumes remain separate.

Mesh operations run in the background. Cancel keeps the original surface; Undo restores prior geometry and settings. A failed automatic remesh disables the automatic option so you can adjust it.

## Thickness modifier

### Preview a shell

In **Submodels → Thickness · open meshes**, enable **Thickness modifier** and set its total thickness in model units. Half is added to each side; walls close boundary loops.

Use **Rim loops** to add wall segments and **Crease rim edges** to preserve the joins during later smooth subdivision. See the [illustrated thickness example](edges.md#add-loops-and-creases-to-thickness).

Orbit to inspect tight folds for intersections. The original surface, masks, levels, and picking remain unchanged. OBJ export uses the original surface while the shell is temporary.

### Apply the shell

Choose **Apply Thickness modifier** to make the preview real geometry. This converts primitives, preserves masks/quads, and resets levels and topology modes. Undo restores the original surface and settings.

Thickness requires consistently oriented open surfaces with nonbranching boundaries. Closed meshes and outlines without faces are not eligible. Front/back/rim groups and masks can be generated in the same controls.

## Slice

### Trim away one side

Before slicing, convert primitives, apply/remove thickness modifier, and show all faces of the active model.

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

Merge bakes primitives and thickness modifier, uses current mesh levels, and resets the result's level/topology settings. Fill unfilled Circles first. It combines surfaces **without welding or boolean union**; use voxel remeshing for fusion. Undo restores both models.

## Related guides

[Brushes and adaptive detail](sculpting.md) · [Masks and selection](selection.md) · [Import/export](files.md)

# Reusable models and object brushes

[Documentation home](../../README.md#documentation) · [Editable paths](paths.md) · [Models](modeling.md)

Open **Submodels** for arrays, copies and path assemblies. To stamp a captured part onto a surface, open the **Brush library**. Copies share their source geometry, with separate placements. Sculpting, painting or changing the source updates its linked copies.

## Arrays and individual copies

Select a submodel and choose **New array…**. Its array belongs to that submodel and replaces its displayed surface. Count includes the source once: Count 1 shows one object. Duplicate the submodel to duplicate its array; hiding or deleting the owner hides or removes its array. Existing standalone arrays keep their original behavior.

The inline viewport bar and **Submodels → Array** edit the same recipe. Choose Linear, Radial or Grid. Drag labelled spacing handles, the origin diamond, or radial radius/arc handles. **Relative** spacing multiplies the source's size; **Distance** uses model units; **Fit span** keeps the distance between the first and last placement fixed while Count changes. Grid uses full rows and columns. Radial offers radius, arc angle, axis and rotation of the copies. A full circle has no duplicate endpoint. **Frame** fits the whole result in view. Add **Active array controls** to a custom menu for the same controls, following the selected submodel.

**Edit array** changes the pattern; the regular Transform controls move, rotate or scale the whole result without changing its source. **Edit source** highlights the original surface in gold; sculpt or transform it while grey linked copies update. **Edit copy…** selects one numbered placement with gold handles and a gold frame; Move/Rotate/Scale affects only that placement, while its shape stays linked. Drag away from handles to orbit, scroll to zoom, and use the usual pan/zoom modifiers.

Each completed drag or numeric edit is one Undo step. Escape cancels a handle drag. **Done** keeps the live array and hides inline editing; **Edit array** opens it again. **Enable array** reveals or replaces the original source. **Remove array** restores the source. **Make editable…** offers replacing the owner's surface with exactly the evaluated result, one new submodel, or separate submodels per copy. The new-submodel options retain the source. Fusion is a separate option. Replacing the owner resets subdivision levels; Undo restores the source and array.

**Advanced instances → Repeat this array…** reuses the complete pattern in another live set. **Bake all…** remains available there. The advanced standalone workflow below retains explicit Apply/Cancel. **Linked copy…** makes a single standalone instance; **Advanced linking → Standalone array…** makes a standalone pattern.

**Add layer** repeats everything in the preceding layers. For example, set Layer 1 to a linear row of six vents along X, then add Layer 2 with four copies along Y: the result is 24 shared vents. Change Layer 1 to five copies and all four rows update. Each layer can be Linear, Grid, or Radial. Click its numbered tab to edit it; **Remove layer** removes that repeat operation. The first layer stays in place.

Each layer has two kinds of transform:

| Control | What it changes |
| --- | --- |
| **Origin / radial centre** or **Layer origin offset** | Places the first layer in the scene, or moves a later layer relative to the preceding layer's origin. |
| **Whole layer transform** | Sets X/Y/Z rotation in degrees and X/Y/Z size for the entire pattern around its origin, including the distance between copies. Works on a single linked copy too. |
| **Step between copies** | Adds the given X/Y/Z distance for each linear repeat. Grid uses X/Y column and row spacing; Radial uses its axis, radius and arc. Both offer an **Additional position step** under **Per-repeat transforms**, for example to raise a ring into a helix. |
| **Per-repeat transforms** | Adds X/Y/Z rotation and multiplies X/Y/Z size for each successive repeat. A Z rotation of 10° and an X size factor of 0.9 give angles of 0°, 10°, 20° and widths of 1, 0.9, 0.81 times the original. The first repeat keeps the input's orientation and size. |

**Repeat this array…** uses a saved array as the live source for a separate array. Leave **Copies** at one to place one linked rack; increase it to repeat the whole rack. You can use the same source array in several patterns. Choose **Edit source array…** beside a linked array to change the pattern they share. Editing its layers, source mesh, or individual copy adjustments updates each use. Each linked array keeps its own placement relative to the source pattern’s origin. Turning off **Show instances** on the source hides that occurrence in the scene; arrays using it still appear. Hiding an individual source copy, in contrast, hides that part in every use. Bake or remove dependent arrays before removing or baking their source.

For standalone sets, inspect the preview and choose **Apply instances**, or **Cancel preview**. Each applied edit is one undo step. Previews never enter project saves or recovery copies. Choose **Edit array…**, **Edit path assembly…** or **Edit stroke…** beside a set to change it later. Blue brackets mark that set's visible copies in the viewport. The viewport and the editor both show the set name and number, so you can distinguish two sets named “Array”.

To move one copy, click it in the viewport or select its numbered row under **Copies**. Gold brackets and a label identify the copy, for example **Array #2 · Copy 3**. Choose **Edit selected copy…** and drag a **Move**, **Rotate** or **Scale** value. Drag a coloured axis handle to move along X, Y or Z, or drag the centre diamond to move in the view plane. The frame follows the copy as you preview its placement. Drag elsewhere to orbit the view. Scroll to zoom and use the usual navigation modifiers to pan or zoom. **Show this copy** controls visibility. Choose **Apply placement** to keep the change, or **Cancel preview** to restore the previous placement. The other slots in this set keep their placements. If another array repeats this set, its matching copies inherit the adjustment. The copy’s shape stays linked to its source. These adjustments belong to numbered slots. Reducing and restoring an array’s count restores the adjustments for those slots. To sculpt the copy itself, choose **Make independent** in its row. Undo restores the link. To hide an entire set, turn off **Show instances** in the whole-set editor.

Source visibility is separate from copy visibility. Bake or remove linked sets before deleting or merging their source. Instances use the source's current mesh, masks, face visibility, attributes, creases and valid UVs/textures. Apply a source's Thickness modifier first if required; inactive subdivision levels are not instanced.

## Build a path object from one model

Keep the source as one welded mesh. Use face selections and **Modeling → Selection to vertex group** to mark three adjoining regions: the beginning, the repeating section, and the end. They can have different lengths and arbitrary group names. Shared boundary vertices stay welded in the source; no separate submodels or internal caps are required. Use edge loops perpendicular to the intended lengthwise axis at the two region boundaries.

1. Create and save an [editable path](paths.md), then leave its editor.
2. Select the source model and choose **Submodels → Instances → Create path object…**. The same action is available in custom menus.
3. Assign its **Start**, **Middle** and **End** groups. Groups named Start/Middle/End are recognised automatically. All source faces must belong to exactly one region; vertices on a shared edge can belong to both neighbouring groups.
4. Check **Forward** and **Up** beside the source picker. Assigning the three groups selects their lengthwise axis, with Start pointing towards End even if the model faces a negative axis. For a model, the thinner cross-section dimension defaults to Up. Change Up to roll the model onto another side, or expand **Contact point** to adjust its offset. Choose the saved route.
5. Set **Middle repeats** and **Piece scale**. Start and End each appear once at their original scaled lengths; the repeated middle sections stretch lengthwise to fill the remaining route. Closed paths use only the middle sections and join the final seam.
6. Use **Thickness along path** and its inline controls for tapers or local width changes, then **Apply instances**. The preview is private until Apply; Cancel restores the prior result. Apply and baking each undo as one operation.

The result is one connected, bent path object. The original welded loops connect Start and End to their neighbouring Middle, preserving shoulders where their widths differ. Between Middle repeats, the outer end loops must match in shape and topology. During recipe previews, invalid assignments or incompatible seams show an explanation and retain the last valid result. Incompatible source-mesh edits show a warning and omit generated copies until the source or assignments are corrected. The path must be longer than the scaled Start and End regions together. Show all source faces and groups before evaluation. Source geometry stays intact; editing it or the saved path updates the generated result. Group roles bind by name, so after renaming a group choose its new name in **Edit path assembly…**. Generated joined geometry has no UVs; the source keeps its UVs.

For a separately sculptable result, use **Bake set…**. Baking removes the live recipe and retains the source model. Keep a saved project for further procedural edits. Native format v6 stores the region assignments; versions 1–5 remain readable, while older applications reject v6 saves.

## Assemble separate pieces along a path (advanced)

Open **Submodels → Instances → Advanced linking → Separate pieces along path…**, or choose **Separate pieces (advanced)** inside the path editor. Create and save an [editable path](paths.md), leave its editor, then open this editor. Select a saved path and Middle piece, with optional Start and End pieces. Sources can be submodels or captured object brushes. Closed paths omit caps.

**Fit count to path length** distributes a chosen number of pieces; fixed spacing leaves leftover length at the end. Piece scale and Twist control size and orientation. Expand **Contact point and axes** to align each source's pivot, forward and up axes. Forward and up must differ.

Choose **Rigid** to position and rotate whole pieces, or **Bend along path** to deform their vertices along the curve. Bending uses existing source topology: subdivide along the Forward axis for smooth results. **Fit section lengths to spacing** stretches sections lengthwise so their adjoining ends meet; the outer ends land on an open path's endpoints. Turn it off to retain source lengths, allowing gaps or overlaps. Very tight bends or large thickness changes can self-intersect.

With Bend enabled, **Thickness along path** changes the overall width/depth around each piece's contact point without changing its length, the path or repeat spacing. Values multiply Piece scale: 1× leaves the cross-section unchanged. **Uniform** sets one size for the whole curve; **End-to-end taper** sets start and end sizes; **Along-curve profile** provides draggable points for local bulges and narrow regions. Select a point to enter an exact position percentage and size, or add/remove intermediate points. **Smooth transitions** softens the transitions; disable it for linear interpolation. Thickness stays positive, between 0.01× and 10×.

**Inline thickness handles** puts the same controls directly on the saved curve during the assembly preview. Gold squares control Start/End; teal diamonds sit at path control points; purple circles sit at repeated-section centres. Drag away from/towards the curve to change thickness. A start/end drag creates a taper from Uniform; an intermediate drag adds a local profile point. The panel and viewport edit the same settings. **Path points** and **Section centres** let you show either group or both. Coincident endpoints share the Start/End control; crowded or obscured handles are hidden until the view provides room. Camera navigation remains available. Apply saves the entire preview in one undo step; Cancel discards it.

Thickness profiles are stored by percentage distance along the path, including values placed with inline handles. Editing the saved route preserves these percentages; repeat counts can change without changing the profile. On closed loops, a taper runs from the seam to halfway round and back; a profile's final endpoint shares the first value to keep the seam continuous. Uniform 1× is the default for older projects. Switching to Rigid preserves thickness settings for the next time Bend is enabled.

Choose **Separate** to keep individual sections, or **Join neighbours** to weld their matching ends into one continuous preview. **Join distance** is the maximum distance between corresponding end vertices; accepted pairs meet at their average position. Ends are detected at the minimum/maximum coordinates along each source's Forward axis. They need compatible planar loops with matching vertex counts and winding. Internal end caps are removed, including the final seam on closed paths. Open-ended tube sections are supported; exposed outer ends retain their original geometry. Gaps beyond the distance limit or incompatible loops produce an error and keep the last valid preview. This does not weld unrelated nearby surfaces.

Joining preserves masks, vertex materials and compatible groups. It removes UVs from the joined result. All sections must be visible and linked before joining. Choose **Separate** to edit or detach individual copies. A joined assembly counts as one piece when you use **Bake set…** or **Bake all…**. Older recipes default to Rigid/Separate.

Applied assemblies follow edits to their saved path and sources. Path assemblies use saved world-space paths; surface changes require the usual path reprojection. Bake or remove an assembly before deleting its path.

Use **Editable paths → Transform whole path** to move, rotate or uniformly scale the complete route. Save path updates linked assemblies; **Scale linked sections** also scales their cross-section size and fixed spacing. Their thickness profiles remain at the same percentages along the curve. Return to **Edit points** to continue ordinary point editing. See [whole-path transforms](paths.md#move-rotate-and-scale-the-whole-path).

## Capture and brush objects

**Ship greebles** is available in the **Brush library** in every project. Select its tile to use the built-in hull vent, access hatch and raised service panel. Each part has its own thumbnail and a flat contact base for placing instances or stamping relief.

A named object brush holds several captured parts. For example, **Ship fittings** can contain a vent, hatch and sensor dome. Each part keeps its own mesh, contact point, orientation and placement settings.

1. Choose **Brush library → + Add brush → Object brush**. The new brush is selected and **Brush settings** opens. Type a name such as **MyOB** and press Enter.
2. In **Brush settings**, use **Include existing parts** for models you have already captured. To add a new one, expand **Capture a model into this brush**, choose the source submodel, set its contact point and axes, and click **Capture active model**. The first model supplies the brush icon.
3. Select the target submodel. The model thumbnails above the library’s brush tiles belong to the selected object brush. Click a thumbnail, then click the target to place one copy or drag to repeat. Drag from empty space to orbit. **Brush settings** controls the selected model’s size, spacing, rotation and offset.
4. While placing, press **B** or click **Choose model…**. Click a model in the popup and continue your stroke on the same target. Each completed stroke is one undo step. Escape cancels an unfinished stroke. Select a sculpt brush to resume sculpting.

To put the picker on another menu, right-click **Object brush models** in the library, or choose that control in the custom menu editor. Every copy of the picker follows the active brush. **B** is configurable in keyboard settings.

**Capture orientation.** The gold **Contact** diamond is the source point placed on the target. **Use base centre** sets it to the centre of the object's bounds on its lowest plane along the chosen **Up** axis; use the coordinates for a custom mount or joint. The labelled arrows show the source axes as you orbit. **Up** aligns with the target surface normal; **Forward** follows the stroke projected onto that surface. The first placement uses the view's rightward direction along the surface. Choose different axes for Forward and Up. **Rotation** turns the object about the surface normal, and **Surface offset** lifts it above or sinks it into the target. For a hull vent made in XY, use Up Z and contact on its back. For a floor prop modelled upright along Y, use Up Y and contact at its foot.

**Save and reuse kits.** Project files embed the captured objects and their brush memberships. Under **Brush settings → Reuse in other projects**, **Save brush to my library** keeps a separate personal copy available across projects and app restarts. In another project, choose it under **+ Add brush → Saved object brushes**. Use **Update saved brush** to publish edits to your local personal copy. Each project keeps its own snapshot, so updating a library brush leaves existing scenes intact. Export/import a whole brush as a portable `.objects.json` file to move it between machines. Use **Transfer a single part (.object.json)** to share one captured model and its placement settings. Standalone captures from older projects are available under **Include existing parts**.

To remove a model from a brush, select its thumbnail and click **Remove model from brush** in **Brush settings**. The captured model stays available under **Include existing parts**. Other brushes and previously placed copies keep using it. Undo restores its membership. If you want the same change in your personal library, choose **Update saved brush**. Removing a whole brush also leaves its captured models and placed copies intact.

Choose **Create instance** or **Stamp mesh** in the selected model’s **Brush settings**. **Create instance** places linked copies. **Stamp mesh** raises the existing target surface to the captured model’s shape above its contact point. Hold **Alt** to press that shape inward. A cone creates a pointed bump or indentation. The outline preview follows the selected model. Holding Alt turns it orange and flips the shape inward.

Click once for one impression, or drag to repeat at **Repeat spacing**. The surface previews the stroke until you release. Escape cancels it and one Undo restores the surface after applying. Masks and hidden faces protect the target. Stamp detail comes from the target’s existing vertices, so subdivide the target or enlarge the stamp for small shapes. Convert a parametric primitive to a sculpting mesh before stamping it.

Placements use model-space spacing on the active visible surface and respect occlusion, masks and hidden faces. Overlap is allowed. By default, copies share the captured mesh. **Create independent geometry** produces editable submodels; you can also bake linked copies later. Object placement does not automatically fuse copies or deform the underlying surface.

## Bake, fuse, export and limits

Choose **Bake set…** beside an array, path assembly or brush stroke, or **Bake all…** to bake every visible instance set together. Choose where the visible copies should go, then click **Bake now**.

| Destination | Result |
| --- | --- |
| **One new submodel per copy** | Each copy becomes a separately selectable, sculptable submodel. A joined path assembly produces one submodel. |
| **One new submodel** | All the baked geometry goes into a single new submodel. |
| **Merge into selected submodel** | The named submodel keeps its existing surface and gains the baked geometry. Select a visible target first. This applies its thickness and clears its primitive and subdivision levels. Live copies that still use this target as their source update too. |

The two single-submodel destinations offer **Fuse overlapping surfaces**. Leave it off to keep the original faces and compatible UVs, including faces inside overlaps. Turn it on to voxel-remesh intersecting, closed surfaces into a continuous surface at the current mesh resolution. For example, to fuse an array of rivets to a hull, select the hull, choose **Merge into selected submodel**, then enable fusion. Gaps stay disconnected. Fusion requires closed, fully visible meshes, rebuilds topology and removes UVs. Thin details may need a higher resolution.

Baking removes each baked set and its link to the source. Hidden or detached copies in that set are skipped. **Bake all…** leaves entirely hidden sets in the project. If another array still uses a source array you want to bake, include the dependent array in **Bake all…** or bake it first. One **Undo** restores the geometry and instance sets for the entire operation.

Mesh exporters export the active submodel: make the array editable first, then select the result to export. Native `.clay` version 6 preserves attached arrays, recipes, links and captured assets. Versions 1–5 remain readable; earlier app versions cannot read version 6 saves.

Limits: 256 copies per project, with at most eight layers per array and 16 levels of linked arrays. Layer counts multiply: a 16 × 16 pattern produces 256 copies. Scale factors must be positive; overly large, small or flat combinations are rejected before Apply. Other limits: 64 captured assets and 64 named object brushes per project; 100,000 vertices / 200,000 triangles per captured asset; one million captured source vertices in total. Instance display/baking is bounded to one million expanded vertices / two million triangles. A bent or joined assembly is limited to 250,000 expanded vertices / 500,000 triangles, with up to 4,096 boundary vertices per joining end. Generated geometry is cached across unchanged frames; projects store source geometry and recipes. A later source edit exceeding a budget or making joins incompatible produces an Instances-panel warning and omits copies until settings or source detail are corrected. Bake also respects the 64-submodel limit and configured mesh capacity.

Create linked array, Create path object and Capture object brush are available through custom menus and configurable shortcuts.

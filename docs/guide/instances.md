# Reusable models and object brushes

[Documentation home](../../README.md#documentation) · [Editable paths](paths.md) · [Models](modeling.md)

Open **Submodels → Instances and object brushes**. Copies share their source geometry, with separate placements. Sculpting, painting or changing the source updates its linked copies.

## Arrays and individual copies

Select a submodel and choose **New array…**. Choose Linear, Radial or Grid, then adjust count, origin and spacing. Radial arrays also offer radius, arc angle, axis and rotation of the copies. A full circle has no duplicate endpoint. Grids use X/Y spacing and a column count.

Inspect the preview and choose **Apply instances**, or **Cancel preview**. Each applied edit is one undo step. Previews never enter project saves or recovery copies. Choose **Edit** beside a set to change its recipe later.

Click a copy in the viewport or expand **Copies**. **Edit selected copy…** exposes Move, Rotate, Scale and Show this copy. Adjustments belong to numbered slots and survive recipe changes; reducing and restoring the count restores those slots' adjustments. **Independent** makes a copy an editable submodel unaffected by subsequent source edits. Undo restores its link. Hide a complete set with **Show instances** in its editor.

Source visibility is separate from copy visibility. Bake or remove linked sets before deleting or merging their source. Instances use the source's current mesh, masks, face visibility, attributes, creases and valid UVs/textures. Apply thickness first if required; inactive subdivision levels are not instanced.

## Assemble pieces along a path

Create and save an [editable path](paths.md), leave its editor, then choose **Along path…**. Select a saved path and Middle piece, with optional Start and End pieces. Sources can be submodels or captured object brushes. Closed paths omit caps.

**Fit count to path length** distributes a chosen number of pieces; fixed spacing leaves leftover length at the end. Piece scale and Twist control size and orientation. Expand **Contact point and axes** to align each source's pivot, forward and up axes. Forward and up must differ.

Choose **Rigid** to position and rotate whole pieces, or **Bend along path** to deform their vertices along the curve. Bending uses existing source topology: subdivide along the Forward axis for smooth results. **Fit section lengths to spacing** stretches sections lengthwise so their adjoining ends meet; the outer ends land on an open path's endpoints. Turn it off to retain source lengths, allowing gaps or overlaps. Cross-section size still follows Piece scale. Very tight bends can self-intersect.

Choose **Separate** to keep individual sections, or **Join neighbours** to weld their matching ends into one continuous preview. **Join distance** is the maximum distance between corresponding end vertices; accepted pairs meet at their average position. Ends are detected at the minimum/maximum coordinates along each source's Forward axis. They need compatible planar loops with matching vertex counts and winding. Internal end caps are removed, including the final seam on closed paths. Open-ended tube sections are supported; exposed outer ends retain their original geometry. Gaps beyond the distance limit or incompatible loops produce an error and keep the last valid preview. This does not weld unrelated nearby surfaces.

Joining preserves masks, vertex materials and compatible groups, but removes UVs; the reusable sources keep their original data. All sections must be visible and linked before joining. Choose Separate to edit or detach individual copies. **Bake joined model** creates one editable submodel from the joined preview; **Bake + fuse** remains available for volumetric remeshing of closed pieces. Apply, Cancel and Undo work with either option, and projects retain both choices and their settings. Older recipes default to Rigid/Separate.

Applied assemblies follow edits to their saved path and sources. Path assemblies use saved world-space paths; surface changes require the usual path reprojection. Bake or remove an assembly before deleting its path.

## Capture and brush objects

Expand **Capture object brush**, enter a name, contact point and axes, then choose **Capture active model**. The embedded geometry snapshot and thumbnail survive editing or deleting the original.

Select the captured brush, set Size, Repeat spacing, Surface offset and Rotation, then choose **Place / brush objects**. Select the target submodel, hover for a wire preview, click to place one object or drag to repeat. **Stop placing** returns to normal tools. A completed gesture is one undo step; Escape or loss of focus cancels it.

Placement uses model-space spacing on the active visible surface. Masked target triangles and hidden faces are protected. Navigation remains available through the navigation frame, right/middle mouse buttons and configured navigation holds. Object placement uses its own size/spacing independently of sculpt pressure, symmetry and alphas. It permits overlaps and does not automatically fuse or detect collisions.

Each stroke normally creates instances linked to its captured asset. **Create independent geometry** instead creates ordinary submodels, within the 64-submodel scene limit. **Object brush files** imports/exports `.object.json` assets with geometry, contact point, axes and placement settings last used with **Place / brush objects**. Relief stamps and seamless rollers are a separate workflow.

## Bake, fuse, export and limits

**Bake** creates independent submodels from visible copies, retaining attributes and UVs. **Bake + fuse** unions overlapping closed, fully visible pieces through voxel remeshing at the current mesh resolution. It rebuilds topology and removes UVs; separated pieces stay disconnected. Thin details may need higher resolution. Open surfaces are rejected; use ordinary Bake for those. Both operations preserve reusable sources, are undoable and run through the cancellable mesh worker.

Mesh exporters export the active submodel: bake first, then select the result to export. Native `.clay` version 4 preserves recipes, links and captured assets. Versions 1–3 remain readable; earlier app versions cannot read version 4 saves.

Limits: 256 copies per project; 64 captured assets; 100,000 vertices / 200,000 triangles per captured asset; one million captured source vertices in total. Instance display/baking is bounded to one million expanded vertices / two million triangles. A bent or joined assembly is limited to 250,000 expanded vertices / 500,000 triangles, with up to 4,096 boundary vertices per joining end. Generated geometry is cached across unchanged frames; projects store source geometry and recipes. A later source edit exceeding a budget or making joins incompatible produces an Instances-panel warning and omits copies until settings or source detail are corrected. Bake also respects the 64-submodel limit and configured mesh capacity.

Create linked array, Create path assembly and Capture object brush are available through custom menus and configurable shortcuts.

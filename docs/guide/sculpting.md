# Sculpting and brushes

[Documentation home](../../README.md#documentation) · New here? [Make your first sculpt](getting-started.md).

Build a rounded form, cut a fold, or pull out a horn: choose the brush for the shape you want, then tune it with a short trial stroke. **Ctrl+Z** lets you try again. Start here:

- [Choose and adjust a brush](#brushes-alphas-and-symmetry)
- [Reset a brush](#example-reset-an-experimental-brush)
- [Cut a fine fold](#detail-brush)
- [Choose a stroke mode](#stroke-modes)
- [Guide a live brush stroke with an editable 3D path](paths.md#sculpt-along-a-saved-guide)
- [Paint colours and materials](materials.md)
- [Mirror a stroke](#symmetry)
- [Add local detail](#snake-brush-and-adaptive-sculpting)
- [Undo an edit](#undo-and-redo)

## Brushes, alphas, and symmetry

### Choose a brush

| Goal | Brushes to try |
| --- | --- |
| Build up or remove clay | Standard, Clay, Inflate |
| Even out a surface | Smooth, Flatten |
| Redistribute crowded or stretched polygons without smoothing the shape | Relax |
| Reduce surplus triangles | Decimate (Adaptive sculpt) |
| Polish bumps into cleaner planes while holding sharp corners | Polish |
| Tighten a fold | Pinch, Crease, Detail |
| Move or pull a form | Grab, Snake |
| Protect part of a surface | Mask |

For broad, clean planes, try **Polish** with a large radius and light pressure. It flattens local bumps while holding sharp corners and marked creases. Masks, hidden surfaces, pressure, alphas, and symmetry still apply. Reversing Polish exaggerates relief, as with Flatten.

To flatten a patch:

1. Select **Flatten** and start a stroke on the surface you want to use as a reference plane.
2. Brush back and forth across bumps. The brush brings the surface towards the average angle and height sampled at the start, even when you reverse direction.
3. Release, then start a new stroke to choose a different plane. Lower Strength or pressure for a gentler result.

The brush footprint follows your cursor while its target plane stays fixed. Each symmetry copy samples its own starting patch. Subtract exaggerates relief away from that plane; use **Polish** to preserve sharp corners and marked creases.

Select a tile in **Brush library**. Use **Add brush** if a brush is missing from an older project.

**Clay** builds a layer against the average surface beneath the brush. It fills hollows faster than raised spots, helping overlapping dabs form a cohesive mass. **Standard** adds depth along the picked normal and retains more of the existing small surface variations. Clay samples the surrounding patch to keep small bumps from steering each dab; repeated passes still add volume. Subtract removes a similarly controlled layer.

Try Clay on a sphere with the default Soft alpha: draw one broad stroke, then pass over it again to build up the form. Increase **Inner radius** for a wider full-strength core, or lower **Spacing** if you can see individual dabs. Subdivide or enable adaptive sculpting when the mesh is too coarse to carry the shape. Clay's drag stamps stop at their target layer even at full strength.

**Shift** temporarily uses your most recently selected Smooth preset, including its size and stroke settings. Release Shift to return to your previous brush. **Alt** reverses the operation by default; you can also choose **Add / Subtract** in Brush settings.

### Adjust the feel

Make one short stroke before changing several settings at once:

- Too strong? Lower **Strength** and build up with several passes.
- Too broad? Reduce **Radius**. Radius is measured on screen, so zoom changes how much of the model it covers.
- A dotted trail? Reduce **Spacing** for closer dabs.
- A shaky line? Try [Stroke stabilisation](#steady-a-freehand-stroke).

| Brush setting | Effect |
| --- | --- |
| Radius | Size of the affected area, in screen pixels |
| Inner radius | Full-strength core, as a fraction of the radius |
| Strength | Amount applied by each dab |
| Spacing | Distance between dabs in a continuous stroke |
| Falloff | How quickly influence fades toward the edge |
| Pressure strength / radius | Whether pen pressure controls strength or size |
| Pressure curve | Response to lighter and heavier pressure |
| Surface footprint and normal | Preview the alpha on the clay, with a tangent ring and normal indicator |

Grab follows pointer movement; spacing does not affect it. Subtracting Smooth sharpens, subtracting Flatten exaggerates relief, and subtracting Mask erases protection.

### Alpha Drag preset

Use **Alpha Drag** to place a single relief stamp:

1. Select **Alpha Drag** in a new document. In an existing project, choose **Brushes → Alpha Drag** to add it.
2. Drag to size and rotate the stamp.
3. Release to commit it.

The preset uses Standard behaviour with **Drag Rect** and **Wood grain** selected. The regular Standard preset remains available for freehand work.

### Choose an alpha

An alpha is the pattern that shapes each brush dab. To try one:

1. Open **Alpha** and select **Soft**, **Hard** or **Square** in the **Alpha library**.
2. Choose **Use on brush** and make a short test stroke.
3. Search by name or category to try a different pattern. Use **Duplicate alpha** to edit a variation, or **Place stencil** to brush through a fixed pattern.

The library includes **39 built-ins**, including **Nature’s curve** (a flowing spiral), **Leaf veins**, **Wood grain** and **Pebbles**. They remain available in older projects. Built-ins cannot be removed; saved copies can.

The library starts four rows tall. Hold and drag **Drag to resize library** up or down to change its height; clicking alone does not resize it. The centred handle stays clear of the sidebar scrollbar.

Your chosen height is retained even below other open panels. Use the outer sidebar scrollbar to bring the library into view.

| Category | Examples |
| --- | --- |
| Masonry | Weathered and broken bricks, cobbles, stone blocks, flagstones, cracked plaster |
| Circuits | Traces, circuit hub, microchip, data grid, cables, riveted plate |
| Sci-fi | Hull panels, hex armor, vent grille, reactor rings, alien glyphs, mech seams |
| Skin | Fine wrinkles, deep folds, crow’s feet, pores, fingerprint, scar tissue |
| Organic | Tree bark, rock strata, cracked earth, scales, fur |
| Materials | Woven cloth, leather grain, hammered metal |

Use **Subtract** for recessed wrinkles, pores, branching plaster cracks, and engraved details. Crow’s feet and other creases taper in width and depth at their ends. Cobbles use rounded stones, flagstones use broad slabs, and cracked earth has curled fracture edges. **Duplicate alpha** lets you invert a copy or adjust its contrast.

![Built-in patterns in the scrollable alpha library](../images/alpha-library.png)

Saved brushes and project alphas embed their height images. To use a revised built-in in an existing brush, select it in the Alpha library and choose **Use on brush** again. See the [alpha library audit](../validation/alpha-library-audit/README.md) for before/after comparisons.

### Create and reuse an image alpha

1. Open **Alpha → Import image → Choose image…**, browse to a PNG/JPEG, and choose **Preview alpha image** (or double-click the file). Images can be up to **2048 × 2048** and **32 MB**. You can also enter a path before opening the picker. Cancel leaves the brush and library unchanged.
2. Name the alpha and give it a category such as *Stone*. Compare **Source** with **Result** while adjusting the crop, black/white levels, gamma, inversion, and **Edge fade**.
3. Choose **Save alpha**. Select its thumbnail in the project library, then choose **Use on brush**. Switch brushes and choose **Use on brush** again to reuse the same image.
4. Search by name or category. Use **Edit alpha** to adjust an asset, **Duplicate alpha** for a variation, or **Export selected alpha → Export PNG** to share a 16-bit height image. Import that PNG to use it in another project.

![Source and result previews beside crop and tonal controls](../images/alpha-editor.png)

*Adjust the image privately, then save it to the project alpha library.*

White affects the surface; black protects it. Imported transparency reduces influence. Image inversion happens after transparency is combined with brightness. **Alpha rotation** turns each dab; brush falloff and inner radius soften its circular footprint. **Spacing** controls freehand and Line dab spacing, and strength and pressure control depth. Drag Rect adds its drag angle to the brush rotation; Drag Dot keeps the preset's angle.

**Copy brush alpha to library** brings an existing embedded alpha into the editor. Cancel leaves the library and brush unchanged. Brushes and stencils keep independent copies: editing or removing a library asset does not change existing strokes, brush presets, or a placed stencil. Use **Use on brush** again to adopt an edited image.

The project saves the library, categories, brush images, and stencil settings. Library edits are outside sculpt Undo/Redo, like brush settings. Exported PNGs contain the adjusted heights; names and categories stay in the project. The project library holds up to 64 saved assets and 16 million pixels in total, in addition to the built-ins.

### Brush through a placed stencil

1. Select an alpha thumbnail and choose **Place stencil**. A translucent guide appears over the model. Start with **Standard**, a **Soft** brush alpha, and either adaptive sculpting or a finely subdivided mesh.
2. Drag the amber square above the stencil to **move** it, a corner to **resize** proportionally, or the round handle above it to **rotate**. Hold **Shift** while rotating to snap to 15°; hold Shift on a corner to stretch freely. The right and bottom edge handles resize width and height separately. **Escape** cancels the current drag. For precise values, use **Horizontal**, **Vertical**, **Width**, **Height**, and **Rotation** under **Alpha → Stencil placement**. View-attached positions and sizes use fractions of the viewport's shorter side. Positive vertical moves up; positive rotation turns counterclockwise.
3. Brush back and forth through the guide. White areas permit the brush, black areas and everything outside the rectangle protect the surface. Brush alpha, falloff, pressure, strength, existing masks, and hidden geometry still apply.
4. Choose **Attach to surface…**, then click the model. Orbit to inspect it: the stencil now stays on that surface plane. Its position and size controls use model units. **Follow view** returns to a centered view stencil. Escape cancels attachment without painting.
5. Use the floating viewport controls to adjust **Guide opacity**, hide **Handles**, or **Reset placement**. Set opacity to zero to inspect the sculpt while continuing to stencil. Disable **Stencil enabled** to sculpt freely, or choose **Remove stencil** to close it. These controls remain available with the Alpha panel closed. Removing the stencil keeps the library alpha and existing sculpted strokes.

![On-canvas stencil handles and floating controls](../images/alpha-stencil-controls.png)

Placement changes save on release and stay outside sculpt Undo/Redo.

![Surface stencil stays on its plane as the camera orbits, with placement controls visible](../images/alpha-surface-stencil.png)

*The guide follows the surface plane; brush strokes only affect its bright regions.*

A view stencil stays in the viewport while you orbit, pan, or zoom. A surface stencil is a fixed planar projection in model coordinates: it follows camera changes but does not track later mesh transforms, deformations, or remeshing. Reattach it after moving the model. It projects through curved surfaces rather than wrapping along them.

Freehand, Line, Drag Rect, and Drag Dot work through the placed stencil. Dragging a stamp changes the brush footprint; the stencil stays placed. Stamp previews are private, release creates one undoable edit, and Escape cancels. Completed freehand strokes use the usual one-step Undo. Symmetry mirrors or rotates the stencil with each brush copy.

Adaptive sculpting works through stencils on closed meshes: refinement samples the projected pattern, and simplification stays inside fully white regions to protect dark gaps and edges. Lower adaptive **Edge size** for finer patterns. Stamp preview and release use the same refinement. **Grab**, **Snake**, and **Detail** require the stencil to be disabled. The viewport explains this restriction. Toggle, attachment, and removal actions can be assigned in **Settings → Key settings** or added to custom controls.

![Adaptive sculpting adds a spiral relief to a coarse cube through Nature’s curve](../images/alpha-adaptive-spiral.png)

*Nature’s curve applied through a stencil with adaptive sculpting enabled; the overlay is hidden to show the relief.*

### Example: reset an experimental brush

1. Select **Standard**, choose **Duplicate brush**, and name it **Fold study**.
2. Change **Radius** to **95**, **Strength** to **0.65**, **Spacing** to **0.30**, and enable **Stroke stabilisation**.
3. Choose **Reset settings**. The operation's defaults return; your name and icon stay.

| Before reset | After reset |
| --- | --- |
| ![Fold study with radius 95, strength 0.65, spacing 0.30 and stabilisation enabled](../images/brush-reset-before.png) | ![Fold study keeps its name while radius returns to 60, strength to 0.25, spacing to 0.15 and stabilisation turns off](../images/brush-reset-after.png) |

*Reset affects the selected preset. While Shift is held, it affects the temporary Smooth preset.*

Resetting brush settings does not undo a sculpt stroke. Use **Undo** for geometry changes.

### Save presets and brush icons

In **Presets**, **Import brush…** and **Import library…** open a file picker for JSON presets/libraries. Export paths remain editable in the panel. Imports update brushes with matching IDs; **Duplicate brush** creates an independent variation. Importing an unchanged library does not modify the project.

Choose **Capture model icon** in Brush settings or the library to preview the active model as a thumbnail. Select **Use as brush icon**, optionally **Save PNG**, or cancel. **Restore default icon** brings back the built-in illustration. Icons are embedded in projects and exported presets.

## Detail brush

Detail cuts a narrow groove with raised shoulders. Use Subtract for a sharp ridge. It supports alpha, pressure, masking, and symmetry.

### Example: cut a fine fold

1. Start with a fresh sphere and select **Detail**.
2. Enable **Mesh → Adaptive sculpt**. Use default Detail settings, **Freehand**, and stabilisation off.
3. Draw one slow, slightly curved stroke down the front.
4. Zoom in to inspect the fold. Use **Ctrl+Z** to undo and **Ctrl+Shift+Z** to restore it.

![A continuous narrow Detail groove with raised shoulders, viewed at 200 percent zoom](../images/detail-groove.png)

*One stroke at 200% zoom. Pressure, speed, and radius change its depth and width.*

Use subdivision or adaptive sculpting if a fine crease looks faceted. Detail's adaptive refinement concentrates resolution around the groove and shoulders.

## Stroke modes

Choose **Brush settings → Stroke mode**. Each preset remembers its own mode.

To brush along a reusable surface guide:

1. Open **Mesh → Editable paths**, draw the route and choose **Save & sculpt**.
2. Enable **Guide brush strokes with paths**, then start near the saved guide and draw along it. Only the section you traverse is sculpted.
3. Inspect the stroke. The captured guide reattaches after its own stroke; unrelated surface edits require reprojection.

Pressure, masks, symmetry and the active brush still apply, including Detail, Grab and Snake with their usual stencil restrictions.

Enable **Only start strokes on paths** to ignore off-guide starts, or use **Stroke entire path** to apply the brush automatically. **New ring** creates a complete level or shortest-surface loop with one pen drag. See [editable 3D paths](paths.md) for editing and application settings.

| Mode | Gesture | Useful for |
| --- | --- | --- |
| Freehand | Drag a continuous stroke | Building and smoothing forms |
| Line | Drag a guide; release to apply spaced dabs | Straight creases or repeated marks |
| Drag Rect | Drag from an anchor to size and rotate one stamp | Placing a relief at a chosen scale |
| Drag Dot | Drag a fixed-size stamp into place | Repositioning a detail before release |

**Esc** cancels an unapplied line or stamp. Holding a drag stamp still does not deepen it; the preview is rebuilt from the original surface. Grab and Snake always use continuous Freehand movement.

### Steady a freehand stroke

Enable **Stroke stabilisation** and adjust **Trailing distance (px)**, from 1–200. The brush tip trails your pointer; a line shows the gap. Larger distances filter more hand movement.

The stroke ends at the trailing tip when you release. Stabilisation does not affect Grab, Snake, or the other stroke modes. Changes take effect on the next stroke.

### Stroke limits

Lines are limited to 257 dabs; drag stamps are limited to a 500-pixel radius. Relief stamps use four times one freehand dab's depth. Adaptive stamp refinement can reach its work limit at very fine detail settings.

Each completed stroke is one undo step. Switching tool or view cancels an unapplied preview; already applied dabs stay in history.

## Symmetry

To copy an existing half and replace the other, use [Mirror one side](modeling.md#mirror-one-side). The controls below repeat new brush strokes.

1. Open **Symmetry** in the Model toolbar row and check **Enable symmetry**.
2. Choose X, Y, or Z mirror planes.
3. Draw on one side of the model. The stroke repeats across the selected planes.

![Symmetry settings with X enabled and matching strokes on the left and right of a sphere](../images/symmetry-stroke.png)

*One input stroke produces two marks across the X mirror plane.*

Enable **Radial symmetry** for 2–16 repeats around an axis. Mirrors and radial repeats can be combined. Set the center in object coordinates or reset it to the origin.

Guides show X in red, Y in green, Z in blue, and other radial boundaries in gold. Adjust guide visibility/opacity in the same controls. Symmetry saves with the project; overlapping copies do not multiply stroke strength at the seam.

## Snake brush and adaptive sculpting

### Pull a tapered form

1. Select **Snake**, start on the clay and drag outwards.
2. Keep dragging to extend the tip; it follows your pointer after leaving the original surface.
3. Hold **Shift** to smooth the result. Enable adaptive sculpting if the stretched form needs more geometry.

You should have a tapered protrusion that joins the original surface.

Smooth can gradually soften a pulled-out bump back into the surrounding surface. Use several passes and a brush wide enough to cover its base. For a long protrusion you want to cut off cleanly, use [Slice → Trim](modeling.md#trim-away-one-side) with **Cap cuts** enabled. Trim cuts through the active model; check **Keep / Remove** before releasing.

| Short Snake pull | After smoothing |
| --- | --- |
| ![A short round protrusion pulled from a sphere](../images/smooth-pull-before.png) | ![The protrusion smoothed back into the surrounding sphere](../images/smooth-pull-after.png) |

*Smooth changes the shape while keeping the same triangles. Decimate afterward if the area no longer needs its extra density.*

### Add detail where you sculpt

Adaptive sculpting adds or removes faces near your brush, so you can add detail without subdividing the whole model. Separate parts stay separate, and mask values are preserved.

1. Enable **Mesh → Adaptive sculpt**. **This discards stored subdivision levels; Undo restores them.**
2. Choose **Triangles** for triangle adaptation, or **Quads** to retain four-sided faces during refinement, simplification and stamps.
3. Make a short stroke and inspect the local density. Reduce **Edge size** for finer detail.

Adaptive sculpting requires a closed, consistently oriented surface. Mask strokes do not change topology.

For **Quads**, start with an all-quad mesh, such as the starting sphere or a **Dynamic mesh → Quads** remesh. A triangle or mixed mesh shows an explanation instead of converting silently.

Local quad refinement can create irregular junctions. It does not promise a regular grid or guided edge flow. Sharp folds, protected boundaries and per-frame work limits can prevent the requested edge size from being reached. **Smooth** and **Relax** keep topology fixed in either mode.

Use **Adapt only unmasked areas** when masked faces must keep their existing connections. It freezes every face touching a nonzero mask weight, including soft masks and boundary faces.

Unmasked regions can still refine or simplify, but operations crossing the protected boundary are skipped. This setting protects topology, not all movement: a soft mask still permits some deformation. Fully mask an area to protect its shape too. Hidden faces remain protected regardless of this option.

![Adaptive sculpt settings with the separate Decimate brush selected](../images/adaptive-settings.png)

*Edge size controls local density; smaller values add detail.*

| Setting | Use |
| --- | --- |
| Edge size | Target edge length in screen pixels |
| Adaptive topology | Keep quads on an all-quad mesh, or generate triangles locally; untouched faces are retained |
| Adapt only unmasked areas | Protect the connectivity of faces touching any mask weight; independent of topology choice |
| Detail relative to brush size | Interpret size as a percentage of brush radius |
| Refine | Split long edges near the brush |
| Simplify while sculpting | Automatically collapse short edges during deformation strokes; Smooth keeps topology fixed |
| Simplify ratio | Threshold for collapsing edges relative to the target |

Adaptive sculpting only changes the area around a supported brush stroke. Use [whole-surface remeshing](modeling.md#remesh-a-whole-surface) when you need to rebuild or fuse the complete surface.

### Redistribute polygons with Relax

Use **Relax** to even out crowded or stretched polygons without softening the shape.

1. Choose **Relax** from the brush library, or **Add brush → Relax** in an older project.
2. After Inflate, paint over the uneven region. Radius, Strength, falloff, alpha, tablet pressure and symmetry control the local influence.
3. Compare the shaded surface and wireframe. Spacing should become more even while the form is retained.

Masks and hidden faces protect the surface. Open boundaries, creases and sharp features stay fixed. Relax keeps vertex counts, faces and attributes, even with Adaptive sculpt enabled.

**Surface tolerance** in Brush settings bounds sampled deviation from the retained surface, in model units. Start with the default 0.002; a larger tolerance allows more redistribution on curved regions. The status reports moved/protected/constrained vertices. If little moves, the tolerance or fixed topology may be limiting the result. Relax does not add missing resolution. Use Smooth/Polish when you intend to soften the shape.

Consecutive Relax strokes use the same reference surface. Releasing and pressing again does not reset the surface-error allowance, so repeated strokes cannot gradually increase the permitted deviation.

A different mesh edit, Undo/Redo, changing the active model or reopening the project captures a new reference on the next Relax stroke. Camera and brush-setting changes keep the existing reference. Geometry and presets save and reopen normally; the reference itself lasts only for the session.

![Relax brush surface tolerance and strength controls](../images/relax-brush-controls.png)

![Inflated mesh before Relax](../images/relax-brush-wire-before.png)

![The same mesh after three Relax strokes](../images/relax-brush-wire-after.png)

*Spacing becomes more even around the inflated region. The bulge, polygon count and camera are retained.*

Relax uses freehand strokes and has no subtract operation. Release commits one undo step; **Escape** cancels the current stroke and preserves redo history. Use matched shaded and wireframe views to judge both shape preservation and spacing.

### Smooth first, then reduce triangles

1. Use **Smooth**, or hold **Shift**, to remove noise or soften the shape. Smooth keeps the same vertices and triangles, including in Adaptive sculpt mode.
2. In **Mesh → Adaptive sculpt**, choose **Decimate brush**, then paint over the smoothed region. This selects your existing Decimate preset or adds one to older projects.
3. Increase **Edge size** or **Simplify ratio** for more reduction. Brush strength, pen pressure and falloff control the local reduction threshold. The status bar reports removed vertices and faces.
4. Inspect the surface with wireframe on and off. Each stroke has its own Undo step, so you can undo decimation while keeping the smoothing.

Decimate works even when **Simplify while sculpting** is off. It removes short edges without applying a smoothing pass or adding vertices. Masks, hidden faces, sharp features and local shape checks can limit reduction. It does not have a reverse operation; use Undo to restore removed triangles.

Adaptive sculpting does not fuse overlapping parts. Use [voxel remeshing](modeling.md#mesh-management) for fusion.

## Undo and redo

Use **Edit → Undo / Redo** or **Ctrl+Z / Ctrl+Shift+Z**. Tooltips and the Edit menu name the next action and show available steps.

| Recorded in geometry history | Outside geometry history |
| --- | --- |
| Sculpt/mask strokes, transforms, cuts, topology edits | Brush settings, symmetry settings, camera navigation, workspace preferences |
| Completed parameter drags as one step | Reference-image placement and settings |

History is temporary: **New/Open clears it, and projects do not save it**. A new geometry edit clears redo; cancelled or unchanged operations preserve it. Save a `.clay` project to keep your work, and use [Diagnostic bundles](files.md#diagnostic-bundles) to preserve retained stages for inspection.

| History limit | Default | Configurable range |
| --- | --- | --- |
| Number of edits | 24 | 1–256 |
| Estimated memory shared by undo and redo | 1024 MiB | 16–1024 MiB |

To change these limits, open **Export diagnostic bundle…** in the file toolbar. Lowering a limit may remove steps immediately.

When the budget is full, the oldest undo steps are removed first, then the most distant redo steps. A single snapshot larger than the budget cannot be retained. The memory estimate includes geometry and captured settings; it excludes the live project and allocator overhead.

## Related guides

[Navigation](navigation.md) · [Masks and selection](selection.md) · [Mesh management](modeling.md#mesh-management) · [Files and presets](files.md)

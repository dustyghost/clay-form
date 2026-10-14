# Sculpting and brushes

[Documentation home](../../README.md#documentation)

- [Brushes, alphas, and symmetry](#brushes-alphas-and-symmetry)
- [Detail brush](#detail-brush)
- [Stroke modes](#stroke-modes)
- [Snake brush and adaptive sculpting](#snake-brush-and-adaptive-sculpting)
- [Undo and redo](#undo-and-redo)

## Brushes, alphas, and symmetry

- Standard, Clay, Inflate, Smooth, Flatten, Pinch, Crease, Grab, Mask, and Snake are included.
- Hold **Shift** (or your configured Smooth modifier) to temporarily use the most recently selected Smooth preset. The brush selector highlights it and its own radius, strength, alpha, pressure, spacing, stroke mode, and stabilisation settings all apply. Release Shift to return to your previous brush. Radius shortcuts and brush controls edit the highlighted preset. Select a Smooth variation once to make it your temporary smoothing brush for the session. Projects without any Smooth preset use independent default Smooth settings until you add one.
- Every brush uses an alpha, including Grab, smoothing and masking. Add/Subtract reverses its operation; subtracting Smooth sharpens, subtracting Flatten exaggerates surface relief, and subtracting Mask erases masking.
- Adjust radius, inner radius, strength, alpha falloff, spacing, rotation, pressure strength/radius mapping, and pressure response curve. Inner radius is a fraction of the outer radius: Soft and image alphas keep full influence inside this core, then fade toward the edge; image pixels still modulate the core. Hard and Square retain their hard edges. Brush thumbnails project the effective alpha in blue over the illustration or captured model, updating with rotation and falloff. Enable **Surface footprint and normal** in Brush settings to preview alpha on the visible surface, with tangent outer/inner rings and a surface-normal indicator showing inclination. The tangent rings indicate the local surface plane; the shaded footprint follows curvature and the brush’s camera-plane projection. Grab moves the surface with the cursor; spacing does not affect Grab.
- Choose Soft, Hard or Square alpha, or load a PNG/JPEG up to 2048 × 2048 pixels. White affects the surface, black protects it, and transparency reduces influence. Custom alpha pixels are embedded in presets and projects.
- Every brush has an icon. In Brush settings or the Brush library, choose **Capture model icon** to preview a square capture of the active model at its current viewing angle. The capture fits the whole model and excludes controls and overlays. Choose **Use as brush icon**, optionally **Save PNG**, or cancel. **Restore default icon** returns to the built-in illustration. Custom icons are embedded in projects and exported brushes.
- Use **Brush settings → Reset settings** to restore the defaults for that brush’s current operation, including alpha, radius, strength, stroke mode, stabilisation, and pressure mapping. Its name and captured icon stay intact. This affects only the selected preset (the Smooth preset while Shift is held); exported preset files stay unchanged. The reset action can also be added to a custom menu or shortcut.
- Duplicate any brush to create a named variation. In **Presets**, use **Export brush / Import brush** for one brush, or **Export library / Import library** for the whole library. Persistent brush IDs let imports update matching brushes without adding copies; duplicate creates a new ID. Importing an unchanged library leaves the document unchanged. Older presets without IDs receive a stable identity from their contents; edited legacy files may be imported as new brushes.
- Enable **Symmetry** in the toolbar, then open **Symmetry settings**. Mirror across X/Y/Z independently or combine planes. Radial symmetry adds 2–16 repeats around X, Y, or Z; mirrors and radial repeats can be combined.
- Adjust the symmetry center in object coordinates, or reset it to the origin. Surface lines show the active planes and radial boundaries as you orbit: X pink/red, Y green, Z blue, other radial boundaries gold. Guide visibility and opacity are adjustable.
- Symmetry settings are saved in projects; older projects open with symmetry disabled. Overlapping symmetry copies use the strongest influence per vertex, preventing stronger strokes at the center seam.
- Undo/Redo keeps up to 24 geometry edits within a 128 MB snapshot budget.

## Detail brush

The **Detail** brush cuts a narrow groove with raised shoulders and pinches the surface toward the crease, for wrinkles and fine folds. Choose Subtract (or use your configured reverse modifier during a stroke) to raise a sharp ridge instead. Adjust radius and strength as usual; masks, alpha, pressure and symmetry are supported. New projects include it; in existing projects use **Add brush → Detail**. For crisp features use sufficient subdivision or adaptive detail. Detail samples a broader surface normal so it follows the surface without chasing individual valley walls. Adaptive refinement prioritises the narrow incision and raised shoulders, simplification preserves that extra resolution, and local relaxation reduces small ridges between triangles.

## Stroke modes

Choose a mode under **Brush → Stroke mode**; it is saved per brush in projects, presets and brush libraries. Existing brushes default to **Freehand**.

- **Freehand:** the existing continuous stroke.
- **Line:** press on the active model and drag a straight guide. Release to apply spaced dabs along that screen-space line, projected onto visible surfaces. Very long lines are limited to 257 dabs.
- **Drag Rect:** press to anchor one stamp, then drag to set its radius and alpha angle. The sculpted result previews live; release to keep it. Drag size is capped at the existing 500-pixel brush-radius limit; the preset's size and rotation remain unchanged.
- **Drag Dot:** move a fixed-size stamp with a live sculpt preview, then release to keep it.

**Stroke stabilisation:** in Brush settings, enable this for Freehand strokes and adjust **Trailing distance (px)** from 1–200 (default 30). The brush tip trails the pointer, absorbing small movements; a line connects the tip to the pointer, and the brush footprint follows the tip. A stroke starts with a dab at the press position. Pause or reverse within the trailing distance without adding more dabs. Release ends at the trailing tip, without jumping to the pointer. Pressure, spacing, masks, symmetry, and temporary Smooth/reverse still apply, and the whole stroke is one undo step.

Settings are saved per brush in projects, presets, and libraries and take effect on the next stroke. Switching presets with the Smooth modifier starts a fresh stroke segment at the pointer using that preset’s mode and trailing distance; an uncommitted line or stamp preview is discarded, and already applied dabs remain in the same undo step. Disable it to restore direct Freehand input; Grab, Snake, and the other stroke modes are unaffected. **Toggle stroke stabilisation** is available for custom menus and shortcuts.

With Adaptive sculpt and Refine enabled, drag stamps refine the surface before applying the alpha, with extra detail along sharp alpha boundaries. Very large stamps or very fine edge sizes can reach the bounded refinement limit.

Drag stamps rebuild their preview from the original mesh, so holding still does not build up depth. Relief brushes (Standard, Clay, Inflate, Crease and Detail) use four times the depth of one freehand dab; Strength and pressure still control the result. Mask opacity and smoothing weights retain their normal values.

Line and drag modes preview before changing document geometry, create one undo step, and support masks, alphas, symmetry and pen pressure. **Esc** cancels a pending stroke. Hidden faces cannot receive stamps. Grab and Snake keep their continuous Freehand behavior because they depend on pointer displacement.

## Snake brush and adaptive sculpting

Choose **Snake** in the brush library, or use **Add brush → Snake** when working in an older project. Start on the clay and drag outward to pull a tapered form. Snake follows a continuous tip handle after the cursor leaves the original surface. Its smooth volumetric influence pulls both sides of a bent tip; adaptive mode also improves triangle shape and redistributes nearby vertices along the surface. It uses the same editable alpha, strength, radius, pressure, Add/Subtract, copy, and preset controls as other brushes. Shift temporarily smooths. Pair it with adaptive sculpting for stretched forms; without adaptation, it deforms the existing mesh.

Select **Mesh → Adaptive sculpt**, or use the Mesh management panel. This is a adaptive sculpting-style local topology workflow, separate from whole-surface voxel remeshing. It splits long edges near each brush dab, shares new vertices across adjacent faces, and optionally collapses short edges in dense areas. It keeps separate parts separate and preserves masks. The Mask brush leaves topology unchanged, and fully protected or all-black-alpha strokes do not adapt it.

**Edge size** sets the desired triangle edge length in screen pixels; lower values add detail. Enable **Detail relative to brush size** to interpret it as a percentage of the brush radius instead. Refinement and simplification can be enabled independently; **Simplify ratio** sets how short an edge must be relative to the target before it can collapse. Sharp edges, normal flips, and invalid manifold collapses are rejected conservatively. Density approaches the requested target over successive dabs with bounded per-frame work, rather than guaranteeing an exact edge length immediately. Existing mesh limits apply.

Enabling adaptive mode keeps the active surface and discards stored subdivision levels; Undo restores them. It requires a closed, consistently oriented surface. Each complete adaptive brush stroke is one undo step, including its topology changes. Adaptive settings and geometry save with the project, and old files remain compatible. This first implementation uses CPU local splits/collapses; it does not use external sculpting application's proprietary algorithm or provide automatic fusion of intersecting parts. Use voxel remeshing when you want fusion. Voxel extraction interpolates distances to the source surface, so smooth shapes stay smooth rather than snapping to voxel steps. Resolution still limits the smallest retained details.

With **Adaptive sculpt → Simplify dense areas** enabled, Smooth (including Shift smoothing) cleans up dense topology without adding vertices. It uses a larger collapse threshold than deformation brushes, capped below the target edge size, and rejects collapses that create thin triangles. This removes excess density while keeping a tidy surface rather than pursuing a minimum vertex count. Increase **Edge size** or **Simplify ratio** for more reduction; reduction remains local and conservative around masks and sharp features. Turn Simplify off to allow ordinary adaptive refinement with Smooth. The Mesh panel shows the current edge threshold, and the status bar reports vertices and faces removed after each cleanup stroke. Cleanup can collapse up to 256 safe edges per input frame, sharing that allowance across interpolated dabs; ordinary deformation brushes retain their smaller allowance. A cleanup stroke remains one undo step.

## Undo and redo

Mask intensity, selection threshold, temporary thickness, and primitive parameter drags each create one undo step. Releasing the pointer, losing focus, or switching operations ends that edit. During a mask-intensity drag, passing through zero and back up restores the original soft proportions. Clearing an already-empty mask preserves redo history and does not mark the project modified.

Use the toolbar buttons or **Edit → Undo / Redo**. The Edit menu shows the action that will be restored, your configured shortcuts, and available step counts. Defaults are **Ctrl+Z** and **Ctrl+Shift+Z**, adjustable in Key settings. Button tooltips name the action; the status bar confirms each undo/redo.

History includes complete brush strokes (including masks), clear mask, subdivision, remeshing, switching mesh modes, changing levels, and deleting levels. A stroke creates one entry only when geometry or masking actually changes. Empty/protected strokes preserve redo; a new geometry edit starts a new history branch. History resets on New/Open and is session-only. Brush settings, symmetry settings, camera movement, and workspace preferences are outside this geometry history. Undo/Redo pause during a mesh operation or an unsaved-changes prompt.

## Related guides

[Navigation and tablet input](navigation.md) · [Masks and selection](selection.md) · [Mesh management](modeling.md#mesh-management)

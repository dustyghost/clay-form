# Clay Form roadmap

[Documentation home](../README.md#documentation)

Updated 14 October 2026.

The next priority is viewport reference images. Stroke stabilisation, mask blur/sharpen, lasso selection, transform rotation/pivot controls, automatic recovery saves, and OBJ import are implemented. The hands-on tablet check for stroke stabilisation passed on 14 October 2026. Reference images and perspective viewing are the remaining priorities. This is the proposed implementation order; dates are not assigned.

## Current foundation

These features are already implemented and should be preserved as the app grows:

- Sculpt brushes, pen pressure, custom alphas, brush presets, and Freehand, Line, Drag Rect and Drag Dot strokes.
- Mirror and radial symmetry, adjustable symmetry centres, and surface guides.
- Subdivision levels, voxel remeshing, and local adaptive sculpting.
- Submodels, duplication, Move, Scale, and Rotate transforms, and Action Line controls.
- Vertex groups and group colours, masking, mask expand/contract and blur/sharpen, hide/isolate, extraction, split hidden, and merge down.
- Parametric primitives, slicing/trimming, and thickness for open meshes.
- Custom menus in either sidebar, removable custom buttons and menus, configurable shortcuts, and saved workspaces with colour settings.
- Undo/redo, project saving/recovery, and OBJ import/export.

Recent fixes address stale groups after Group all, Alt-drag panning when starting off the model, Standard brush overlap at symmetry seams, and the vertical orbit angle limit. Continue checking these during normal sculpting, particularly seam behaviour on curved surfaces and navigation with a pen.

## Implementation priorities

Stroke stabilisation, mask blur/sharpen, lasso selection, transform rotation/pivot controls, automatic recovery saves, and OBJ import are implemented. Viewport reference images are next; the remaining items are planned.

| Order | Feature | Main benefit |
| --- | --- | --- |
| 1 | Stroke stabilisation — implemented | More controlled wrinkles, eyelids, seams, and detail strokes |
| 2 | Mask blur/sharpen and lasso selection — implemented | Faster selection and smoother transitions when sculpting or posing |
| 3 | Transform rotation and pivot controls — implemented | Rotate models or unmasked regions directly |
| 4 | Automatic recovery saves — implemented | Recover recent work after a crash or interrupted session |
| 5 | OBJ import — implemented | Sculpt base meshes made in other software |
| 6 | Viewport reference images — next | Keep photographs and drawings beside or behind the sculpt |
| 7 | Perspective view | Judge proportions with an adjustable perspective camera |

### 1 Stroke stabilisation

Implemented 14 October 2026: optional trailing stabilisation for Freehand strokes, with a 1–200 px distance and visible guide. Settings are saved per brush in projects, presets, and libraries; the toggle is available through custom menus and shortcuts. Existing brushes default to disabled. Grab and Snake retain their current behaviour.

Automated checks cover jitter, spacing, changing pen pressure, masks, symmetry, temporary Smooth/reverse, subdivision, release without a jump, one-step undo, interrupted strokes, and file compatibility. The user completed the hands-on tablet check on 14 October 2026 and reported that the feel works well.

Completion criteria:

- Mouse and pen strokes follow a smooth, predictable path, with pressure and spacing still working.
- Symmetry, masks, and temporary Smooth/reverse modifiers behave consistently.
- Ending a stroke produces no sudden jump; each stroke remains one undo step.
- Disabling stabilisation restores the existing Freehand behaviour. Grab and Snake retain their current behaviour initially.

### 2 Mask editing and lasso selection

Implemented 14 October 2026: Blur mask and Sharpen mask alongside expand/contract, with custom-menu and shortcut support. Blur averages along polygon edges; Sharpen increases contrast around 50%. Both apply to the active submodel's stored subdivision levels, including hidden faces, without changing geometry or saved vertex group weights. Each changed operation has one undo step, and unchanged masks preserve redo history.

Implemented 14 October 2026: Mask lasso and visibility Select lasso, with a live outline, Alt to erase/hide, Space to move, and Escape to cancel. Both explicitly select through all depths on the active submodel; face visibility selects whole quads by their centers, while masking selects vertices and protects hidden faces and shared boundary vertices. Each changed gesture has one undo step and applies to all stored subdivision levels. Both tools support custom menus and configurable shortcuts. Front-surface-only selection is outside this phase.

Regression checks cover closed and concave outlines, crossings, multiple samples in one frame, multiple view angles, through-depth selection, cancellation and interrupted input, translation, undo/redo, hidden-face protection, subdivision levels, and project persistence. GPU preview and commit checks cover the rendered outline and resulting mask/visibility. Hands-on lasso pen feel remains available for user feedback.

Completion criteria:

- Blur softens a mask boundary and Sharpen tightens it without changing mesh geometry or saved vertex-group weights.
- Lasso selections can be cancelled, respect the active submodel, and work from different viewing angles.
- Selection depth behaviour is explicit, so users can tell whether a lasso affects only visible surfaces or passes through the model.
- Each completed operation is undoable and handles stored subdivision levels consistently.

### 3 Transform rotation and pivot controls

Implemented 14 October 2026: Rotate extends the existing World axes and Action Line transform controls. World mode offers X/Y/Z rings and a placeable/resettable pivot; Action Line reuses its start and direction for rotation around the line. The same operation controls serve the floating toolbar and sidebar, including custom-menu/shortcut actions. No parallel transform subsystem or project-format change is introduced.

Soft masks reduce rotation angles, stored subdivision levels follow, and primitives retain their existing affine placement when regenerated. Each changed drag has one undo step. Pivot/line placement is session-only and preserves clean state and redo. Automated checks cover angle direction and wraparound, edge-on views, pivot reset, focus loss and tool switching, final release movement, masks, stored levels, primitive dimension edits and persistence, and real OpenGL control rendering. Hands-on pen feedback remains welcome before refining ring feel.

Completion criteria:

- Axis rotation handles and angle feedback make the direction and amount of rotation clear.
- Respect mask works with soft masks, and stored subdivision levels follow the transformation.
- Parametric primitives retain their placement when their dimensions are edited afterward.
- One drag creates one undo step; view orbiting remains separate from geometry rotation.

### 4 Automatic recovery saves

Implemented 14 October 2026: automatic recovery defaults to every two minutes with five retained copies, configurable through Settings → Recovery saves. Complete project snapshots are taken between edits and written in a background worker using flushed temporary files and atomic replacement. Unchanged snapshots are deduplicated; retention runs only after a successful write. Recovery never changes the manual save target or clears modified state.

Startup offers retained copies explicitly. Restoring marks the project modified and selects `recovered.clay`, preserving the original project. Saving or deliberately discarding clears the session's copies through the same ordered worker, including any in-flight write. Failed writes remain visible and retry at the configured interval. Recovery settings and Save recovery now are reusable shortcut/custom-menu actions. Automated checks cover complete-project round trips, retention, interrupted/invalid writes, worker failures and retries, save/discard races, edit boundaries, restart recovery, and unsaved-change decisions.

The accompanying interface pass makes Transform, Slice, Primitive, and Add brush use the existing pinnable/dockable menu system. Compact pin/unpin and left/right sidebar icons with tooltips replace long placement buttons in built-in and custom menus. Transform retains its shared controls and suppresses duplicate automatic floating controls when explicitly placed.

Completion criteria:

- Recovery captures a complete, consistent project between edits, including work that has never been manually saved.
- Recovery files are separate from the user's chosen project file and survive an interrupted write.
- Startup offers the available recovery copy without silently replacing the current project.
- Users can control the interval and retention count; write failures appear clearly without blocking sculpting indefinitely.

### 5 OBJ import

Implemented 14 October 2026: Submodels → OBJ path → Import OBJ loads a new submodel through the existing cancellable mesh worker and commits one undo step. The rest of the scene, project save path, and brush settings remain intact. Position, scale, winding, and quads are preserved; simple planar n-gons are triangulated with the existing triangulation library. OBJ groups become vertex groups, with the first membership determining each face's category. Multiple objects form one imported submodel. UVs, supplied normals, materials, and vertex colors are outside this phase; unsupported geometry and malformed files fail explicitly.

Checks cover relative/absolute and UV/normal indices, concave and reversed polygons, overlapping group membership, size limits, invalid/interrupted/cancelled imports, primitive preservation, undo/redo, project persistence, and export/import round trips. The importer uses the existing scene/history/action paths and requires no new project format or dependency.

Completion criteria:

- Common triangle and quad meshes import with correct geometry and winding; other polygon handling is documented.
- OBJ face groups map to the app's vertex group workflow where possible.
- Invalid files and meshes exceeding supported limits produce a clear error without partially changing the document.
- Import is undoable, and an exported mesh can be imported again with its shape and groups preserved.

### 6 Viewport reference images

Load photographs or drawings as movable reference images, with scale, opacity, visibility, and position locking. Start with images fixed to the screen; references attached to front/side view planes can follow.

Completion criteria:

- Images can be moved, resized, hidden, locked, and removed without modifying the sculpt.
- Locked images do not intercept sculpting or navigation gestures.
- Placement and image data survive project save/reopen.
- Multiple references remain manageable in a small viewport.

### 7 Perspective view

Add an orthographic/perspective toggle and adjustable field of view. Keep orthographic mode available for precise comparisons against drawings.

Completion criteria:

- Rendering, surface picking, brush footprints, symmetry guides, and transform handles agree in both modes.
- Orbit, pan, and zoom remain predictable, including close views and turns over the poles.
- Switching projection keeps the model framed sensibly and does not alter geometry.
- Near and far clipping do not unexpectedly hide ordinary working views.

## Longer term candidates

These are separate possibilities, outside the seven priorities above. Each needs its own scope before implementation.

| Candidate | Intended use |
| --- | --- |
| Sculpt layers and morph targets | Adjust or remove a set of sculpted changes without undoing later work |
| Materials and vertex colour painting | Inspect forms under different shading and paint colour directly on the surface |
| Quad retopology and detail projection | Rebuild cleaner topology while recovering sculpted detail |
| Boolean operations | Combine or subtract submodels for construction and hard-surface work |
| Texture painting and UV workflows | Prepare textured assets for other applications |

## Delivery checks

Prioritise the user experience over feature-for-feature or gesture-for-gesture parity with external sculpting application. Reference workflows explain the intended task; they do not require copying awkward interactions. Follow the [interaction principles](ui-design.md#interaction-principles), reuse existing functionality, and assess discoverability, predictable mouse/pen behaviour, and ease of cancelling or finishing each workflow.

New actions should be available through custom menus and configurable shortcuts where appropriate. Preserve existing projects, brush presets, and saved workspaces. Geometry changes need coherent undo behaviour; camera and interface changes must not dirty sculpt geometry.

For each feature, run focused regression tests and check its interaction with masks, symmetry, subdivision, and pen input where relevant. Mark it complete only after its completion criteria have been checked, then update the relevant user guide linked from the README.

## external sculpting application workflow references

These references describe the comparison workflows; Clay Form's scope and completion criteria are defined above.

- stroke stabilization and sculpting
- Masking and lasso selection
- Gizmo operations
- automatic save
- OBJ import and export
- image reference reference images
- Orthographic and perspective cameras

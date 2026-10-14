# Clay Form roadmap

Updated 14 October 2026.

The next priority is to make everyday sculpting more controlled and dependable: better masking, complete model transforms, and recovery saves. Stroke stabilisation is implemented, with physical pen feel still to be checked. Importing models, using reference images, and checking perspective follow. This is the proposed implementation order; dates are not assigned.

## Current foundation

These features are already implemented and should be preserved as the app grows:

- Sculpt brushes, pen pressure, custom alphas, brush presets, and Freehand, Line, Drag Rect and Drag Dot strokes.
- Mirror and radial symmetry, adjustable symmetry centres, and surface guides.
- Subdivision levels, voxel remeshing, and local adaptive sculpting.
- Submodels, duplication, Move and Scale transforms, and Action Line controls.
- Vertex groups and group colours, masking, mask expand/contract and blur/sharpen, hide/isolate, extraction, split hidden, and merge down.
- Parametric primitives, slicing/trimming, and thickness for open meshes.
- Custom menus in either sidebar, removable custom buttons and menus, configurable shortcuts, and saved workspaces with colour settings.
- Undo/redo, project saving, and OBJ export.

Recent fixes address stale groups after Group all, Alt-drag panning when starting off the model, Standard brush overlap at symmetry seams, and the vertical orbit angle limit. Continue checking these during normal sculpting, particularly seam behaviour on curved surfaces and navigation with a pen.

## Implementation priorities

Stroke stabilisation and mask blur/sharpen are implemented. Lasso selection is next; the remaining items are planned.

| Order | Feature | Main benefit |
| --- | --- | --- |
| 1 | Stroke stabilisation — implemented | More controlled wrinkles, eyelids, seams, and detail strokes |
| 2 | Mask blur/sharpen — implemented; lasso selection — next | Faster selection and smoother transitions when sculpting or posing |
| 3 | Transform rotation and pivot controls | Rotate models or unmasked regions directly |
| 4 | Automatic recovery saves | Recover recent work after a crash or interrupted session |
| 5 | OBJ import | Sculpt base meshes made in other software |
| 6 | Viewport reference images | Keep photographs and drawings beside or behind the sculpt |
| 7 | Perspective view | Judge proportions with an adjustable perspective camera |

### 1 Stroke stabilisation

Implemented 14 October 2026: optional trailing stabilisation for Freehand strokes, with a 1–200 px distance and visible guide. Settings are saved per brush in projects, presets, and libraries; the toggle is available through custom menus and shortcuts. Existing brushes default to disabled. Grab and Snake retain their current behaviour.

Automated checks cover jitter, spacing, changing pen pressure, masks, symmetry, temporary Smooth/reverse, subdivision, release without a jump, one-step undo, interrupted strokes, and file compatibility. Physical tablet feel still needs a hands-on check.

Completion criteria:

- Mouse and pen strokes follow a smooth, predictable path, with pressure and spacing still working.
- Symmetry, masks, and temporary Smooth/reverse modifiers behave consistently.
- Ending a stroke produces no sudden jump; each stroke remains one undo step.
- Disabling stabilisation restores the existing Freehand behaviour. Grab and Snake retain their current behaviour initially.

### 2 Mask editing and lasso selection

Implemented 14 October 2026: Blur mask and Sharpen mask alongside expand/contract, with custom-menu and shortcut support. Blur averages along polygon edges; Sharpen increases contrast around 50%. Both apply to the active submodel's stored subdivision levels, including hidden faces, without changing geometry or saved vertex group weights. Each changed operation has one undo step, and unchanged masks preserve redo history.

Still planned: lasso tools for both masking and face visibility, with clear add/remove behaviour and a preview before committing.

Completion criteria:

- Blur softens a mask boundary and Sharpen tightens it without changing mesh geometry or saved vertex-group weights.
- Lasso selections can be cancelled, respect the active submodel, and work from different viewing angles.
- Selection depth behaviour is explicit, so users can tell whether a lasso affects only visible surfaces or passes through the model.
- Each completed operation is undoable and handles stored subdivision levels consistently.

### 3 Transform rotation and pivot controls

Extend World axes and Action Line transforms with Rotate. Allow the pivot to be placed and reset so a user can turn an entire submodel or pose an unmasked region around a useful centre.

Completion criteria:

- Axis rotation handles and angle feedback make the direction and amount of rotation clear.
- Respect mask works with soft masks, and stored subdivision levels follow the transformation.
- Parametric primitives retain their placement when their dimensions are edited afterward.
- One drag creates one undo step; view orbiting remains separate from geometry rotation.

### 4 Automatic recovery saves

Add configurable periodic recovery copies with a bounded number of versions and an option to recover work on startup. Bring this forward if sessions become long enough that losing recent work is a practical concern.

Completion criteria:

- Recovery captures a complete, consistent project between edits, including work that has never been manually saved.
- Recovery files are separate from the user's chosen project file and survive an interrupted write.
- Startup offers the available recovery copy without silently replacing the current project.
- Users can control the interval and retention count; write failures appear clearly without blocking sculpting indefinitely.

### 5 OBJ import

Import an OBJ as a new submodel, preserving its placement and usable face groups. Keep existing project content intact.

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

New actions should be available through custom menus and configurable shortcuts where appropriate. Preserve existing projects, brush presets, and saved workspaces. Geometry changes need coherent undo behaviour; camera and interface changes must not dirty sculpt geometry.

For each feature, run focused regression tests and check its interaction with masks, symmetry, subdivision, and pen input where relevant. Mark it complete only after its completion criteria have been checked, then update the user instructions in the README.

## external sculpting application workflow references

These references describe the comparison workflows; Clay Form's scope and completion criteria are defined above.

- stroke stabilization and sculpting
- Masking and lasso selection
- Gizmo operations
- automatic save
- OBJ import and export
- image reference reference images
- Orthographic and perspective cameras

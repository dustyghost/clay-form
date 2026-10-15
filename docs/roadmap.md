# Clay Form roadmap

[Documentation home](../README.md#documentation)

Updated 15 October 2026.

The seven priorities below are implemented. Perspective viewing is ready for hands-on mouse/pen feedback. The hands-on tablet check for stroke stabilisation passed on 14 October 2026. The longer-term candidates remain separate possibilities; edge loops and creases are now implemented.

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

## Edge loops and creases

Current implementation: [Edges and creases](guide/edges.md) targets hole borders, masked region borders, visible polygroups, and selected edges. It includes support-loop insertion, border relaxation, crease/uncrease/clear, edge and loop selection, and smooth subdivision with a 0–1 Smoothness slider. Thickness modifier replaces the old Temporary thickness label and adds previewed rim loops and crease options.

Smooth subdivision currently bakes the active level; the existing linear subdivision stack is unchanged. Loop cleanup is relaxation, without automatic retopology or loop dissolution. Verification covers geometry, selection, attributes, modifier preview/apply, cancellation, persistence, undo, and offscreen UI captures. The rendered UI walkthrough covers selection, empty-space navigation, crease removal, both subdivision operations, target scopes, and thickness application. Selecting creases or borders enters edge-selection mode. Linear and smooth subdivision share the Mesh → Subdivision controls. Mouse/pen feel remains available for user feedback.

## Implementation priorities

All seven priorities are implemented; interaction refinements can follow hands-on feedback.

| Order | Feature | Main benefit |
| --- | --- | --- |
| 1 | Stroke stabilisation — implemented | More controlled wrinkles, eyelids, seams, and detail strokes |
| 2 | Mask blur/sharpen and lasso selection — implemented | Faster selection and smoother transitions when sculpting or posing |
| 3 | Transform rotation and pivot controls — implemented | Rotate models or unmasked regions directly |
| 4 | Automatic recovery saves — implemented | Recover recent work after a crash or interrupted session |
| 5 | OBJ import — implemented | Sculpt base meshes made in other software |
| 6 | Viewport reference images — implemented | Keep photographs and drawings beside or behind the sculpt |
| 7 | Perspective view — implemented | Judge proportions with an adjustable perspective camera |

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

Implemented 14 October 2026: screen-fixed PNG/JPEG references, added by dropping files onto the viewport or through the shared References menu. Arrange images provides explicit move/resize controls and a visible Done/Escape exit; all references are passive while sculpting. Per-image opacity, visibility, and position/size locking, plus a session-only show/hide-all action, keep references manageable. The menu can be pinned or docked through the existing placement controls; its actions support custom menus and shortcuts.

Images and placement are embedded in projects and recovery copies, independently of sculpt geometry/history. Loading applies photo orientation and bounds image dimensions, file size, count, and aggregate memory. Checks cover file removal after import, legacy projects, recovery, invalid files and placements, drag release/cancellation/focus loss, undo during a drag, locked-image navigation, and normal/small viewport OpenGL composition. See the [reference image guide](guide/references.md) for controls and limits. Front/side view-plane references remain a possible extension; hands-on pen feedback will guide interaction refinements.

Completion criteria:

- Images can be moved, resized, hidden, locked, and removed without modifying the sculpt.
- Locked images do not intercept sculpting or navigation gestures.
- Placement and image data survive project save/reopen.
- Multiple references remain manageable in a small viewport.

### 7 Perspective view

Implemented 14 October 2026: **View → Perspective view** switches projection, with a 20–75° field-of-view control and a reusable toggle for custom menus and shortcuts. Projection changes fit the eye distance to the visible scene and adjust zoom to preserve apparent size within the normal zoom limits. Zoom enlarges the projection without moving the eye through the surface. Camera state remains session-only; changing projection ends active gestures and discards unapplied previews without adding a geometry undo step.

GPU surfaces, wireframe and surface overlays share the perspective projection. CPU picking casts rays against the same mesh snapshot; brush radii, drag movement, rotation rings, primitive controls, selections, symmetry guides and cuts account for depth. Perspective path cuts converge at the eye and preserve closed caps. Geometry crossing the eye is clipped for viewing; a path cut through it reports an explicit error. Tests cover surface/pixel agreement, clipping, field-of-view limits, framing, shortcut migration, stamp cancellation, undo, rotation and path-cap volume. The View-panel preview has been visually checked; hands-on pen feedback remains welcome. See the [navigation guide](guide/navigation.md#perspective-view).

Completion criteria:

- Rendering, surface picking, brush footprints, symmetry guides, and transform handles agree in both modes.
- Orbit, pan, and zoom remain predictable, including close views and turns over the poles.
- Switching projection keeps the model framed sensibly and does not alter geometry.
- Near and far clipping do not unexpectedly hide ordinary working views.

## Pen modeling foundation

First pass implemented 15 October 2026. Mesh now exposes an explicit Pen modeling mode with face/edge/vertex targets, visible Replace/Add/Remove selection controls, paint selection, directional face loops, edge loops and parallel edge rings, component-to-vertex-group assignment, flat-region extrusion previews, single-face matching tunnel joins, and draggable cuts through quad strips. A tap centers the cuts; dragging slides them, with 1–16 loops controlled by visible buttons or the wheel while held. Modeling buttons are available to custom menus, shelves, and shortcuts. Complete quad-loop dissolution and topology-checked mirror/radial symmetry are also implemented for pen modeling. Existing support loops and creasing remain available.

Extrusions use vertical pen travel independent of pressure. Release commits one undo step; Escape discards the preview. Tunnel candidates require reversed, aligned boundary correspondence and snap to the first matching depth. Joins cancel coincident side patches at outer edges and beside existing holes, including subdivided side strips; the remaining walls conform to existing depth loops. Edits validate orientation, degeneracy, duplicate topology, mesh limits, and selection revision before commit. Stored levels are replaced only on a successful topology edit. Selection and previews are session-only, so the project schema remains unchanged.

Focused geometry/controller checks cover closed tunnel topology, attribute/crease transfer, selection modes, group assignment, loop cuts, perspective previews, cancel, stale selections, modal blocking, navigation, undo/redo, and saved-project validation. The rendered workflow exercises the actual controls, selection-to-group, extrusion cancellation, tunnel commit, and undo. Hands-on tablet feedback is still needed before treating these interactions as settled.

### Modeling delivery specification — 15 October 2026

The four workflows below are implemented in incremental commits. Geometry/controller checks and rendered interaction exercises cover the supported operations; the user guide records their limits. Boundary bevel is a single chamfer on compatible exterior corners, bridge requires matching boundary counts, and smooth preview is one level on a low-resolution cage. These milestones establish a usable construction workflow; they do not imply arbitrary edge bevels, Boolean fusion or component modeling feature parity.

| Milestone | Deliverable | Acceptance exercise |
| --- | --- | --- |
| 1. Select and manipulate | Shared component targets, connected/flat/group regions, explicit conversion, component move/rotate/scale with constraints and numeric amounts, visible protection rules | Paint a region, convert its boundary to edges, manipulate it, assign a group, target that group again, repeat with symmetry; cancel and Undo restore the original |
| 2. Construct shapes | Coordinated extrusion, inset, bevel and fuse; region/individual options, numeric amounts, independent contact snapping, group propagation | Make a recessed panel, inset/extrude a rim, bevel its boundary, create adjacent tunnels and push an exterior slab face to an intermediate loop |
| 3. Edit topology | Insert/slide/dissolve loops, bridge/fill boundaries, connect/split/weld/collapse and explicit face deletion | Adjust support loops, remove an unnecessary loop, open and bridge a pair of boundaries, close a hole, connect and weld vertices |
| 4. Finish the workflow | Configuration presets, explicit repeat with remembered amounts, subdivision preview, integrated pen walkthroughs | Build symmetric inset/extruded features with visible controls, save/reload a preset, repeat a measured feature, inspect the smooth result without changing the cage |

All edits use a draft, visible affected components, an explanation for unsupported topology, cancellation and one Undo per completed operation. Symmetry is atomic and requires matching sides. Topology edits preserve surviving masks, group weights and creases. Component transforms and loop slides retain selection; inset, bevel and ordinary extrusion retain their cap selection. Hidden geometry is protected. Mask protection must be explicit: a topology operation that touches protected vertices is rejected as a whole rather than partially applied. Modeling keeps its own selection and never silently replaces the sculpt mask. Geometry validity checks are not a general intersection/Boolean solver.

The interaction model is **Component → Action → Target → Options**. Component modes stay locked until deliberately changed; picking tolerance is measured in screen pixels, separate from sculpt brush size. Targets preview before contact and lock at pen-down. Shared targets include individual components, current selection, loops/rings, connected regions, flat regions and face groups; only applicable combinations are offered. Direction-sensitive targets show their direction. A visible panel is the reliable entry point, with custom menus and shortcuts using the same action identities. Parameters have visible names and values; modifiers are optional accelerators. Measured tools offer **Tap to repeat**, using the visible amount and current options; dragging sets the next amount. The toggle can disable tap edits and is stored in presets. Select keeps selection gestures and Loop cut retains its centered tap. A separate Repeat last edit command remains available for the previous measured operation.

Ordinary extrusion, contact fusion, vertex welding and loop dissolution have different meanings. Contact fusion gets a visible snap distance; geometric matching uses a separate conservative tolerance. Increasing tolerance does not make incompatible topology safe. Face labels describe polygon regions; weighted vertex groups remain a distinct selection/weighting concept even where they share existing storage.

#### Milestone 5: construct from edges and points — implemented

| Workflow | Delivered behavior |
| --- | --- |
| Collapse an edge | Explicit endpoint/midpoint destination, numeric fraction, marked endpoint and pen preview; connected-set collapse remains a separate topology command |
| Split an edge | Drag insertion along the edge or enter a fraction; neighboring faces receive matching vertices |
| Extrude an edge | Extend an open border with a quad strip, or build a surface ridge with a visible adjacent-side choice locked at pen-down |
| Extrude a point | Build a pointed protrusion with independent base radius, averaged-normal height, circularization and ring creasing |
| Split a point into a patch | Replace a closed vertex fan with a ring and center patch; radius, circularization, triangulation and crease controls; patch can be extruded into a tube |
| Add topology inside a face | Center-point insertion and quad-preserving face split; shared edge splits propagate into neighbors without T-junctions |
| Angle-based flat islands | Select connected visible faces within 0–180° of the first face normal, with hover feedback and a preset-compatible slider |

These tools reuse numeric input, tap repeat, presets, mask protection, exact face labels, atomic symmetry and one-step Undo. Directional actions identify the chosen side or endpoint before contact and retain it throughout the stroke. Face-center actions are direct taps. New component construction clears selection after committing. Unsupported overlapping selections reject atomically; point rings require closed visible fans and a radius below half the shortest incident edge. Larger polygons are represented by triangles, including neighboring faces that need new edge points. These operations do not perform intersection detection or Boolean fusion.

Geometry and controller tests cover closed surfaces, border extension, point patches and tube extrusion, edge positions, attributes, symmetry, masks, cancellation, tap repeat, Undo and preset compatibility. The rendered exercise uses the actual controls for a symmetric creased point patch, tube extrusion, surface-edge preview and face split. See [the component construction guide](guide/modeling.md#construct-from-edges-points-and-face-centers) for controls and limits.

The documentation audit used external software vendor's Edge Actions, Point Actions and Polygon Actions to identify missing construction workflows. Clay Form exposes explicit modes and visible values rather than duplicating modifier-dependent behavior.

#### Research informing the design

external software vendor's Actions and Targets and Options and Modifiers provide a useful compositional model. Polygon Actions separates extrusion from fused extrusion's fusing behavior. Clay Form uses explicit controls and its own terminology rather than reproducing external sculpting application's modifier timing or brush-size-dependent attraction.

Community reports are qualitative evidence, not a prevalence survey or a claim that every external sculpting application version has the same problems:

- Pen edge targeting and unintended component targeting motivate locked component modes and screen-space picking.
- Selection/masking workflow motivates explicit selection, component transforms and visible subdivision constraints.
- Unexpected dynamic brush sizing motivates settings independent of sculpt brushes and retained user choices.
- Hidden state affecting operations motivates visible protection state and actionable rejection messages.

Curve/mesh instancing authoring, arbitrary Boolean fusion, detail reprojection onto changed topology and full component modeling feature parity are outside these milestones. See [Pen modeling](guide/modeling.md#pen-modeling) for shipped behavior and limits.

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

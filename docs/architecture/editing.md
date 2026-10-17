# Editing and history

[Architecture overview](../architecture.md) · [Documentation home](../../README.md#documentation)

Use this reference when changing these systems. Start with the overview for ownership and the shared edit rules.

## Continuous edit sessions

`document/edit.rs` stores one typed session for primitive parameters, thickness modifier, mask intensity, or mask threshold. The mask-intensity session owns its original peak and masks, so dragging through zero retains the information needed to restore soft values. `Command::edit_kind` identifies which commands can continue a session. The dispatcher closes a session when the command changes kind; commands blocked by modal or background-job guards do not interrupt it. The first changed update records history. Repeated updates reuse that entry; unchanged controls do not create one.

Pointer release and focus loss finish parameter sessions at the application frame boundary. Switching models or applying a discrete document edit also finishes them. `end_primitive_edit` is specific to primitive gestures; it no longer resets unrelated mask or thickness state. Controller tests in `app/parameter_tests.rs` exercise dispatch plus these input boundaries, including switching controls, undo/redo, hidden masks on stored levels, and clearing an already-empty mask.


## Mesh workflows

`Geometry` owns the active `Mesh` and `MeshSettings`: lower levels in ascending order, higher levels in reverse order, and dynamic remesh settings. `sculpt/subdivision.rs` shares edge midpoints and retains original vertex indices. Going down samples that original-vertex prefix; going up interpolates the difference from the saved parent's vertices and adds it to the stored high mesh.

This preserves high-level residual detail rather than replacing the level with a fresh subdivision. The approach uses linear triangle refinement, with no Loop smoothing or tangent-frame displacement transport. Mesh-stack topology is validated against deterministic refinement at load and before topology commits.

### Voxel remeshing

`sculpt/remesh.rs` rasterizes oriented triangles onto yz scanlines, fills their nonzero winding intervals along x, and extracts the voxel boundary using six consistently arranged tetrahedra per grid cube. A global grid-edge cache shares surface vertices; each triangle is oriented toward the outside. Winding occupancy supplies the sign for a narrow-band distance field measured against the input triangles.

Surface crossings interpolate these distances instead of using grid-edge midpoints, avoiding quantized voxel bands. Two mild relaxation passes regularize the triangulation. Subvoxel details and masks are still lost.

For background on tetrahedral isosurface extraction, see [Paul Bourke's polygonisation reference](https://www.paulbourke.net/geometry/polygonise/); for multiresolution editing concepts, see [Zorin, Schröder, and Sweldens](https://graphics.stanford.edu/~dzorin/multires/meshed/). The implementation here is a simpler linear refinement and world-space detail scheme.

### Worker and history boundaries

The controller locks geometry edits while a worker holds its snapshot. New/Open/Quit and explicit cancellation discard the in-flight result. Mode changes, level changes, and worker completions record the entire geometry stack in history. Brush, symmetry, camera, and file actions retain their existing separate responsibilities. Automatic remeshing is triggered by a completed deformation stroke, never a mask-only stroke, and waits for release before sculpting can resume after an in-flight topology change.

History entries pair a geometry snapshot with an action label shared by undo and redo. The document exposes read-only labels and counts for UI feedback. A stroke holds a provisional snapshot until its first actual geometry or mask change, then transfers it into history without another clone; an unchanged stroke discards it and preserves redo. Completed background jobs supply explicit subdivision/remesh labels when the controller commits their results.


## Local adaptive sculpting and Snake

`sculpt/adaptive.rs` provides local conforming edge splits, conservative edge collapses, and quality-improving edge flips. Candidate faces intersect the brush footprint and front-facing dab hemisphere (Snake uses a volumetric tip); masks, pressure, and alpha eligibility restrict work. Every selected edge midpoint is reused across incident faces, and triangles with one, two, or three split edges are triangulated consistently.

Collapses require the closed-manifold link condition, nonduplicated faces, similar masks, smooth edge normals, and no nearby triangle flips. Accepted collapses have disjoint affected vertex neighborhoods. Compaction remaps the entire index array while leaving untouched vertex coordinates intact.

### Density and work budgets

`AdaptiveSettings` is an optional, default-omitted mesh-settings field. Its mode is mutually exclusive with subdivision stacks and voxel mode. Density uses a screen-pixel target converted through the world-space dab radius, or a fraction of brush radius. The document applies refinement/simplification before deformation and refinement after it, using all symmetry-transformed dabs together.

A shared frame budget caps work at 256 edge splits, one batch of at most 32 collapses, eight refinement scans, and two quality scans with at most 128 edge flips. Heavy density changes therefore converge over later freehand dabs. Drag Rect/Dot additionally refine their undeformed footprint before applying displacement, bounded to 8,192 extra splits and 32 scans of at most 512 splits.

Edges crossing a strong alpha transition target half the normal edge length. This lets newly inserted vertices sample the original alpha instead of interpolating an already jagged stamp; preview and commit share this pass. Extremely large stamps or very fine detail may still reach the bound. A provisional stroke snapshot covers topology and deformation together, yielding one undo step.

Continuous density-parameter edits do not fill geometry history; entering/leaving the mode records a geometry edit.

### Frame consistency and Snake

The renderer shares its cached geometry snapshot with picking in each frame. Picks and surface guides use the same geometry snapshot as projected coordinates, preventing stale face/vertex indices when a dab changes topology. Guides are painted before mesh edits.

`Viewport` seeds Snake's handle from the nearest unmasked surface vertex once, keeps its world-space tip, mask mobility, and starting normal for the duration of a stroke, continues dabs outside the original surface, and advances a continuous handle by the pressure/alpha-weighted motion rather than snapping between vertices. `Kind::Snake` supplies alpha/pressure-weighted drag displacement with a small outward lift; subtract reverses it.

Its influence has smooth volumetric support and includes the sides of a bent tip. Soft alpha uses a rigid inner core with falloff applied to the surrounding volume, preventing repeated dabs from pinching every off-center vertex into a needle. Other alphas retain their projected pattern inside that volume.

Adaptive Snake also redistributes nearby vertices tangentially with face-inversion checks, and protects the tip core from collapse and redistribution. Drag interpolation limits Snake steps to a tenth of the brush radius. This is a hook-style deformation brush, without a separate tube-generation or curve system.

For the manifold safety principle, see [CGAL's edge-collapse reference](https://doc.cgal.org/Manual/3.6.1/doc_html/cgal_manual/Surface_mesh_simplification_ref/Function_Surface_mesh_simplification--edge_collapse.html). Clay Form implements its own bounded CPU routines with no added dependencies.

### Adaptive Smooth

Adaptive Smooth with simplification enabled skips edge splits and doubles the ordinary collapse ratio, capped at 95% of the target edge length. Smooth collapses also check resulting triangle quality (a 0.2–0.35 quality floor based on the original face). Quality flips remain available. This prevents cleanup strokes from replacing removed vertices with newly split ones, while preserving the shared work budget and conservative manifold/mask checks. Shift smoothing uses the same path.


## Model scenes and transforms

Transforms prepare new vertex coordinates for the active surface and its stored levels without cloning inactive submodels or rebuilding unchanged topology. Every resulting coordinate is checked before any mutation. The first changed update captures a complete scene snapshot for undo; later updates in the same drag retain the existing face, mask, and group buffers. Parametric primitives still regenerate and validate their recipe before commit. Invalid updates leave geometry, dirty state, and history intact.

### Scene storage and display

`Geometry` snapshots contain the active surface and its subdivision stack plus inactive `Submodel` records and `SceneSettings`. Inactive records retain full geometry; display uses the lowest existing level. The active index describes its position among the ordered inactive records, so selection preserves scene order. Per-model metadata owns visibility and group membership; scene settings own group order. Optional default-omitted project fields preserve the legacy single-model JSON fixture. Model organization and transforms use the same bounded scene history as sculpting.

The viewport caches merged visible display surfaces, normals, and triangle ownership by a session-only document display revision. OpenGL draws model geometry with a depth buffer; CPU picking projects the same immutable geometry snapshot. Active faces alone supply symmetry guides. A pick on an inactive foreground surface cannot sculpt the active model behind it; a selection click waits for release before permitting a new sculpt stroke.

### Transform gestures

`sculpt/transform.rs` provides positive affine scale, translation, directional stretch, and axis-angle rotation around a pivot. Rotation uses `Vec3::rotated`, also shared by radial symmetry. Direction transforms omit translation and keep primitive basis vectors independent of center-coordinate precision.

Soft mask mobility scales a rotation angle, so repeated updates preserve the radius and reversing a drag restores soft-weight positions. `Document::apply_transform` applies increments to all stored active levels, optionally weighted by mask mobility, validates before mutation, and records a single history entry per drag. World handles constrain movement/scale to an axis or provide view-plane movement/uniform scale.

Action Line stores temporary world-space endpoints in the viewport and reuses its start and direction for scale and rotation. World mode has an optional placed pivot; both modes share the same ring implementation in `viewport/transform/rotation.rs`. Ring drags intersect the projected rotation plane, falling back to a latched screen tangent for edge-on views.

Signed incremental angles cross the ±π boundary without jumping; the accumulated angle is shown during a drag. Releasing or losing focus ends the drag. Operation switches preserve pivot/line placement, while document/history resets clear viewport state. Placement is session-only and does not create history or dirty the document.

`ui/transform.rs` renders one shared operation row for the floating toolbar and sidebar, using the existing action/command catalog. The renderer and UI retain no ownership of persistent transforms or organization.


## Parametric primitives

`sculpt/primitive.rs` defines bounded, serializable primitive recipes and deterministic welded triangle generators. Cube axis spans share integer lattice keys; UV surfaces share poles and seams; icospheres refine a convex icosahedron and project vertices onto the unit sphere. Cylinder/cone poles avoid duplicate tip vertices. Positive affine placement lets ordinary transform tools move, scale, or rotate a recipe without baking it.

The removed Human head primitive has no generator or controls. When loading projects or recovery copies, `MeshSettings` drops only its legacy `HumanHead` recipe and preserves the stored mesh as sculptable geometry. Other primitive recipes still deserialize and validate normally; unknown kinds remain errors.

An optional default-omitted `MeshSettings::primitive` keeps the recipe beside its preview surface, including inactive submodels. Validation regenerates the recipe and requires the stored preview to match, with no subdivision stack, masks, or adaptive/voxel mode. Conversion removes only the recipe; the surface stays exact. Document operations gate brush and topology edits until conversion, and parameter drags record one history entry until release.

The UI captures the active model for a modal replacement warning. The collapsible Primitive section within Submodels emits validated updates; viewport handles edit the same recipe. Shape, resolution, wireframe and conversion share one inline section, rendered only for an active primitive. The former parameter window and floating resolution panel are removed. Sidebar input cannot also manipulate the viewport.

Parameter editing and conversion are persistent document operations, while section expansion and handle gestures stay session-only. Legacy projects still omit default primitive fields and retain their original JSON shape.

### Brush icons and library identity

Brush icons use bounded square RGBA buffers shared through `Arc`, stored as compressed PNG data in optional brush fields. `brush_icon.rs` generates default illustrations and renders an auto-fitted capture of the active mesh using the camera basis, interpolated lighting and a depth buffer. The capture modal previews the snapshot before assigning it; it blocks viewport gestures and shortcuts while open.

Brush IDs persist across edits and file exports. Built-in brushes have stable IDs, duplicates receive new random IDs, and legacy brushes derive deterministic IDs from their contents. `Document::import_brushes` validates a complete batch and merges by identity before committing, so errors leave the library intact and repeated imports do not add copies. `.brushes.json` libraries use a versioned wrapper around the same brush schema.

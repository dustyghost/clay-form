# Edges and modeling

[Architecture overview](../architecture.md) · [Documentation home](../../README.md#documentation)

Use this reference when changing these systems. Start with the overview for ownership and the shared edit rules.

## Dynamic Mesh quad reconstruction

Whole-surface quad remeshing reconstructs the active surface from a voxel grid. The user workflow and destructive-operation warnings remain in [Remesh a whole surface](../guide/modeling.md#remesh-a-whole-surface).

| Algorithm | Implementation |
| --- | --- |
| Fast quads | Builds a voxel skin, slightly redistributes its vertices and projects it back to the pre-remesh sculpt. Projection is always included. |
| Optimized quads | A native Rust adaptation of [Practical Quad Mesh Simplification](https://vcg.isti.cnr.it/publication/2010/TPCPP10/). Simplifies and redistributes a coarse quad layout, then subdivides and projects it back onto the reconstructed surface. |

Fast quads retains grid junctions. Sharp or under-resolved regions can retain a partial fit; the result is not guided retopology or animation-ready edge flow.

Optimized quads changes connectivity and may slightly change the face count. If the coarse layout creates severely uneven quads, it tries a less aggressive layout and keeps it only when spacing improves and the same shape checks pass. It takes longer than Fast quads and can soften small features.

Optimized quads checks each disconnected part separately, requires closed quad topology, and checks surface samples in both directions against a 0.4-voxel limit relative to its internal Surface nets reconstruction. This is a sampled bound, not a guarantee at every point. If simplification, shape checks or bounded work cannot complete, it reports failure and keeps the original model.

Both quad methods require valid closed output. Unresolved crossings or folded output report an error and keep the original model; there is no automatic triangle fallback. All remesh algorithms preserve supported vertex colours/materials and transfer vertex groups; quad faces receive a single group assignment. Global voxel remeshing still clears masks and can lose subvoxel details.

## Edge loops, creases, and smooth subdivision

`sculpt/edges.rs` builds polygon incidence from retained quads and remaining triangles. Shared boundary targeting covers open borders, fully masked polygons, visible face categories, and explicit edge selections. `edges/loops.rs` inserts inset quad strips without duplicating the original border, projects new positions onto adjacent source triangles, and interpolates masks and group weights there. Face-orientation checks reject folded local results. Border relaxation fixes junctions, sharp corners, and vertices touching hidden faces.

### Crease storage and selection

`Mesh::creases` is a sorted, unique, default-omitted list of hard polygon edges. Topology operations split or reindex tags; voxel/adaptive modes reject creased meshes until the user clears them. Smooth brush protects creased vertices. Shading fans split at crease edges without disconnecting the sculpt topology. Wire rendering colors creases orange; session-only viewport selection uses cyan overlays and invalidates on document revisions.

Clicking depth-tests candidates against the active visible surface. Edge-selection modifiers take precedence over primary-button zoom/visibility gestures inside the surface; drags beginning in empty space use the standard orbit/pan/zoom rules and retain navigation when crossing the model. The frame and secondary button also remain available.

### Baked smooth subdivision

`edges/smooth.rs` applies Catmull–Clark face, edge, vertex, boundary and hard-crease rules, blended with unsmoothed positions by Smoothness. This is an explicit baked operation using the active level, distinct from the existing linear multiresolution contract. Rules follow [Pixar's subdivision description](https://graphics.pixar.com/library/GPUSubdivRenderingA/paper.pdf); this implementation uses persistent hard creases rather than fractional sharpness levels.

Edge operations use the cancellable mesh worker and one undo transaction; unchanged results do not dirty the document or clear redo. Thickness modifier retains its legacy serialized field name. New default-omitted rim-loop and rim-crease settings generate deterministic shell geometry shared by preview and Apply. Mesh and project size validation applies to the generated loops and smooth subdivisions.


<a id="pen-modeling"></a>

## Modeling

`sculpt/modeling.rs` builds extrusion and loop-cut results without changing the input mesh. Polygon indices come from the shared quad-aware polygon view. Extrusion duplicates the selected flat region, adds walls only on its boundary, and optionally joins a translated cap region to opposite-wound faces with the same layout.

`contact.rs` indexes existing positions and tests candidate depths nearest first. A cap join maps every selected face; intermediate-loop fusion maps only the exterior rim and translates any interior cap vertices. Connected planar regions are processed as units, so internal selected edges never generate walls. Fuse partitions separate selected regions and symmetry copies, resolving their contact outcomes against the original mesh before building the combined result. Every region must retain its original contact after earlier regions are applied; interference cancels the whole edit. Regions sharing only a corner or folding across an edge are rejected. Mixed free/snapped regions are reported explicitly; when distances differ, the numeric amount retains requested travel. The selected target controls the final distance shared by preview and commit; farther pointer travel cannot pass it. A free extrusion reports that no compatible contact was found.

At a matching-face join, `sculpt/modeling/join.rs` follows connected coplanar side patches and verifies full, reversed perimeter coverage before cancelling coincident walls. It reuses the patch boundary vertices, interpolates additional depth vertices on free wall paths, and connects unequal boundary spacing with triangles when necessary.

Adjacent holes therefore merge, and boundary cuts open notches without leaving overlapping side strips or T junctions. `sculpt/modeling/loops.rs` propagates oriented opposite edges through quads and shares interpolated vertices for 1–16 evenly spaced cuts. The seed direction sets the slide parameter consistently around a closed ring. Both preserve interpolated masks/group weights, face labels, hidden state, and surviving creases.

### Selection and private drafts

`viewport/modeling.rs` owns revision-bound component selection, extrusion drafts, and loop-cut drafts. A loop press latches the seed edge; ray/axis projection maps drag travel onto its parameter. Taps stay centered, wheel input adjusts the count, and preview segments are drawn over the unchanged surface until release.

`viewport/modeling/selection.rs` keeps selection strokes in a private draft, samples coalesced pen paths, and latches Replace/Add/Remove at pen-down. Release commits only selection state; cancellation discards the draft. Loop targeting uses the nearest edge of a face for direction.

`sculpt/modeling/selection.rs` follows opposite edges through visible quads for face loops and parallel edge rings, stopping at hidden/non-quad faces and non-manifold branches. Picking uses the active visible surface; vertex/edge targets use screen-space tolerance. The renderer shows a draft mesh while picking remains bound to the original surface.

`document/modeling.rs` checks the captured revision and commits one geometry snapshot on release; no preview touches document history. Face component group assignment carries exact polygon IDs separately from vertex weights, so shared corners cannot label unselected neighboring faces. Edge and vertex assignments retain the mask-based weighting path on a private geometry copy.

### Dissolve and symmetry

`sculpt/modeling/dissolve.rs` validates complete nonbranching quad loops, merges incident face patches, removes their redundant vertices, and reconnects boundary crease paths. Partial, crossing, hidden, border, or face-label-conflicting selections fail before commit.

`sculpt/modeling/symmetry.rs` constructs bijective vertex/polygon maps for the enabled mirror and radial transforms using a scale-relative spatial tolerance and cyclic winding checks. The viewport caches correspondence by display revision and symmetry settings.

Selections expand through these maps; extrusion partitions coplanar regions, loop cuts combine oriented parameters before building a shared quad grid, and dissolve unions corresponding edge sets. Result correspondence is validated before committing symmetric topology changes. A mismatch cancels the whole operation, never just one symmetry side.


### Construction and topology

The expanded modeling workflow shares component/region targets between selection and edits. `sculpt/modeling/transform.rs` transforms selected vertices, propagates displacement through the exact symmetry maps (averaging seam stabilizers), and verifies protected source polygons survive unchanged. `construction.rs` implements connected/individual extrusion, planar inset and exterior boundary chamfers; `contact.rs` finds compatible intermediate cross-sections and reuses swept-wall cancellation. `topology.rs` implements complete-loop sliding, deletion, boundary filling/bridging, diagonal connection, edge splitting and component merging. Concave planar fills and split polygons use earcut triangulation. These are bounded topology operations, not general intersection detection or Boolean repair.

### Component construction

`sculpt/modeling/components.rs` implements edge extrusion/split/collapse, point rings/protrusions, and face-center construction on private meshes. Edge orientation comes from the pressed endpoint and the adjacent face nearest the press anchor; symmetry maps that orientation and rejects conflicting seam choices. Shared edge insertions propagate to neighboring polygons.

A centroid fan preserves every boundary segment when a resulting polygon exceeds four corners, including collinear split points. Point construction orders a closed vertex fan before replacing corners with a shared ring. Every result validates manifold topology and symmetry correspondence, then passes the common mask/visibility protection check.

New component topology clears selection because source component indices no longer describe the result.

### Flat islands

Flat-island targets flood connected visible faces against the seed normal, using a 0–180° angle in `ModelingOptions`; they do not accumulate neighboring normal changes. Selection strokes capture that angle at pen-down. Construction options and the flat angle have serde defaults so older preferences still load.

### Selection transforms

`viewport/modeling/transform.rs` provides selection-local Free and Gizmo interactions. Movement projects onto the view plane, an axis, or a selected world plane; scaling uses axis or uniform factors; rotation shares the existing projected-ring implementation and adds a virtual trackball. Gizmos and geometry use the same symmetry reference vertices and pivot.

Direct drags feed the existing private edit draft and one-revision commit path, carrying a normalized direction and amount into repeat/presets. Mode changes cancel active drags. New sessions default to Free; missing interaction fields in older presets default to Axis to preserve their behavior.

### Starter confirmation

The modeling starter uses the primitive confirmation modal with a separate cube recipe and captured document revision. Confirming creates an editable mesh and resets active mesh settings/temporary Thickness in a single geometry transaction, retaining other submodels. Cancellation never replaces geometry, and stale confirmation is rejected.

### Measured edits, repeat, and presets

`viewport/modeling/edit.rs` owns measured drafts, their actual snapped amounts, cap-selection retention, and repeat. Measured gestures latch movement beyond three screen pixels: a release without a drag applies the visible settings when tap repeat is enabled, while a real drag never falls back to repeat. Tap extrusion uses the same measured result/commit path as numeric extrusion.

Source/result polygon ordering supplies cap correspondence for construction; operations that remove caps clear it. Geometry-preserving edits retain component indices. One smooth preview is cached by document revision, capped at 20,000 source triangles and rendered independently of the picking cage. Tool presets store validated `ModelingOptions` under monotonic IDs in `Preferences`, preserving existing project formats.

Presets have stable `Action` identities shared by buttons, shelves and shortcuts; loading a preset never edits document geometry.

### Exact face group assignment

Face-mode group assignment carries explicit polygon IDs into `Document::group_model_selection`. Symmetry expands faces and vertices separately: vertex weights receive selected corners, while categorical group labels update only triangles belonging to the selected polygons. Mask-based grouping remains available for vertex/edge selections; face selections never infer additional faces from shared corners.

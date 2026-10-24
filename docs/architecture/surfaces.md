# Surfaces, masks, and cuts

[Architecture overview](../architecture.md) · [Documentation home](../../README.md#documentation)

Use this reference when changing these systems. Start with the overview for ownership and the shared edit rules.

## Mask edits and cuts

The Mask menu emits explicit invert/delete commands, gated during topology jobs. Inversion complements active and stored-level masks without changing positions. Deletion removes fully masked triangles at the per-submodel selection threshold (50% by default), retains quads only when their complete triangulation survives, and compacts remaining vertices in their original order.

Surviving masks, model metadata, and inactive submodels are preserved. A cut resets the active subdivision stack and closed-surface modes; one undo snapshot restores the original geometry and settings. Both edits invalidate the GPU display cache.

Deleting all surface faces leaves an empty active model with no vertices. Mesh/project validation accepts this state; rendering and picking handle it, subdivision/remeshing report that there are no faces, and initializing a primitive places it at the origin. Transform handles are hidden for empty meshes. Empty models remain saveable and undoable.



## Thickness modifier

An optional, default-omitted `ModelMetadata::temporary_thickness` stores a total centered shell thickness per submodel. `sculpt/thickness.rs` verifies oriented manifold edges and boundary loops, offsets front/back layers along source normals, reverses the back-layer winding, and joins boundary edges with quad walls. It preserves masks and polygon topology and observes the ordinary mesh-size limits. This is a normal-offset shell, without self-intersection repair.

The renderer caches a separate shell display mesh when thickness is enabled. The CPU scene still uses the original geometry for picking, guides, and controls, so preview faces never introduce invalid sculpting indices. Render-only shells are rebuilt when the document display revision changes and use the existing GPU pipeline. Subdivision LOD selection remains per model. Invalid/inapplicable preview settings on changed topology fall back to the original display surface; Apply validates before changing the document.

Thickness-value drags share one history entry until release. Apply generates the same shell as the preview, clears the setting, converts primitive recipes, and resets the active subdivision/topology settings in one undoable geometry transaction. Default metadata serialization keeps legacy projects unchanged.

### Weighted groups

`Mesh::vertex_groups` stores named dense vertex-weight arrays (maximum 64); `Mesh::polygroups` stores optional 1-based face categories referencing that registry, with zero for ungrouped. Quad halves share one category. Both fields are default-omitted for legacy project compatibility. Registry names must match across subdivision levels. Mask threshold and thickness grouping options belong to `ModelMetadata`.

Document group edits affect the active mesh and its stored levels transactionally, record one history entry, and invalidate display geometry. Mask intensity drags keep a baseline so a drag through zero can recover its falloff. Group weights remain separate from the transient mask.

User-facing assignment replaces prior memberships in the selected region, preserves corners required by remaining categorical faces, and clears obsolete weights when a face group loses its last face. Empty registries are compacted using the union of usage across all subdivision levels, remapping face IDs and the UI target together. Low-level mesh construction retains independent weighted storage.

Midpoint/center subdivision and adaptive edits interpolate weights; cuts compact them alongside geometry. Voxel remeshing uses spatial nearest-neighbor transfer for weights and face centroids for categories. History memory accounting includes the new arrays; saving enforces the same 128 MB cap as loading.

### Mask filters

Mask blur and sharpen use the same transactional document path for the active mesh and each stored subdivision level, including hidden faces. Blur deduplicates polygon edges, excludes quad diagonals, and takes a half step toward each vertex's neighbour mean using the unchanged source mask. Isolated vertices and uniform masks remain unchanged. Sharpen applies `clamp(2 * mask - 0.5, 0, 1)`.

Neither operation changes geometry, groups, visibility, or inactive submodels. Existing subdivision mask propagation applies when switching levels. No-op edits preserve dirty state and redo; shortcut dispatch cancels viewport input and waits for release before resuming sculpting.

### Group visibility

Each vertex group has a default-visible, default-omitted `visible` flag. The display cache removes hidden categorical faces and their quads from both rendering and CPU picking while retaining original vertex indices and normals. Brush edits protect hidden face vertices, including shared boundaries; adaptive collapse and relaxation preserve those boundaries. Visibility edits apply across subdivision levels and use scene history. The default document and New action assign the sphere to a Base group; loading older projects retains their original group data.

### Line and stamp previews

`Brush::stroke_mode` defaults to Freehand and omits that default from serialized data, preserving legacy brush identities and JSON. `viewport/stroke.rs` owns pending Line/Drag Rect/Drag Dot gestures: a brush snapshot, screen endpoints, an anchored surface hit and the last down-frame pressure.

Drag stamps render a temporary mesh through the CPU/GPU scene path, using the same alpha, symmetry, mask and adaptive operations as the final stamp. Picking stays on the original geometry, and every preview starts from that geometry to prevent buildup. Line retains a guide preview. Neither preview touches Document geometry or history; release applies dabs through the ordinary document/symmetry/history pipeline.

Drag Rect's posed radius and rotation are transient, including the radius used for adaptive detail. Relief brushes use a fourfold single-stamp displacement gain; masks and smoothing/flattening/pinching retain their existing weights. Escape, disabled interactions and viewport cancellation discard the pending gesture; the release frame remains blocked so canceled clicks cannot restart a stamp.

Camera gestures and Grab/Snake retain their existing behavior. Line sampling is bounded to 257 dabs per gesture.

### Freehand stabilization

`viewport/stabilizer.rs` optionally filters Freehand input through a screen-space trailing tip before the existing dab spacing and picking path. The distance is latched at stroke start. Stationary input never catches up, and release discards the trailing state without an endpoint dab. Pressure and temporary modifiers remain live inputs to the existing document pipeline. `Brush::stabilize` and `stabilizer_distance` omit their defaults, retaining legacy brush serialization and identities; Grab, Snake, and patterned modes bypass the filter.

### Deformation safety

`sculpt/deformation.rs` checks every sculpt brush proposal before committing positions. Double-precision predicates check the minimum projected face area throughout the movement, including intermediate collapse even when the endpoint is correctly wound. A scale-independent triangle-quality floor prevents new slivers without rejecting existing thin topology outright.

Unsafe incident vertex movements are reduced locally and rechecked together; a bounded fallback rejects the dab if it cannot be made safe. The same path serves fixed and adaptive meshes, symmetry and stamp previews, without changing mask/group data or topology. This prevents new local foldovers; it does not repair existing self-intersections or detect collisions between distant faces.

### Polish sampling

`sculpt/polish.rs` estimates an area- and alpha-weighted local plane for each symmetry dab from visible, unmasked, similarly facing triangles. Polish moves vertices normally toward that plane, preserving marked creases and vertices at sharp normal discontinuities. All samples come from the same pre-dab surface; the shared stamp pipeline handles pressure, falloff, masks, inversion, and overlap blending.


## Slice tools

`sculpt/slice.rs` intersects a view-defined plane with triangle edges, sharing each new intersection across incident faces and interpolating masks and all weighted vertex groups. Distances use double precision and a scale-relative on-plane tolerance. Whole quads survive where their triangles and face categories remain intact. Polygroups splits categories crossing the plane without disconnecting geometry; Trim compacts one half; Separate produces two independently indexed surfaces and inserts the second after the active submodel.

### Cap validation

Capped cuts require a closed, oriented source. Directed boundary edges form loops in the cut plane. Positive outer loops and their contained negative holes feed [Earcutr](https://docs.rs/earcutr/0.5.0/earcutr/fn.earcut.html) for planar triangulation. Near-collinear points are simplified for triangulation, then exact original boundary chains are restored, including points omitted by the triangulator, avoiding T junctions.

Boundary-chain checks use the same scale-relative geometric tolerance along and across an edge, allowing endpoint roundoff on tiny projected edges without dropping their original vertices. Caps receive a new face/vertex group and closed-manifold validation runs before any commit. Invalid boundaries, group/mesh limits, misses, and cancellation return an error without changing the document.

`viewport/slice.rs` owns the session-only line, side labels, and minimum drag threshold. Preview leaves document geometry intact. Geometry, projection, option changes, modal input, and Escape invalidate the pending line. Release emits the plane and settings for `Operation::Slice`; the existing background worker and document replacement path provide cancellation, display invalidation, and a single undo snapshot. A successful cut resets only active mesh settings; scene metadata and unrelated submodels survive. The project format needs no additional fields.

### Path cutting

`sculpt/slice_path.rs` cuts a simple open polyline extruded along the camera depth axis. It rejects intersecting paths (including extended end rays), splits source faces against each segment's supporting plane, then assigns the resulting faces to the nearest oriented path side, using corner bisectors where adjacent segments share their closest endpoint. Supporting planes can add triangulation outside the visible path.

Caps are triangulated in unfolded arc-length/depth coordinates, split at every bend, and folded back to world space. Near-bend coordinates are aligned within a scale-relative tolerance to avoid splitting shared boundary edges twice due to floating-point error. Final closed-manifold validation occurs after folding, before committing. Mesh and group limits and worker cancellation apply throughout.

### Path editing and cancellation

Path drafts store editable points and per-corner quadratic rounding in viewport pixels. The displayed sampled curve is exactly the curve passed to the cutter. Point dragging, segment insertion, deletion, signed turn editing, 15° snapping, and Space translation only modify session state. Selected interior corners expose a diamond rounding handle and linked tangent handles with hit priority over path insertion.

Handle drags update the existing rounding value, bounded by adjacent segment lengths; they never move or insert control points. Sharp/Rounded presets and the numeric control use the same value. Apply/Enter emits `Operation::SlicePath`; drafts survive worker failure/cancellation, while successful geometry changes invalidate them. Mode/side/cap options can change without losing a draft; projection or source revision changes invalidate it.

Preview and input controls do not persist in project files.

### Shell rim safety

Thickness offsets use angle-weighted vertex normals. Near open boundaries, the offset directions are regularized on short edges with bounded, deterministic passes, retaining unit direction lengths and centered thickness. This prevents the mismatched one-sided normal estimates of sliced sliver triangles from folding the inner shell or twisting the rim.

Generated front/back faces must retain their source orientation, and the two triangles of each rim quad must agree in orientation; invalid local folds fail before committing. This remains a normal-offset shell, without general self-intersection repair. Temporary preview and Apply use the same generation path.


## Partial mesh visibility

`Mesh::hidden_faces` stores optional per-triangle hiding independently of masks and categorical polygroups. `face_visible` combines both sources; display geometry filters faces and retains source-face indices for polygroup picking. Sculpting fixes vertices touching any hidden face. Subdivision, adaptive edge operations, thickness, and face extraction carry visibility with their face mappings. Older projects omit the field and remain fully visible apart from existing group visibility.

### Visibility and mask lassos

`viewport/visibility.rs` handles latched rectangle and lasso gestures ahead of zoom and sculpt routing, including Alt, Space and Escape. `SelectionTool` distinguishes visibility rectangles, visibility lassos, and mask lassos; Ctrl+Shift still selects rectangles when no explicit tool is active.

Lasso paths retain pointer samples within each frame, close on release, and use an even/odd point-in-polygon test for concave and crossing outlines. A live outline previews the operation without mutating the document. Degenerate paths are ignored, and paths beyond 4,096 samples are cancelled explicitly. Focus/pointer loss, document revision changes, projection changes, tool switches, and Escape discard the draft.

Mask lassos use transactional `Document::mask_region` edits on every stored level, protecting hidden faces and boundary vertices via `sculptable_vertices`; geometry, groups, and inactive submodels are unchanged.

Region selection uses projected polygon centers through all depths and selects complete quads. Document visibility edits apply the same region to each stored level, normalize visibility consistently, and avoid history entries for unchanged effective visibility. Split Hidden compacts visible/hidden face sets into two meshes and records the entire scene once, preserving attributes and model order while resetting only the split model's topology settings.

### Hide/Isolate lines and closed paths

`Preferences::visibility_selection` retains shape, Hide/Isolate, and side inversion independently of the active tool. Explicit visibility settings do not reinterpret Alt, allowing any configured hold chord. `Mesh::face_region_selection` supplies identical whole-polygon classification for the red face preview and `Document::select_face_region`; line boundaries include zero distance before inversion. Each center is projected at its own depth in perspective views.

`viewport/visibility_path.rs` owns a session-only closed polygon with up to 64 editable points. It uses the same even/odd boundary-inclusive region as the lasso, including concavity and self-intersections. Apply/Clear/Delete point have reusable configurable actions and visible buttons. Preview, point movement, and menu interaction do not enter document history. The draft is invalidated by camera/revision/tool changes, focus loss, cancellation, and release of the temporary hold; no draft survives to reappear on a later hold. These workflows preserve topology, masks, groups, inactive models, and stored subdivision levels.

### Merge Down

`Mesh::merged_with` concatenates indexed surfaces, offsets face/group references, and extends each independent group's weights with zeroes over the other surface. `Document::merge_down_target` uses the panel's folder order. Merge Down prepares and validates both surfaces before installing the combined model, bakes recipes/thickness, preserves the active identity, and records the scene once for undo.

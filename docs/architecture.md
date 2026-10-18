# Architecture

[Documentation home](../README.md#documentation)

Start here to find the owner of a change. Follow the linked references when you need an algorithm, validation rule, or interaction contract.

Clay Form uses immediate-mode egui, one application controller, and one document owner. It has no MVC framework, ECS, event bus, or plugin layer.

The main paths through the app are:

- **Controls:** UI widgets send commands to the application controller, which coordinates document edits, file operations, and tablet input.
- **Sculpting:** viewport gestures send sculpt dabs to the document, which uses the brush and mesh algorithms and records history.
- **Navigation:** viewport gestures update the camera.
- **Display:** rendering and picking read the document’s mesh data.

## Ownership and responsibilities

| If you are changing… | Start in… | Keep in mind |
| --- | --- | --- |
| Commands, files, or background jobs | `app.rs`, `app/`, `command.rs` | The controller coordinates; UI widgets emit commands. |
| Persistent scene state or edits | `document.rs`, `document/` | Document content is private. Views receive read-only data. |
| Undo and redo | `history.rs` | History is bounded to 24 entries and 128 MB. |
| Brushes, topology, or validation | `sculpt/` | No egui, filesystem, or tablet dependencies. |
| Project files, image alphas, or OBJ export | `persistence.rs` | Preserve version 1 compatibility. |
| Subdivision or remesh jobs | `mesh_job.rs` | Compute privately; commit through the controller. |
| Navigation or projection | `camera.rs`, `viewport/projection.rs` | Camera state is session-only. |
| Gestures, picking, or drawing | `viewport/` | Picking and rendering share one immutable mesh snapshot per frame. |
| Controls and menus | `ui/` | File and mesh operations belong outside widgets. |
| Shortcuts, layouts, or workspaces | `preferences.rs` | Preferences are separate from the project. |
| Pen input | `tablet.rs`, `tablet/linux.rs` | Device workers stop and join when dropped. |

Desktop startup lives in `main.rs`. See [integration details](architecture/integration.md#ownership-and-responsibilities) for ownership, symmetry blending, projection, and file-dialog behavior.

## Rules to preserve

- **One gesture, one undo step.** A stroke or continuous parameter edit records history on its first real change. No-op edits preserve redo and dirty state.
- **Validate before changing the document.** Invalid geometry, failed loads, cancelled workers, and stale drafts leave the existing document intact.
- **Preview privately.** Drafts and modifier previews do not mutate persistent geometry. Commit through a document method.
- **Keep indices in sync.** Display, picking, and surface guides use the same frame snapshot, including during adaptive topology changes.
- **Keep navigation separate.** Camera changes and temporary handles do not dirty the document or create history.
- **Respect protected geometry.** Hidden faces and their boundary vertices remain protected during edits; each algorithm's reference describes its additional checks.

## Extending the app

1. Put geometry data and algorithms in `sculpt/`.
2. Add the persistent operation to `Document`.
3. Route the user operation through a command handled by `App`.
4. Expose settings in `ui/` and gesture handling in `viewport/` as needed.
5. Check no-op, cancellation, undo, and invalid-input behavior alongside the successful edit.

For a reusable shortcut or custom control, also add an `Action`, catalog label, availability check, and dispatch mapping. **Append to `Action::ALL`**: the first 19 positions currently determine default shortcuts. See [custom interface actions](architecture/integration.md#custom-interface-actions) for the complete registration and preference rules.

Introduce a trait when a second implementation needs one. Current concrete modules are enough for the existing app.

## Compatibility and storage

The `.clay` and `.brush.json` formats remain version 1. Optional fields omit defaults, keeping older single-model files valid. Camera and undo state are never saved in a project; preferences use an independent versioned JSON file.

Use `tests/fixtures/v1-project.json` to check legacy compatibility. See [storage details](architecture/integration.md#compatibility-and-storage) for references, shared image buffers, and fixture coverage.

## Find the detailed contract

These references retain the implementation limits, data mappings, and failure behavior needed for changes in each subsystem.

| Reference | Read it when working on… |
| --- | --- |
| [Editing and history](architecture/editing.md) | Edit sessions, subdivision, remesh, adaptive sculpting, Snake, scene transforms, primitives, brush libraries |
| [Surfaces, masks, and cuts](architecture/surfaces.md) | Mask deletion, thickness, weighted groups, stroke previews, deformation safety, slices, visibility, merging |
| [Edges and pen modeling](architecture/modeling.md) | Edge loops, creases, smooth subdivision, component selection, construction, symmetry, modeling drafts and presets |
| [Application integration](architecture/integration.md) | Ownership, symmetry, projection, files, references, preferences, GPU lifecycle, custom menus and actions |
| [GPU rendering](gpu-rendering.md) | Render passes, offscreen correctness tests, screenshot exports, performance profiles |

<!-- Keep existing inbound section links usable after moving the detailed contracts. -->

## Continuous edit sessions

[Session boundaries, baseline masks, and history rules →](architecture/editing.md#continuous-edit-sessions)

## Mesh workflows

[Subdivision detail transport, voxel remeshing, worker commits, and history labels →](architecture/editing.md#mesh-workflows)

## Local adaptive sculpting and Snake

[Topology budgets, manifold checks, stamp refinement, and Snake behavior →](architecture/editing.md#local-adaptive-sculpting-and-snake)

## Model scenes and transforms

[Scene ordering, display revisions, protected picking, and transform gestures →](architecture/editing.md#model-scenes-and-transforms)

## Parametric primitives

[Recipe validation, conversion, brush icons, and library identities →](architecture/editing.md#parametric-primitives)

## GPU viewport

[Buffers, invalidation, shader agreement, and GL resource lifecycle →](architecture/integration.md#gpu-viewport)

## Mask edits and cuts

[Mask inversion, deletion, and empty-model behavior →](architecture/surfaces.md#mask-edits-and-cuts)

## Thickness modifier

[Shell generation, preview picking, and Apply →](architecture/surfaces.md#thickness-modifier)

### Weighted groups

[Weight/category storage, visibility, stroke modes, and deformation safeguards →](architecture/surfaces.md#weighted-groups)

## Slice tools

[Plane/path cuts, capping, rounding drafts, and shell rims →](architecture/surfaces.md#slice-tools)

## Partial mesh visibility

[Rectangle/lasso drafts, level propagation, Split Hidden, and Merge Down →](architecture/surfaces.md#partial-mesh-visibility)

### Custom interface actions

[Menus, sidebars, workspaces, shortcut registration, and gesture priority →](architecture/integration.md#custom-interface-actions)

## Edge loops, creases, and smooth subdivision

[Edge targeting, crease propagation, Catmull–Clark, and transaction rules →](architecture/modeling.md#edge-loops-creases-and-smooth-subdivision)

## Pen modeling

[Selection, construction, joins, symmetry, transforms, measured drafts, and presets →](architecture/modeling.md#pen-modeling)

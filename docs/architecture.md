# Architecture

[Documentation home](../README.md#documentation)

Start here to find the owner of a change. Follow the linked references when you need an algorithm, validation rule, or interaction contract.

Clay Form uses immediate-mode egui: controls are described each frame. One application controller coordinates actions, and one document owner holds persistent scene data.

There is no MVC framework, ECS, event bus or plugin layer. Start with the ownership table below rather than looking for those abstractions.

The main paths through the app are:

- **Controls:** UI widgets send commands to the application controller, which coordinates document edits, file operations, and tablet input.
- **Sculpting:** viewport gestures send sculpt dabs to the document, which uses the brush and mesh algorithms and records history.
- **Navigation:** viewport gestures update the camera.
- **Display:** rendering and picking read the document’s mesh data.

## Ownership and responsibilities

| If you are changing… | Start in… | Keep in mind |
| --- | --- | --- |
| Commands, files, or background jobs | `app.rs`, `app/`, `command/`, `background_job.rs` | `app/policy.rs` shares availability rules between controls and dispatch; feature handlers execute after edit-boundary checks. |
| Persistent scene state or edits | `document.rs`, `document/` | Document content is private. Views receive read-only data. |
| Undo and redo | `history.rs` | History is bounded to 24 entries and 1024 MiB. |
| Brushes, topology, or validation | `sculpt/` | Scene data lives in `sculpt/geometry.rs`; subdivision algorithms live in `sculpt/subdivision.rs`. |
| Project files, image alphas, or OBJ export | `persistence.rs` | Preserve version 1 compatibility. |
| Subdivision or remesh jobs | `mesh_job.rs` | Compute privately; commit through the controller. |
| Surface relaxation | `sculpt/relaxation.rs`, `app/projection.rs`, `document/projection.rs` | Fixed local reference fans, protected features, bounded cancellable work, shared preview/apply lifecycle. |
| Detail projection | `sculpt/projection.rs`, `document/projection.rs`, `app/projection.rs` | Preserve reference submodels; preview privately and commit once. |
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

For a reusable shortcut or custom control, also add an `Action`, catalogue label, availability check and dispatch mapping. Declare default shortcuts explicitly in `Action::default_shortcut`; catalogue order does not determine bindings.

See [custom interface actions](architecture/integration.md#custom-interface-actions) for the complete registration and preference rules. The result should be a command that follows the same availability, preference and dispatch rules as existing controls.

Introduce a trait when a second implementation needs one. Current concrete modules are enough for the existing app.

## Compatibility and storage

New `.clay` projects use version 3 for UVs and texture images; versions 1 and 2 remain readable. The `.brush.json` format remains version 1. Optional fields omit defaults, keeping older single-model files valid. Camera and undo state are never saved in a project; preferences use an independent versioned JSON file.

Use `tests/fixtures/v1-project.json` to check legacy compatibility. See [storage details](architecture/integration.md#compatibility-and-storage) for references, shared image buffers, and fixture coverage.

## Find the detailed contract

These references retain the implementation limits, data mappings, and failure behavior needed for changes in each subsystem.

| Reference | Read it when working on… |
| --- | --- |
| [Editing and history](architecture/editing.md) | Edit sessions, subdivision, remesh, adaptive sculpting, Snake, scene transforms, primitives, brush libraries |
| [Surfaces, masks, and cuts](architecture/surfaces.md) | Mask deletion, thickness, weighted groups, stroke previews, deformation safety, slices, visibility, merging |
| [Edges and modeling](architecture/modeling.md) | Edge loops, creases, smooth subdivision, component selection, construction, symmetry, modeling drafts and presets |
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

[Buffers, invalidation, shader agreement, and wgpu resource lifecycle →](architecture/integration.md#gpu-viewport)

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

<a id="pen-modeling"></a>

## Modeling

[Selection, construction, joins, symmetry, transforms, measured drafts, and presets →](architecture/modeling.md#pen-modeling)

## Interaction and worker ownership

The viewport retains its renderer, path editor, model selection, and reference overlay. Temporary gesture state belongs to `viewport/interaction.rs`; cancelling drops that session and ends the document stroke. Stroke buffers and previews belong to the nested sculpt session.

Mesh, projection, and path workers use `BackgroundJob<T>` for cancellation, panic reporting, and completion. Mesh cancellation keeps the worker tracked until it finishes, preventing overlapping cancel/retry jobs. Mesh workers return `ValidatedGeometry`; document commits consume that result without rebuilding subdivision levels on the UI thread. Synchronous document operations still validate before committing.

History caches settings allocation estimates per entry and counts shared alpha/icon buffers once across undo and redo. Estimates exclude allocator overhead and the live document; diagnostic serialization retains separate export size limits.

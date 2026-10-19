# Application integration

[Architecture overview](../architecture.md) · [Documentation home](../../README.md#documentation)

Use this reference when changing these systems. Start with the overview for ownership and the shared edit rules.

## Ownership and responsibilities

| Module | Responsibility |
| --- | --- |
| `main.rs` | Create the desktop window and application. |
| `app.rs`, `app/`, `command.rs` | Own the application frame; resolve actions, dispatch commands, coordinate files and background jobs in separate modules. |
| `document.rs`, `document/` | Own persistent state and history; implement scene, mesh, masks, attributes, brushes, strokes, primitives, thickness, and transforms in focused modules. Expose read-only content to views. |
| `history.rs` | Keep geometry snapshots within 24 entries and a 128 MB budget, undo/redo, and invalidate redo on a new mesh edit. |
| `sculpt/` | Geometry math, brush/alpha data, mesh deformation, project schema and validation. No egui, files, or tablet dependencies. |
| `persistence.rs` | Read/write version 1 JSON, import image alphas, and export OBJ. |
| `mesh_job.rs` | Compute subdivision/remesh on a cancellable background worker; deliver results for transactional controller commits. |
| `camera.rs` | Session-only orbit, pan, zoom, projection, field of view and camera basis. |
| `viewport/` | Lock a drag into sculpt/orbit/gizmo mode; translate pointer movement into camera changes or dabs. Rendering/picking, the gizmo, and symmetry surface guides each have their own module. |
| `preferences.rs` | Validate shortcut assignments and tool layout; load/save independent user preferences. |
| `ui/` | Render toolbar, brush controls and confirmation dialog; emit commands without file or mesh operations. |
| `tablet.rs`, `tablet/linux.rs` | Publish normalized pressure and diagnostics; discover/read Linux devices in a worker that stops and joins when dropped. |

The application owns the document, camera, UI state, viewport gesture state and tablet service. Document content stays private. Mesh edits go through document methods; camera changes never dirty the document or create undo entries. Each stroke records one geometry snapshot regardless of its number of dabs. Symmetry belongs to document editing rather than the renderer.

`sculpt/symmetry.rs` transforms dabs across mirror planes and radial sectors around an object-space center. The mesh evaluates all copies against one geometry snapshot and blends their displacement directions by influence, normalized to the strongest per-vertex influence. Opposing mirrored motion cancels at the symmetry seam, avoiding sideways drift and folds without multiplying brush strength.

Standard blends the direction separately from depth, retaining the strongest pressure/mask-weighted displacement length so diverging normals do not weaken the seam. For soft Standard alphas, `sculpt/overlap.rs` joins closely overlapping footprints along short segments through their influence-weighted centre, bounded by the contributing centres and tapering the correction by footprint spread and contribution.

It fills the central falloff valley up to the usual pressure/mask-weighted peak without accumulating depth; coincident and separated copies keep their profiles, and image/hard/square alphas are unchanged. Other tools retain displacement averaging (including intentional cancellation for mirrored Grab motion). Mask opacity retains the strongest influence.

The renderer independently intersects those symmetry planes with visible surface triangles for guides.

### Projection and picking

`viewport/projection.rs` shares projection, depth-aware screen scale, rays, and control placement. Perspective fits the eye distance to the visible scene when changing projection and uses near clipping and a scene-sized far plane; zoom enlarges the projection without moving the eye through the sculpt. Projection/FOV commands preserve visible-model framing by adjusting zoom once, rather than reframing during sculpting.

GPU surfaces and wireframe use homogeneous projection; CPU perspective picking intersects rays with the same cached triangles and interpolates world-space normals. The CPU-only test painter omits triangles crossing the near plane; the production GPU renderer clips them, and picking supports them. Perspective path cuts converge at the eye and unfold caps through the same projection.

The viewport is an input adapter. It emits domain `Dab` values to the document, independent of mesh rendering details. The scene renderer reads mesh data and prepares projected triangles for display and surface picking. The wgpu renderer uses cached GPU geometry; sculpt algorithms remain in the document and geometry modules. CPU projection and picking share the cached surface with the renderer.

### File actions and confirmation

UI commands are executed between panels so the viewport sees the latest document. Destructive operations capture their target when requested. An Open confirmation therefore retains its target even if the toolbar path changes. Save-and-continue uses a separate path and refuses to overwrite that target. Failed loads leave content, dirty state and history intact.

Open and first-time Save use a modal project file chooser; Save As always uses it and requires explicit confirmation before overwriting. Directory reads and writes stay in the application controller, while the UI holds listing and draft path state. An unsaved-change continuation can survive the chooser; cancellation leaves both the pending decision and document intact.

A session-only document flag distinguishes a suggested filename from an established save destination.


## Compatibility and storage

Screen reference images belong to `Document` and the optional project `references` collection, separately from geometry snapshots. `reference.rs` bounds and decodes PNG/JPEG input, applies photo orientation, and shares decoded pixels and embedded PNG bytes through `Arc`. Saving and recovery reuse those bytes; legacy projects omit the empty collection.

Reference edits mark the document modified without changing sculpt history or display geometry revisions. `viewport/references.rs` caches egui textures and previews move/resize gestures until release; cancellation leaves the document intact. The shared References menu and action catalog route file access and document updates through the controller.

The version 1 `.clay` and `.brush.json` formats remain compatible. Projects support optional `mesh_settings` (inactive levels and dynamic settings) and `symmetry` fields; defaults are omitted when saving, and old files load at one subdivision level with symmetry disabled. Vector positions remain JSON triples, and image-alpha pixels remain JSON arrays. Pixels are shared immutable `Arc<[f32]>` buffers in memory, making brush copies inexpensive without changing serialization. Mesh history is session-only, as before. The saved project does not include camera or undo state.

`tests/fixtures/v1-project.json` is an explicit version 1 compatibility fixture. Tests cover format preservation, brush/mask behavior, disk persistence, bounded history, stroke undo/redo, failed opens, save/open protection, and navigation gestures.


## Extending the app

Add brush data and algorithms in `sculpt/`, then expose their settings in `ui/`. Add user operations as commands handled by `App`; put their document effects in `Document` methods. Keep device-specific logic under `tablet/`, and keep render or picking changes under `viewport/`. Introduce a trait or a new subsystem when a second implementation needs it; the existing concrete modules are sufficient for the current app.

User preferences use their own versioned JSON file in the user configuration directory. UI edits bindings and layout without touching the document. The controller loads preferences at desktop startup, saves after a debounce and on exit, and translates keyboard actions into existing commands. Viewport options carry the chosen hold modifiers. Tests use explicit temporary paths and do not read or write the user configuration.


## GPU viewport

`viewport/gpu/state.rs` defines renderer-neutral view and preview inputs. `viewport/gpu.rs` owns wgpu pipelines, packed vertex/index/face-color storage buffers, lazy wireframe vertex buffers, an R32Float brush-alpha texture, and viewport-sized RGBA8/Depth32Float targets. `gpu/display.rs` builds the unchanged camera-independent display geometry and edge adjacency; `gpu/viewport.wgsl` performs projection, lighting, masking, front-facing edge rejection, and surface alpha/symmetry projection. An `egui_wgpu` callback encodes the offscreen passes during prepare, then composites color during egui’s render pass, before overlays. The target resizes in physical pixels; egui applies viewport/scissor clipping. `gpu/composite.wgsl` preserves display-RGB shading on both linear and sRGB swapchain formats.

Geometry-changing edits, mask edits, model selection/visibility, undo/redo, topology operations, and New/Open advance the display revision. Brush settings, symmetry settings, and camera changes do not invalidate geometry buffers. The CPU scene retains projected coordinates and front-facing ownership for picking and gizmos, without sorting/shading faces or rasterizing a wireframe depth buffer in the GPU path. The CPU paint path remains available for headless interaction tests.

`App::with_wgpu` initializes resources from eframe’s render state. Gesture cancellation preserves the renderer; `App::on_exit` releases cached resources, and wgpu reference counting handles callback lifetimes without context-dependent deletion. GPU alpha uses explicit bilinear texture loads, preserving rotation, image interpolation, inner core and falloff without requiring float32 filtering support. Alpha and symmetry colors apply to active surfaces in the solid shader, so inactive foreground models occlude them. Tangent rings and the normal indicator remain egui controls. See [GPU rendering](../gpu-rendering.md) for offscreen checks and the load profile.

## Custom interface actions

`ui/menus.rs` shares built-in menu controls between toolbar popups, pinned windows and sidebar sections, using stable `BuiltinMenu` identities. Preferences and named workspaces store optional `PinnedMenu` records; old settings default to no pins. Pinned windows use the saved current position, constrain themselves to the screen, and remain available with the compact toolbar. Closing or unpinning removes only that record.

Menu-only close requests leave floating controls open. Their rectangles block viewport gestures, including on the frame a window is closed; moving or pinning controls never edits the document.

### Sidebar placement

`ui/sidebar.rs` renders whole built-in menus as collapsible sections and provides header movement options plus a shared order editor. Preferences and workspaces store `MenuSection` records (menu, side and expansion) and stable `SidebarSection` identities interleaving built-in tools, custom menus and converted menus.

Absent ordering preserves legacy custom-first/panel-vector order; reordering swaps adjacent visible sections on the same side, leaving the other sidebar unchanged. Docking a pinned menu removes its floating record, and undocking leaves its controls available from the toolbar. New fields default empty when reading older layouts. The sidebar receives live camera zoom for the shared View controls.

### Gesture priority

Gizmo gestures latch the axis under the press and reserve a 6-pixel movement tolerance for taps. Ordinary orbiting starts only after crossing that threshold, applying accumulated motion once and then tracking frame deltas. The drag decision stays latched until release, so a drag returning to its start cannot snap accidentally. Modified pan/zoom gestures retain their existing direct movement.

### Action dispatch

`action.rs` provides persistent action identities shared by key bindings and custom controls. `Command::Action` resolves through `App::execute_action` against the current document, then uses the existing command dispatcher, undo, confirmation, and mesh-job checks. `UiState::action_enabled` also drives custom button availability. Brush-specific selections resolve persistent IDs rather than library indices.

### Workspace persistence

`ui/customization.rs` owns the action catalog, source-button context menus, key capture, and the menu/shelf editor. Preferences store ordered items, separators, labels, placement, floating positions, collapsed menu states, named workspaces, per-workspace appearance, and a monotonically increasing menu ID. The active named workspace synchronizes before switching and saving, so edits survive switches.

Workspace snapshots exclude key bindings. `LeftPanel`/`RightPanel` embed custom sections into the built-in sidebars; `Left`/`Right` remain separate edge shelves. Background and header context menus queue edits until the settings pass, preserving source-button hit testing. Custom item removal affects only that menu; deleting a menu keeps global bindings available for other workspaces that contain it.

Workspace appearance updates egui visuals and both CPU/GPU viewport backgrounds. Old preferences gain new unbound actions without losing existing bindings, explicit unbindings, or dock layouts. Number-row normalization is shared by shortcut capture and dispatch.

### Register an action

To add a reusable operation, add an `Action`, its label/catalog entry, availability and dispatch mapping, then attach `.customize(action)` to its source response. Append new entries to `Action::ALL`: `Preferences::default` currently assigns the first 19 shortcuts by catalog position, so inserting an action into that prefix shifts existing defaults. New actions are unbound unless a default is deliberately added.

Confirmation buttons and per-object numeric editors keep their local context. Run the preference and application tests for key/layout behavior; the optional Vulkan test `custom_interface_offscreen_preview` renders the editor and key settings. Set `CLAY_CUSTOM_UI_PREVIEW=/tmp/clay-custom-ui` to export PNGs.

# Clay Form

A Rust desktop sculpting prototype built with eframe/egui. Start from a sphere and sculpt real mesh vertices with a pen or mouse.

## Run

Install a current stable Rust toolchain, then:

```sh
cargo run --release
```

Linux requires a working Wayland or X11 desktop and desktop OpenGL 3.3 or newer. The model viewport uses cached GPU buffers, depth-tested rendering, shader lighting, and surface alpha/symmetry overlays; egui draws the interface. The first build downloads Cargo dependencies. `Cargo.lock` is committed for reproducible application builds.

## Sculpting

- Left drag or pen tip on the clay: sculpt. Start a left/pen drag in empty space to orbit; it stays an orbit gesture even if you cross the clay. Right drag: orbit. Middle drag: pan. Wheel: zoom.
- The top-right orientation gizmo shows positive and negative X/Y/Z axes. Drag it to orbit, or tap an axis to snap to that view. Taps tolerate up to 6 screen pixels of mouse/pen movement and keep the axis you first pressed; larger drags orbit. Dimmed axes point away from you.
- **Alt + left/pen drag pans the view when starting off the model.** Starting on the model sculpts, with Alt reversing the brush as usual. Alt + right drag also pans; releasing Alt during a navigation drag returns to orbiting. Shift temporarily uses Smooth.
- Standard, Clay, Inflate, Smooth, Flatten, Pinch, Crease, Grab, Mask, and Snake are included.
- Every brush uses an alpha, including Grab, smoothing and masking. Add/Subtract reverses its operation; subtracting Smooth sharpens, subtracting Flatten exaggerates surface relief, and subtracting Mask erases masking.
- Adjust radius, inner radius, strength, alpha falloff, spacing, rotation, pressure strength/radius mapping, and pressure response curve. Inner radius is a fraction of the outer radius: Soft and image alphas keep full influence inside this core, then fade toward the edge; image pixels still modulate the core. Hard and Square retain their hard edges. Brush thumbnails project the effective alpha in blue over the illustration or captured model, updating with rotation and falloff. Enable **Surface footprint and normal** in Brush settings to preview alpha on the visible surface, with tangent outer/inner rings and a surface-normal indicator showing inclination. The tangent rings indicate the local surface plane; the shaded footprint follows curvature and the brush’s camera-plane projection. Grab moves the surface with the cursor; spacing does not affect Grab.
- Choose Soft, Hard or Square alpha, or load a PNG/JPEG up to 2048 × 2048 pixels. White affects the surface, black protects it, and transparency reduces influence. Custom alpha pixels are embedded in presets and projects.
- Every brush has an icon. In Brush settings or the Brush library, choose **Capture model icon** to preview a square capture of the active model at its current viewing angle. The capture fits the whole model and excludes controls and overlays. Choose **Use as brush icon**, optionally **Save PNG**, or cancel. **Restore default icon** returns to the built-in illustration. Custom icons are embedded in projects and exported brushes.
- Duplicate any brush to create a named variation. In **Presets**, use **Export brush / Import brush** for one brush, or **Export library / Import library** for the whole library. Persistent brush IDs let imports update matching brushes without adding copies; duplicate creates a new ID. Importing an unchanged library leaves the document unchanged. Older presets without IDs receive a stable identity from their contents; edited legacy files may be imported as new brushes.
- Enable **Symmetry** in the toolbar, then open **Symmetry settings**. Mirror across X/Y/Z independently or combine planes. Radial symmetry adds 2–16 repeats around X, Y, or Z; mirrors and radial repeats can be combined.
- Adjust the symmetry center in object coordinates, or reset it to the origin. Surface lines show the active planes and radial boundaries as you orbit: X pink/red, Y green, Z blue, other radial boundaries gold. Guide visibility and opacity are adjustable.
- Symmetry settings are saved in projects; older projects open with symmetry disabled. Overlapping symmetry copies use the strongest influence per vertex, preventing stronger strokes at the center seam.
- The **Mask** menu provides **Clear mask**, **Invert mask**, and **Delete masked**. Invert swaps protected and editable regions using `1 − mask`, including soft values and every stored subdivision level. Delete removes faces whose vertices all meet the selection threshold (50% by default) from the active model, compacts unused vertices, and preserves surviving masks and complete quads. Cutting resets the active subdivision stack and turns off adaptive/voxel modes and automatic remeshing because the result may have open boundaries. Deleting a fully masked model leaves an empty model; you can undo, initialize a primitive, or choose New. Other submodels are kept. Each operation can be undone; deletion restores all previous levels and mesh settings on undo.
- Undo/Redo keeps up to 24 geometry edits within a 128 MB snapshot budget.

## Mesh management

Use the **Mesh** menu or the **Mesh management** panel. New spheres use an all-quad cube-sphere surface with 4,706 vertices and 4,704 quad faces, leaving room for subdivision and avoiding triangle fans at the poles. Opening older projects preserves their geometry.

**Subdivision:** add a level with Subdivide, then select a level with the slider or Lower/Higher buttons. Each face splits into four triangles with shared edge midpoints, preserving the existing shape. This is linear subdivision rather than a smoothing subdivision surface. Lower-level edits propagate upward while retaining stored fine detail; sculpting higher levels updates their parent vertices when you step down. Mask values interpolate between levels. Subdivide from the highest level. **Delete lower** removes every level below the active level, making it the new level 1 while retaining higher detail. **Delete higher** removes every level above it, allowing a fresh subdivision from the current surface. Both preserve the active geometry and masks and can be undone. The active mesh and all stored levels save in `.clay` projects. OBJ export writes the active level.

**Dynamic mesh:** switch modes to keep the active surface as a single mesh, discarding stored subdivision levels. Choose resolution (16–96 voxels along the longest bounding-box axis) and click Remesh now. Voxel reconstruction redistributes geometry, joins overlapping outward-oriented closed shells, and preserves separated volumes. It requires a closed, consistently oriented mesh. Optional Remesh after each stroke runs when a geometry stroke ends; painting masks does not trigger it. This is whole-surface voxel remeshing, rather than local adaptive triangulation while the brush moves.

Remeshing clears masks and can remove features thinner than a voxel. Start with a moderate resolution and raise it for detail. Subdivision and remeshing run in background workers; sculpt input pauses during the operation. Cancel keeps the original mesh. Mode switches, level changes, strokes, and completed topology operations can be undone, including restoring discarded levels. Failed operations leave geometry intact; a failed automatic remesh disables the automatic setting so you can adjust it before retrying.

## Files

Type a relative or absolute file path in the toolbar, then use Save or Open. `.clay` projects are versioned JSON containing geometry, masks, and the brush library. Presets use `.brush.json`; libraries use `.brushes.json`. Existing files at the selected save path are replaced. Saving uses a temporary sibling file followed by rename. Parent folders must already exist.

New, Open and closing the window prompt when the project has unsaved changes. The prompt has a separate save path, protecting the file you intend to open. Export OBJ writes a mesh beside the project path, using the `.obj` extension. Projects restore the sculpt and presets; camera position and undo history are session-only. OBJ import adds a new submodel through the Submodels menu.

### OBJ import

Open **Submodels**, enter a file in **OBJ path**, and click **Import OBJ**. The importer adds and selects a new sculptable submodel, keeping the existing scene, brush settings, and project save path. Coordinates, scale, winding, and shared vertex indices are preserved; import does not center, resize, weld, or move the source. Multiple OBJ objects are combined into this one submodel. The filename supplies a unique submodel name. Import runs in the existing cancellable mesh worker and creates one Undo step; errors and cancellation keep the original scene. **Import OBJ at path** is also available in custom menus and configurable shortcuts.

Triangles and quads are supported, with quads retained for topology tools. Simple planar n-gons (up to 256 corners) are triangulated, including concave polygons. Degenerate, self-intersecting, and non-planar n-gon faces are rejected with a line number; triangulate these in the source application. Positive/negative indices, UV/normal index forms, comments, line continuations, and UTF-8 names are supported. The parser follows the polygonal syntax in the [original Wavefront Appendix B1](https://github.com/mokiat/java-data-front/blob/master/documents/OBJ%20Specification.txt).

OBJ `g` names become vertex groups with full weights on their member vertices. If a face belongs to multiple groups, all memberships are retained as vertex selections; its first group supplies the face category/color. Long group names are shortened with unique suffixes. UVs, supplied normals, materials, smoothing flags, and vertex colors are not retained; normals are generated from the sculpt surface. External material files are not opened. Point/line-only and free-form geometry are unsupported and rejected rather than partly imported.

Limits: 128 MB per file, 500,000 vertices, 1,000,000 triangles after triangulation, 64 groups, and 64 scene submodels. Ordinary exported Clay Form surfaces can be imported with their coordinates, quad topology, and face groups; exported group names use their existing safe OBJ spelling. Projects and recovery copies include the imported geometry normally.

### Recovery saves

**Settings → Recovery saves** controls automatic recovery (enabled by default, every **2 minutes**, keeping the newest **5 copies across projects**). Choose an interval from 1–60 minutes and retain 1–20 versions. Copies include the entire project, including never-saved work, submodels, masks, groups, subdivision levels, primitives, brushes, and symmetry. Snapshots wait for strokes, parameter drags, transforms, and background mesh operations to finish. Serialization and disk writing run in a background worker; unchanged content does not consume another version.

Recovery files live beside preferences in `~/.config/clay-form/recovery/` (or `$XDG_CONFIG_HOME/clay-form/recovery/`). They never change the manual project path or clear its modified state. Writes use flushed temporary files and atomic replacement; incomplete temporary files are ignored. A failed write leaves earlier copies available and shows a persistent error with access to recovery settings. Automatic retries use the configured interval.

After an interrupted session, startup offers retained copies, newest first. **Recover** restores a modified project with the save path `recovered.clay`; choose your desired path before saving. Existing work gets the normal Save/Discard/Cancel prompt. **Continue without restoring** keeps the copies available under Settings. Successfully saving a project, or deliberately discarding it on New/Open/Quit, clears that session's copies. Recovery preferences are global, independent of workspaces. **Recovery saves** and **Save recovery now** are also available in custom menus and configurable shortcuts.

## Huion tablets

The tablet's normal desktop driver provides pen cursor movement and tip/button input. On Linux, Clay Form additionally discovers devices named Huion under `/sys/class/input` and reads normalized `ABS_PRESSURE` from their `/dev/input/event*` device without taking exclusive control. The sidebar shows connection errors and live pressure. Native touch/pen pressure events are also accepted where eframe provides them. Otherwise strokes use full pressure.

For the connected GS1331 (`256c:006d`), install the included device-specific rule from a normal terminal:

```sh
sudo install -m 0644 packaging/linux/72-clay-form-huion.rules /etc/udev/rules.d/72-clay-form-huion.rules
sudo udevadm control --reload-rules
```

Unplug and reconnect the tablet, then restart Clay Form. The rule grants access to the active local desktop user for this model's tablet event devices using [systemd's uaccess mechanism](https://cgit.freedesktop.org/systemd/systemd/tree/src/login/70-uaccess.rules). It does not grant access to all input devices. Other models need their vendor/model IDs in a separate rule.

Your normal user needs read access to the tablet event device. If the sidebar reports permission denied, configure device-specific access through your distribution's input/udev facilities. Do not run the application as root. Device access and pressure have not yet been verified with a physical Huion tablet; model names/drivers that do not contain “Huion” need a discovery extension. Tilt, eraser detection, and tablet express-key configuration are not implemented.

## Scope

This is a desktop sculpting prototype. Meshes retain quad topology where available and use triangles for rendering and picking. The orthographic viewport uses OpenGL rendering; sculpting, picking, and topology operations run on the CPU. Subdivision, voxel remeshing, and local adaptive topology are implemented; quad retopology, sculpt layers, materials, and texture painting are future work. Meshes are limited to 500,000 vertices and 1,000,000 triangles; large scenes can still be limited by CPU editing and picking work. Mesh/alpha file sizes are bounded on load.

## Architecture

See the [development roadmap](docs/roadmap.md) for planned sculpting improvements, priorities, and completion criteria.

The app separates document ownership, sculpting algorithms, UI commands, camera/navigation, rendering/picking, persistence, and tablet input. See [the architecture guide](docs/architecture.md) for responsibilities and extension points. Existing version 1 projects and brush presets remain compatible.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Tests cover brush and mask behavior, legacy JSON compatibility, project/preset disk saves, stroke and bounded undo/redo, failed loads, save/open protection, and simulated viewport/gizmo gestures.

CI runs formatting, the ordinary test suite, and Clippy, plus a separate Mesa/EGL job for the GPU correctness tests. On Linux with EGL installed, run the same graphics checks locally:

```sh
EGL_PLATFORM=surfaceless LIBGL_ALWAYS_SOFTWARE=1 cargo test --locked --lib viewport::gpu:: -- --ignored --skip gpu_load_profile --test-threads=1
```

For manual performance measurements, see [GPU rendering and profiling](docs/gpu-rendering.md). `cargo run --release --locked --example edit_profile` measures document transform and mask-edit costs without opening a window.

## Key settings and workspace

**Keep menus open:** open a built-in or custom menu, then click its **pushpin icon** (tooltip: **Pin menu open**). Transform, Slice, Primitive, and Add brush use the same placement controls as Mask and Symmetry settings. Its controls stay available in a floating window while you sculpt or use other menus. Drag the title bar to move it. The **crossed-pin icon** (Unpin menu) returns it to a normal drop-down. Built-in windows also return to their toolbar menu when closed with **×** or by clicking their highlighted toolbar button; closing a custom floating menu hides it. Multiple menus can stay open together, including with the compact toolbar. Pin state and positions save automatically with your workspace and are restored when you reopen the app.

**Turn a whole menu into a collapsible section:** open a built-in or custom menu and click its **left-sidebar** or **right-sidebar icon** (tooltips: **Make collapsible on left/right**). Pinned menu windows offer the same choices. The whole menu, including sliders and numeric controls, becomes a sidebar section; click its header to collapse or expand it. Right-click any sidebar section header and choose **Move section up / down** to reorder it among built-in tools, custom sections, and converted menus. The same order is editable under **Customize workspace → Built-in panels and toolbar**. Header options also move sections to the other sidebar, return menus to the toolbar, or float a converted built-in menu. Placement, order, and expanded state save with the workspace.

**Custom menus and button shelves:** right-click an action button (including a brush tile) and choose **Add to custom menu / shelf…**, or open **Settings → Customize workspace**. Create a menu, choose actions from the searchable catalog, and give it a name. Placement can be either sidebar, the menu bar, a top/bottom/left/right shelf, floating, or hidden. Right-click empty space or a section header in either sidebar and choose **New collapsible menu here…** to create a section in that sidebar. Click its header to collapse or expand it; right-click its header to edit or delete it. Right-click a copied button and choose **Remove item from this menu**, or use the editor to remove buttons and separators. Removing custom controls leaves their underlying actions available. Set the shelf's column count; narrow shelves wrap to fewer columns. Rename individual buttons, add separators, and drag the `::` grip or use **Up / Down** to reorder them. Floating shelves move by their title bar; closing one hides it until restored in the editor.

The catalog includes project commands, undo/redo, masking, visibility, mesh operations, transforms, brush settings, individual brushes, panel visibility, and opening your custom menus. Actions operate on the current model/brush and keep the usual availability checks and undo behavior. Buttons requiring a dialog decision and per-object numeric editors remain in their original panels. Brush selections use persistent brush IDs; a missing brush is disabled until that brush is available again.

Use **Built-in panels and toolbar** to move, hide, or reorder the existing panels. **Compact toolbar** leaves Settings, Edit, custom menus, and mesh-job cancellation accessible while freeing space for your shelves. Use **Save as workspace** in the workspace editor to name the current arrangement, then switch with the toolbar’s **Workspace** selector. Named workspaces automatically remember later edits, including collapsed menus, panel sizes, and colours. **Workspace colours and style** offers Dark, Light, and Warm presets plus panel, button, accent, text, and model-view colours, spacing, and corner rounding. **Manage saved workspaces** lets you delete workspaces or start from defaults; starting from defaults preserves saved workspaces. Key bindings are shared across all workspaces.

**Number keys:** right-click a button and choose **Assign shortcut…**, then press **1–0** or **Shift+1–0**. These are 20 distinct combinations and are initially unbound. For example, assign `1` to a brush, `Shift+1` to Invert mask, and `0` to **Open [your menu]** in Key settings. Shifted number-row keys are normalized using the physical key, so punctuation such as `!` still binds as `Shift+1`. Other keys continue to follow the keyboard layout. Modifier holds still apply when you use pointer gestures.

Open **Settings → Key settings** to rebind or clear shortcuts. Click a binding and press its replacement; Escape cancels. Conflicting shortcuts are rejected. Smooth, reverse, zoom and pan hold modifiers are configurable too. Shortcuts pause during text entry and settings editing.

Defaults: Ctrl+S saves, Ctrl+O opens the project path, Ctrl+N creates a sphere, Ctrl+Z undoes, Ctrl+Shift+Z redoes, F resets the view, X toggles symmetry, W toggles wireframe, G toggles symmetry guides, brackets change brush radius, and comma/period select brushes. Ctrl+D subdivides, Shift+D selects a lower level, D selects a higher level, and Ctrl+R remeshes in dynamic mode. Existing preferences upgrade without losing custom bindings; new shortcuts that conflict with yours start unbound.

Open **Settings → Customize workspace** to place brush library, brush settings, alpha, presets, tablet/navigation, and mesh management groups on the left, right, in floating windows, or hidden. Arrows reorder groups, headers collapse them, panel edges resize them, and floating titles move them. Closing a floating tool hides it; restore it in the workspace editor. Reset buttons restore key or layout defaults independently.

Preferences save automatically after a short delay and on exit, separately from sculpt projects, to `$XDG_CONFIG_HOME/clay-form/preferences.json` or `~/.config/clay-form/preferences.json`. Invalid preferences fall back to defaults with a status message. Tool groups can be rearranged, and reusable action buttons can also be copied into your custom menus.

Vertical orbit continues over the poles and through full turns; there is no top/bottom angle stop.

## Undo and redo

Mask intensity, selection threshold, temporary thickness, and primitive parameter drags each create one undo step. Releasing the pointer, losing focus, or switching operations ends that edit. During a mask-intensity drag, passing through zero and back up restores the original soft proportions. Clearing an already-empty mask preserves redo history and does not mark the project modified.

Use the toolbar buttons or **Edit → Undo / Redo**. The Edit menu shows the action that will be restored, your configured shortcuts, and available step counts. Defaults are **Ctrl+Z** and **Ctrl+Shift+Z**, adjustable in Key settings. Button tooltips name the action; the status bar confirms each undo/redo.

History includes complete brush strokes (including masks), clear mask, subdivision, remeshing, switching mesh modes, changing levels, and deleting levels. A stroke creates one entry only when geometry or masking actually changes. Empty/protected strokes preserve redo; a new geometry edit starts a new history branch. History resets on New/Open and is session-only. Brush settings, symmetry settings, camera movement, and workspace preferences are outside this geometry history. Undo/Redo pause during a mesh operation or an unsaved-changes prompt.

## Zoom

Use the toolbar **− / +** buttons, **View → Zoom in / Zoom out**, or the mouse wheel over the viewport. Default keys are **=** to zoom in and **−** to zoom out, configurable in Key settings. The View menu also has a zoom slider (20–800%). Click the toolbar percentage or View → Reset zoom to return to 100% while keeping rotation and pan; Reset view resets all camera navigation. Zoom is session-only and does not affect geometry or undo history.

Hold **Ctrl** during an orbit drag to zoom instead of rotating: drag up to zoom in, down to zoom out. This works with right drag, empty-space left/pen drag, and gizmo dragging, including switching the modifier mid-drag. Starting a left/pen drag with Ctrl held also zooms over the clay without sculpting. Change the modifier (or disable it) under **Settings → Key settings → Hold to zoom while dragging**. Existing settings gain Ctrl as the default. **Alt-drag starting off the model pans** horizontally and vertically without changing rotation, zoom, or geometry; starting on the model sculpts with the reverse modifier. The gesture stays sculpting or navigation until release, even when crossing the model edge. Set or disable it under **Hold to pan off the model**; existing preferences gain Alt as the default. Zoom takes priority if pan and zoom share a modifier. Ctrl+Shift visibility selection retains priority, so Alt can still switch that gesture to Hide.

## Snake brush and adaptive sculpting

Choose **Snake** in the brush library, or use **Add brush → Snake** when working in an older project. Start on the clay and drag outward to pull a tapered form. Snake follows a continuous tip handle after the cursor leaves the original surface. Its smooth volumetric influence pulls both sides of a bent tip; adaptive mode also improves triangle shape and redistributes nearby vertices along the surface. It uses the same editable alpha, strength, radius, pressure, Add/Subtract, copy, and preset controls as other brushes. Shift temporarily smooths. Pair it with adaptive sculpting for stretched forms; without adaptation, it deforms the existing mesh.

Select **Mesh → Adaptive sculpt**, or use the Mesh management panel. This is a adaptive sculpting-style local topology workflow, separate from whole-surface voxel remeshing. It splits long edges near each brush dab, shares new vertices across adjacent faces, and optionally collapses short edges in dense areas. It keeps separate parts separate and preserves masks. The Mask brush leaves topology unchanged, and fully protected or all-black-alpha strokes do not adapt it.

**Edge size** sets the desired triangle edge length in screen pixels; lower values add detail. Enable **Detail relative to brush size** to interpret it as a percentage of the brush radius instead. Refinement and simplification can be enabled independently; **Simplify ratio** sets how short an edge must be relative to the target before it can collapse. Sharp edges, normal flips, and invalid manifold collapses are rejected conservatively. Density approaches the requested target over successive dabs with bounded per-frame work, rather than guaranteeing an exact edge length immediately. Existing mesh limits apply.

Enabling adaptive mode keeps the active surface and discards stored subdivision levels; Undo restores them. It requires a closed, consistently oriented surface. Each complete adaptive brush stroke is one undo step, including its topology changes. Adaptive settings and geometry save with the project, and old files remain compatible. This first implementation uses CPU local splits/collapses; it does not use external sculpting application's proprietary algorithm or provide automatic fusion of intersecting parts. Use voxel remeshing when you want fusion. Voxel extraction interpolates distances to the source surface, so smooth shapes stay smooth rather than snapping to voxel steps. Resolution still limits the smallest retained details.

With **Adaptive sculpt → Simplify dense areas** enabled, Smooth (including Shift smoothing) cleans up dense topology without adding vertices. It uses a larger collapse threshold than deformation brushes, capped below the target edge size, and rejects collapses that create thin triangles. This removes excess density while keeping a tidy surface rather than pursuing a minimum vertex count. Increase **Edge size** or **Simplify ratio** for more reduction; reduction remains local and conservative around masks and sharp features. Turn Simplify off to allow ordinary adaptive refinement with Smooth. A cleanup stroke remains one undo step.

### Model transforms

Click **Transform** to open the existing World axes controls. Pin the menu or dock it to either sidebar using its compact icons; its automatic viewport toolbar is suppressed while pinned or docked. The menu, floating toolbar, and Transform panel share **Move**, **Scale**, **Rotate**, and pivot controls. **Done** returns to sculpting.

- **Move / Scale:** drag a colored X/Y/Z handle for one axis, or the center for view-plane movement / uniform scale. Orbit to expose a Move/Scale axis facing the camera.
- **Rotate:** drag a colored ring around its axis. The live signed angle shows the amount of rotation. Rings viewed edge-on use a straight drag along the projected ring, so snapped front/side views remain usable. Drag outside the handles to orbit the camera.
- **Place pivot:** click the active surface to set the World pivot; clicking empty space uses the current pivot depth. Drag to adjust it in the view plane. The pivot stays placed across operation changes and follows Move. **Reset pivot** returns it to the active model's bounding-box center.

**Action Line** reuses its start endpoint as the pivot and its direction as the transform axis. Choose **Set line** and drag to place it; either endpoint can be adjusted. **Move** uses the middle handle. **Scale** uses the end to stretch along the line, or the middle for uniform scale. **Rotate** turns around the line using its amber ring. **Reset pivot** moves the line's start to the model center while retaining the line's direction and length. A zero-length line must be extended before rotation.

**Respect mask** protects fully masked vertices; soft masks reduce the rotation angle without shrinking their radius around the axis. Disable it to transform the whole model. Every stored subdivision level follows, and each changed geometry drag is one undo step. Parametric primitives keep their rotated placement when dimensions or resolution are edited, and when saved/reopened.

Pivot and line placement do not change geometry, dirty the project, or consume undo steps. They are temporary viewport controls retained across transform operation changes, but reset when the viewport is cleared by actions such as model switching, project opening, undo/redo, or mesh/mask operations. They are not stored in projects. Focus loss, a tool switch, or a camera command finishes the current drag and prevents further changes until release. Rotate, Place pivot, and Reset pivot are available in custom menus and Key settings; they have no new default shortcuts.

### Submodels

The graphical **Submodels** panel shows shape previews, selection, eye icons for visibility, and up/down reorder controls. Visible models stay in the viewport; inactive models use their lowest stored subdivision level when one exists, without losing higher detail. The active model is clay-colored; inactive models are blue-gray. Click an inactive model in Sculpt mode to select it; sculpting begins on the next drag. Hidden models cannot be sculpted or transformed.

Duplicate copies the active surface, mask, and subdivision stack. Extract masked copies faces whose vertices meet the selection threshold (50% by default) into a new active, unmasked submodel; the source remains intact. Extracted boundaries are open and use a fresh subdivision stack. OBJ export applies to the active model.

Create named groups, assign the active model through its group selector, reorder models within groups, reorder groups, and toggle a group's visibility. Ungroup removes the group while keeping every model. Selection preserves list order. Projects and undo/redo preserve visibility, groups, ordering, and all models; older single-model projects open as Model. Groups and ordering provide organization for future boolean workflows; boolean operations are not implemented yet.

### Parametric primitives

Open **Primitive → Create primitive…** for the active submodel. A confirmation warns that its current geometry, masks, and subdivision levels will be replaced; Cancel leaves it untouched and Undo can restore it. For an existing primitive, the Primitive menu shows its shared parameter controls and can be pinned or docked. Docking or pinning moves those controls out of Submodels to avoid duplication.

The shape remains an editable recipe: **Plane, Cube, Circle, UV sphere, Icosphere, Cylinder, Cone, Torus, Grid, or Human head**. The collapsible **Submodels → Primitive** section supplies shape-specific dimensions, spans/segments/rings/subdivisions, caps/fill, a live vertex/triangle count, and wireframe display. Cube X/Y/Z spans control face resolution independently; one span per axis yields eight welded corner vertices. Plane/Grid and Circle lie in the front XY plane; cylinders, cones, and tori use the Y axis.

Axis-colored size handles and teal radius handles on the preview let you drag dimensions and radii directly. Hover a handle for its label. The human-head base adds jaw width/length, chin, cheeks, nose projection/width, eye spacing/depth/size/height, brow, ears, mouth width/opening and lip fullness. The revised base uses a shaped cranium, jaw and chin profile, fitted almond-shaped lids, a nose bridge/tip and upper/lower lips. Eyeballs with geometric iris detail, 32 tooth crowns arranged in upper/lower dental arches, and a tongue are separate closed surfaces inside the same submodel; the skin includes a recessed mouth cavity. **Open mouth** rotates the lower jaw, lower teeth and tongue; upper teeth remain fixed. **Upper eyelid closure** and **Lower eyelid closure** have independent sliders and viewport handles, with Open eyes / Close eyes buttons. **Reset head proportions** applies the revised anatomy to an earlier saved head; Undo restores its previous recipe. The head includes a welded neck, with controls for length, width, muscle definition and throat prominence; Include neck can remove it. Neck landmarks follow the sternocleidomastoid path from behind the ears toward the sternum and medial clavicles ([anatomy reference](https://www.ncbi.nlm.nih.gov/books/NBK532881/)). Facial controls range from 0 to 2 for broader adjustments. Dragging a depth control directly facing the camera uses horizontal mouse movement; orbit for a projected depth direction. Facial controls are symmetric and intended to provide a sculpting base rather than finished anatomy. Whole-model placement remains available through World axes or Action Line; recipes retain those transforms when regenerated. Return to Sculpt mode (Done) to show the teal shape controls again.

Choose **Convert to sculpting mesh** to keep the previewed surface and remove its parametric recipe. Brushes, subdivision, and adaptive/voxel workflows become available after conversion; Undo restores the recipe. An unfilled Circle has no faces and must be filled before conversion; planes, grids, disks, and uncapped tubes remain open surfaces, so closed-surface remeshing/adaptive modes require closing them first. Primitive recipes save with each submodel and survive duplication, switching, undo/redo, and reopening projects. Collapsing the Primitive section keeps the parametric model. Its resolution controls support dragging/typing numbers or minus/plus. Shape, resolution, wireframe, and conversion controls share this one section; it disappears after conversion.

The **Detail** brush cuts a narrow groove with raised shoulders and pinches the surface toward the crease, for wrinkles and fine folds. Choose Subtract (or use your configured reverse modifier during a stroke) to raise a sharp ridge instead. Adjust radius and strength as usual; masks, alpha, pressure and symmetry are supported. New projects include it; in existing projects use **Add brush → Detail**. For crisp features use sufficient subdivision or adaptive detail.

Toolbar actions share 16-pixel line icons and 28-pixel minimum heights. Project actions and modeling/view actions occupy separate rows and wrap on narrower windows. Brush previews use equal-sized responsive tiles, with names beneath the images, full-name tooltips and teal selection outlines.

In **View**, enable **Faint back-facing surfaces** to see the reverse sides of open surfaces translucently. **Back-face opacity** adjusts their visibility (15% by default). Front-facing surfaces remain opaque; backs behind solid foreground surfaces stay hidden. This is a session-only viewing option and does not change normals or sculpting/picking.

Wireframe options beside the Wireframe button select visible surfaces (edges hidden by other surfaces), front-facing edges, or transparent viewing of the whole frame. Triangle diagonals can be shown separately. Primitives use quad faces where possible; enable Triangle topology in the primitive editor to use triangles throughout. Quad faces survive subdivision, project saving, and OBJ export. Poles, caps, icospheres, and adaptive or voxel remeshing still use triangles.

Primitive resolution can also be changed with the diamond handles beside the model: drag left or right to change spans, sides, segments, rings or subdivision levels. The live count updates as you drag, and each drag is one Undo step. Teal circles identify radii and anatomical controls, axis-colored squares identify size controls, and the world transform and camera gizmo share labelled X/Y/Z colours. Hovering or dragging adds a white ring while retaining the control colour.

For renderer ownership, validation, and the opt-in load profile, see [GPU rendering](docs/gpu-rendering.md).


### Temporary thickness

In **Submodels → Thickness · open meshes**, enable **Temporary thickness** and set the total thickness in model units. The preview offsets half the amount to each side of the surface and adds walls around boundary loops. Orbit to inspect it. Each submodel saves its own value; duplication retains it. The original sculpting vertices, masks, subdivision levels, and surface picking remain unchanged until you apply it. OBJ exports the original surface while thickness is temporary.

**Apply thickness** turns the exact preview into a closed sculpting mesh, preserves masks and complete quads, clears the temporary setting, and resets subdivision levels and topology modes. Applying a primitive preview converts it to a sculpting mesh. Other submodels stay unchanged. Undo restores the original surface, recipe/levels, and thickness setting. Thickness works on consistently oriented open surfaces with nonbranching boundary loops; closed meshes and outlines without faces are not eligible. Large normal offsets can intersect at tight folds or concave areas, so inspect the preview before applying.

### Vertex groups

In **Submodels → Vertex groups**, choose **Group masked** to add a group from the mask, or **Group all** to replace the active model’s existing groups with one group containing every vertex. Group all updates every subdivision level, preserves masks and face visibility, leaves other submodels unchanged, and can be undone. Blank names automatically become **Group 1**, **Group 2**, and so on; enter a name when you want a custom label. Groups belong to each submodel and store independent soft vertex weights; creating, changing or recalling a group is undoable. Select a group to **Mask group**, **Add masked**, **Remove masked**, rename or delete it. Complete selected faces receive that group's color. Vertex groups can overlap, while each face has one category; partial face selections retain vertex membership without coloring the entire face. **View → Show vertex group colors** toggles the colors.

The **Mask** menu adds **Expand mask** and **Contract mask** (one mesh-edge ring per click), **Mask intensity** (scales soft falloff), and **Selection threshold** (minimum vertex mask for grouping, deletion and extraction). Zero mask values are excluded even at a zero threshold. These adjustments leave saved group weights intact. Reducing intensity to zero clears the mask after the adjustment ends; undo or recall a group to restore it.

**Blur mask** softens boundaries by blending each mask value halfway toward its immediate mesh-edge neighbours. Quad diagonals are ignored; repeat for a wider transition. **Sharpen mask** doubles contrast around 50%, pushing softer values toward clear or fully protected while keeping 50% unchanged. This midpoint is independent of Selection threshold. Both commands affect the active submodel, including hidden faces, and apply to every stored subdivision level at that level's mesh density. Geometry, face visibility, saved vertex group weights, and other submodels stay unchanged. Each click that changes the mask creates one undo step; both actions can be added to custom menus or assigned shortcuts. The resulting masks save in ordinary `.clay` projects.

Thickness adds **Group front / back / rim**, **Mask rim vertices**, and **Group masked vertices** options per submodel. Generated groups appear in the preview and become editable when thickness is applied. Grouping the generated mask can replace part face colors where complete faces meet the threshold; the part vertex groups remain available.

Groups survive project save/load, duplication, subdivision, extraction and deletion. Adaptive topology interpolates weights and preserves face categories at boundaries. Voxel remeshing transfers weights from nearest source vertices and categories from nearest source face centers, so boundaries are approximate after remeshing. OBJ exports face groups using `g` records; independent soft vertex weights are saved in `.clay` projects. The weights provide a foundation for future vertex painting; painting itself is not included yet.

New projects start with the sphere assigned to a **Base** vertex group. The eye beside each vertex group hides or shows its assigned faces, including wireframe and picking. Hidden faces and their shared boundary vertices are protected from brush edits. Visibility is saved with the project and supports undo. Groups containing only partial vertex selections have no complete assigned faces to hide. To hide just the colors while keeping the whole surface visible, use **View → Show vertex group colors**.

Primitive span cut lines, span controls, and size controls share **X red, Y green, Z blue** colors. Plane/Grid X spans create red vertical cuts; Y spans create green horizontal cuts. Cube cut loops follow their local controlling axis, including after rotation or scaling. Cylinder/Cone height-span rings are green. The span labels and numeric controls use the same colors in the collapsible Primitive section in Submodels. Enable wireframe to see the colored topology; non-axis topology controls remain violet.

### Stroke modes

Choose a mode under **Brush → Stroke mode**; it is saved per brush in projects, presets and brush libraries. Existing brushes default to **Freehand**.

- **Freehand:** the existing continuous stroke.
- **Line:** press on the active model and drag a straight guide. Release to apply spaced dabs along that screen-space line, projected onto visible surfaces. Very long lines are limited to 257 dabs.
- **Drag Rect:** press to anchor one stamp, then drag to set its radius and alpha angle. The sculpted result previews live; release to keep it. Drag size is capped at the existing 500-pixel brush-radius limit; the preset's size and rotation remain unchanged.
- **Drag Dot:** move a fixed-size stamp with a live sculpt preview, then release to keep it.

**Stroke stabilisation:** in Brush settings, enable this for Freehand strokes and adjust **Trailing distance (px)** from 1–200 (default 30). The brush tip trails the pointer, absorbing small movements; a line connects the tip to the pointer, and the brush footprint follows the tip. A stroke starts with a dab at the press position. Pause or reverse within the trailing distance without adding more dabs. Release ends at the trailing tip, without jumping to the pointer. Pressure, spacing, masks, symmetry, and temporary Smooth/reverse still apply, and the whole stroke is one undo step.

Settings are saved per brush in projects, presets, and libraries and take effect on the next stroke. Disable it to restore direct Freehand input; Grab, Snake, and the other stroke modes are unaffected. **Toggle stroke stabilisation** is available for custom menus and shortcuts.

With Adaptive sculpt and Refine enabled, drag stamps refine the surface before applying the alpha, with extra detail along sharp alpha boundaries. Very large stamps or very fine edge sizes can reach the bounded refinement limit.

Drag stamps rebuild their preview from the original mesh, so holding still does not build up depth. Relief brushes (Standard, Clay, Inflate, Crease and Detail) use four times the depth of one freehand dab; Strength and pressure still control the result. Mask opacity and smoothing weights retain their normal values.

Line and drag modes preview before changing document geometry, create one undo step, and support masks, alphas, symmetry and pen pressure. **Esc** cancels a pending stroke. Hidden faces cannot receive stamps. Grab and Snake keep their continuous Freehand behavior because they depend on pointer displacement.

### Slice

Choose **Slice** in the toolbar, then choose a result:

- **Vertex groups** cuts faces along the line while keeping one connected surface. Existing face categories stay on one side; categories spanning the line get new **Slice** counterparts on the other. Existing weighted vertex groups remain available.
- **Trim** removes one side. The preview labels **Keep** and **Remove**; **Flip kept side** reverses them.
- **Separate** keeps both sides as independent submodels, placing the new **Slice** model immediately after the active one. Both pieces initially stay in place.

Drag a straight line in the viewport, starting on the model or in empty space. The faint line extension shows the full cut: a plane through all depths of the active model. Hold **Space** while dragging to move the whole line without changing its angle. Release the mouse to apply; **Esc** cancels the gesture and exits Slice mode. Right drag still orbits between cuts. **Done slicing** returns to sculpting. Slice does not mirror with brush symmetry or use masks to restrict the cut; surviving mask values and existing vertex-group weights are preserved and interpolated at new vertices.

Choose **Path** for a cut with editable bends. Click to add points (up to 24), click an edge to insert a point, and drag a point to change the shape. Select an interior point to reveal corner handles: drag the blue diamond away from the point to soften the corner, or back toward it to sharpen it. Rounded corners also show two linked tangent handles where the curve meets the straight edges; dragging either changes the rounding. **Sharp** removes rounding immediately; **Rounded** adds it. The **Turn** angle and **Round (px)** controls remain available; rounding is limited by the neighboring segment lengths. **Shift-drag** snaps to 15°; **Space-drag** moves the whole path. **Delete point** (or Delete/Backspace) removes the selected point. Choose **Apply cut** or press Enter to commit; **Clear** discards the draft; **Esc** discards it and exits Slice mode. The faint end extensions continue through all depths of the model. Paths and their extensions must not cross themselves. The camera stays fixed while editing a path; clear or apply it before orbiting. A failed or cancelled cut keeps the draft available for correction.

For Trim and Separate, **Cap cuts** is on by default. It closes the cut with triangles in a **Cut** vertex group, including concave boundaries and holes; path caps follow the bends and rounded corners. Caps require a closed, consistently oriented source surface. Turn caps off to slice open surfaces, or apply temporary thickness first. Convert primitives before slicing, apply or remove temporary thickness, and show all vertex group faces of the active model.

A slice uses the active subdivision level and resets that model's subdivision stack, adaptive/voxel modes, and automatic remeshing. Uncut quads are retained; intersected faces and caps use triangles. Other submodels remain unchanged. The operation runs in a cancellable background worker and creates one Undo step; failures and cancellation keep the original model. Slice results save in ordinary `.clay` projects and export through the existing OBJ workflow.

### Selection and visibility

Open **Visibility → Select rectangle**, or hold **Ctrl+Shift** in Sculpt mode for a temporary selection gesture. The modifier gestures are inspired by external software vendor's external sculpting application partial mesh visibility reference (section 6.9); Clay Form's vertex group face clicks hide the clicked group first:

- **Ctrl+Shift-drag:** isolate the rectangle (green). Release the keys after starting if preferred; the gesture stays a selection.
- **Alt while dragging:** hide the rectangle (red).
- **Space while dragging:** move the rectangle. **Esc:** cancel without changing the model.
- **Ctrl+Shift-click a vertex group’s face:** hide the clicked group. If it is the last visible group, flip visibility: that group becomes hidden and previously hidden faces become visible.
- **Ctrl+Shift-click background:** show all faces. Drag a rectangle entirely in empty background to invert visibility.

The explicit Select rectangle tool uses the same gestures without holding Ctrl+Shift; choose **Done** to return to sculpting. Right drag still navigates. Selection takes priority over Ctrl-drag zoom. This first version selects face centers through all depths and keeps quads whole. Lasso selection is also available as described below.

**Visibility → Select lasso** lets you draw a freehand outline to isolate faces. Hold **Alt** to hide the enclosed faces instead. **Mask → Mask lasso** adds full masking inside the outline; hold **Alt** to erase masking there. Existing mask values outside the outline stay unchanged. Both tools are available in custom menus and configurable shortcuts.

While drawing, a green outline previews isolate/add and a red outline previews hide/erase; a closing line joins the pointer to the starting point. Release to apply. **Space-drag** moves the whole outline, **Esc** cancels the current gesture, and **Done** returns to sculpting. The camera stays fixed during drawing; right drag navigates between selections. Tool changes, loss of focus, and changes to the model or view cancel an unfinished outline. Taps and line-only outlines do nothing. Concave outlines work; crossing an outline uses alternating inside/outside regions. Overlong outlines are cancelled with a message rather than silently simplified.

Both lassos select **through all depths**, including the back of the model, on the **active submodel only**. There is no front-surface-only mode. Visibility uses face centers and keeps quads whole, just like rectangles; isolate keeps only currently visible faces inside the outline, while hide removes those faces. Mask lasso uses vertex positions and protects hidden faces and their shared boundary vertices, just like the Mask brush. Selection acts on the original mesh under temporary thickness, and does not repeat with brush symmetry or vary with pen pressure. The projected region is applied independently to every stored subdivision level. Each changed gesture creates one Undo step; cancelling or an unchanged selection preserves redo history. Results save in ordinary `.clay` projects. The temporary Ctrl+Shift gesture remains a rectangle when no explicit selection tool is active.

**Visibility → Show all / Invert visibility** also work without gestures. **Submodels → Vertex groups → Flip visibility** swaps the visible and hidden faces of the active submodel, including partial selections. A hidden-face count appears in the viewport and menu. Hiding affects only the active submodel, leaves masks and vertex group face assignments intact, and excludes hidden faces from display, wireframe, picking, and brush edits. Shared boundary vertices stay fixed. Region selections are applied to each stored subdivision level; subdividing carries visibility into the new faces. Visibility is saved in `.clay` projects and supports Undo/Redo. Automatic voxel remeshing pauses while faces are hidden; show all before manual remeshing or slicing. Selection acts on the original surface when temporary thickness is enabled. Apply or remove temporary thickness before splitting.

**Visibility → Split hidden** moves hidden faces into a new, visible submodel immediately after the active one, following external sculpting application's Split Hidden behavior. Both pieces stay in place, retaining masks, vertex groups, and complete quads. Boundaries stay open. Both visible and hidden faces are required. Splitting uses the current subdivision level and resets the resulting pieces' subdivision stacks and adaptive/voxel settings; other submodels stay intact. One Undo restores the complete pre-split state.

### Merge down

Choose **Submodels → Merge down** to combine the active submodel with the next one in the panel's grouped order, following external sculpting application's MergeDown operation. The tooltip identifies the target; the button is disabled for the last model. The active model keeps its name and folder. Positions, masks, complete quads, weighted vertex groups, and face visibility are preserved; duplicate vertex-group names receive a suffix. Hidden submodels contribute hidden faces.

Merge uses each model's current mesh level, bakes primitive previews and temporary thickness, and resets the merged result's subdivision and adaptive/voxel settings. Fill unfilled Circle primitives first. It combines surfaces without welding vertices or performing a boolean union. Other submodels stay untouched. **Undo** restores both original models, their settings and their order; merged results save normally in `.clay` projects. Mesh and group limits are checked before changing the document.

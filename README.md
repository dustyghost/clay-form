# Clay Form

A Rust desktop sculpting prototype built with eframe/egui. Start from a sphere and sculpt real mesh vertices with a pen or mouse.

## Run

Install a current stable Rust toolchain, then:

```sh
cargo run --release
```

Linux requires a working Wayland or X11 desktop and OpenGL support. The first build downloads Cargo dependencies. `Cargo.lock` is committed for reproducible application builds.

## Sculpting

- Left drag or pen tip on the clay: sculpt. Start a left/pen drag in empty space to orbit; it stays an orbit gesture even if you cross the clay. Right drag: orbit. Middle drag: pan. Wheel: zoom.
- The top-right orientation gizmo shows positive and negative X/Y/Z axes. Drag it to orbit, or click an axis to snap to that view. Dimmed axes point away from you.
- Alt temporarily reverses the current brush. Shift temporarily uses Smooth.
- Standard, Clay, Inflate, Smooth, Flatten, Pinch, Crease, Grab, Mask, and Snake are included.
- Every brush uses an alpha, including Grab, smoothing and masking. Add/Subtract reverses its operation; subtracting Smooth sharpens, subtracting Flatten exaggerates surface relief, and subtracting Mask erases masking.
- Adjust radius, strength, alpha falloff, spacing, rotation, pressure strength/radius mapping, and pressure response curve. Grab moves the surface with the cursor; spacing does not affect Grab.
- Choose Soft, Hard or Square alpha, or load a PNG/JPEG up to 2048 × 2048 pixels. White affects the surface, black protects it, and transparency reduces influence. Custom alpha pixels are embedded in presets and projects.
- Duplicate any brush to create a named variation. Save/load a preset using its path field. Loading appends it to the library.
- Enable **Symmetry** in the toolbar, then open **Symmetry settings**. Mirror across X/Y/Z independently or combine planes. Radial symmetry adds 2–16 repeats around X, Y, or Z; mirrors and radial repeats can be combined.
- Adjust the symmetry center in object coordinates, or reset it to the origin. Surface lines show the active planes and radial boundaries as you orbit: X pink/red, Y green, Z blue, other radial boundaries gold. Guide visibility and opacity are adjustable.
- Symmetry settings are saved in projects; older projects open with symmetry disabled. Overlapping symmetry copies use the strongest influence per vertex, preventing stronger strokes at the center seam. Clear mask removes all masking. Undo/Redo keeps up to 24 geometry edits within a 128 MB snapshot budget.

## Mesh management

Use the **Mesh** menu or the **Mesh management** panel. New spheres start at 4,514 vertices, leaving room for subdivision; opening older projects preserves their geometry.

**Subdivision:** add a level with Subdivide, then select a level with the slider or Lower/Higher buttons. Each face splits into four triangles with shared edge midpoints, preserving the existing shape. This is linear subdivision rather than a smoothing subdivision surface. Lower-level edits propagate upward while retaining stored fine detail; sculpting higher levels updates their parent vertices when you step down. Mask values interpolate between levels. Subdivide from the highest level. **Delete lower** removes every level below the active level, making it the new level 1 while retaining higher detail. **Delete higher** removes every level above it, allowing a fresh subdivision from the current surface. Both preserve the active geometry and masks and can be undone. The active mesh and all stored levels save in `.clay` projects. OBJ export writes the active level.

**Dynamic mesh:** switch modes to keep the active surface as a single mesh, discarding stored subdivision levels. Choose resolution (16–96 voxels along the longest bounding-box axis) and click Remesh now. Voxel reconstruction redistributes geometry, joins overlapping outward-oriented closed shells, and preserves separated volumes. It requires a closed, consistently oriented mesh. Optional Remesh after each stroke runs when a geometry stroke ends; painting masks does not trigger it. This is whole-surface voxel remeshing, rather than local adaptive triangulation while the brush moves.

Remeshing clears masks and can remove features thinner than a voxel. Start with a moderate resolution and raise it for detail. Subdivision and remeshing run in background workers; sculpt input pauses during the operation. Cancel keeps the original mesh. Mode switches, level changes, strokes, and completed topology operations can be undone, including restoring discarded levels. Failed operations leave geometry intact; a failed automatic remesh disables the automatic setting so you can adjust it before retrying.

## Files

Type a relative or absolute file path in the toolbar, then use Save or Open. `.clay` projects are versioned JSON containing geometry, masks, and the brush library. Presets use `.brush.json`. Existing files at the selected save path are replaced. Saving uses a temporary sibling file followed by rename. Parent folders must already exist.

New, Open and closing the window prompt when the project has unsaved changes. The prompt has a separate save path, protecting the file you intend to open. Export OBJ writes a mesh beside the project path, using the `.obj` extension. Projects restore the sculpt and presets; camera position and undo history are session-only. OBJ import is not yet implemented.

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

This is a first functional prototype, not a replacement for a production sculpting package. It uses triangle meshes and an orthographic viewport with CPU projection and egui rendering. Subdivision, voxel remeshing, and local adaptive topology are implemented; quad retopology, sculpt layers, materials, and texture painting are future work. Meshes are limited to 500,000 vertices and 1,000,000 triangles; very large meshes are slower in this CPU renderer. Mesh/alpha file sizes are bounded on load.

## Architecture

The app separates document ownership, sculpting algorithms, UI commands, camera/navigation, rendering/picking, persistence, and tablet input. See [the architecture guide](docs/architecture.md) for responsibilities and extension points. Existing version 1 projects and brush presets remain compatible.

## Development

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Tests cover brush and mask behavior, legacy JSON compatibility, project/preset disk saves, stroke and bounded undo/redo, failed loads, save/open protection, and simulated viewport/gizmo gestures.

Initialize the repository if it has not already been initialized:

```sh
git init -b main
git add Cargo.toml Cargo.lock .gitignore README.md src
git commit -m "Create Clay Form sculpting prototype"
```

## Key settings and workspace

Open **Settings → Key settings** to rebind or clear shortcuts. Click a binding and press its replacement; Escape cancels. Conflicting shortcuts are rejected. Smooth and reverse hold modifiers are configurable too. Shortcuts pause during text entry and settings editing.

Defaults: Ctrl+S saves, Ctrl+O opens the project path, Ctrl+N creates a sphere, Ctrl+Z undoes, Ctrl+Shift+Z redoes, F resets the view, X toggles symmetry, W toggles wireframe, G toggles symmetry guides, brackets change brush radius, and comma/period select brushes. Ctrl+D subdivides, Shift+D selects a lower level, D selects a higher level, and Ctrl+R remeshes in dynamic mode. Existing preferences upgrade without losing custom bindings; new shortcuts that conflict with yours start unbound.

Open **Settings → Customize workspace** to place brush library, brush settings, alpha, presets, tablet/navigation, and mesh management groups on the left, right, in floating windows, or hidden. Arrows reorder groups, headers collapse them, panel edges resize them, and floating titles move them. Closing a floating tool hides it; restore it in the workspace editor. Reset buttons restore key or layout defaults independently.

Preferences save automatically after a short delay and on exit, separately from sculpt projects, to `$XDG_CONFIG_HOME/clay-form/preferences.json` or `~/.config/clay-form/preferences.json`. Invalid preferences fall back to defaults with a status message. This is group-level layout customization; individual controls stay within their tool groups.

## Undo and redo

Use the toolbar buttons or **Edit → Undo / Redo**. The Edit menu shows the action that will be restored, your configured shortcuts, and available step counts. Defaults are **Ctrl+Z** and **Ctrl+Shift+Z**, adjustable in Key settings. Button tooltips name the action; the status bar confirms each undo/redo.

History includes complete brush strokes (including masks), clear mask, subdivision, remeshing, switching mesh modes, changing levels, and deleting levels. A stroke creates one entry only when geometry or masking actually changes. Empty/protected strokes preserve redo; a new geometry edit starts a new history branch. History resets on New/Open and is session-only. Brush settings, symmetry settings, camera movement, and workspace preferences are outside this geometry history. Undo/Redo pause during a mesh operation or an unsaved-changes prompt.

## Zoom

Use the toolbar **− / +** buttons, **View → Zoom in / Zoom out**, or the mouse wheel over the viewport. Default keys are **=** to zoom in and **−** to zoom out, configurable in Key settings. The View menu also has a zoom slider (20–800%). Click the toolbar percentage or View → Reset zoom to return to 100% while keeping rotation and pan; Reset view resets all camera navigation. Zoom is session-only and does not affect geometry or undo history.

Hold **Ctrl** during an orbit drag to zoom instead of rotating: drag up to zoom in, down to zoom out. This works with right drag, empty-space left/pen drag, and gizmo dragging, including switching the modifier mid-drag. Starting a left/pen drag with Ctrl held also zooms over the clay without sculpting. Change the modifier (or disable it) under **Settings → Key settings → Hold to zoom while dragging**. Existing settings gain Ctrl as the default.

## Snake brush and adaptive sculpting

Choose **Snake** in the brush library, or use **Add brush → Snake** when working in an older project. Start on the clay and drag outward to pull a tapered form. Snake follows the tip after the cursor leaves the original surface, unlike an ordinary surface dab. It uses the same editable alpha, strength, radius, pressure, Add/Subtract, copy, and preset controls as other brushes. Shift temporarily smooths. Pair it with adaptive sculpting for stretched forms; without adaptation, it deforms the existing mesh.

Select **Mesh → Adaptive sculpt**, or use the Mesh management panel. This is a adaptive sculpting-style local topology workflow, separate from whole-surface voxel remeshing. It splits long edges near each brush dab, shares new vertices across adjacent faces, and optionally collapses short edges in dense areas. It keeps separate parts separate and preserves masks. The Mask brush leaves topology unchanged, and fully protected or all-black-alpha strokes do not adapt it.

**Edge size** sets the desired triangle edge length in screen pixels; lower values add detail. Enable **Detail relative to brush size** to interpret it as a percentage of the brush radius instead. Refinement and simplification can be enabled independently; **Simplify ratio** sets how short an edge must be relative to the target before it can collapse. Sharp edges, normal flips, and invalid manifold collapses are rejected conservatively. Density approaches the requested target over successive dabs with bounded per-frame work, rather than guaranteeing an exact edge length immediately. Existing mesh limits apply.

Enabling adaptive mode keeps the active surface and discards stored subdivision levels; Undo restores them. It requires a closed, consistently oriented surface. Each complete adaptive brush stroke is one undo step, including its topology changes. Adaptive settings and geometry save with the project, and old files remain compatible. This first implementation uses CPU local splits/collapses; it does not use external sculpting application's proprietary algorithm or provide automatic fusion of intersecting parts. Use voxel remeshing when you want fusion.

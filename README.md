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
- The top-right orientation gizmo shows positive and negative X/Y/Z axes. Drag it to orbit, or click an axis to snap to that view. Dimmed axes point away from you.
- Alt temporarily reverses the current brush. Shift temporarily uses Smooth.
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

Choose **Snake** in the brush library, or use **Add brush → Snake** when working in an older project. Start on the clay and drag outward to pull a tapered form. Snake follows a continuous tip handle after the cursor leaves the original surface. Its smooth volumetric influence pulls both sides of a bent tip; adaptive mode also improves triangle shape and redistributes nearby vertices along the surface. It uses the same editable alpha, strength, radius, pressure, Add/Subtract, copy, and preset controls as other brushes. Shift temporarily smooths. Pair it with adaptive sculpting for stretched forms; without adaptation, it deforms the existing mesh.

Select **Mesh → Adaptive sculpt**, or use the Mesh management panel. This is a adaptive sculpting-style local topology workflow, separate from whole-surface voxel remeshing. It splits long edges near each brush dab, shares new vertices across adjacent faces, and optionally collapses short edges in dense areas. It keeps separate parts separate and preserves masks. The Mask brush leaves topology unchanged, and fully protected or all-black-alpha strokes do not adapt it.

**Edge size** sets the desired triangle edge length in screen pixels; lower values add detail. Enable **Detail relative to brush size** to interpret it as a percentage of the brush radius instead. Refinement and simplification can be enabled independently; **Simplify ratio** sets how short an edge must be relative to the target before it can collapse. Sharp edges, normal flips, and invalid manifold collapses are rejected conservatively. Density approaches the requested target over successive dabs with bounded per-frame work, rather than guaranteeing an exact edge length immediately. Existing mesh limits apply.

Enabling adaptive mode keeps the active surface and discards stored subdivision levels; Undo restores them. It requires a closed, consistently oriented surface. Each complete adaptive brush stroke is one undo step, including its topology changes. Adaptive settings and geometry save with the project, and old files remain compatible. This first implementation uses CPU local splits/collapses; it does not use external sculpting application's proprietary algorithm or provide automatic fusion of intersecting parts. Use voxel remeshing when you want fusion. Voxel extraction interpolates distances to the source surface, so smooth shapes stay smooth rather than snapping to voxel steps. Resolution still limits the smallest retained details.

With **Adaptive sculpt → Simplify dense areas** enabled, Smooth (including Shift smoothing) cleans up dense topology without adding vertices. It uses a larger collapse threshold than deformation brushes, capped below the target edge size, and rejects collapses that create thin triangles. This removes excess density while keeping a tidy surface rather than pursuing a minimum vertex count. Increase **Edge size** or **Simplify ratio** for more reduction; reduction remains local and conservative around masks and sharp features. Turn Simplify off to allow ordinary adaptive refinement with Smooth. A cleanup stroke remains one undo step.

### Model transforms

Click **Transform** once to activate World axes and show the floating controls immediately. Use those controls to switch to Action Line or choose Move/Scale. A compact toolbar appears in the viewport while transforming, with direct mode, Move/Scale, mask, and Done controls; Action Line also offers Set line. Drag a colored X/Y/Z handle for one axis, or the center handle for movement in the view plane / uniform scale. An axis pointing directly into the camera has no usable screen direction; orbit to expose it. **Respect mask** protects masked vertices and can be disabled to transform the whole model. Every stored subdivision level follows, and each drag is one undo step.

**Action Line** is a line-based alternative. Choose **Set line**, then drag in the viewport to place a start pivot and an end direction; either endpoint can be adjusted. Choose **Move** and drag the middle handle to reposition the model. Choose **Scale** and drag the end to stretch along the line, or the middle to uniformly shrink/enlarge around the start pivot. The line is a temporary viewport control and resets when changing models, opening a project, or undoing.

### Submodels

The graphical **Submodels** panel shows shape previews, selection, eye icons for visibility, and up/down reorder controls. Visible models stay in the viewport; inactive models use their lowest stored subdivision level when one exists, without losing higher detail. The active model is clay-colored; inactive models are blue-gray. Click an inactive model in Sculpt mode to select it; sculpting begins on the next drag. Hidden models cannot be sculpted or transformed.

Duplicate copies the active surface, mask, and subdivision stack. Extract masked copies faces whose vertices meet the selection threshold (50% by default) into a new active, unmasked submodel; the source remains intact. Extracted boundaries are open and use a fresh subdivision stack. OBJ export applies to the active model.

Create named groups, assign the active model through its group selector, reorder models within groups, reorder groups, and toggle a group's visibility. Ungroup removes the group while keeping every model. Selection preserves list order. Projects and undo/redo preserve visibility, groups, ordering, and all models; older single-model projects open as Model. Groups and ordering provide organization for future boolean workflows; boolean operations are not implemented yet.

### Parametric primitives

Click **Primitive** for the active submodel. A confirmation warns that its current geometry, masks, and subdivision levels will be replaced; Cancel leaves it untouched and Undo can restore it. For an existing primitive, the button expands its controls in Submodels.

The shape remains an editable recipe: **Plane, Cube, Circle, UV sphere, Icosphere, Cylinder, Cone, Torus, Grid, or Human head**. The collapsible **Submodels → Primitive** section supplies shape-specific dimensions, spans/segments/rings/subdivisions, caps/fill, a live vertex/triangle count, and wireframe display. Cube X/Y/Z spans control face resolution independently; one span per axis yields eight welded corner vertices. Plane/Grid and Circle lie in the front XY plane; cylinders, cones, and tori use the Y axis.

Axis-colored size handles and teal radius handles on the preview let you drag dimensions and radii directly. Hover a handle for its label. The human-head base adds jaw width/length, chin, cheeks, nose projection/width, eye spacing/depth/size/height, brow, ears, mouth width/opening and lip fullness. The revised base uses a shaped cranium, jaw and chin profile, fitted almond-shaped lids, a nose bridge/tip and upper/lower lips. Eyeballs with geometric iris detail, 32 tooth crowns arranged in upper/lower dental arches, and a tongue are separate closed surfaces inside the same submodel; the skin includes a recessed mouth cavity. **Open mouth** rotates the lower jaw, lower teeth and tongue; upper teeth remain fixed. **Upper eyelid closure** and **Lower eyelid closure** have independent sliders and viewport handles, with Open eyes / Close eyes buttons. **Reset head proportions** applies the revised anatomy to an earlier saved head; Undo restores its previous recipe. The head includes a welded neck, with controls for length, width, muscle definition and throat prominence; Include neck can remove it. Neck landmarks follow the sternocleidomastoid path from behind the ears toward the sternum and medial clavicles ([anatomy reference](https://www.ncbi.nlm.nih.gov/books/NBK532881/)). Facial controls range from 0 to 2 for broader adjustments. Dragging a depth control directly facing the camera uses horizontal mouse movement; orbit for a projected depth direction. Facial controls are symmetric and intended to provide a sculpting base rather than finished anatomy. Whole-model placement remains available through World axes or Action Line; recipes retain those transforms when regenerated. Return to Sculpt mode (Done) to show the teal shape controls again.

Choose **Convert to sculpting mesh** to keep the previewed surface and remove its parametric recipe. Brushes, subdivision, and adaptive/voxel workflows become available after conversion; Undo restores the recipe. An unfilled Circle has no faces and must be filled before conversion; planes, grids, disks, and uncapped tubes remain open surfaces, so closed-surface remeshing/adaptive modes require closing them first. Primitive recipes save with each submodel and survive duplication, switching, undo/redo, and reopening projects. Collapsing the Primitive section keeps the parametric model. Its resolution controls support dragging/typing numbers or minus/plus. Shape, resolution, wireframe, and conversion controls share this one section; it disappears after conversion.

The **Detail** brush cuts a narrow groove with raised shoulders and pinches the surface toward the crease, for wrinkles and fine folds. Hold Alt (or your configured reverse modifier) to raise a sharp ridge instead. Adjust radius and strength as usual; masks, alpha, pressure and symmetry are supported. New projects include it; in existing projects use **Add brush → Detail**. For crisp features use sufficient subdivision or adaptive detail.

Toolbar actions share 16-pixel line icons and 28-pixel minimum heights. Project actions and modeling/view actions occupy separate rows and wrap on narrower windows. Brush previews use equal-sized responsive tiles, with names beneath the images, full-name tooltips and teal selection outlines.

In **View**, enable **Faint back-facing surfaces** to see the reverse sides of open surfaces translucently. **Back-face opacity** adjusts their visibility (15% by default). Front-facing surfaces remain opaque; backs behind solid foreground surfaces stay hidden. This is a session-only viewing option and does not change normals or sculpting/picking.

Wireframe options beside the Wireframe button select visible surfaces (edges hidden by other surfaces), front-facing edges, or transparent viewing of the whole frame. Triangle diagonals can be shown separately. Primitives use quad faces where possible; enable Triangle topology in the primitive editor to use triangles throughout. Quad faces survive subdivision, project saving, and OBJ export. Poles, caps, icospheres, and adaptive or voxel remeshing still use triangles.

Primitive resolution can also be changed with the diamond handles beside the model: drag left or right to change spans, sides, segments, rings or subdivision levels. The live count updates as you drag, and each drag is one Undo step. Teal circles identify radii and anatomical controls, axis-colored squares identify size controls, and the world transform and camera gizmo share labelled X/Y/Z colours. Hovering or dragging adds a white ring while retaining the control colour.

For renderer ownership, validation, and the opt-in load profile, see [GPU rendering](docs/gpu-rendering.md).


### Temporary thickness

In **Submodels → Thickness · open meshes**, enable **Temporary thickness** and set the total thickness in model units. The preview offsets half the amount to each side of the surface and adds walls around boundary loops. Orbit to inspect it. Each submodel saves its own value; duplication retains it. The original sculpting vertices, masks, subdivision levels, and surface picking remain unchanged until you apply it. OBJ exports the original surface while thickness is temporary.

**Apply thickness** turns the exact preview into a closed sculpting mesh, preserves masks and complete quads, clears the temporary setting, and resets subdivision levels and topology modes. Applying a primitive preview converts it to a sculpting mesh. Other submodels stay unchanged. Undo restores the original surface, recipe/levels, and thickness setting. Thickness works on consistently oriented open surfaces with nonbranching boundary loops; closed meshes and outlines without faces are not eligible. Large normal offsets can intersect at tight folds or concave areas, so inspect the preview before applying.

### Vertex groups and polygroups

In **Submodels → Vertex groups / Polygroups**, choose **Group masked** or **Group all**. Blank names automatically become **Group 1**, **Group 2**, and so on; enter a name when you want a custom label. Groups belong to each submodel and store independent soft vertex weights; creating, changing or recalling a group is undoable. Select a group to **Mask group**, **Add masked**, **Remove masked**, rename or delete it. Complete selected faces receive that group's polygroup color. Vertex groups can overlap, while each face has one category; partial face selections retain vertex membership without coloring the entire face. **View → Show polygroup colors** toggles the colors.

The **Mask** menu adds **Expand mask** and **Contract mask** (one mesh-edge ring per click), **Mask intensity** (scales soft falloff), and **Selection threshold** (minimum vertex mask for grouping, deletion and extraction). Zero mask values are excluded even at a zero threshold. These adjustments leave saved group weights intact. Reducing intensity to zero clears the mask after the adjustment ends; undo or recall a group to restore it.

Thickness adds **Group front / back / rim**, **Mask rim vertices**, and **Group masked vertices** options per submodel. Generated groups appear in the preview and become editable when thickness is applied. Grouping the generated mask can replace part face colors where complete faces meet the threshold; the part vertex groups remain available.

Groups survive project save/load, duplication, subdivision, extraction and deletion. Adaptive topology interpolates weights and preserves face categories at boundaries. Voxel remeshing transfers weights from nearest source vertices and categories from nearest source face centers, so boundaries are approximate after remeshing. OBJ exports face groups using `g` records; independent soft vertex weights are saved in `.clay` projects. The weights provide a foundation for future vertex painting; painting itself is not included yet.

New projects start with the sphere assigned to a **Base** vertex group/polygroup. The eye beside each vertex group hides or shows its assigned polygroup faces, including wireframe and picking. Hidden faces and their shared boundary vertices are protected from brush edits. Visibility is saved with the project and supports undo. Groups containing only partial vertex selections have no complete assigned faces to hide. To hide just the colors while keeping the whole surface visible, use **View → Show polygroup colors**.

Primitive span cut lines, span controls, and size controls share **X red, Y green, Z blue** colors. Plane/Grid X spans create red vertical cuts; Y spans create green horizontal cuts. Cube cut loops follow their local controlling axis, including after rotation or scaling. Cylinder/Cone height-span rings are green. The span labels and numeric controls use the same colors in the collapsible Primitive section in Submodels. Enable wireframe to see the colored topology; non-axis topology controls remain violet.

### Stroke modes

Choose a mode under **Brush → Stroke mode**; it is saved per brush in projects, presets and brush libraries. Existing brushes default to **Freehand**.

- **Freehand:** the existing continuous stroke.
- **Line:** press on the active model and drag a straight guide. Release to apply spaced dabs along that screen-space line, projected onto visible surfaces. Very long lines are limited to 257 dabs.
- **Drag Rect:** press to anchor one stamp, then drag to set its radius and alpha angle. The sculpted result previews live; release to keep it. Drag size is capped at the existing 500-pixel brush-radius limit; the preset's size and rotation remain unchanged.
- **Drag Dot:** move a fixed-size stamp with a live sculpt preview, then release to keep it.

With Adaptive sculpt and Refine enabled, drag stamps refine the surface before applying the alpha, with extra detail along sharp alpha boundaries. Very large stamps or very fine edge sizes can reach the bounded refinement limit.

Drag stamps rebuild their preview from the original mesh, so holding still does not build up depth. Relief brushes (Standard, Clay, Inflate, Crease and Detail) use four times the depth of one freehand dab; Strength and pressure still control the result. Mask opacity and smoothing weights retain their normal values.

Line and drag modes preview before changing document geometry, create one undo step, and support masks, alphas, symmetry and pen pressure. **Esc** cancels a pending stroke. Hidden faces cannot receive stamps. Grab and Snake keep their continuous Freehand behavior because they depend on pointer displacement.

# Clay Form

A Rust desktop sculpting prototype built with eframe/egui. Start from a dense sphere and sculpt real mesh vertices with a pen or mouse.

## Run

Install a current stable Rust toolchain, then:

```sh
cargo run --release
```

Linux requires a working Wayland or X11 desktop and OpenGL support. The first build downloads Cargo dependencies. `Cargo.lock` is committed for reproducible application builds.

## Sculpting

- Left drag or pen tip: sculpt. Right drag: orbit. Middle drag: pan. Wheel: zoom.
- Alt temporarily reverses the current brush. Shift temporarily uses Smooth.
- Standard, Clay, Inflate, Smooth, Flatten, Pinch, Crease, Grab, and Mask are included.
- Every brush uses an alpha, including Grab, smoothing and masking. Add/Subtract reverses its operation; subtracting Smooth sharpens, subtracting Flatten exaggerates surface relief, and subtracting Mask erases masking.
- Adjust radius, strength, alpha falloff, spacing, rotation, pressure strength/radius mapping, and pressure response curve. Grab moves the surface with the cursor; spacing does not affect Grab.
- Choose Soft, Hard or Square alpha, or load a PNG/JPEG up to 2048 × 2048 pixels. White affects the surface, black protects it, and transparency reduces influence. Custom alpha pixels are embedded in presets and projects.
- Duplicate any brush to create a named variation. Save/load a preset using its path field. Loading appends it to the library.
- Mirror X applies strokes on opposite sides of the object's X axis. Clear mask removes all masking. Undo/Redo keeps the last 24 sculpt strokes in memory.

## Files

Type a relative or absolute file path in the toolbar, then use Save or Open. `.clay` projects are versioned JSON containing geometry, masks, and the brush library. Presets use `.brush.json`. Existing files at the selected save path are replaced. Saving uses a temporary sibling file followed by rename. Parent folders must already exist.

New, Open and closing the window prompt when the project has unsaved changes. The prompt has a separate save path, protecting the file you intend to open. Export OBJ writes a mesh beside the project path, using the `.obj` extension. Projects restore the sculpt and presets; camera position and undo history are session-only. OBJ import is not yet implemented.

## Huion tablets

The tablet's normal desktop driver provides pen cursor movement and tip/button input. On Linux, Clay Form additionally discovers devices named Huion under `/sys/class/input` and reads normalized `ABS_PRESSURE` from their `/dev/input/event*` device without taking exclusive control. The sidebar shows connection errors and live pressure. Native touch/pen pressure events are also accepted where eframe provides them. Otherwise strokes use full pressure.

Your normal user needs read access to the tablet event device. If the sidebar reports permission denied, configure device-specific access through your distribution's input/udev facilities. Do not run the application as root. Device access and pressure have not yet been verified with a physical Huion tablet; model names/drivers that do not contain “Huion” need a discovery extension. Tilt, eraser detection, and tablet express-key configuration are not implemented.

## Scope

This is a first functional prototype, not a replacement for a production sculpting package. It uses a fixed triangle mesh and an orthographic viewport with CPU projection and egui rendering. There is no remeshing, subdivision, multiresolution, topology editing, layers, materials, or texture painting. Very deep folds can exceed the fixed mesh's useful resolution. Mesh/alpha file sizes are bounded on load.

## Development

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Tests cover the sphere's closed topology and outward normals, add/subtract deformation, mask protection, black alphas across every brush, project/preset disk saves with embedded custom alphas, and a simulated viewport pen stroke with undo.

Initialize the repository if it has not already been initialized:

```sh
git init -b main
git add Cargo.toml Cargo.lock .gitignore README.md src
git commit -m "Create Clay Form sculpting prototype"
```

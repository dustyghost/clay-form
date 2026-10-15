# Clay Form

A Rust desktop sculpting prototype built with eframe/egui. Start from a sphere and sculpt real mesh vertices with a pen or mouse.

## Run

Install Rust through rustup, then run from the repository root (rustup selects the version pinned in `rust-toolchain.toml`):

```sh
cargo run --release
```

Linux requires a working Wayland or X11 desktop and desktop OpenGL 3.3 or newer. The model viewport uses cached GPU buffers, depth-tested rendering, shader lighting, and surface alpha/symmetry overlays; egui draws the interface. The first build downloads Cargo dependencies. `Cargo.lock` is committed for reproducible application builds.

## Documentation

**Start here:** [Make your first sculpt](docs/guide/getting-started.md) — make a mark, smooth it, navigate, undo, and save.

| Guide | Topics |
| --- | --- |
| [Sculpting and brushes](docs/guide/sculpting.md) | Brushes, alphas, symmetry, stroke modes, adaptive sculpting, undo/redo |
| [Models and mesh tools](docs/guide/modeling.md) | Pen modeling, face extrusion, tunnels, component selection, submodels, primitives, transforms, mesh tools |
| [Edge loops and creases](docs/guide/edges.md) | Hole borders, support loops, edge selection, creasing, smooth subdivision, thickness rims |
| [Masks and selection](docs/guide/selection.md) | Mask editing, vertex groups, rectangle/lasso selection, visibility, split hidden |
| [Files and recovery](docs/guide/files.md) | Projects, brush files, OBJ import/export, automatic recovery |
| [Navigation and tablet input](docs/guide/navigation.md) | Orbit, pan, zoom, display options, Huion setup |
| [Reference images](docs/guide/references.md) | Embedded photographs/drawings, placement, opacity, locking, visibility |
| [Workspace and shortcuts](docs/guide/workspace.md) | Pinned menus, collapsible panels, custom shelves, workspaces, key bindings |

The guides include worked examples and app screenshots. Try [comparing view projections](docs/guide/navigation.md#example-compare-orthographic-and-perspective-views), [sculpting a fine fold](docs/guide/sculpting.md#example-cut-a-fine-fold), or [importing the sample pyramid](docs/guide/files.md#example-import-the-sample-pyramid).

For contributors: [development checks](docs/development.md), [architecture](docs/architecture.md), [GPU rendering and profiling](docs/gpu-rendering.md), [UI design rules](docs/ui-design.md), and [documentation style](docs/documentation-style.md).

See the [roadmap](docs/roadmap.md) for completed work, upcoming phases, and delivery checks. Keep detailed feature instructions in the relevant guide above when adding functionality.

## Scope

This is a desktop sculpting prototype. Meshes retain quad topology where available and use triangles for rendering and picking. The viewport supports orthographic and perspective viewing with OpenGL rendering; sculpting, picking, and topology operations run on the CPU. Subdivision, voxel remeshing, and local adaptive topology are implemented; quad retopology, sculpt layers, materials, and texture painting are future work. Meshes are limited to 500,000 vertices and 1,000,000 triangles; large scenes can still be limited by CPU editing and picking work. Mesh/alpha file sizes are bounded on load.

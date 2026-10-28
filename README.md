# Clay Form — Alpha 1

Version **0.1.0-alpha.1** is an early testing release.
On startup, a development notice explains the current version and testing
expectations. Choose **Continue to Clay Form** to begin; **Don’t show again for
this version** remembers your choice until the version changes.

A Rust desktop sculpting prototype built with eframe/egui. Start from a sphere and sculpt real mesh vertices with a pen or mouse.

Clay Form is free and open source under [GPLv3](LICENSE). Official downloads
are free of charge. **Your creations belong to you:** use, sell, license or
keep your models, textures and other artwork private without crediting Clay
Form or publishing source. Imported third-party material keeps its own terms.
See [licensing and ownership](COPYRIGHT.md).

Cross-platform packages are being prepared. See [release readiness and current
limits](docs/open-source-readiness.md); a configured build target is not yet a
claim of tested support. [Release instructions](.github/RELEASING.md) cover
Windows, macOS, Linux and ARM64 artifacts, corresponding source and checksums.

## Run

Windows packages require the [Microsoft Visual C++ v14 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)
for their CPU architecture if it is not already installed.

Install Rust through rustup, then run from the repository root (rustup selects the version pinned in `rust-toolchain.toml`):

```sh
cargo run --release
```

Linux needs a working Wayland or X11 desktop and a Vulkan-capable GPU and driver (Mesa lavapipe also supports software rendering). The first build downloads dependencies; later builds reuse them. The pinned Rust toolchain and committed `Cargo.lock` keep builds reproducible.

## Documentation

**Start here:** [Make your first sculpt](docs/guide/getting-started.md) — turn the starting sphere into a cratered pocket moon while learning the brushes, navigation, undo, and saving.

| Guide | Topics |
| --- | --- |
| [Sculpting and brushes](docs/guide/sculpting.md) | Brushes, alphas, symmetry, stroke modes, adaptive sculpting, undo/redo |
| [Editable 3D paths](docs/guide/paths.md) | Surface and free paths, one-drag rings, live brush guidance, automatic strokes, seam control |
| [Models and mesh tools](docs/guide/modeling.md) | Modeling, face extrusion, tunnels, component selection, submodels, primitives, transforms, remeshing and detail projection |
| [UVs, textures and GIMP](docs/guide/textures.md) | Seam-aware UVs, island editing, image painting and external texture editing |
| [Paint colours and materials](docs/guide/materials.md) | Vertex painting, material swatches, colour palette/sampler, fill and saving |
| [Edge loops and creases](docs/guide/edges.md) | Hole borders, support loops, edge selection, creasing, smooth subdivision, thickness rims |
| [Masks and selection](docs/guide/selection.md) | Mask editing, vertex groups, rectangle/lasso selection, visibility, split hidden |
| [Files and recovery](docs/guide/files.md) | Projects, brush files, OBJ import/export, diagnostic bundles, automatic recovery |
| [Navigation and tablet input](docs/guide/navigation.md) | Orbit, pan, zoom, display options, Huion setup |
| [Reference images](docs/guide/references.md) | Embedded photographs/drawings, placement, opacity, locking, visibility |
| [Workspace and shortcuts](docs/guide/workspace.md) | Pinned menus, collapsible panels, custom shelves, workspaces, key bindings |

The guides include worked examples and app screenshots. Try [comparing view projections](docs/guide/navigation.md#example-compare-orthographic-and-perspective-views), [sculpting a fine fold](docs/guide/sculpting.md#example-cut-a-fine-fold), or [importing the sample pyramid](docs/guide/files.md#example-import-the-sample-pyramid).

## For contributors

Start with the page that matches your change.

| What you’re doing | Read |
| --- | --- |
| Setting up or checking a change | [Development](docs/development.md) |
| Finding the code to change | [Architecture](docs/architecture.md) |
| Changing the viewport or measuring performance | [GPU rendering and profiling](docs/gpu-rendering.md) |
| Adding or changing controls | [UI design rules](docs/ui-design.md) |
| Writing a guide or updating screenshots | [Documentation style](docs/documentation-style.md) |
| Choosing the next feature | [Roadmap](docs/roadmap.md) |

When a feature changes, update its user guide alongside the code.

## Scope

This is a desktop sculpting prototype. Meshes retain quad topology where available and use triangles for rendering and picking. The viewport supports orthographic and perspective viewing with wgpu rendering; sculpting, picking, and topology operations run on the CPU. Subdivision, voxel remeshing, and local adaptive topology are implemented. Vertex and texture colour/metallic/roughness painting, UV unwrapping and GIMP image exchange are supported; quad retopology and sculpt layers are future work. Meshes are limited to 500,000 vertices and 1,000,000 triangles; large scenes can still be limited by CPU editing and picking work. Mesh/alpha file sizes are bounded on load.


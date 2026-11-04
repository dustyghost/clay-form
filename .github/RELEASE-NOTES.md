**Alpha 3 — 0.1.0-alpha.3.** An early testing release, with features and file
formats still evolving. Keep backups of important projects.
The startup notice can be dismissed for the current version; new prereleases
show it again.

Changes since Alpha 2:

- **Live arrays and path assemblies.** Attach Linear, Radial or Grid arrays to a submodel, adjust them with viewport handles, and edit the source or individual copies. Grouped path objects repeat a middle section between end regions, with bending, thickness and welding controls. Make results editable when ready to sculpt or export.
- **Reusable object brushes.** Capture multiple parts into a named brush, place linked objects or stamp raised/inset relief. A built-in Ship greebles brush provides reusable details.
- **Deformation and editable Boolean stacks.** Push or pull a deformation-frame corner, bend or twist a model. Combine grouped submodels with Add, Subtract and Intersect roles; choose Voxel or Preserve surfaces, preview the result, then bake it into a sculptable mesh.
- **Local mesh joining and modelling.** Join open ends or intersecting surfaces with an affected-area preview. Cut one cell from faces or edges, preview fuse targets, and inspect geometry with optional flat shading.
- **Faster dense sculpting.** Picking, brush work, shading and GPU uploads reuse more unchanged data. **Settings → Mesh capacity** lets you raise per-mesh limits after a restart, subject to available hardware.
- **Separate materials, colours and textures.** Choose stroke materials and Primary/Secondary colours independently. The texture library manages images separately from model assignment, with explicit painting destinations and shared-material editing.
- **Navigation and pen fixes.** Orbit follows the current screen axes. Windows pen routing preserves pressure and completed UI taps while keeping finger navigation separate.

**Project compatibility:** Alpha 3 reads native project formats 1–5 and saves
format 6 for the new reusable-model workflows. Earlier application versions
cannot open format 6 projects. Keep a separate backup before saving an older
project in Alpha 3.

Clay Form is free and open source under GPLv3. Your models, textures, renders
and other creations remain yours to use, sell, license or keep private.

Download the archive for your operating system and CPU. `aarch64` means ARM64;
Apple Silicon Macs use `macos-aarch64`. Linux archives target glibc 2.35+
desktops with a working Vulkan driver. They are not Alpine/musl builds.

Windows requires the Microsoft Visual C++ v14 runtime for the selected
architecture. Use [Microsoft's supported redistributable installer](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)
if it is not already installed. The runtime is not bundled with Clay Form.

**Corresponding source:** the `source.tar.gz` asset contains the exact app
source, build scripts, lockfile and vendored Cargo dependencies for these
binaries. See its README and pinned Rust toolchain to rebuild. The compiler
and operating-system SDKs are installed separately. Licence notices are
included in each binary archive and in the app's About window. `SHA256SUMS`
contains archive hashes.

Testing to date includes maintainer use on Arch Linux, automated CPU and headless Vulkan checks, and recorded visual/interaction reviews of arrays, path assemblies, object brushes, deformation, Boolean results, mesh joining and material/texture workflows. Windows x86-64 and ARM64 packaging checks also exercise pen routing and brush selection. Desktop, GPU and pen compatibility still needs broader user testing, particularly on Windows and macOS; a native build alone does not establish runtime compatibility.

Windows packages are unsigned. macOS packages have an ad-hoc signature and are not notarized. Operating systems may show download or launch warnings.

Meshes default to 500,000 vertices and 1,000,000 triangles. Configurable limits extend to 16 million vertices and 32 million triangles per mesh, but these are safety ceilings, not a performance guarantee. Large scenes can become slow or exceed available memory well below them. Live instances have separate project and expanded-geometry budgets; make an array editable before exporting its result. Preserve surfaces requires closed, consistently oriented solids, and local joins still need wider production-sculpt testing. Please include your OS, GPU and input device when reporting problems.

[Changes since Alpha 2](https://github.com/dustyghost/clay-form/compare/v0.1.0-alpha.2...v0.1.0-alpha.3).

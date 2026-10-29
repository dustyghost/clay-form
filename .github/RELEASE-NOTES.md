**Alpha 2 — 0.1.0-alpha.2.** An early testing release, with features and file
formats still evolving. Keep backups of important projects.
The startup notice can be dismissed for the current version; new prereleases
show it again.

Changes since Alpha 1:

- Paint's **+ Colour** opens a colour picker with Add colour and Cancel.
- New models use the approved teal material and improved three-point studio lighting. Vertex-group colours start disabled so the material is visible. Existing saved lighting is retained; use **View → Lighting → Reset studio lighting** to apply the new rig.
- **Import → OBJ…** is available beside Export. OBJ import accepts the trailing NUL padding written by ZBrush while preserving geometry and UVs.
- **F / Reset view** restores the front orthographic view, centers visible models and zooms out enough to show them with a margin. Hidden geometry does not affect the fit, and large imports can zoom below 20%.
- The README features the new Head Sculpt workspace image, and the navigation and import guides describe the updated behavior.

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

Testing to date includes maintainer use on Arch Linux, automated CPU and headless Vulkan checks, and visual review of the palette, studio lighting and imported-model framing. The supplied ZBrush OBJ imported with 70,896 vertices, 70,894 quads and UV mappings intact. Windows, macOS and other Linux desktop environments await user testing; a native build does not establish runtime or pen compatibility.

Windows packages are unsigned. macOS packages have an ad-hoc signature and are not notarized. Operating systems may show download or launch warnings.

Meshes are limited to 500,000 vertices and 1,000,000 triangles; large scenes can become slow before these limits. This remains an early alpha with evolving features and file formats. Please include your OS, GPU and input device when reporting problems.

[Changes since Alpha 1](https://github.com/dustyghost/clay-form/compare/v0.1.0-alpha.1...v0.1.0-alpha.2).

# wgpu renderer migration

Status: proposed, implementation pending.

## Objective
Replace the OpenGL/glow viewport with wgpu while retaining egui/eframe, CPU sculpting, picking, scene data, file formats, and pen input behaviour.

## Current integration points
- `Cargo.toml`: eframe enables `glow`, not `wgpu`.
- `src/main.rs`: explicitly selects `eframe::Renderer::Glow`.
- `src/app.rs`: application initialization and shutdown depend on `eframe::glow::Context`.
- `src/viewport/gpu.rs`: custom OpenGL draw callback, buffers, render targets.
- `src/viewport/gpu/shaders.rs`: OpenGL shader programs to port to WGSL.
- `src/viewport/gpu/ui_capture.rs` and `profile.rs`: GL-dependent screenshot and profiling paths.

## Implementation sequence
1. Introduce a renderer-neutral viewport interface and keep existing behaviour unchanged.
2. Enable eframe wgpu and implement the viewport via egui_wgpu paint callbacks, preserving egui overlays, clipping, DPI scaling and depth.
3. Port mesh buffers, masks, materials, wireframe, brush alpha and symmetry overlays to wgpu/WGSL.
4. Move initialization, lifecycle, resource invalidation and resize handling to the wgpu implementation.
5. Migrate screenshot capture, profiling and headless tests. Ensure CI can run GPU checks using software Vulkan where available.
6. Remove glow/OpenGL dependencies and update architecture, setup and development documentation.

## Acceptance criteria
- App launches and viewport renders correctly on supported Linux Wayland/X11 environments.
- Sculpting, navigation, picking, undo/redo, topology operations and pen interaction remain unchanged.
- Wireframe, masking, lighting, symmetry and brush alpha overlays match existing behaviour.
- Screenshots and UI tests work with wgpu, and CI is green.
- No regression in project save/load or geometry limits.
- Document tested GPU adapters, backends and known limitations.

Do not merge until migration is implemented and validated. No graphics API rewrite is required for CPU sculpting logic.

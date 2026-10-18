# wgpu renderer migration

Status: implemented. Linux software Vulkan validation passes; retain the draft PR for CI completion and physical GPU/tablet sign-off.

## Scope and implementation

The viewport and egui use eframe's wgpu renderer. CPU sculpting, picking, scene data, pen event handling, undo/history, and project/preset formats are unchanged.

- `Cargo.toml` enables eframe wgpu and explicit Vulkan, Metal and DX12 backends; glow is no longer an application dependency.
- `src/main.rs` selects `Renderer::Wgpu`; `App::with_wgpu` initializes from eframe's render state. Reference-counted wgpu resources replace context-dependent GL destruction.
- `src/viewport/gpu/state.rs` defines renderer-neutral view/preview inputs. The existing `display.rs` geometry and revision cache remain unchanged.
- `src/viewport/gpu.rs` implements prepare/paint callbacks, packed indexed mesh storage, face colors, lazy wireframe buffers, separate opaque/backface/line pipelines, and RGBA8/Depth32Float targets. Surface and edge batches respect device buffer limits without reducing document geometry limits.
- `viewport.wgsl` preserves projection, lighting, masks, polygroups, brush rotation/falloff/image interpolation, symmetry guides and front-facing wireframe semantics. Clip depth is converted to WebGPU's range. Bilinear R32Float loads avoid optional float-filtering requirements.
- `composite.wgsl` draws the viewport inside egui's viewport/scissor and preserves display RGB on both linear and sRGB attachments. Physical target dimensions use egui's endpoint rounding at fractional DPI.
- `ui_capture.rs` replaces EGL with surface-free Vulkan capture, padded buffer copies and asynchronous mapping. The fixture's bottom-up pixel convention remains at the readback boundary; PNG exports remain upright.
- The release load profile waits for wgpu completion and records adapter/backend metadata. CI and screenshot regeneration use Mesa Vulkan.

## Validation record — 2026-10-18

Environment: Debian 13 x86_64, Rust 1.99.0, wgpu 27.0.1, Mesa 25.0.7 lavapipe (`llvmpipe`, LLVM 19.1.7, CPU adapter), Vulkan. Mesa and the headless servers were extracted into the validation workspace; production only needs a working Vulkan driver on Linux.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo test --locked` | 412 passed, 33 opt-in tests ignored |
| Ignored viewport correctness tests, excluding load profile | 12 passed |
| Ignored `offscreen_preview` app/control captures | 10 passed (includes the viewport customization capture) |
| Release `gpu_load_profile` | Passed at all six scene densities/model counts |
| Native X11 on Xvfb 21.1.16 | Window mapped; survived 900×650, 1600×1000 and 1280×850 resizes |
| Native Wayland on Weston 14.0.2, headless/pixman | App remained running through a timed startup smoke test, with no reported surface/shader errors |

The viewport regressions retain depth/cache, CPU-alpha agreement, mask, backface, symmetry, quad/triangle wireframe, clipping, slice, visibility, lasso, rotation and perspective assertions. Additional checks compare forced small-buffer batches with unbatched pixels, verify non-aligned readback rows, and check fractional-DPI sizing, scissor clipping and sRGB composition. Existing application regressions cover save/load, legacy JSON, undo/redo, topology and simulated pen/navigation input.

See [GPU rendering and profiling](gpu-rendering.md) for reproducible commands and software profile data.

## Remaining acceptance checks

- Confirm GitHub CI on the final branch head.
- Validate sustained sculpting and real pressure/tilt input on supported physical Linux Wayland/X11 desktops.
- Measure hardware GPU performance and compare against the glow baseline on the same machine. Lavapipe timings validate the profiling path, not hardware performance parity.
- Metal and DX12 are enabled native backends but have not been exercised here; Linux Vulkan is the tested configuration.

Keep PR #2 in draft until outstanding acceptance checks are satisfied. Do not merge before validation is complete.

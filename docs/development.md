# Development

[Documentation home](../README.md#documentation)

Run commands from the repository root.

## Checks and profiling

Before opening a pull request, run:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

CI runs these checks and a separate Mesa Vulkan job for GPU correctness. For graphics setup, GPU tests and app previews, follow [GPU validation](gpu-rendering.md#validation).

The ordinary tests cover brushes and masks, legacy JSON, project and preset saves, strokes, bounded undo/redo, failed loads, save/open protection, and simulated viewport/gizmo gestures.

**Changing Rust?** `rust-toolchain.toml` pins the compiler, Clippy and rustfmt for local work, CI and releases. Rustup installs that toolchain when needed. Upgrade the pin deliberately; run the checks above and GPU validation before committing it.

**Measuring performance?** See [GPU rendering and profiling](gpu-rendering.md). To measure document transforms and mask edits without a window:

```sh
cargo run --release --locked --example edit_profile
```

## Update guide screenshots

1. On Linux, install the [Mesa Vulkan prerequisites](gpu-rendering.md#validation).
2. Run:

   ```sh
   bash scripts/update-doc-screenshots.sh
   ```

3. Inspect the images in `docs/images/` and any extra frames in the temporary directory printed by the script.
4. Update affected guide steps and captions, then the [screenshot inventory](images/README.md). Follow the [documentation style](documentation-style.md) before committing.

The script captures the real app UI and wgpu renderer offscreen using synthetic scenes. It needs no desktop window, uses software rendering and defaults to two Mesa worker threads. It copies selected PNGs only after **every capture test passes**.

The edge workflow review also clicks the real controls: selection, navigation, crease removal, both subdivision operations, loop targets, thickness application and Undo. These captures check appearance and behavior; use the profiling tools for performance measurements.

To render a local `.clay` project for a bug investigation, set `CLAY_REVIEW_PROJECT` to its path and `CLAY_GUIDE_PREVIEW` to an output directory, then run the ignored `project_file_offscreen_review` test. Optionally set `CLAY_REVIEW_MANIFEST` to a diagnostic bundle's `manifest.json` to restore its current camera. This writes `project-review.png` using the real renderer without editing the project.

## Architecture and roadmap

- [Architecture](architecture.md): document ownership, sculpting algorithms, UI commands, camera/navigation, rendering/picking, persistence and tablet input.
- [Roadmap](roadmap.md): planned sculpting improvements, priorities and completion criteria.
- [UI design rules](ui-design.md): requirements for new or changed controls.

Keep existing version 1 projects and brush presets compatible.

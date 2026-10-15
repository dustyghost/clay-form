# Development

[Documentation home](../README.md#documentation)

Run these commands from the repository root.

`rust-toolchain.toml` pins the compiler, Clippy, and rustfmt used locally and by CI/release builds. Rustup installs the specified toolchain when needed. Upgrade the version in that file deliberately, then run the checks below and the GPU validation before committing the upgrade.

## Checks and profiling

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Tests cover brush and mask behavior, legacy JSON compatibility, project/preset disk saves, stroke and bounded undo/redo, failed loads, save/open protection, and simulated viewport/gizmo gestures.

CI runs formatting, the ordinary test suite, and Clippy, plus a separate Mesa/EGL job for the GPU correctness tests. See [GPU validation](gpu-rendering.md#validation) for the Linux graphics checks, including the app previews.

For manual performance measurements, see [GPU rendering and profiling](gpu-rendering.md). `cargo run --release --locked --example edit_profile` measures document transform and mask-edit costs without opening a window.

## Update guide screenshots

On Linux with Mesa/EGL installed, run:

```sh
bash scripts/update-doc-screenshots.sh
```

The script runs the real application UI and OpenGL renderer offscreen with synthetic scenes, then copies the selected PNGs into `docs/images/` only after every capture test passes. It uses software rendering and defaults to two Mesa worker threads; no desktop window is needed. The temporary capture directory is printed at the end and retains extra frames for inspection. The edge workflow review also clicks the real controls to check selection, navigation, crease removal, both subdivision operations, loop targets, thickness application, and Undo.

Review the resulting images before committing them, and update the guide's steps and captions whenever controls or behavior change. Follow the [documentation style](documentation-style.md): short tasks, searchable headings, and images that show the relevant control or result. See the [screenshot inventory](images/README.md) for capture sources and the [GPU validation prerequisites](gpu-rendering.md#validation) for graphics setup. These captures check appearance and behavior; they are not performance measurements.

## Architecture and roadmap

See the [development roadmap](roadmap.md) for planned sculpting improvements, priorities, and completion criteria.

The app separates document ownership, sculpting algorithms, UI commands, camera/navigation, rendering/picking, persistence, and tablet input. See [the architecture guide](architecture.md) for responsibilities and extension points. Existing version 1 projects and brush presets remain compatible.

See also the [UI design rules](ui-design.md) when adding or changing controls.

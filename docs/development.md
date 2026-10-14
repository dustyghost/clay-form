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

## Architecture and roadmap

See the [development roadmap](roadmap.md) for planned sculpting improvements, priorities, and completion criteria.

The app separates document ownership, sculpting algorithms, UI commands, camera/navigation, rendering/picking, persistence, and tablet input. See [the architecture guide](architecture.md) for responsibilities and extension points. Existing version 1 projects and brush presets remain compatible.

See also the [UI design rules](ui-design.md) when adding or changing controls.

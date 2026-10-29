# Development

[Documentation home](../README.md#documentation)

Use this guide to prepare a focused contribution, check it and update its documentation. Run commands from the repository root.

## Set up a contribution

1. Follow the [run instructions](../README.md#run) and launch Clay Form from your checkout. Rustup selects the compiler pinned in `rust-toolchain.toml`.
2. Create a branch for one change. For substantial work, agree the scope in an issue first.
3. Find the responsible module in [Architecture](architecture.md#ownership-and-responsibilities). Read its detailed contract before changing an edit or interaction.
4. Reproduce the problem or try the workflow you plan to improve. Choose the smallest useful check for that change.

You should now have a working app, a clear change boundary and a way to recognise success. Keep existing version 1 projects and brush presets compatible.

## Prepare a pull request

1. Make the focused change and update the affected user guide when behaviour changes.
2. Run the relevant checks described below. Review both the successful path and any affected cancellation, invalid-input or undo behaviour.
3. Review your diff for unrelated edits, private paths, artwork and diagnostic bundles.
4. Open a PR describing the problem, resulting behaviour and validation. Link its issue and record any checks that still need a real device or platform.

PR CI runs the full configured checks. A successful build alone does not establish that a feature looks right or works on physical input hardware. For Alpha 1 documentation and publication checks, use the [Alpha 1 review checklist](alpha-1-documentation-review.md).

## Checks and profiling

During development, run focused tests for the changed behaviour. PR CI runs the full checks below; do not duplicate them locally before opening or updating a PR:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

CI runs these checks and a separate Mesa Vulkan job for GPU correctness. For graphics setup, GPU tests and app previews, follow [GPU validation](gpu-rendering.md#validation).

The ordinary tests cover brushes and masks, legacy JSON, project and preset saves, strokes, bounded undo/redo, failed loads, save/open protection, and simulated viewport/gizmo gestures.

### Change the Rust toolchain

`rust-toolchain.toml` pins the compiler, Clippy and rustfmt for local work, CI and releases. Rustup installs that toolchain when needed.

Upgrade the pin deliberately. Run the checks above and GPU validation before committing it.

### Measure performance

See [GPU rendering and profiling](gpu-rendering.md) for viewport measurements. To measure document transforms and mask edits without a window:

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

The edge workflow review also clicks the real controls: selection, navigation, crease removal, both subdivision operations, loop targets, thickness application and Undo. These captures check appearance and behaviour; use the profiling tools for performance measurements.

### Render a project for investigation

1. Set `CLAY_REVIEW_PROJECT` to the local `.clay` file and `CLAY_GUIDE_PREVIEW` to an output directory.
2. Optionally set `CLAY_REVIEW_MANIFEST` to a diagnostic bundle's `manifest.json` to restore its current camera.
3. Run the ignored project review test:

   ```sh
   cargo test --locked --lib project_file_offscreen_review -- --ignored --test-threads=1
   ```

The test writes `project-review.png` using the real renderer without editing the project. Inspect that image before using it as evidence.

## Architecture and roadmap

- [Architecture](architecture.md): document ownership, sculpting algorithms, UI commands, camera/navigation, rendering/picking, persistence and tablet input.
- [Roadmap](roadmap.md): planned sculpting improvements, priorities and completion criteria.
- [Adaptive sculpt research review](research/adaptive-sculpt-review.md): smoothing export findings, published methods, and proposed quality and performance work.
- [UI design rules](ui-design.md): requirements for new or changed controls.

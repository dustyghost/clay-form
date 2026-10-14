# Guide screenshots

These PNGs are actual offscreen captures of Clay Form's egui UI and OpenGL viewport, using synthetic practice scenes. They are not mockups or edited illustrations. Each image is displayed beside steps and a caption in the corresponding user guide.

Regenerate from the repository root on Linux with Rust and Mesa/EGL installed:

```sh
bash scripts/update-doc-screenshots.sh
```

The script preserves the original PNGs without resizing or annotation. It sets `EGL_PLATFORM=surfaceless`, `LIBGL_ALWAYS_SOFTWARE=1`, and defaults `LP_NUM_THREADS` to `2`. Capture paths are isolated in a temporary directory; documentation images are copied only after all selected tests pass. See [Development](../development.md#update-guide-screenshots) and [GPU validation](../gpu-rendering.md#validation).

| Images | Capture test | Guide |
| --- | --- | --- |
| `view-orthographic.png`, `view-perspective.png` | `perspective_view_offscreen_preview` | [Navigation](../guide/navigation.md#example-compare-orthographic-and-perspective-views) |
| `detail-groove.png` | `detail_groove_offscreen_preview` | [Detail brush](../guide/sculpting.md#example-cut-a-fine-fold) |
| `brush-settings.png` | `navigation_and_smooth_offscreen_preview` | [Brush reset](../guide/sculpting.md#example-reset-an-experimental-brush) |
| `mask-lasso-preview.png`, `mask-lasso-result.png` | `gpu_lasso_preview_and_commit` | [Mask lasso](../guide/selection.md#example-protect-a-region-with-a-lasso) |
| `reference-images.png` | `reference_images_offscreen_preview` | [References](../guide/references.md#example-keep-two-studies-beside-the-sculpt) |
| `obj-import.png` | `obj_import_offscreen_preview` | [OBJ import](../guide/files.md#example-import-the-sample-pyramid) |
| `recovery-dialog.png`, `menu-placement.png` | `recovery_and_menu_placement_offscreen_preview` | [Recovery](../guide/files.md#example-make-a-recovery-checkpoint), [Workspace](../guide/workspace.md#example-keep-frequently-used-controls-open) |
| `rotation-controls.png` | `gpu_rotation_controls_and_primitive_regeneration` | [Transforms](../guide/modeling.md#example-rotate-a-cube-and-keep-its-recipe) |

The OBJ example's geometry is also available as [pyramid.obj](../examples/pyramid.obj). The reference images are generated test fixtures. Temporary filenames and recovery paths visible in captures are test data and may change when regenerated. Your own sidebar layout, brush library, and project names can differ.

When refreshing images, inspect all changed PNGs at readable size, check that captions still match, and keep their relative Markdown links intact. Update the capture fixture when a clearer example requires a different scene or layout.

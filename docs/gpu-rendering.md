# GPU model rendering

[Documentation home](../README.md#documentation)

Clay Form retains egui for the interface and uses eframe’s wgpu backend for a dedicated 3D viewport. Linux requires Vulkan; the enabled native backends also include Metal and DX12. CPU sculpting and project file formats are unchanged.

Use this page to choose a rendering check, export a preview, or measure a slowdown. For ownership and cache lifecycle, see [GPU architecture](architecture/integration.md#gpu-viewport).

| Your task | Start here |
| --- | --- |
| Check shaders, depth, or picking agreement | [Validation](#validation) |
| Export guide images or a regression preview | [Preview images](#preview-images) |
| Measure rendering and picking costs | [Load profile](#load-profile) |
| Measure transforms and masks without a window | [Document edit profile](#document-edit-profile) |
| Change shading, groups, or wireframe | [Surface rendering](#surface-rendering) |
| Check slices, visibility, or rotation controls | [Focused regressions](#focused-regressions) |

## Rendering behavior

The renderer splits surface and wireframe buffers at device allocation/binding limits, preserving the existing per-model geometry limits across dense multi-model scenes. It caches positions, normals, masks, materials, triangle indices, and model ownership. Camera changes update shader uniforms. Geometry/mask edits and model visibility/selection changes invalidate the cache; undo, redo, New, Open, and completed topology jobs do the same. Wireframe edge buffers are generated only when wireframe is requested after an edit. Quad diagonals remain optional.

An offscreen color/depth framebuffer depth-tests overlapping submodels.

| Wireframe mode | Behavior |
| --- | --- |
| Visible | Uses the surface depth buffer. |
| Front | Shows front-facing edges through surfaces. |
| Transparent | Shows every edge without surface fill. |

The View menu optionally draws back-facing surfaces in a separate translucent pass (15% default opacity). That pass depth-tests against opaque front faces but does not write depth, allowing surfaces behind a back to remain visible. Lighting reverses the display normal for that pass; geometry normals and CPU front-surface picking are unchanged. Transparent wireframe continues to omit surface fills.

Lighting runs per fragment. The alpha footprint and symmetry guides run in the solid surface shader and stay on active visible surfaces. Tangent rings, the normal indicator, gizmos, and ordinary controls remain egui overlays.

Mesh deformation, subdivision, remeshing, undo, and surface picking remain CPU operations. Picking scans triangles, using projected barycentrics in orthographic view and ray intersections in perspective view; moving rendering to the GPU does not remove that cost. The renderer uses the same immutable mesh snapshot for display and picking throughout each frame, including frames containing adaptive topology edits.

## Validation

The ordinary test suite retains a CPU paint path for headless UI and picking tests. CI also runs every ignored GPU correctness test in a separate Linux job using Mesa lavapipe software Vulkan. Install `mesa-vulkan-drivers` and `libvulkan1` (plus the usual build dependencies). No EGL library, display server or window is required:

```sh
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib viewport::gpu:: -- --ignored --skip gpu_load_profile --test-threads=1
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib recovery_and_menu_placement_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib obj_import_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib reference_images_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib navigation_and_smooth_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib perspective_view_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib closed_cylinder_path_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib live_detail_path_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib one_drag_ring_authoring_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib rounded_path_corners_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 cargo test --locked --lib automatic_path_settings_offscreen_preview -- --ignored
```

The load profiler, diagnostic export reviews and manual-only brush captures are excluded from routine CI. Software rendering checks shader, framebuffer, picking/display, and UI composition correctness; it is not a hardware performance measurement.

The focused checks remain available for local debugging. On a Linux host with Vulkan installed, the opt-in wgpu test compiles the real shaders and reads back rendered pixels:

```sh
cargo test --locked gpu_offscreen -- --ignored --nocapture
```

For the translucent back-face pass, run `cargo test --locked gpu_backfaces -- --ignored --nocapture`. This checks adjustable opacity, front/behind occlusion, closed-sphere appearance, and geometry/cache stability.

The main offscreen test checks depth occlusion, wireframe modes, quad topology, cache reuse, alpha texture replacement, agreement between shader and CPU alpha weights, inactive-model protection, symmetry guides, framebuffer resizing, cleanup, and egui composition. To save its composed preview:

```sh
CLAY_GPU_PREVIEW=/tmp/clay-gpu-preview.png cargo test --locked gpu_offscreen -- --ignored --nocapture
```

### Preview images

The following captures are **manual visual review**, not routine CI checks. They render images without asserting visual correctness. Inspect both shaded and wireframe images after running:

```sh
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_BUILDUP_PREVIEW=/tmp/clay-buildup cargo test --locked --lib clay_buildup_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_DETAIL_PREVIEW=/tmp/clay-detail cargo test --locked --lib detail_groove_offscreen_preview -- --ignored
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_GUIDE_PREVIEW=/tmp/clay-inflate-relax cargo test --locked --lib inflate_relaxation_manual_capture -- --ignored
CLAY_GPU_SOFTWARE=1 LP_NUM_THREADS=2 CLAY_GUIDE_PREVIEW=/tmp/clay-relax-brush cargo test --locked --lib relax_brush_manual_capture -- --ignored
```

Remaining automated coverage:

| Manual capture | Ordinary regression coverage retained in CI |
| --- | --- |
| Clay buildup | `sculpt::clay::tests` checks buildup, sampled planes, pressure, masks, alpha and symmetry; `clay_viewport_stroke_restores_geometry_and_topology_with_one_undo` checks fixed/adaptive strokes and Undo/Redo. |
| Detail groove | `adaptive_detail_groove_does_not_fold_with_or_without_stabilization` uses the same scene and checks closed geometry, normals, incision, shoulders, untouched back faces and Undo/Redo; sculpt tests retain depth/inversion and protected-region checks. |
| Inflate → Relax | `relaxation_after_aggressive_inflate_reduces_stretch_and_retains_the_bulge` checks stretch, surface deviation, bulge, masks, creases, topology and attributes. Separate relaxation tests retain groove, wider redistribution, boundaries, hidden faces, nearby sheets and cancellation coverage. |

`mesh_refinement_offscreen_review` remains in GPU CI: it clicks Preview/Apply/Cancel, verifies Undo/Redo and point-split controls, and checks compact-window scrolling. Only its independent Inflate image sequence moved to `inflate_relaxation_manual_capture`. Core GPU pixel, depth and clipping tests are unchanged. Export-review tools and the load profiler were already manual and remain available.

The perspective regression compares real framebuffer coverage against CPU picking, checks brush-alpha agreement, wireframe, symmetry guides and camera-only cache reuse, and exercises near-plane crossings and distant geometry. `perspective_view_offscreen_preview` renders the View controls in both projections, exporting `/tmp/clay-perspective-orthographic.png` and `/tmp/clay-perspective-ui.png` for visual inspection. Set `CLAY_PERSPECTIVE_PREVIEW` to change the output prefix.

`closed_cylinder_path_offscreen_preview` renders an editable surface loop on a cylinder from front and back views. It checks that the path overlay appears over the real GPU-rendered model while camera changes preserve the document, and exports `/tmp/clay-paths-front.png` and `/tmp/clay-paths-back.png`. Set `CLAY_PATHS_PREVIEW` to change the output prefix. Inspect the solid front-facing guide, dim dashed rear-facing guide, seam marker, and path controls in these images.

`live_detail_path_offscreen_preview` checks full trajectory coverage over three Detail passes and exports `/tmp/clay-live-path-*.png` for visual review. `one_drag_ring_authoring_offscreen_preview` exercises pen-drag authoring in both loop modes and exports `/tmp/clay-ring-level.png` and `/tmp/clay-ring-shortest.png`, including the ring controls. `rounded_path_corners_offscreen_preview` exports `/tmp/clay-rounded-path-corners.png` with a rounded surface guide, selected-corner controls, and draggable rounding handles.

`automatic_path_settings_offscreen_preview` renders the shared brush settings and Undo/Redo controls, then undoes an automatic stroke with the dialog open. Inspect `/tmp/clay-automatic-path-settings.png` and `/tmp/clay-automatic-path-undone.png`.

When the local ring diagnostic export is available, `CLAY_GPU_SOFTWARE=1 CLAY_RING_DIAGNOSTICS=stroke-along-ring-path cargo test --release --locked --lib exported_ring_automatic_stroke_offscreen_preview -- --ignored --nocapture` replays its final stroke with the recorded brush and adaptive settings. It writes `/tmp/clay-ring-exported.png` and `/tmp/clay-ring-fixed.png`, plus `-wire.png` versions, without changing the export. These images compare the seam and refinement around the ring; the private diagnostic files are not required by CI.

For the recent brush previews, set `CLAY_DETAIL_PREVIEW=/tmp/clay-detail` when running `detail_groove_offscreen_preview` to save `-solid.png` and `-wire.png` images. Set `CLAY_NAVIGATION_PREVIEW=/tmp/clay-navigation` when running `navigation_and_smooth_offscreen_preview` to save `-standard.png` and `-smooth.png` images at 800% zoom, plus `-brush-settings.png` at 100%. These tests still run without the variables; only the image export is optional.

To regenerate all checked-in guide images, use `bash scripts/update-doc-screenshots.sh`; see the [screenshot inventory](images/README.md). The OBJ and recovery/menu previews also accept full output paths through `CLAY_OBJ_PREVIEW`, `CLAY_RECOVERY_PREVIEW`, and `CLAY_MENU_PREVIEW`, retaining their existing `/tmp/clay-*.png` defaults when unset.

## Load profile

Run the repeatable profile in release mode, while other CPU/GPU-intensive work is idle:

```sh
CLAY_GPU_PROFILE=/tmp/clay-gpu-profile.csv cargo test --release --locked gpu_load_profile -- --ignored --nocapture
```

The workload includes approximately 4.7k, 19k, 75k, and 301k vertices in a single model, plus four separate models at the low and medium densities. All models use the requested density rather than hiding the cost behind inactive-model LODs. The viewport is 256 × 256 physical pixels.

The CSV records display-mesh preparation, initial buffer upload, CPU view preparation, picking, solid/wireframe frame times, and Standard-brush dab time. Renderer initialization is submitted and completed before upload timing starts. Upload timing includes buffer creation, staging copies, queue submission, and GPU completion. Rendering measurements also finish GPU work before stopping the clock and exclude the first framebuffer/edge-buffer creation draw. Median and approximate 95th percentile use 16 camera frames. These are diagnostic samples, not CI performance thresholds or a complete end-to-end input-latency benchmark. The first dab includes the undo snapshot; later dabs share the stroke.

The output identifies the wgpu adapter and backend. A `llvmpipe` result measures software rendering on the CPU and cannot establish hardware GPU performance. Compare runs on the same driver, hardware, resolution, and workload. Native application profiling at normal window size should also cover sustained strokes, adaptive topology, masks, symmetry, high-DPI screens, and repeated New/Open/undo/redo to check memory stability.

Use those measurements to choose the next optimization. Likely candidates to investigate are a spatial index for picking, limiting uploads and normal updates to changed mesh regions, and spatial brush-neighborhood queries. Higher-density sculpting may need CPU geometry work even when drawing is fast.

### Software Vulkan validation sample

The [2026-10-19 CSV](validation/wgpu-lavapipe-2026-10-19.csv) records a release run on Arch Linux, Rust 1.99.0 and Mesa 26.2.4 lavapipe (LLVM 23.1.1), at 256 × 256 pixels with two Mesa worker threads. Solid-frame medians were 4.003 ms for 4,706 vertices and 105.177 ms for 301,058 vertices; visible-wireframe medians were 8.737 ms and 261.962 ms respectively. This replaces the initial 2026-10-18 sample, whose upload timing omitted pending GPU copies. Upload measurements now include submission and completion, with renderer initialization completed beforehand. The environment also changed, so the two runs are not a performance comparison. These CPU software-renderer timings are diagnostic and do not establish hardware performance parity with glow. The profile excludes surface presentation and real input latency.

## Adaptive Detail path profile

Use a project containing a saved surface path to profile adaptive sculpting without rendering. Build once, then run the executable while other CPU-intensive work is idle:

```sh
cargo build --release --locked --example path_profile
target/release/examples/path_profile before-stroke.clay 0.21 96
target/release/examples/path_profile before-stroke.clay 0.21 0 /tmp/detail-result.clay
```

The arguments are the source project, brush radius in model units, dab limit (`0` for the complete path), and an optional output project. The profile uses the first saved path, the default Detail brush (strength 0.35, spacing 0.08), constant full pressure, and the project's adaptive and symmetry settings. It refreshes the adaptive budget for each dab, matching automatic path refinement, and reports sampling, each 32-dab interval, vertex growth, and final path reattachment separately. Its streaming session resets the live work counter between dabs; automatic whole-path strokes additionally enforce a cumulative work limit. The input project is never modified.

Compare the same project, radius and dab count across builds. Check resulting projects as well as elapsed time: reducing refinement or skipping relaxation can make a run faster while changing the sculpt. These timings exclude viewport rendering and pen input latency.

A local release comparison on 2026-10-21 (Intel Core i7-7500U) used the exported cylinder ring, radius 0.21, and the default Detail brush. The 430-dab loop grew from 1,056 to 61,223 vertices. Measured stroke times, excluding project loading/saving and final path reattachment:

| Detail workload | Before | After |
| --- | ---: | ---: |
| Complete 430-dab loop | 72.58 s | 28.23 s |
| Another 32 dabs on the completed, dense ring | 7.87 s | 2.76 s |

The resulting projects matched byte for byte, including an intermediate 96-dab replay. Adaptive processing now rejects distant triangles early, builds collapse neighborhoods only for candidate endpoints, accumulates relaxation data only for active vertices, and avoids global edge hashing and unnecessary edge allocations/sorting. Off-patch incident faces still participate in safety checks; refinement limits and brush calculations are unchanged. These are shared adaptive routines, but the measured speedups above apply to this Detail workload, not every brush or mesh density.

## Document edit profile

`cargo run --release --locked --example edit_profile` measures the document layer without a window. It times the first translation (including its undo snapshot), the median of five further translations in that stroke, and one half-space mask operation. The larger cases retain two additional subdivision levels. Each inactive model retains the same stored geometry as the active model.

A local sample on 2026-10-14, using an Intel Core i7-7500U with other validation work finished, compared the same harness linked against the pre-cleanup release library and the rebuilt library. All values are milliseconds:

| Active vertices | Models | First transform before → after | Subsequent transform before → after | Mask operation after |
| ---: | ---: | ---: | ---: | ---: |
| 4,706 | 1 | 4.447 → 0.196 | 3.081 → 0.036 | 4.730 |
| 75,266 | 1 | 104.101 → 4.439 | 97.703 → 0.769 | 264.345 |
| 75,266 | 4 | 137.445 → 11.345 | 107.092 → 0.852 | 268.960 |

Transforms now prepare and check coordinates without cloning inactive models or validating unchanged topology on every update. The first update still snapshots the scene for undo. These samples measure document work, not complete viewport latency: display preparation and GPU uploads still follow an edit. Bulk mask operations still clone and validate the mesh stack, and CPU picking still scans triangles. Those remain separate scaling work; this change adds no spatial index or hardware-rendering performance claim.

## Surface rendering

### Polygroups

Polygroup colors are cached per display triangle in a storage buffer. The vertex shader pulls indexed position/normal data and face colors using the draw vertex index, retaining sharp category boundaries without optional primitive-index features. Group registry order selects deliberately separated hues from a muted palette, so renaming keeps the color; masks, guides and brush previews still compose over the surface. The View toggle changes uniforms without rebuilding geometry.

### Wireframe depth

Visible wireframe uses slope-scaled polygon offset on filled surfaces and unshifted edge positions. This prevents depth fighting on tilted primitive faces while retaining depth occlusion by foreground surfaces. Axis colors are per-edge attributes in the cached line buffers.

### Shading normals

Surface lighting uses angle-weighted corner normals with separate smoothing fans at edges of 60 degrees or more. `sculpt/shading.rs` builds display vertices mapped back to the original welded vertices; surface index order still matches polygroup buffer entries. This prevents cap triangles and thin clipped faces from producing bright or dark triangular patches around slice rims. GPU rendering, CPU preview, brush-icon captures, and picked surface normals use these same fans. Sculpting, subdivision, wireframe topology, and saved geometry keep the original welded mesh; polygroup color boundaries alone do not introduce hard shading edges.

## Focused regressions

### Slice rims

The focused slice render check uses real wgpu pixel readback to detect lighting spikes below a trim rim and renders touching separated pieces too:

```sh
CLAY_SLICE_PREVIEW=/tmp/clay-slice.png CLAY_SLICE_SEPARATE_PREVIEW=/tmp/clay-separate.png cargo test --locked gpu_slice -- --ignored
```

For an uncapped sliced hemisphere with thickness modifier, `CLAY_SLICE_THICKNESS_PREVIEW=/tmp/clay-slice-thickness.png cargo test --locked gpu_sliced_hemisphere -- --ignored` renders the rim and verifies that the preview and applied shell have identical geometry and shading.

### Visibility and undo

The visibility regression renders a full mesh, a hidden half, the two split submodels, and Undo. It checks framebuffer pixels and exact restoration of the hidden display:

```sh
CLAY_VISIBILITY_PREVIEW=/tmp/clay-visibility-preview.png cargo test gpu_visibility -- --ignored --nocapture
```

### Rotation controls

The transform-control regression renders colored World rotation rings, the shared floating toolbar, live angle feedback, and a rotated primitive after a dimension edit. It shares its offscreen capture fixture with the lasso test:

```sh
CLAY_GPU_SOFTWARE=1 CLAY_ROTATION_PREVIEW=/tmp/clay-rotation cargo test --release --locked gpu_rotation_controls -- --ignored
```

This writes `-1.png` (ready), `-4.png` (rotation drag), and `-6.png` (regenerated dimensions) at the requested prefix.


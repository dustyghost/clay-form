# GPU model rendering

Clay Form retains egui for the interface and uses its existing glow/OpenGL backend for a dedicated 3D viewport. Desktop OpenGL 3.3 or newer is required. No new rendering dependency or file-format change is introduced.

The renderer caches positions, normals, masks, materials, triangle indices, and model ownership. Camera changes update shader uniforms. Geometry/mask edits and model visibility/selection changes invalidate the cache; undo, redo, New, Open, and completed topology jobs do the same. Wireframe edge buffers are generated only when wireframe is requested after an edit. Quad diagonals remain optional.

An offscreen color/depth framebuffer gives intersecting and overlapping submodels proper depth testing. Visible wireframe uses that depth buffer; Front shows front-facing edges through surfaces; Transparent shows every edge with no surface fill. The View menu optionally draws back-facing surfaces in a separate translucent pass (15% default opacity). That pass depth-tests against opaque front faces but does not write depth, allowing surfaces behind a back to remain visible. Lighting reverses the display normal for that pass; geometry normals and CPU front-surface picking are unchanged. Transparent wireframe continues to omit surface fills.

Lighting runs per fragment. The alpha footprint and symmetry guides run in the solid surface shader and stay on active visible surfaces. Tangent rings, the normal indicator, gizmos, and ordinary controls remain egui overlays.

Mesh deformation, subdivision, remeshing, undo, and surface picking remain CPU operations. Picking still projects and scans triangles; moving rendering to the GPU does not remove that cost. The renderer uses the same immutable mesh snapshot for display and picking throughout each frame, including frames containing adaptive topology edits.

## Validation

The ordinary test suite retains a CPU paint path for headless UI and picking tests. On a Linux host with EGL installed, the opt-in OpenGL test compiles the real shaders and reads back rendered pixels:

```sh
cargo test --locked gpu_offscreen -- --ignored --nocapture
```

For the translucent back-face pass, run `cargo test --locked gpu_backfaces -- --ignored --nocapture`. This checks adjustable opacity, front/behind occlusion, closed-sphere appearance, and geometry/cache stability.

The main offscreen test checks depth occlusion, wireframe modes, quad topology, cache reuse, alpha texture replacement, agreement between shader and CPU alpha weights, inactive-model protection, symmetry guides, framebuffer resizing, cleanup, and egui composition. To save its composed preview:

```sh
CLAY_GPU_PREVIEW=/tmp/clay-gpu-preview.png cargo test --locked gpu_offscreen -- --ignored --nocapture
```

## Load profile

Run the repeatable profile in release mode, while other CPU/GPU-intensive work is idle:

```sh
CLAY_GPU_PROFILE=/tmp/clay-gpu-profile.csv cargo test --release --locked gpu_load_profile -- --ignored --nocapture
```

The workload includes approximately 4.7k, 19k, 75k, and 301k vertices in a single model, plus four separate models at the low and medium densities. All models use the requested density rather than hiding the cost behind inactive-model LODs. The viewport is 256 × 256 physical pixels.

The CSV records display-mesh preparation, initial buffer upload, CPU view preparation, picking, solid/wireframe frame times, and Standard-brush dab time. Rendering measurements finish GPU work before stopping the clock and exclude the first framebuffer/edge-buffer creation draw. Median and approximate 95th percentile use 16 camera frames. These are diagnostic samples, not CI performance thresholds or a complete end-to-end input-latency benchmark. The first dab includes the undo snapshot; later dabs share the stroke.

The output identifies the GL driver. A `llvmpipe` result measures software rendering on the CPU and cannot establish hardware GPU performance. Compare runs on the same driver, hardware, resolution, and workload. Native application profiling at normal window size should also cover sustained strokes, adaptive topology, masks, symmetry, high-DPI screens, and repeated New/Open/undo/redo to check memory stability.

Use those measurements to choose the next optimization. Likely candidates to investigate are a spatial index for picking, limiting uploads and normal updates to changed mesh regions, and spatial brush-neighborhood queries. Higher-density sculpting may need CPU geometry work even when drawing is fast.

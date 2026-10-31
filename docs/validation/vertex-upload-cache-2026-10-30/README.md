# Reuse mesh buffers and upload changed vertex blocks

Follow-up to [Hidden-surface release refresh](../hidden-release-refresh-2026-10-30/README.md), under #103. The comparison baseline is commit `bba1f1e`. Final measurements use source SHA-256 `aa4e917e44785f1f20779286551868d657394bf151e470c7e41d27ec6f3eb708`. The [raw GPU results](gpu/index.json) retain all samples, memory stages and environment metadata; the [comparison](gpu-comparison.csv) includes every measured phase.

The release-upload follow-up is recorded in [Independent buffer reuse at stroke release](../release-buffer-reuse-2026-10-30/README.md).

## Change and boundaries

Previously, each changed display recreated vertex, index, face-colour and UV buffers. Compatible displays now reuse those buffers and write only changed blocks of packed vertex data. Reuse requires the same vertex count and stride, shading triangle indices, face colours, UV corners and device binding limit. Changes to those take the existing full-upload path. Displays that need multiple buffer batches retain the existing chunking path.

The uploader retains the previous packed vertex array and compares bytes in 4 KiB blocks, including extended material channels. Adjacent dirty blocks are coalesced. More than 64 write ranges, or changes covering more than half the array, use one complete vertex write instead. That still reuses unchanged index, face-colour and UV buffers. Identical packed values require no vertex write, even when the display allocation changed.

Cache identity follows the geometry actually represented by the buffers. Changing between forced batching and the normal device limit invalidates it, and renderer clearing releases the retained vertex array. Texture-only updates continue through the existing atlas update path. Wire buffers retain their existing invalidation and rebuild behavior.

Vertex packing and layout comparisons still scan complete arrays. This is a reduction in buffer allocation and transfer work, not regional CPU display preparation. Shaders, sculpting, normals, geometry limits, history and project formats are unchanged.

## Matched hardware results

Same i7-7500U, 7.63 GiB usable RAM, Intel HD Graphics 620, Mesa 26.2.4/Vulkan and release toolchain as the baseline, at 800×600. The browser remained closed. The twelve hardware cases ran sequentially without concurrent builds. Each brush has three repetitions of twelve deterministic dabs. Every fixture, count, pressure sequence and output checksum passed the strict comparison; the benchmark workload and metrics were not changed.

The upload phase includes CPU vertex packing, buffer preparation, queued transfers and their GPU completion wait. It is not a complete frame time. Sustained metrics have 33 samples per brush; first-dab and release metrics have three, so their p95 is the maximum of those three here.

| Fixture | Vertices | Brush | Before sustained upload p50 / p95 | After p50 / p95 |
| --- | ---: | --- | ---: | ---: |
| Sphere | 75,266 | Standard | 16.69 / 19.02 ms | 6.84 / 8.18 ms |
| Organic | 75,266 | Standard | 17.15 / 19.22 ms | 7.24 / 8.13 ms |
| Hidden organic | 75,266 | Standard | 14.43 / 15.87 ms | 6.42 / 6.88 ms |
| Sphere | 301,058 | Standard | 49.10 / 67.47 ms | 25.49 / 26.57 ms |
| Organic | 301,058 | Standard | 47.43 / 71.35 ms | 26.85 / 28.93 ms |
| Hidden organic | 301,058 | Standard | 71.77 / 90.18 ms | 23.41 / 24.80 ms |
| Hidden organic | 301,058 | Clay | 70.90 / 90.93 ms | 23.46 / 26.45 ms |
| Hidden organic | 301,058 | Paint | 71.38 / 91.54 ms | 23.21 / 24.32 ms |

Largest hidden Standard sustained upload preparation is about 67% shorter, or 3.1 times faster. Its first-dab upload also drops from 91.19 to 29.07 ms p50. The fully visible sphere and organic Standard sustained medians fall by about 48% and 43% respectively.

For total upload-phase work during a twelve-dab stroke, sum each trial's first upload and eleven sustained uploads, then take the median of the three trial sums:

| 301,058-vertex hidden fixture | Before | After |
| --- | ---: | ---: |
| Standard | 894.85 ms | 287.41 ms |
| Clay | 888.00 ms | 287.14 ms |
| Paint | 896.90 ms | 280.02 ms |

These totals exclude initial viewport upload, release upload, brush evaluation, display construction, drawing and OS presentation. They are not sums of independent percentiles and do not imply a frame rate.

## Release and other phases

The saving is strongest during strokes. Canonical release can change the shading layout and still require a complete upload. For the largest hidden fixture, Standard release upload remains 100.65 ms p50 versus 103.57 ms before, and Clay is 68.40 ms versus 69.36 ms. Hidden Paint already reused its exact release display; its upload/completion phase remains below 0.5 ms. The largest fully visible sphere does benefit at release: Standard upload drops from 66.62 to 30.95 ms, and Paint from 62.83 to 22.70 ms.

Largest hidden Standard brush evaluation is 16.77 ms versus 17.11 ms; sustained display preparation is 26.00 ms versus 25.81 ms. Release display preparation is 121.56 ms versus 125.20 ms. Completed orbit draw is 26.51 ms versus 29.50 ms. The shader and draw path were not changed, so this run alone does not establish a general rendering-speed improvement.

Not every measured phase improved. Largest hidden Standard display p95 rose from 27.10 to 33.55 ms, and its release-upload p95 rose from 104.43 to 119.59 ms. Fully visible sphere Paint release display preparation rose from 198.80 to 225.49 ms p50. These short runs do not establish statistical significance; all metrics are retained rather than claiming an across-the-board latency gain.

## Memory cost

The retained compact vertex array is about 14 MiB at 301,058 shading vertices. Extended material records are larger, and sharp edges can add shading vertices. Each update also temporarily packs a new array; queue staging, allocator retention and the benchmark's history/save-load verification affect process peak memory.

| Workload | Before process peak RSS | After process peak RSS |
| --- | ---: | ---: |
| Sphere, 75,266 vertices | 389.0 MiB | 383.6 MiB |
| Organic, 75,266 vertices | 427.0 MiB | 427.8 MiB |
| Hidden organic, 75,266 vertices | 425.1 MiB | 431.8 MiB |
| Sphere, 301,058 vertices | 813.8 MiB | 835.3 MiB |
| Organic, 301,058 vertices | 904.9 MiB | 908.7 MiB |
| Hidden organic, 301,058 vertices | 850.4 MiB | 912.2 MiB |

The largest hidden workload adds about 62 MiB of measured peak RSS. Its highest recorded peak occurs during save/load verification; this is not an isolated measurement of cache size. The retained array is a real space-for-time trade-off even where process peak fell. All twelve runs stayed inside the 1536 MiB RSS and 600-second per-process guards. No new multi-million-polygon capacity claim follows from these fixtures.

## Correctness and visual checks

Two CPU range tests passed. They reconstruct sparse changes including an aligned tail, check adjacent-range coalescing, empty/unchanged data, fragmentation limits, broad updates, signed zero and NaN bit patterns.

Nine focused hardware GPU regressions passed. The two new tests compare solid and wire pixels against newly created reference buffers for visible and hidden Standard, Clay, Paint and Mask strokes, release and Undo/Redo. They assert stable vertex/index/colour/UV buffer identities and less than half the vertex bytes uploaded on localized second dabs. Further cases check same-count index changes, face colours, UVs, stride changes, fan-count changes, empty displays, sparse extended-material channels beyond the compact vertex fields, zero-write updates, forced batching, switching back to normal limits and clearing the cache.

The seven existing hardware regressions cover visibility and picking, hidden-surface history, batching with depth and face colours, UV texture/checker batching, material channels, depth/wireframe caching and incremental texture-atlas updates.

The rendering capture was visually inspected and is byte-for-byte identical to the [previous capture](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png), SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`. Hidden faces stay absent, Paint remains visible, and Undo restores the original view.

Formatting and diff whitespace checks passed. Full-suite CI remains pending; no PR was opened for this batch.

## Reproduce

Run hardware tests and benchmarks sequentially with other intensive applications closed. Use a fresh output directory on a local disk:

```sh
cargo test --release --locked --lib vertex_upload -- --test-threads=1
cargo test --release --locked --lib gpu_vertex_upload -- --ignored --nocapture --test-threads=1
CLAY_HIDDEN_STROKE_PREVIEW=/tmp/clay-vertex-upload.png cargo test --release --locked --lib viewport::gpu::visibility_test:: -- --ignored --test-threads=1

python3 scripts/benchmark-capacity.py run target/vertex-upload-repeat/gpu --gpu
python3 scripts/benchmark-capacity.py compare docs/validation/hidden-release-refresh-2026-10-30/gpu target/vertex-upload-repeat/gpu
```

The additional existing GPU tests run with the same ignored-test command using these filters: `gpu_batches_preserve_face_colors_and_depth`, `gpu_uv_texture_pixels_and_checker_survive_batching`, `gpu_material_channels_match_linear_reference_and_survive_batching`, `gpu_offscreen_depth_wireframe_and_cache`, and `gpu_paint_inspection_and_incremental_texture_upload_match_stored_channels`.

Only final-run results are retained here. Remaining work includes reducing CPU vertex packing and display-array copying, and improving upload handling when canonical release changes the shading layout.

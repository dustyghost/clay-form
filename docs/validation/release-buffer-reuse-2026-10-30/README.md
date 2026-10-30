# Independent buffer reuse at stroke release

Follow-up to [Reuse mesh buffers and upload changed vertex blocks](../vertex-upload-cache-2026-10-30/README.md), under #103. The comparison baseline is commit `bfc02ba`. Final measurements use source SHA-256 `3618eeb38120639781ce6421ffb567eba6c8f7d1088b9c08614ef8db265e8ba2`. The [raw GPU results](gpu/index.json) retain all samples, memory stages and environment metadata; the [comparison](gpu-comparison.csv) includes every measured phase.

## Change and boundaries

The previous uploader reused buffers only when vertex layout, shading indices, face colours and UVs all matched. A smoothing-fan change at canonical release could therefore recreate all four buffers, even though face colours and UVs had not changed.

Reuse is now independent for each buffer. With a compatible single-buffer display and device binding limit, unchanged face-colour and UV data are retained without writes. Changed indices, colours or UVs update their existing allocation when the payload size is unchanged; a size change replaces only that buffer. Vertex data keeps the existing bounded dirty-block path when its count and stride match. A vertex-count or stride change replaces vertex storage without invalidating the face buffers. The draw count follows the current index count.

Growing and shrinking buffers use exact-size allocations, with the existing small placeholder for empty data. No extra retained payload arrays or spare GPU capacity were added; the packed-vertex cache from the previous change remains. Displays exceeding the single-buffer binding limit still use the existing batching path, with explicit state tracking when switching between single and batched layouts.

Shaders, canonical normal calculation, sculpting, history and project format are unchanged. CPU vertex packing still scans the full display. Vertex resizing can still be expensive, and a changed index buffer currently receives its complete new index payload.

## Matched hardware results

Same i7-7500U, 7.63 GiB usable RAM, Intel HD Graphics 620, Mesa 26.2.4/Vulkan and release toolchain as the baseline, at 800×600. The twelve hardware cases ran sequentially without concurrent builds. Each brush has three repetitions of twelve deterministic dabs. Every fixture, count, pressure sequence and output checksum passed the strict comparison. Benchmark workloads, phases and metrics were not changed.

The upload phase includes CPU packing and buffer preparation, queued transfers and their GPU completion wait. Release metrics have three samples per brush, so p95 is the maximum of those three here. These are upload timings, not complete pointer-up latency.

| Fixture | Vertices | Brush | Before release upload p50 / p95 | After p50 / p95 |
| --- | ---: | --- | ---: | ---: |
| Hidden organic | 75,266 | Standard | 10.30 / 18.54 ms | 8.67 / 15.47 ms |
| Hidden organic | 75,266 | Clay | 12.92 / 13.05 ms | 11.03 / 18.15 ms |
| Hidden organic | 301,058 | Standard | 100.65 / 119.59 ms | 69.66 / 70.42 ms |
| Hidden organic | 301,058 | Clay | 68.40 / 71.11 ms | 35.31 / 37.57 ms |
| Organic | 301,058 | Standard | 95.00 / 95.20 ms | 68.72 / 81.19 ms |
| Organic | 301,058 | Clay | 62.73 / 62.80 ms | 34.99 / 49.17 ms |
| Sphere | 301,058 | Standard | 30.95 / 31.74 ms | 31.42 / 31.78 ms |
| Sphere | 301,058 | Clay | 30.88 / 31.61 ms | 31.16 / 31.90 ms |

Largest hidden Standard release upload is about 31% shorter; Clay is about 48% shorter. The fully visible organic fixture also benefits. The sphere control, whose previous release uploads were already much cheaper, remains close to its baseline. Hidden Paint already reused its exact display allocation and remains below 0.5 ms for upload/completion.

## Combined release work and other phases

To include the surrounding work, sum each trial's `end_stroke`, `end_display` and `end_upload_wait` samples, then take the median of the three trial sums:

| Hidden fixture | Brush | Before combined release work | After |
| --- | --- | ---: | ---: |
| 75,266 vertices | Standard | 39.62 ms | 40.93 ms |
| 75,266 vertices | Clay | 42.78 ms | 42.97 ms |
| 301,058 vertices | Standard | 222.22 ms | 195.02 ms |
| 301,058 vertices | Clay | 190.76 ms | 161.46 ms |
| 301,058 vertices | Paint | 40.11 ms | 41.16 ms |

These paired totals show about 27–29 ms less measured release work for the largest hidden deformation strokes. They are not sums of independent percentiles and exclude final drawing, event scheduling and OS presentation. The smaller hidden cases do not show an overall release improvement in this run despite their lower upload medians.

Largest hidden Standard sustained upload remains 22.97 ms p50 versus 23.41 ms before; Clay is 22.81 ms versus 23.46 ms, and Paint 23.46 ms versus 23.21 ms. Standard sustained display preparation is 25.77 ms versus 26.00 ms. The existing during-stroke gains are preserved.

Canonical display preparation remains a substantial separate cost: largest hidden Standard release display is 125.13 ms versus 121.56 ms, and Clay 124.55 ms versus 121.99 ms. Completed hidden orbit draw rose from 26.51 to 29.65 ms p50; its p95 rose from 32.91 to 33.50 ms. These short runs do not establish statistical significance or an across-the-board frame-rate improvement. Changes in phases outside upload should not be attributed to buffer reuse.

## Memory

| Workload | Before process peak RSS | After process peak RSS |
| --- | ---: | ---: |
| Sphere, 75,266 vertices | 383.6 MiB | 379.5 MiB |
| Organic, 75,266 vertices | 427.8 MiB | 429.3 MiB |
| Hidden organic, 75,266 vertices | 431.8 MiB | 437.3 MiB |
| Sphere, 301,058 vertices | 835.3 MiB | 791.1 MiB |
| Organic, 301,058 vertices | 908.7 MiB | 901.0 MiB |
| Hidden organic, 301,058 vertices | 912.2 MiB | 909.2 MiB |

The largest hidden workload's measured peak is slightly lower, rather than adding another retained cache. Process peaks include queue staging, allocator retention, benchmark reference snapshots, history and save/load verification; they are not isolated buffer-size measurements. All twelve cases stayed inside the 1536 MiB RSS and 600-second per-process guards. No geometry limits were raised.

## Correctness and visual checks

Two CPU dirty-range tests and ten focused hardware GPU regressions passed. The new release regression deforms a dense hidden surface to create a sharp boundary, then restores it. At both canonical releases it verifies that smoothing fans actually split or merge, canonical normals and indices match a fresh display, vertex storage changes as needed, the index allocation is reused, and no face-colour or UV payload is written. Solid and wire pixels match newly created reference buffers exactly.

The existing layout regression now verifies independent writes: same-count index, face-colour and UV changes reuse their allocations and write only the affected face channel. Vertex-stride and fan-count changes preserve the face buffers. Empty displays, sparse extended-material updates, identical-value updates, batching transitions and renderer clearing are also checked.

Other hardware regressions retain visible/hidden Standard, Clay, Paint and Mask strokes, protected visibility and picking, release, Undo/Redo, depth and face-colour batching, UV/checker batching, material channels, depth/wireframe caching and incremental texture-atlas updates.

The capture was visually inspected and is byte-for-byte identical to the [previous capture](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png), SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`. Hidden faces remain absent, Paint remains visible, and Undo restores the original view.

Formatting and diff whitespace checks passed. Full-suite CI remains pending; no PR was opened for this batch.

## Reproduce

Run sequentially with other intensive applications closed. Hardware checks require graphics-device access. Use a fresh benchmark directory on a local disk:

```sh
cargo test --release --locked --lib vertex_upload -- --test-threads=1
cargo test --release --locked --lib gpu_vertex_upload -- --ignored --nocapture --test-threads=1
CLAY_HIDDEN_STROKE_PREVIEW=/tmp/clay-release-buffer-reuse.png cargo test --release --locked --lib viewport::gpu::visibility_test:: -- --ignored --test-threads=1

python3 scripts/benchmark-capacity.py run target/release-buffer-repeat/gpu --gpu
python3 scripts/benchmark-capacity.py compare docs/validation/vertex-upload-cache-2026-10-30/gpu target/release-buffer-repeat/gpu
```

The five additional existing GPU tests use these filters with the same ignored-test command: `gpu_batches_preserve_face_colors_and_depth`, `gpu_uv_texture_pixels_and_checker_survive_batching`, `gpu_material_channels_match_linear_reference_and_survive_batching`, `gpu_offscreen_depth_wireframe_and_cache`, and `gpu_paint_inspection_and_incremental_texture_upload_match_stored_channels`.

Only the final implementation's matched measurements are retained here. Full CPU vertex packing and canonical display preparation remain the next substantial costs to investigate.

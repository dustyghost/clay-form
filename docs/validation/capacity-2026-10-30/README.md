# Sculpting capacity baseline

Part of #103. This batch measures Clay Form's existing implementation and adds repeatable improvement checks. It does not change mesh caps or introduce a new sculpting architecture.

## Measured baseline

Production source base: `3e10b98ec637ac90425ade357d50c2f26ae5f2d5`, with the benchmark additions in this change. Each run records the combined source SHA-256 and compiler in `environment.json`. Hardware: Intel Core i7-7500U, two cores/four threads, 7.63 GiB usable RAM, Intel HD Graphics 620, Arch Linux, Rust 1.99.0. The browser was closed before the retained runs. About 4.4 GiB RAM was available at CPU-run start, although swap was already nearly full; this was not a dedicated benchmark host. Files were written under the repository's `target` directory on the local NVMe filesystem, not RAM-backed `/tmp`.

The [CPU results](cpu/index.json) cover all three fixtures at all four resolutions, with three repeated strokes per brush, restoring the same geometry between them. Times below are medians in milliseconds, not an end-to-end frame budget.

| Plain sphere vertices | Quad polygons | Standard sustained dab | Live display preparation | Display refresh after release | First Paint dab |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 4,706 | 4,704 | 0.28 | 3.04 | <0.01 | 3.70 |
| 18,818 | 18,816 | 1.04 | 11.62 | <0.01 | 15.63 |
| 75,266 | 75,264 | 4.11 | 5.99 | 46.88 | 69.31 |
| 301,058 | 301,056 | 17.02 | 23.56 | 192.90 | 310.61 |

The display cost decreases between 18,818 and 75,266 vertices because the existing live-display cache starts at 20,000 vertices. Smaller meshes rebuild canonical display data on every dab and can reuse it on release; cached deformation uses temporary normals and needs a canonical rebuild on release.

At the largest resolution, all fixtures have 602,112 triangles and retain four levels containing 399,848 vertices in total:

| Fixture | Visible triangles | Standard display p50 / p95 | First Paint dab p50 | Sustained Paint dab p50 | Process peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Plain sphere | 602,112 | 23.56 / 32.14 ms | 310.61 ms | 3.57 ms | 604 MiB |
| Attributed organic form | 602,112 | 28.86 / 32.56 ms | 311.89 ms | 3.15 ms | 677 MiB |
| Partially hidden form | 362,494 | 271.98 / 283.76 ms | 182.10 ms | 2.25 ms | 686 MiB |

Hiding the rear surface makes display preparation about 9.4 times slower than the equivalent fully visible attributed form, despite reducing the displayed triangle count. Code inspection confirms this fixture disables `DisplayMesh::during_stroke` and takes the full rebuild path, including visibility filtering. It already has canonical shading after each dab, so its release display refresh is below 0.01 ms rather than another full rebuild. A focused regression protects that distinction in the CPU-only harness.

The first Paint dab is roughly 100 times slower than subsequent dabs on the largest fully visible fixtures. Normals, eligibility and spatial-query structures are initialized on the first dab; their separate contributions remain to be profiled. This is evidence for investigating cache lifetime, not proof that one particular structure explains the entire delay.

Largest-fixture save medians range from 1.82 to 2.10 seconds and load medians from 1.83 to 2.19 seconds, for files of about 45–72 MiB. Both operations include validation. Standard Undo/Redo is below 0.01 ms on the unpainted sphere but about 7 ms with explicit vertex materials, before any display refresh. CPU-only orbit preparation is about 9–11 ms and picking about 1.6 ms. These figures identify expensive operations; they do not isolate serialization, validation, allocation or adjacency subphases.

The [hardware GPU results](gpu/index.json) repeat the same 12 workloads using Intel HD Graphics 620, Mesa 26.2.4, Vulkan, at 800×600. The adapter is recorded in every case; these are not software-renderer numbers. At 301,058 vertices:

| Fixture | Standard buffer preparation/upload/wait p50 / p95 | Completed orbit draw p50 / p95 | Process peak RSS |
| --- | ---: | ---: | ---: |
| Plain sphere | 48.16 / 68.21 ms | 35.83 / 40.16 ms | 845 MiB |
| Attributed organic form | 45.34 / 69.13 ms | 35.39 / 39.08 ms | 886 MiB |
| Partially hidden form | 71.26 / 107.20 ms | 29.61 / 33.36 ms | 833 MiB |

The hidden form draws faster but costs more to prepare and upload. Reducing visible triangles alone therefore does not establish a faster sculpting pipeline. Upload timings include CPU packing and buffer creation; GPU timestamps or additional instrumentation are needed to distinguish those from device transfer cost. CPU-only and GPU runs remain separate comparison sets; their independently measured percentiles must not be combined into an advertised frame rate.

All 24 retained CPU/GPU workloads completed within the 1536 MiB RSS and 600-second per-process guards. The three focused Rust tests and five Python comparison tests passed; formatting and diff whitespace checks passed. The benchmark also asserts real edits, deterministic repeated outputs, exact Undo/Redo restoration, and saved/reloaded mesh and subdivision equality at every measured resolution. CI includes the correctness and comparison tests, while the timing workload remains excluded. Full-suite CI has not been run for this local branch.

## Suggested improvement experiments

1. **Reuse display topology with hidden faces.** The measured 29-to-272 ms gap is the clearest CPU display target. Preserve visibility, masks, materials and canonical shading; compare both visible and hidden fixtures so a faster path does not move the cost elsewhere.
2. **Retain GPU buffers and update changed regions.** The 45–71 ms upload phase is large enough to justify separating packing, allocation and transfer costs before choosing an implementation. Keep GPU pixel/depth checks alongside the benchmark's geometry/history checks.
3. **Reduce first-dab Paint setup.** Profile normals, eligibility, spatial-query construction and history separately, then evaluate cache reuse with explicit geometry/topology/visibility invalidation. Test first-dab latency as well as sustained dabs.
4. **Profile native persistence subphases.** Separate validation, serialization and parsing before choosing a format or caching change; the current measurements include all of them.

Each experiment should retain the workload and output checksums, report p50/p95 and memory on the same machine, and pass the relevant correctness tests. This baseline does not yet justify a larger capacity cap or an HD architecture.

## Reproduce

From the repository root, on Linux:

```sh
# Focused correctness checks; these have no timing thresholds.
cargo test --release --locked --lib capacity_ -- --test-threads=1
python3 scripts/test-benchmark-capacity.py

# New output directories are required. Each fixture/level runs in a fresh process.
python3 scripts/benchmark-capacity.py run /path/on/local/disk/capacity-before
# Optional real-device rendering, completed uploads and completed draws:
python3 scripts/benchmark-capacity.py run /path/on/local/disk/capacity-gpu-before --gpu

# Repeat after an implementation change, on the same machine and build settings.
python3 scripts/benchmark-capacity.py compare /path/on/local/disk/capacity-before /path/on/local/disk/capacity-after
```

The runner builds the optimized library test executable once, then runs only the explicitly ignored benchmark. Do not run concurrent builds or other benchmarks. Close unnecessary applications, keep the power mode consistent, and use a local disk output directory when measuring file operations; `/tmp` may be RAM-backed. The existing GPU CI filter excludes tests prefixed `gpu_load_profile`, including this one.

Use `--fixtures sphere organic hidden`, `--levels 0 1 2 3`, and `--repeats 3` to select workloads. `--max-rss-mib` defaults to 1536 and `--timeout` to 600 seconds per process. The runner requires at least 1 GiB available before each workload and stops a workload if available system memory drops below 256 MiB. RSS is sampled every 100 ms: this is a best-effort guard, not a guaranteed allocation ceiling. A failed or skipped workload stops the batch and is recorded in `index.json`; it is never presented as a successful measurement.

## Workload contract

All fixtures are generated deterministically from the application's closed quad sphere. Levels 0–3 mean zero through three additional subdivisions, with lower levels retained. Counts are recorded separately for vertices, quad polygons, triangulated surface faces, visible triangles and all stored levels.

| Fixture | Purpose |
| --- | --- |
| `sphere` | Ordinary unpainted closed surface with a vertex group and retained subdivision levels. |
| `organic` | Synthetic elongated form with fine sinusoidal relief, explicit vertex materials and a fully masked region intersecting the brush footprint. |
| `hidden` | The same attributed form with a rear region hidden, exercising the full display rebuild fallback. |

These are controlled synthetic fixtures, not artist-authored production sculpts. Each repetition starts each brush from the same geometry. Standard, Clay and vertex Paint each receive 12 moving dabs with a fixed radius, strength and pressure sequence. Symmetry is disabled, all fixtures have one model, and there are no textures, creases or adaptive topology changes. Brush definitions and fixture checksums accompany comparable source versions; changed input geometry or output checksums invalidate automatic comparisons.

The routine correctness tests verify that these strokes edit the surface, retain topology, masks, groups and hidden-face state, protect fully masked vertices and material values, and preserve exact geometry through single-step Undo/Redo. A separate dense fixture exercises live display updates above their 20,000-vertex threshold and checks release requires canonical shading. These checks complement existing brush and GPU correctness tests; they are not a visual quality assessment.

## Meaning of the measurements

- `document_open`: installing and validating an already generated fixture; excludes file reads and fixture generation.
- `geometry_snapshot` and `project_snapshot`: full snapshot construction, timed separately.
- `first_dab`: document edit including the first history snapshot. `sustained_dab`: subsequent edits in that stroke.
- `first_display` and `sustained_display`: preparation of live display data. CPU-only runs call the same live-display implementation and fallback directly; GPU runs use the actual renderer's invalidation and cache path.
- `end_stroke`: document finalization. `end_display`: the required display refresh after release. This rebuilds temporary cached shading, or reuses already canonical data when the stroke took the full display path. The former can dominate pen-up cost even when document finalization is cheap.
- `orbit_cpu_projection`: CPU projection and pick-face preparation for a moving camera. `pick`: a surface hit query; every sample must hit.
- `*_upload_wait`: CPU buffer preparation, upload submission and completion wait together, not isolated GPU execution time.
- `orbit_gpu_draw_wait`: draw encoding, submission and completion at 800×600, after a warm-up draw. Orbit samples are pooled across the three post-stroke shapes. Excludes OS presentation, pointer processing and display refresh. It is not a desktop frame-rate measurement.
- `undo` and `redo`: document history operations; excludes the viewport refresh that would follow them.
- `save_validate_serialize_sync`: validation, JSON serialization and atomic synchronized writing to the output directory. `load_read_parse_validate`: reading, parsing and validation, usually from a warm filesystem cache.
- `subdivide`: one additional geometry subdivision, excluding worker dispatch and UI commit. Only attempted when the next level fits the existing caps.

JSON retains raw samples, sample counts and nearest-rank p50/p95/max. First-dab, completion and persistence measurements have only three samples by default, so their p95 is the maximum of three observations, not a reliable population-tail estimate. Sustained brush measurements have 33 samples per brush. Do not add independently computed percentiles and call the result end-to-end latency.

Process RSS and lifetime high-water RSS are recorded separately from the application's estimated retained history bytes. Benchmark memory includes a reference snapshot retained for correctness checking and temporary verification data. RSS after fixture construction may retain allocator arenas from subdivision and validation; it is not the application's minimum idle footprint. GPU driver allocations may not all appear in RSS.

The comparison command requires compatible hardware/compiler settings, the same workload coverage, input checksums, output checksums and sample counts. It prints after/before ratios for both percentiles. Optional `--max-regression-percent N` enables a local threshold; routine CI deliberately has no machine-dependent performance assertions. Thermal state, background load and filesystem caching still require human review and repeated runs.

Increment the workload schema if sampling or benchmark semantics change. An implementation optimization should preserve the input and output checksums; a changed result needs a correctness review before any speed claim.

## Boundaries

Current mesh validation caps are 500,000 vertices and 1,000,000 triangles per mesh. A stored subdivision stack is limited to 1,000,000 vertices and eight levels. For these all-quad fixtures, triangle counts are twice polygon counts. The next subdivision above level 3 exceeds the mesh limits and is reported as skipped without allocating it. Multi-million single-mesh workloads cannot currently be exercised through the supported document path; no constants were raised to manufacture a benchmark.

Projects permit at most 64 models, and native JSON files are limited to 2 GiB; neither limit guarantees that the aggregate scene fits RAM. OBJ input is separately limited to 128 MiB. Default history retention is 24 entries within an estimated 1024 MiB budget shared by undo and redo. These are policy/validation limits, not measured interactive capacities.

This first benchmark set does not measure multi-model aggregate capacity, remeshing, texture painting, allocation counts, separate GPU timestamps or full input-to-present latency. Those need additional bounded workloads rather than extrapolation from this set. A rendering improvement alone must not be reported as an editing-capacity improvement.

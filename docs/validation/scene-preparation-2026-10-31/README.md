# Scene face-list allocation

Follow-up to [brush-footprint candidates](../footprint-picking-2026-10-31/README.md), under #103.

## Investigation

The preceding batch reduced footprint work but left a repeatable increase in hidden orthographic scene preparation: 37.7 → 47.9 ms in the first pair and 38.5 → 44.9 ms in the repeat. The scene constructor's arithmetic had not changed.

A controlled run of the saved pre-footprint and post-footprint executables with `MALLOC_MMAP_THRESHOLD_=131072` removed the difference: **41.26 → 41.25 ms** for the same hidden orthographic row. Other scene rows were also close. This demonstrates sensitivity to allocation policy; it does not identify a particular allocator-internal decision or establish a general benefit from that setting. It was used only for diagnosis. [Earlier executable](allocator-before/) and [current baseline executable](allocator-after/) records retain the environment, raw samples and memory guards.

A test-only phase probe found 5–9 ms spent building a second active-face list. For single-model scenes, it duplicates the front-facing list. The first list also grew geometrically while filtering triangles, allowing repeated allocation and copying of large buffers. Recording phases itself changed allocation history and did not reproduce the hidden-row gap, so phase timings locate work but do not supply the main speedup claim.

## Change

The front-facing list now reserves its known upper bound—one entry per displayed triangle—before collection. Single-model scenes expose the same list as their active-face view, avoiding a second owner scan, allocation and copy. Multi-model scenes retain owner filtering. The display builder places active vertices first, allowing the single-model condition to be checked from existing counts without scanning owners.

Face order, depth values, sorting, projection, surface picking and brush arithmetic are unchanged. The borrowed view is tied to its scene; there is no cache spanning frames. Empty scenes, hidden active models and active models with nonzero indices are covered by regression tests.

Reserving the upper bound can reserve more unused capacity than a short visible-face list needs. It avoids growth copies, and sharing removes the duplicate list in the single-model case. Whole-process memory measurements below are the evidence for practical memory behavior; the reserved capacity is not itself a peak-RSS claim.

## Final measurements

Scene preparation took **19–46% less time** across the six fixture/camera combinations. The previously affected hidden orthographic row fell **43.8 → 23.6 ms**, below its pre-footprint measurements as well.

| Fixture | Camera | Scene before p50 / p95 (ms) | Scene after p50 / p95 (ms) | p50 reduction |
| --- | --- | --- | --- | --- |
| Sphere | Orthographic | 44.4 / 46.4 | 31.9 / 32.4 | 28% |
| Sphere | Perspective | 39.1 / 42.0 | 31.7 / 33.0 | 19% |
| Organic | Orthographic | 45.9 / 55.4 | 32.1 / 33.1 | 30% |
| Organic | Perspective | 41.6 / 43.3 | 32.0 / 35.7 | 23% |
| Hidden | Orthographic | 43.8 / 44.9 | 23.6 / 27.0 | 46% |
| Hidden | Perspective | 35.3 / 36.1 | 23.3 / 23.7 | 34% |

Footprint p50 stayed within about −2% to +3%, preserving the previous improvement. Ordinary hit/miss p50 remained within about ±3%. Adding scene and footprint samples gives a further **10–28% reduction** in their combined p50, with the hidden orthographic row at **72.3 → 52.3 ms**. All controls and p95 values are retained in the [comparison CSV](comparison.csv), [baseline](primary-before/) and [optimized](primary-after/) profiles.

All timings use 800×600, 12 orbit angles per camera mode, three fixtures, a 50-pixel optional footprint and the existing isolated CPU diagnostic. Each hit/miss control has 36 samples. Construction of the document/display mesh is outside the scene timer. Drawing and presentation are excluded. The hidden fixture has 2,408,448 source triangles and 1,448,235 displayed triangles; sphere and organic display all 2,408,448.

Percentiles use the diagnostic's nearest-rank convention. Combined timings add corresponding scene and footprint samples before calculating percentiles. Source/display/candidate counts and sample counts must match across every compared row. These are operation timings, not desktop frame rates.

Whole-process peak RSS was effectively unchanged: **1124.0 → 1122.2 MiB** in the primary pair. Every recorded run completed within the 3072 MiB RSS / 768 MiB reserve / 600-second guards, with no sampled process swap use. Minimum sampled available system memory across these runs was **3041 MiB**. These short probes do not establish long-session memory behavior.

In the [phase comparison](phases.csv), active-face selection fell from 5.3–8.5 ms to less than 0.01 ms in all six single-model cases. The hidden orthographic facing/list-construction phase fell from 23.8 to 18.0 ms. Other phase timings varied; the unrecorded-phase primary pair above is the end-to-end performance comparison. [Before](phases-before/) and [after](phases-after/) retain the complete phase samples.

## Correctness

**24 focused CPU tests and four Vulkan tests pass** on the final optimized test binary. These cover scene visibility and picking, footprint candidate correctness, projection/clipping, stroke previews, symmetry-guide geometry, and visible/hidden stroke-cache pixels and history. [Logs](checks/) retain the results.

The eight-case [footprint capture](checks/footprint.png) was visually inspected and is byte-identical to the preceding commit's capture. [Capture hashes](checks/capture-comparison.json) retain the comparison. The Vulkan footprint regression also requires exact equality with exhaustive picking and a visible difference from rings alone. GPU checks used Intel HD Graphics 620.

The new active-face regression verifies values and traversal order against the owner-filtered face sequence across hidden faces, active-model changes, hidden models, empty display geometry, camera modes and sorted/unsorted scenes. It also checks that single-model views borrow their existing face slice and that retained scenes remain unchanged after document edits.

The manual camera diagnostic now supports `CLAY_CAMERA_FIXTURE=sphere|organic|hidden`, `CLAY_CAMERA_CURSOR=footprint|rings|none`, and optional `CLAY_SCENE_STAGES=1`. The defaults preserve the existing workload. Stage hooks compile only into test binaries; the diagnostic stays excluded from routine CI. Phase timing records cover vertex projection, facing/sorting, active bounds and active-face selection. Small level-zero runs also validate the [no-cursor](smoke-none/) and [rings-only](smoke-rings/) diagnostic modes; these are smoke checks, not additional performance claims.

## Provenance and reproduction

Baseline: `80b3543`, source SHA-256 `59819d8fb5f8ce950aa33d6d1687bf89d00c487efbd26aa5631e9562e4f35bd9`. Final measured source SHA-256: `34b9a939367007cf54f5d16031143a806d95560a006bdb26b38b48ca24112618`. Final test executable SHA-256: `368565be03339e0efcd5a2b9cdb46d57be283319964140ccc9bc07bc630bb9c7`. The baseline binary was saved only after checking its source and executable hashes against the previous report. The separately saved instrumented baseline retains [its own provenance](instrumented-provenance.json); it adds diagnostic hooks without the allocation fix.

Runs were sequential on the same docked, AC-powered i7-7500U with about 7.63 GiB RAM. [Power readings](power.json), source/executable hashes, guards and raw samples are retained. The controlled allocator runs are separate from the primary comparison, which uses the normal environment. Only optimized library tests were built. Full-suite/lint/configured GPU CI remain for the eventual PR.

```sh
CLAY_CAMERA_LEVEL=4 CLAY_CAMERA_PROFILE=/tmp/camera.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_camera_picking -- --exact --ignored --test-threads=1
CLAY_CAMERA_LEVEL=4 CLAY_CAMERA_FIXTURE=hidden CLAY_CAMERA_CURSOR=none CLAY_SCENE_STAGES=1 CLAY_CAMERA_PROFILE=/tmp/scene-phases.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_camera_picking -- --exact --ignored --test-threads=1
cargo test --release --locked --lib viewport::scene::tests:: -- --test-threads=1
```

Recorded diagnostic processes used the bounded runner with a 3072 MiB process RSS limit, 768 MiB minimum available memory and a 600-second timeout. The previous report's combined p50 table was also corrected to use the same nearest-rank convention; its 33–36% conclusion is unchanged.

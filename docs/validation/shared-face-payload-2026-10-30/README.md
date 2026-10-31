# Share unchanged face attributes across display snapshots

Follow-up to [discarded shading-copy removal](../release-shading-copy-2026-10-30/README.md), under #103.

## Change and invalidation

Display face colours and UV corners now use independent `Arc<Vec<...>>` payloads. Fresh display construction builds each vector as before, trims unused capacity once, then wraps the vector without copying its contents into another array. Cached live snapshots and hidden canonical release share those payloads instead of deep-copying them.

Uploads first check whether the old and new payloads share immutable storage. Identical storage establishes that the existing GPU bytes are unchanged. Separately rebuilt data falls back to the previous value comparison. Colours and UVs remain independent; shading triangle indices still receive their own comparison and update. Fan splitting/merging, material-stride changes, empty displays, binding-limit changes and batching retain their existing handling.

Mutation requires a distinct allocation or copy-on-write, preserving old display snapshots. The identity check is specifically an upload-reuse predicate, not a replacement for floating-point equality: unchanged shared NaN bytes can be reused, while separately allocated NaNs still trigger the old comparison/write path. The signed-zero fallback also retains its previous semantics. Document revisions, shaders, sculpt calculations, history and file formats are unchanged.

## Baseline and measurement scope

Baseline commit: `7523ede`, source SHA-256 `fa3fd45a88334e6514bd957a3a5fa27994f82b28a4f3c1a4100268361da1db9a`, verified against the commit. The fresh baseline reused the already validated optimized library executable for that identical source; only documentation/Git metadata had changed since its build. Final measured source SHA-256: `8b1e3e3a3b8b76970917e8056ed878e1a38698ddb712e23a8860398607fc0794`. Environment metadata identifies the parent revision and source digest for dirty-tree measurements.

The before and final after matrices each cover all twelve sphere, organic and hidden workloads, with three repetitions of twelve dabs per brush. Largest sphere and hidden cases were repeated, plus a smaller hidden case to investigate Undo timing. All runs were sequential with the existing 1,536 MiB process RSS and 600-second case guards. Strict comparisons passed environment, fixture/count, output-hash, metric coverage and sample-count checks. The upload probe's frame/write metadata also matches; no necessary transfer was omitted.

Hardware remained the docked i7-7500U and Intel HD Graphics 620, Mesa 26.2.4/Vulkan and Rust 1.99.0, with an 800×600 benchmark viewport. [Before](dock-before.json) and [after](dock-after.json) monitor/power settings match. Workload definitions were unchanged. Largest fixtures contain 301,058 vertices and 602,112 source triangles, with 362,494 visible triangles in the hidden case.

## Upload-stage diagnosis

The existing ignored release-upload probe measures the actual upload path. The table shows the face-comparison stage's p50 and completed upload p50 / p95, in milliseconds. Completed upload combines paired CPU-upload and submit/completion samples before taking percentiles.

| Brush | Face-data check before → after p50 | Completed upload before → after p50 / p95 |
| --- | ---: | ---: |
| Standard | 4.7488 → 0.000467 | 61.44 / 61.60 → 56.68 / 57.31 |
| Clay | 4.6356 → 0.000449 | 28.92 / 28.98 → 24.79 / 25.39 |

The unchanged-payload check is below 0.001 ms in this probe. Full upload still includes vertex packing/storage and index work. Raw [before](upload-before/profile.json) and [after](upload-after/profile.json) results preserve all stages, sample counts and writes. There are only three release samples per brush, so p95 is the observed maximum, not a robust tail estimate.

## Sustained stroke work

The following totals pair each dab's brush calculation, display preparation and completed upload before calculating p50 / p95. They have 33 sustained samples per brush and exclude drawing, event scheduling and OS presentation. These are not desktop frame-rate measurements. All rows use the largest fixtures and milliseconds.

| Fixture | Brush | Before p50 / p95 | After p50 / p95 | Repeat p50 / p95 | Median time reduction |
| --- | --- | ---: | ---: | ---: | ---: |
| Sphere | Standard | 56.38 / 62.64 | 45.50 / 47.32 | 45.54 / 47.59 | 19.3% |
| Sphere | Clay | 60.99 / 62.22 | 50.03 / 51.40 | 50.18 / 52.50 | 18.0% |
| Sphere | Paint | 43.27 / 50.42 | 32.74 / 35.09 | 31.70 / 35.15 | 24.3% |
| Organic | Standard | 67.41 / 79.27 | 56.45 / 70.07 | — | 16.3% |
| Organic | Clay | 72.25 / 77.47 | 60.06 / 66.01 | — | 16.9% |
| Organic | Paint | 41.48 / 49.47 | 30.43 / 34.50 | — | 26.6% |
| Hidden organic | Standard | 60.46 / 69.95 | 54.41 / 58.21 | 54.27 / 58.05 | 10.0% |
| Hidden organic | Clay | 64.30 / 66.40 | 57.42 / 59.38 | 57.11 / 58.93 | 10.7% |
| Hidden organic | Paint | 34.01 / 35.21 | 27.42 / 30.86 | 27.09 / 28.92 | 19.4% |

Both display copying and upload comparisons contribute to these observed savings. Full phase data, including smaller fixtures, is retained in the [comparison](comparison.csv) and [repeat comparison](repeat-comparison.csv). No claim is made that every operation became faster.

## Release, initial display and slower observations

Release totals combine paired stroke-finalisation, display and upload/completion samples. Times remain p50 / p95 milliseconds. Hidden deformation releases can reuse the shared face payloads; a fully rebuilt display still needs value comparisons.

| Fixture | Brush | Before | After | Repeat |
| --- | --- | ---: | ---: | ---: |
| Sphere | Standard | 212.68 / 212.77 | 215.78 / 218.23 | 210.16 / 214.17 |
| Sphere | Clay | 211.38 / 214.24 | 213.34 / 218.31 | 234.54 / 243.03 |
| Sphere | Paint | 215.20 / 217.52 | 213.66 / 223.14 | 220.15 / 220.81 |
| Organic | Standard | 271.01 / 281.90 | 257.33 / 276.75 | — |
| Organic | Clay | 236.68 / 250.44 | 234.18 / 243.59 | — |
| Organic | Paint | 223.55 / 226.10 | 226.22 / 226.81 | — |
| Hidden organic | Standard | 118.33 / 119.57 | 109.55 / 115.33 | 114.33 / 115.17 |
| Hidden organic | Clay | 85.50 / 89.39 | 78.83 / 78.89 | 79.44 / 88.55 |
| Hidden organic | Paint | 23.11 / 24.17 | 23.49 / 24.27 | 23.44 / 25.14 |

Largest initial display preparation medians were:

| Fixture | Before ms | After ms |
| --- | ---: | ---: |
| Sphere | 187.47 | 185.21 |
| Organic | 195.20 | 196.18 |
| Hidden organic | 279.71 | 284.64 |

Release results outside the cached hidden path are mixed. For example, sphere Clay release rose to 234.54 ms on repeat versus 211.38 ms before. The smaller hidden Paint Undo median rose from 2.62 ms to 6.57 ms and remained 6.32 ms in the [history repeat](history-repeat/hidden-l2.json). Smaller hidden Clay Undo measured 4.39 → 7.26 ms initially, then 4.71 ms on repeat. History code is unchanged and exact restoration tests pass, but these slower observations are retained rather than dismissed. No load/save, Undo or whole-release improvement is claimed across all fixtures.

## Capacity investigation and memory limits

The initial sharing experiment kept vector growth capacity alive. A [small reproduction](capacity-check.rs), using the same payload types and face counts, found 8,413,288 spare bytes for the hidden case and 23,216,128 for visible cases. Its [output](checks/capacity-check.log) confirms trimming reduces capacity to length on this toolchain. The final implementation trims once at construction, avoiding that persistent unused capacity.

The untrimmed experiment is retained in [GPU results](untrimmed-gpu/index.json), [upload stages](untrimmed-upload/profile.json), [comparison](untrimmed-comparison.csv) and a [source patch](untrimmed-source.patch) against `7523ede`. Applying that patch reproduces its recorded source digest, `e335f53ad0147a3699b71b8edc184ab51af804f3547dab21f61692c157e27a0b`. It is not the final implementation.

Whole-process peak RSS remains mixed. Values below are MiB and include benchmark reference snapshots, history, save/load verification, allocator retention and driver behaviour:

| Fixture | Baseline | Untrimmed experiment | Final | Final repeat |
| --- | ---: | ---: | ---: | ---: |
| Sphere | 779.5 | 801.8 | 810.2 | 826.9 |
| Organic | 902.4 | 925.7 | 902.0 | — |
| Hidden organic | 882.8 | 908.4 | 911.6 | 898.1 |

Trimming unused payload capacity does not establish lower process RSS. The sphere and hidden peaks remain above this baseline, and no overall memory-footprint saving is claimed. Sharing replaces duplicate face arrays rather than adding another per-face cache; every measured case stayed within the guard. Geometry limits were not raised.

## Focused validation

Eighteen focused tests passed on the final trimmed implementation:

- A new upload-reuse regression checks copy-on-write snapshot preservation, independently rebuilt equal values, length changes, signed zero and shared/separate NaNs.
- Seven hidden-stroke regressions cover topology, canonical shading, visibility, material edits, unseen final dabs and exact history.
- Two existing CPU checks cover texture-only geometry reuse and stable/distinct group colours.
- Three upload GPU regressions now explicitly verify shared storage during visible/hidden strokes and canonical fan changes, independent colour/UV copy-on-write edits, unchanged writes for rebuilt equal payloads, empty buffers, material layout changes and batching. Rendered results match fresh buffers.
- Five further GPU checks cover hidden visibility/picking/history, texture pixels and UV checker edits/batching, face-colour/depth batching, and offscreen depth/wireframe/cache behaviour.

[Test logs](checks/) are retained. The final hidden-stroke capture matches the inspected [reference](../hidden-stroke-cache-2026-10-30/hidden-stroke-cache.png) byte-for-byte, SHA-256 `6a2fad9305165315e1840ae24a36ab740d92ee8ee00a576f1f1a0ceea7e597c3`. Focused formatting and whitespace checks passed. Full-suite and lint checks remain for the eventual PR; no PR was opened.

## Reproduce

Use fresh output directories, matching dock/power conditions, hardware GPU access and sequential runs:

```sh
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/shared-face-run --gpu
python3 scripts/benchmark-capacity.py compare docs/validation/shared-face-payload-2026-10-30/before target/shared-face-run
```

The ignored `viewport::gpu::profile::capacity::gpu_load_profile_release_upload` optimized library test supplies the upload-stage breakdown with `CLAY_UPLOAD_PROFILE` set to a new JSON path. The capacity-growth reproduction can be built independently with `rustc`; it models builder capacity and does not measure the renderer's complete memory footprint.

The next [brush-loop preparation batch](../brush-loop-preparation-2026-10-30/README.md) measures repeated rotation/pressure work and broader brush footprints separately from default capacity strokes.

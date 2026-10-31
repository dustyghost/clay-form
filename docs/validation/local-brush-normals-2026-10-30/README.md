# Local brush normal calculation

Follow-up to [deformation scan reduction](../deformation-scan-2026-10-30/README.md), under #103.

## Change and correctness boundary

Most sculpt brushes only read vertex normals inside the existing distance bound around their dabs. Normal preparation now intersects that bound with the existing sculptable-vertex flags, skips triangles with no requested corner, and normalizes only requested vertices. Every requested normal still includes **all incident triangles in the original floating-point summation order**. The same narrowed flags also avoid entering the brush loop for vertices its distance test would reject. No brush weights, displacement arithmetic or safety guards change.

Polish keeps full normals because its corner protection reads them across the surface. Snake uses the dab frame rather than mesh normals and skips their calculation. A small evenly spaced sample chooses the full calculation for broadly covering brushes, avoiding an unproductive selection pass. Sampling can only select the full path; missed outliers still receive exact normals and the original brush-loop distance rejection. Empty eligible regions need no normals.

There is no retained cache or new invalidation state. Visibility and topology are read anew on each operation. The partial path still scans vertices and triangle indices and allocates a vertex-sized normal array; the work still scales with mesh size. Smooth, Paint, projection and other callers of the general `Mesh::normals()` remain unchanged.

## Direct operation measurements

The existing stage probe uses 301,058 vertices / 602,112 triangles, visible and partially hidden spheres, radii 0.18 and 0.65, Standard/Clay and an oversized Grab proposal. Each case has one warm-up and five measured operations. Clone/hash fixture overhead is outside the timer; the stroke’s own snapshot remains inside. The normal stage now includes footprint selection, and eligibility is measured before it.

For the small footprints, normal preparation falls from 6.13–6.26 ms to 3.50–4.01 ms in the first run. Avoiding distant vertices in the brush loop provides further savings.

Total direct-operation medians in milliseconds:

| Mesh | Brush | Radius | Before | After | Before repeat | After repeat |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Visible | Standard | 0.18 | 13.68 | 8.52 | 13.78 | 8.35 |
| Visible | Standard | 0.65 | 21.28 | 17.62 | 20.85 | 16.49 |
| Visible | Clay | 0.18 | 19.48 | 13.08 | 18.59 | 13.25 |
| Visible | Clay | 0.65 | 26.82 | 22.68 | 25.77 | 22.44 |
| Visible | Grab | 0.18 | 17.42 | 11.71 | 17.00 | 11.74 |
| Visible | Grab | 0.65 | 62.18 | 58.18 | 61.65 | 57.55 |
| Hidden | Standard | 0.18 | 13.34 | 9.39 | 13.27 | 9.25 |
| Hidden | Standard | 0.65 | 18.05 | 14.53 | 17.57 | 13.67 |
| Hidden | Clay | 0.18 | 16.52 | 12.97 | 17.17 | 12.80 |
| Hidden | Clay | 0.65 | 21.48 | 17.65 | 20.60 | 18.18 |
| Hidden | Grab | 0.18 | 16.62 | 12.00 | 16.04 | 11.87 |
| Hidden | Grab | 0.65 | 41.67 | 38.01 | 41.30 | 38.89 |

All 60 workload output hashes, change flags and safety-pass counts match across all four runs. [Stage comparison](stage-comparison.csv) includes every phase; total medians are measured directly, not summed from separate phase medians. These are CPU brush-operation times, not frame rates.

## Broad brushes and symmetry

An additional opt-in probe alternates the full and local paths within the same executable, with one warm-up and five measured pairs. It covers five brushes, visible/hidden geometry, radii 0.18/0.65/2.0, and one/eight symmetry copies: 60 cases and 300 measured pairs. Every pair asserts equal output hashes and change flags. [Full comparison](local-comparison.csv) retains all gains and regressions. Selected operation medians, milliseconds:

| Mesh | Brush | Radius | Copies | Full | Local | Reduction |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Visible | Standard | 0.18 | 1 | 12.24 | 7.21 | 41.0% |
| Hidden | Standard | 0.18 | 1 | 11.98 | 7.54 | 37.0% |
| Visible | Clay | 0.18 | 8 | 50.37 | 45.93 | 8.8% |
| Visible | Inflate | 0.65 | 8 | 36.73 | 33.63 | 8.5% |
| Visible | Snake | 0.18 | 1 | 15.76 | 8.35 | 47.0% |
| Visible | Polish | 0.18 | 1 | 26.38 | 26.21 | 0.6% |
| Visible | Standard | 2.0 | 1 | 33.96 | 33.77 | 0.6% |
| Visible | Clay | 2.0 | 1 | 42.81 | 42.82 | 0.0% |
| Visible | Inflate | 2.0 | 1 | 32.64 | 32.57 | 0.2% |
| Visible | Standard | 2.0 | 8 | 208.90 | 210.14 | -0.6% |
| Hidden | Standard | 2.0 | 8 | 205.31 | 207.14 | -0.9% |

Polish intentionally retains its original work. Broad-brush fallback cases are expected to stay broadly unchanged, with timing variation retained above and in the CSV. No improvement is claimed for every brush or footprint.

## Sustained strokes and memory

The existing CPU-only capacity workload covers the largest sphere, organic and hidden fixtures, three 12-dab strokes per brush, with 33 sustained samples. All workload hashes and sample counts match. Entries are brush calculation p50 / p95 milliseconds:

| Fixture | Brush | Before | After | Median reduction |
| --- | --- | ---: | ---: | ---: |
| Sphere | Standard | 12.26 / 16.36 | 6.69 / 7.42 | 45.5% |
| Sphere | Clay | 16.85 / 17.83 | 11.32 / 12.48 | 32.8% |
| Sphere | Paint | 3.50 / 4.09 | 3.52 / 4.38 | -0.4% |
| Organic | Standard | 12.34 / 13.44 | 6.71 / 8.01 | 45.7% |
| Organic | Clay | 16.68 / 17.73 | 11.45 / 12.64 | 31.4% |
| Organic | Paint | 3.17 / 3.91 | 3.08 / 5.84 | 2.8% |
| Hidden | Standard | 12.08 / 12.92 | 7.78 / 9.33 | 35.5% |
| Hidden | Clay | 15.83 / 17.31 | 11.28 / 12.49 | 28.8% |
| Hidden | Paint | 2.15 / 3.35 | 2.17 / 3.34 | -0.6% |

The [complete comparison](capacity-comparison.csv) also retains display preparation, release, Undo/Redo, picking and I/O timings. Paint does not use this code path; its organic p95 increased from 3.91 to 5.84 ms, while its medians stayed broadly similar. GPU drawing/upload was not remeasured for this CPU-only change.

| Fixture | Before peak RSS (MiB) | After peak RSS (MiB) |
| --- | ---: | ---: |
| Sphere | 605.6 | 612.2 |
| Organic | 670.9 | 675.9 |
| Hidden | 708.0 | 705.9 |

These whole-process peaks include history, verification and allocator retention. No RAM reduction is claimed. Runs stayed within the 1536 MiB / 600 second process guard. Capacity settings and defaults are unchanged; multi-million-polygon performance remains unmeasured.

## Validation and provenance

**55 focused tests passed** on the final measured source. New regressions compare selected normals bit for bit on empty, isolated, degenerate, opposing-winding and ordinary meshes; check the exact radius boundary; and exercise the broad-path sampling fallback with an unsampled outlier. The full-path comparison covers 396 brush applications across 11 brush kinds, cube/sphere fixtures, masks, hidden faces, mirror/radial symmetry, alpha/stencil modes, repeated/inverted strokes, subdivision and revealing geometry between strokes. Existing checks cover deformation safety, smoothing, projection, relaxation, Clay, Polish, Detail, Snake, hidden groups and adaptive strokes. [Logs](checks/) are retained.

Baseline production commit: `cf77c94`. Baseline source SHA-256: `55f942a70729f254e0ceb981b32603d9efe829076ed6a41bdd62e08d8684d64f`. Final measured source: `871958e483ead0b1565e4edff3a42a38c8f65c1a023a0f75a9b5796fceb66984`. The baseline executable was saved immediately after building; repeat and capacity runs record their original build source and executable hash rather than attributing a reused executable to current source.

All CPU workloads ran sequentially on the AC-powered i7-7500U (2 cores / 4 threads, approximately 7.63 GiB RAM), using the same optimized library-test build settings, `powersave` / `balance_performance`, and bounded runner. [Power configuration](power.json), environment metadata, raw timings and hashes are retained. Targeted formatting and whitespace checks passed. Full-suite/lint/CI checks remain for the eventual PR; no PR was opened.

## Reproduce

```sh
CLAY_STAMP_PROFILE=/tmp/stamp-stages.json cargo test --release --locked --lib profile_stamp_stages -- --ignored --nocapture --test-threads=1
CLAY_NORMAL_PROFILE=/tmp/local-normals.json cargo test --release --locked --lib profile_local_stamp_normals -- --ignored --nocapture --test-threads=1
cargo test --release --locked --lib sculpt::stamp_normals::tests:: -- --test-threads=1
GIT_OPTIONAL_LOCKS=0 python3 scripts/benchmark-capacity.py run target/normal-capacity --fixtures sphere organic hidden --levels 3
python3 scripts/benchmark-capacity.py compare docs/validation/local-brush-normals-2026-10-30/capacity-before target/normal-capacity
```

Recorded diagnostic runs used the capacity script’s `bounded_run` helper with the exact test names and output-path variables above. Timing probes are ignored in routine tests and have no wall-clock assertions.

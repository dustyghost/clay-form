# Matched-input retopology timing — 6 November 2026

**Historical baseline:** the subsequent [native integer port](native-integer-port.md#release-timings)
reduces the prepared-input Rust solve to 8.319 s and accepts the resulting mesh.
The measurements below retain the earlier implementation and failure outcome.

The current release Rust reconstruction takes **17.83 seconds**, versus
**8.51 seconds** for Blender's C++ QuadriFlow operator, on the same saved
prepared character. Shared preparation is excluded from both timings. This
is about 2.10 times longer for the current Rust implementation; it is not a
language-performance comparison or evidence of equivalent output quality.

Both solvers receive 52,083 vertices / 104,162 triangles, request 10,000 quads,
use uniform density and no guides, and disable sharp-feature preservation.
The native solver runs serially; Blender uses `-t 1`. Four runs were serialized
in Rust/C++/C++/Rust order with the shared lock and the same 700 MiB, no-swap
memory cap. Native uses standard Cargo release optimisation (`opt-level=3`).

| Work | First run | Second run | Median |
| --- | ---: | ---: | ---: |
| Rust `reconstruct_with_progress`, including final rejection | 17.805 s | 17.845 s | 17.825 s |
| Blender `quadriflow_remesh` operator | 8.458 s | 8.556 s | 8.507 s |
| Entire Rust comparison process | 17.938 s | 17.988 s | 17.963 s |
| Entire Blender comparison process | 11.133 s | 10.577 s | 10.855 s |

Solver timers exclude file loading and export. The process measurements include
different startup and export workloads and should not be substituted for solver
timings. No audit snapshot serialization or reference-stage import occurs in
the timed native solve.

The previously measured shared preparation costs **15.958 seconds** in release.
Adding the separately measured preparation and solver medians gives about
**33.78 seconds for Rust** and **24.46 seconds for C++ using that same
preparation**. These sums describe separate stages, not a fused application
benchmark. They do not claim that original QuadriFlow inherently requires this
additional preparation.

## Where native time goes

Durations come from the existing progress events, not a sampling profiler:

| Native stage | Median elapsed |
| --- | ---: |
| Input checks, fields and initial integer offsets | 1.787 s |
| Integer correction | 10.187 s |
| Integer subdivision | 0.060 s |
| Inversion repair | 1.276 s |
| Initial fitting and extraction | 0.333 s |
| Quad optimization | 2.325 s |
| Final surface fitting and shape rejection | 1.858 s |

Integer correction accounts for approximately 57% of the native solver time.
This reinforces the [same-input mapping control](native-input-preparation.md):
the integer stage is both a quality difference and the largest measured native
cost. Guide paths and density paint are disabled, so their enabled cost has not
been measured here. Supporting their data structures may still incur baseline
overhead; this experiment does not isolate that overhead.

## Outcomes and limits

Both native runs finish by rejecting the same finger-coverage error. The timer
includes final fitting and validation up to that rejection; it does not produce
an accepted layout. Blender returns 9,868 quads. Both saved Blender OBJ files
match the earlier control byte for byte, so its recorded quality measurement
still applies: it preserves the sampled shape but has one outward-orientation
failure under our checker. Those independent checks are outside Blender's
operator timer. Thus the timings are for two current execution paths, not two
equally acceptable production results.

No solver code, acceptance limit, iteration count or runtime dependency changed.
No new visual rendering was necessary: C++ output identity is verified and the
native outcome matches the retained rejected case. Broader test suites were
not rerun for this read-only benchmark.

The [command, timing and provenance record](native-retopology-performance.json)
contains binary/source hashes, complete stage events, outcomes, memory pressure
and CPU/process timings. Local artifacts are in
`retopology-review/retopology-timing-20261106/` beside the worktree. The build
command was:

```sh
cargo build --locked --release --manifest-path crates/clay-retopology/Cargo.toml \
  --features audit --example compare -j1
```

The next performance target is integer correction on the fixed saved input,
retaining the existing quality checks and comparing against the pinned original
mapping implementation. Preparation remains a separate optimization target.

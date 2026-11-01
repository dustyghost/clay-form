# Local mesh fusion validation

Implementation evidence for [#120](https://github.com/dustyghost/clay-form/issues/120) and [#121](https://github.com/dustyghost/clay-form/issues/121), 1 November 2026. These fixtures establish local preservation and the interaction flow. They are not an acceptance claim for every sculpt or every surface Boolean configuration.

## Preservation and transactions

`cargo test --locked --lib fusion -- --test-threads=1` runs the focused geometry and application tests. Coverage includes:

- 8/8, 8/12 and 7/13 open rims; several segment counts; alignment offset; different radii; rotated, offset and mildly nonplanar ends. Unselected holes remain unchanged and selected rim edges have exactly two incident triangles.
- Intersecting spheres, triangle-only versus mixed quad/triangle inputs, 48/36 source tessellation, two separated intersection pairs, and shallow overlap. Closed results have consistent edge winding and connected vertex fans.
- Exact source-index correspondence before compaction and an explicit old-to-new map through compaction. Retained positions, triangles/quads, material values, masks, vertex groups, face groups, hidden flags, UV corners/masks and crease/seam edges are compared.
- Obstructing faces, protected rims, inward shells, invalid options, malformed vertex fans, a small mesh budget and cooperative cancellation are rejected. A translated disjoint sphere case guards against numerical false contacts.
- Preview leaves document geometry/history/dirty state untouched; stale settings and stale scenes cannot apply. Apply is one Undo step, restores originals with Redo, survives project serialization, and retains hidden source subdivision stacks. Same-model joins are covered separately.

The existing equal-count Modeling bridge implementation is unchanged. Full tests, formatting, lint and GPU checks run in PR CI.

## Actual controls and visual review

The ignored `local_fusion_controls_offscreen_review` test runs in GPU CI. Locally it was rendered through the real egui/wgpu application using llvmpipe (Mesa 26.2.6, LLVM 23.1.4), one software-rendering thread, and an unoptimized Rust build. Pointer events select both viewport rims and click Preview/Apply/Cancel. The walkthrough checks Undo/Redo, closed intersection output and a 640×480 window. It does not replace a physical pen/driver check.

| Capture | Inspect |
| --- | --- |
| [Open ends before](01-open-ends-before.png) | Two separated sources and explicit hole selection |
| [Open ends preview](02-open-ends-preview.png) | Selected rims and the new connector; source surfaces remain in place |
| [Open ends wireframe](03-open-ends-wire.png) | Unequal boundary transition and untouched source quads |
| [Open ends applied](04-open-ends-applied.png) | Joined result after Apply, Undo and Redo |
| [Intersection preview](05-intersection-preview.png) | One local patch at the crossing of two closed surfaces |
| [Intersection wireframe](06-intersection-wire.png) | Retained source tessellation surrounding the patch |
| [Intersection applied](07-intersection-applied.png) | Shaded silhouette and local transition quality |
| [Compact window](08-small-window-controls.png) | Fixed action footer above floating viewport controls |

Colored rim and patch overlays show through surfaces to make the affected topology inspectable. Turn the overlay off or apply to assess the shaded surface. The cut follows source polygon boundaries, so jagged rims and local dimples can remain. Curvature shapes the connector without relaxing the retained sources. This is not an exact intersection curve or a global remesh.

To regenerate the images on a local Vulkan host:

```sh
CLAY_GPU_SOFTWARE=1 WGPU_BACKEND=vulkan LP_NUM_THREADS=1 \
CLAY_GUIDE_PREVIEW=docs/validation/local-fusion \
cargo test --locked --lib local_fusion_controls_offscreen_review -j 1 -- --ignored --nocapture --test-threads=1
```

## Geometry timing and peak resident memory

The opt-in `fusion_dense_source_profile` uses two outward UV spheres, centers separated by 1.7 units with a small Y/Z offset, a two-ring collar, two connector segments and zero curvature. This produces a small join while retaining most source triangles. Measurements below use the unoptimized geometry test process with debug information disabled; elapsed times exclude source creation and the UI/rendering/undo system. Peak RSS is the process high-water mark, cumulative across the three cases, not the operation's incremental allocation.

| Source triangles | Retained triangles | New patch triangles | Geometry time | Process peak RSS |
| ---: | ---: | ---: | ---: | ---: |
| 4,416 | 3,732 | 322 | 0.941 s | 32,840 KiB |
| 18,048 | 16,352 | 508 | 3.856 s | 41,532 KiB |
| 72,960 | 67,824 | 934 | 16.988 s | 76,512 KiB |

Commands were serialized under `/home/mark/.cache/codex-build.lock`, with one compiler job after checking host RAM and pressure. No release build or broad local suite was run.

The deeper-overlap 96-segment sphere experiment with a one-ring collar was rejected by connector intersection validation. A larger source density is not a promise that every collar setting succeeds. Invalid previews leave both sources intact and require a different collar, correspondence or overlap; there is no fallback to whole-mesh remeshing.

## Remaining acceptance work

Review this first implementation on representative detailed production sculpts, including thin nearby sheets and attribute seams, and measure the complete application's preview/Apply memory at larger scene sizes. Visually judge transition deviation and silhouette across those fixtures. Intersection pairing currently requires mutually nearest simple loops; arbitrary junctions, tangencies, coincident faces, containment without a crossing seam and inward cavity shells are unsupported. This evidence does not close either story by itself.

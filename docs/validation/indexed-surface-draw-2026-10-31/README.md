# Indexed surfaces and GPU paint preparation

Follow-up to [picking and scene reuse](../picking-and-scene-reuse-2026-10-31/README.md), under #103.

## Changes

GPU paint preparation no longer filters and copies the full facing-triangle list for the CPU painter. The CPU fallback retains that filtering; GPU depth/clipping continues in the render pipeline.

Basic surfaces now use indexed draws over the existing shading vertices. Shared vertices can reuse vertex-shader work instead of repeating it for every triangle corner. Hard-edge splits and split-buffer local indices remain intact. Visible-wireframe depth bias and translucent backs have matching indexed pipeline variants. Index buffer resizing preserves buffer usage. Backface uniforms and bindings are prepared only when that pass will draw.

The indexed path requires the 12-float basic material layout, no texture maps, no displayed polygroups and no UV inspection. Other surfaces retain the existing per-corner path. Both paths use the same fragment shader, including lighting, masks, brush alpha, symmetry guides and basic channel inspection. This does not change editable meshes, history or capacity limits.

## Dense drawing measurements

Intel HD Graphics 620, Mesa 26.2.4 Vulkan, 800×600 offscreen, docked AC-powered i7-7500U. Each row has 12 measured orbit frames after three warmups. Fixtures have 2,408,448 source triangles; the hidden fixture displays 1,448,235. Upload and scene preparation occur outside the timers. Paint preparation starts with the current scene cached and measures egui paint-call construction. Drawing includes command encoding, submission and waiting for completion, but excludes desktop presentation and UI composition.

| Fixture | Camera | Draw and wait p50 (ms) | Less draw time | Paint preparation p50 (ms) |
| --- | --- | --- | --- | --- |
| Sphere | Orthographic | 80.5 → 19.6 | 76% | 20.9 → 0.034 |
| Sphere | Perspective | 83.6 → 23.9 | 71% | 10.6 → 0.033 |
| Organic | Orthographic | 80.1 → 16.5 | 79% | 8.2 → 0.033 |
| Organic | Perspective | 83.5 → 17.7 | 79% | 10.0 → 0.032 |
| Hidden | Orthographic | 55.3 → 17.4 | 69% | 11.4 → 0.031 |
| Hidden | Perspective | 57.7 → 15.3 | 74% | 10.6 → 0.031 |

Drawing takes **69–79% less time** on these plain-surface fixtures. Paint preparation is about 0.03 ms after removing the redundant scan. These are separate operation timings, not application frame rates. Textured, polygroup and extended-material surfaces do not receive the indexed-draw gain, though their GPU paint calls also skip the CPU-only face list. [Comparison CSV](comparison.csv), [baseline samples](before/profile.json) and [final samples](after/profile.json) include p95 values and every sample.

## Pixel correctness and rounding

The focused indexed regression compares 60 combinations of camera, single/split buffers, basic channel inspection, masked/hidden/inactive geometry, image brush alpha, symmetry, visible/transparent wireframe and translucent backs against the general pipeline. Those comparisons are exact. The existing eight-case brush-footprint capture is also byte-identical to the preceding CPU batch.

At dense resolution, both pipelines are rendered repeatedly in the same process outside the timers. Repeated renders of each path are exact. Every general-path capture matches the pre-change binary byte for byte. The indexed captures differ in **5 pixels across 2,880,000 pixels**, at most 2 pixels in one image, with a maximum difference of one 8-bit colour step. Alpha and background/surface coverage match exactly. This isolates the small shading-rounding difference to the GPU draw path; it is not a change to fixture geometry or lighting inputs. The manual dense diagnostic checks the one-step bound while the ordinary indexed regression retains exact equality. [Comparison hashes and counts](capture-comparison.json) and both capture sets are retained; the sphere and hidden-face captures were visually inspected.

## Validation and memory

**28 focused CPU tests and 16 Vulkan tests pass.** Coverage includes prepared-scene invalidation, picking, brush previews, split buffers, upload layout/fan changes, depth/clipping, compositing, ghost backs, floating-point textures, UV checkers, material layers, and stroke/history consistency. [Logs and executable provenance](checks/) retain the results. The dense same-binary comparison was added afterward without changing production code or shaders; its final run is retained separately.

An additional [integrated workload](integrated/) passed on sphere, organic and hidden fixtures at 602,112 source triangles: one trial each of Standard, Clay and Paint, 12 dabs per stroke, release display/upload, orbit drawing, picks and exact undo/redo restoration. It used at most **739.4 MiB process RSS** and no sampled process swap, under 2048 MiB RSS / 768 MiB reserve / 600-second guards. This was a correctness and memory check, not a paired timing comparison. The full sculpt workload ran at this smaller density to preserve available memory headroom; the isolated drawing measurement ran at 2.4 million triangles.

The dense draw probe reached 1182.3 → 1186.7 MiB peak process RSS, with no sampled process swap. Both completed within 3072 MiB RSS / 768 MiB reserve / 600 seconds. No general memory saving is claimed. GPU allocation totals, long sessions, arbitrary imported topology and input-to-present latency are not measured here.

## Reproduction and provenance

The baseline is `2ffdc0a` plus the new manual draw diagnostic, before production GPU edits. Runs were sequential and used optimized library test executables; no release application build was run. Full-suite/lint/CI remain for the eventual PR. The branch has been pushed without opening a PR.

The established source digest covers Rust and build inputs; the embedded viewport shader has a separate hash:

| Input | Before | Final dense probe |
| --- | --- | --- |
| Source SHA-256 | `53765d0d4929b004421deba5bfa4f017ab3990681392a33dcfb8b2c5b58016b0` | `456d7b9c16a16dc35adb65b64e16f4ef9161f430c6c04f8150479db81e60b34d` |
| Viewport WGSL SHA-256 | `cbad531113f72a9bd37c183f144061154ac578ce640c9374726250a5d21b480a` | `d60b70a1df53553bd300d120eb8b95daba729e01e2bca53fe82fec303fc0d7c4` |
| Test executable SHA-256 | `190fdf8b5977a42f97e368cfbdf9e97e18b502d4339860a0b9ad9045056fa510` | `eaf683493bff1117b68ab39e142bb77703577756875a26b0d7991393a2ccad2d` |

```sh
mkdir -p /tmp/clay-draw
CLAY_CAMERA_LEVEL=4 CLAY_DRAW_PROFILE=/tmp/clay-draw/profile.json cargo test --release --locked --lib viewport::gpu::profile::capacity::gpu_load_profile_draw -- --exact --ignored --test-threads=1
cargo test --release --locked --lib gpu_indexed_surfaces_match_general_pixels_and_split_buffers -- --ignored --test-threads=1
python3 scripts/benchmark-capacity.py run /tmp/clay-integrated --fixtures sphere organic hidden --levels 3 --repeats 1 --gpu --workload interactive --mesh-vertices 2000000 --mesh-triangles 4000000 --max-rss-mib 2048 --min-available-mib 768 --timeout 600
```

The recorded dense runs used the existing capacity runner's `bounded_run` guard around the diagnostic executable. The direct Cargo diagnostic command above does not itself impose a process-memory limit. Use level 3 or a process guard on constrained machines. The diagnostic is excluded from routine GPU CI by its `gpu_load_profile` name; the exact indexed-pixel regression is included.

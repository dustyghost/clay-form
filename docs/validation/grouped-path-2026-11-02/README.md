# Grouped path objects — 2 November 2026

A single connected source mesh supplies Start, Middle and End regions through existing vertex groups. The source stays welded and unchanged. Role bindings use group names, so registry reordering does not retarget them; a missing/renamed group needs explicit reassignment. Only transient evaluation extracts sections. Start/End retain their scaled lengths, Middle repeats and fits the remaining route, and matching boundary loops weld into one result. Closed paths repeat Middle and weld the seam. Generated results intentionally have no UVs; source UVs remain intact.

The primary **Create path object…** action opens the controls even if the initial draft needs group assignments. The separate-source workflow remains under **Advanced linking → Separate pieces along path…**. Its original profile-control layout is retained.

The focused CPU/interaction selection passed **31 tests**, with four deliberately ignored graphics/profiling tests. Coverage includes unequal region lengths, middle-only face and vertex-group membership, unchanged source, connected/manifold output, curves, alternate axes, group-registry order, weighted-group fallback, closed seams, cancellation, invalid assignments/joins, expansion rejection before repeated geometry allocation, private preview, one-step Apply/Undo/Redo, source/path changes, project save/reopen, and baking exactly the preview. Existing attached-array, path and version-1 JSON compatibility regressions are included.

```sh
flock -n /home/mark/.cache/codex-build.lock env CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -j 1 --lib -- grouped_path reusable_path legacy_version_one_json attached_array --test-threads=1
```

The grouped-object real egui/wgpu walkthrough passed on **Mesa llvmpipe, Vulkan** with one software-rendering thread. It chooses arbitrary group names Foot/Repeat/Tip through all three dropdowns, holds each popup open, drags the inline End thickness handle, applies once, bakes through the real controls, and checks Undo/Redo. Captures were inspected for a continuous surface, visible region assignments, reachable controls, and a distinct source retained beside the result. The sample has a flared start, one bulge in Middle, and a tapered end so repetition can be inspected directly.

- [Grouped source preview](01-grouped-path-preview.png)
- [Inline end taper](02-grouped-path-taper.png)
- [Baked result](03-grouped-path-baked.png)

```sh
flock -n /home/mark/.cache/codex-build.lock env CARGO_PROFILE_TEST_DEBUG=0 LP_NUM_THREADS=1 CLAY_GUIDE_PREVIEW=/tmp/clay-grouped-path-review cargo test --locked -j 1 --lib -- grouped_path_object_controls_offscreen_review reusable_path_bend_join_offscreen_review whole_path_transform_offscreen_review reusable_models_offscreen_review --ignored --test-threads=1 --nocapture
```

All four Vulkan walkthroughs in the command above passed: grouped authoring, legacy bend/join/profile controls, whole-path transforms, and the original reusable-model workflow. The grouped workflow is now explicitly included in graphics CI.

Each generated path object remains bounded to 250,000 expanded vertices / 500,000 triangles; project display checks count actual generated geometry against the existing aggregate budget. Unchanged evaluations use the existing placement/display cache. This is allocation/geometry validation, not a whole-process RSS benchmark. Very tight curves can still self-intersect. Guide boundaries must be planar compatible loops. The project format is v6; versions 1–5 remain readable and older applications reject v6 saves. Full suites, Clippy, packaging and GPU CI remain on the PR.

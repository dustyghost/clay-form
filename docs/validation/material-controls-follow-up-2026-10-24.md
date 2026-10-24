# Material-control and stroke-response follow-up

The material library now uses the same centered vertical drag handle as the alpha library. A fixed minimum scroll height prevents earlier sidebar panels from squeezing the swatches. The interactive check resizes it in both directions, scrolls a crowded sidebar, selects the complete second row and edits a material name. Personal names persist with material preferences.

Viewport lighting supports directional and point sources, drag/angle controls, distance, linear-light colour/intensity, ambient fill and camera-relative/world-fixed orientation. The GPU test compares pixels with the CPU shading model; the interactive check moves a light, changes its type and resets the studio setup without editing geometry or history.

## Optimised stroke profile

Manual profile: `cargo test --release --locked --lib gpu_load_profile_strokes -- --ignored --nocapture`. One startup sphere, two or three subdivisions, three consecutive dabs per brush. Timing is diagnostic and excluded from routine CI. These are CPU display-preparation measurements, **not total frame times**.

| Brush, 301,058 vertices | Display rebuild before (ms, mean) | Temporary display after (ms, mean) |
| --- | ---: | ---: |
| Paint | 308.6 | 25.1 |
| Standard | 307.5 | 25.8 |
| Clay | 314.5 | 24.3 |

The fast path reuses display topology and smoothing fans during a dense fixed-topology stroke. Paint retains exact normals; deformation uses temporary area-weighted normals while moving. The renderer rebuilds full angle-weighted shading when the pen lifts. Authored paint/positions remain full resolution throughout, and history/cancellation semantics stay unchanged.

The initial fast path applies to a single visible sculpt model with at least 20,000 vertices, no hidden faces, creases or temporary thickness, and unchanged triangles/quads. Other scenes retain the existing full display preparation. This keeps topology, visibility, multi-model and hard-edge behavior correct while addressing the reported dense-sphere case.

Focused regressions compare full-resolution attributes, exercise Undo/Redo and incompatible topology, and verify that the renderer refreshes exact final shading even when the document revision does not change on pen-up. Full checks remain in PR CI.


The final profile includes the expanded material schema. Copy-on-write layer storage keeps a vertex material at 80 bytes instead of copying the entire layer stack per vertex. The live display copies current mesh data once; ordinary materials keep compact GPU packing. A transient implementation with inline layers measured 73–94 ms for dense paint display preparation, which prompted this memory fix before handoff. These measurements do not include the complete GPU upload, frame rendering or input pipeline.

## Layers, presets and exchange

The material checks cover shared definitions and partial paint weights, channel-only edits, all stored subdivision levels, Undo/Redo, legacy defaults and native persistence. The final UI walkthrough resizes a crowded swatch grid, renames a material, adds a layer to an already filled surface and opens the PLY chooser through the normal toolbar. Export tests verify PLY colour bytes, residual/definition reconstruction, explicit overwrite confirmation and cancellation without replacing the destination. Lighting and dense-stroke GPU checks were rerun after shader and storage changes.

The gallery exercises all eleven presets, including Matte clay, Cool clay, Beeswax, Ivory wax, Warm skin and Deep skin. Wax and skin use subsurface weight/radius/scale and an additional subtle sheen. Their previews were visually inspected. GPU tests check both compact and extended packing, including layer and subsurface channels, against CPU pixels and across buffer-batch boundaries. The viewport's subsurface effect is a fast wrapped-diffuse approximation; Blender export uses Principled subsurface scattering.

A Blender 5.2.2 LTS round trip validates a 4,706-vertex fixture: 70 scalar channels, shared definition values, paint weights, split normals, axis conversion and standard node connections. Editing the shared layer emission changed 33,287 rendered pixels in the 256×256 comparison. Render noise from the eight-sample manual fixture is unrelated to viewport shading. No full render-equivalence claim is made.

Manual reproduction:

```sh
CLAY_GUIDE_PREVIEW=/tmp/clay-new-presets cargo test --locked --lib material_examples_manual_capture -- --ignored
CLAY_EXPORT_REVIEW=/tmp/clay-layer-export cargo test --locked --lib material_blender_export_manual_review -- --ignored
blender -b /tmp/clay-layer-export/layered.blend -noaudio --python-exit-code 1 --python scripts/validate-layered-export-blender.py -- /tmp/clay-layer-export
```

Blender validation should run outside a sandbox that denies its audio/runtime IPC. The application exporter bounds conversion time and cleans its private temporary directory. New native saves use project version 2; focused checks confirm version 1 JSON remains readable without changing its data shape. Full suite/lint and the configured GPU matrix remain for PR CI; physical tablet feel remains for hands-on testing.

## Layer removal and material reset follow-up

The focused material run passed 21 tests. The real-control walkthrough also passed: remove an added layer, verify its row disappears and the painted surface returns to its previous values, Undo, reset the renamed material to its factory preset, retain the name, then Undo again. Captures `material-layer-removed.png` and `material-reset.png` were visually inspected. Additional checks preserve remaining layer order and muted layers, retain copy reset settings through preferences persistence, and restore shared material values across subdivision levels.

## Whole-material removal follow-up

A focused regression removes a personal material blended over another definition on a small mesh with lower and higher subdivision levels and a duplicated submodel. All surface values and other definition weights remain unchanged. Project serialization, removal of the personal library entry, Undo/Redo restoration and persisted built-in visibility pass. The real-control walkthrough removes the renamed swatch, verifies it disappears while paint stays unchanged, then exercises Undo/Redo and restoring built-in swatches without rebinding paint. The `material-removed.png` capture was visually inspected.

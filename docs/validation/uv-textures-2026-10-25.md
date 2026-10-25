# UV / texture batch validation

Batch: #12, #11, #22. PR CI owns the full test suite, formatting, Clippy and the existing graphics matrix.

Focused local checks:

- Cube, sphere and a welded cylinder with one side seam and two cap rims unwrap into finite, positive-area charts. The cylinder produces three islands; cube charts preserve sculpt vertex sharing. Cancellation and closed-chart rejection keep input intact.
- The OBJ tests cover forward/negative indices, partial UVs, UV seams without geometric vertex splitting, quad topology and UV export/import.
- Texture strokes protect masks and hidden faces, respond to pressure, avoid painting a sheet only 0.001 units behind another, and pad into unoccupied gutters. Shared-image edits, Undo/Redo, native serialization and stale-topology handling are covered.
- Reload tests cover stable identity, changed dimensions, local-edit conflicts, corrupt/incomplete input, explicit conflict replacement, failed GIMP launch and undoable updates.
- Vulkan pixel checks verify actual texture rendering, checker mode and forced buffer batching. The interaction test clicks the UV editor and texture controls, paints through the viewport, verifies the selected red colour in the image, and checks Undo/Redo. The rendered image was visually inspected. Hardware Intel HD 620 and Mesa llvmpipe were both exercised during iteration.
- GIMP 3.2.6 loaded a generated RGBA16 PNG, exported unchanged data, applied a foreground colour and exported with an explicit RGB ICC profile. Clay Form's decoder preserved the unchanged scalar/alpha values within one 16-bit unit. It recovered the expected sRGB colour from GIMP's linear foreground colour and retained the ICC profile through another PNG export/import. The GIMP foreground fill is opaque; the unchanged fixture separately verifies nonopaque alpha.

The manual GIMP fixture is reproducible with:

```sh
gimp -n -i -d -f -s --batch-interpreter=python-fu-eval \
  --batch="exec(open('scripts/validate-textures-gimp.py').read())" --quit
cargo test --locked --lib gimp_texture_colour_alpha_and_data_precision_review -- --ignored
```

The script writes deterministic `/tmp/clay-gimp-*` fixtures. GIMP interoperability stays outside routine CI. The GPU pixel test and real-control interaction test run in CI.

Initial limits: RGB/roughness/metallic images only, up to 2048² and four million unique pixels per scene, 0–1 painting, surface occlusion, four-pixel gutter padding. Whole-scene Blender round-trip, UDIM painting, normal/height image painting, and topology reprojection are separate work. UV changes move image placement without rebaking pixels. Native-file size limits still apply.

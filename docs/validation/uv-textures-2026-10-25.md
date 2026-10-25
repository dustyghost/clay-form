# UV / texture batch validation

Batch: #12, #11, #22. PR CI owns the full test suite, formatting, Clippy and the existing graphics matrix.

Focused local checks:

- Cube, sphere and a welded cylinder with one side seam and two cap rims unwrap into finite, positive-area charts. The cylinder produces three islands; cube charts preserve sculpt vertex sharing. Cancellation and closed-chart rejection keep input intact.
- Full-window workspace and arrangement: controls are checked to be in the right sidebar while the canvas fills the remaining height. Actual clicks exercise Arrange selected and Auto arrange all with Undo. A 45° square fills over 95% of the image at 1% padding after straightening; mixed rectangular islands retain a common scale and padding with no overlaps. Selected-only packing preserves unselected and masked corners bit-for-bit, and failed arrangements leave the mapping unchanged.
- Follow-up UV controls: rendered interaction checks click Expand/Restore, Move/Rotate/Scale handles, Paint/Clear mask, Remove islands, and Modeling Mark/Clear UV seams. Drags create one Undo step; Escape cancels transforms and mask strokes. The expanded editor capture was inspected for visible controls. Masks pin corners during transforms and keep whole islands fixed during packing; persistence and OBJ checks retain removed mapping without losing mesh geometry or quads. Modeling eraser tests cover face/edge/point brush reach, depth protection and cancellation.
- The OBJ tests cover forward/negative indices, partial UVs, UV seams without geometric vertex splitting, quad topology and UV export/import.
- Texture strokes protect masks and hidden faces, respond to pressure, avoid painting a sheet only 0.001 units behind another, and pad into unoccupied gutters. Shared-image edits, Undo/Redo, native serialization and stale-topology handling are covered.
- Reload tests cover stable identity, changed dimensions, local-edit conflicts, corrupt/incomplete input, explicit conflict replacement, failed GIMP launch and undoable updates.
- Vulkan pixel checks verify actual texture rendering, checker mode and forced buffer batching. A stroke crosses a welded geometric edge with separated UV copies; the test checks rendered fragments across the join for colour continuity, and the model/atlas captures were visually inspected. The interaction test clicks the UV editor and texture controls, paints through the viewport, verifies the selected red colour in the image, and checks Undo/Redo. Hardware Intel HD 620 and Mesa llvmpipe were both exercised during iteration.
- The 4K workflow test creates all three channels at 4096², paints, exercises default-budget Undo/Redo, saves/reopens the native project and exports/imports images with exact pixels. Both an all-integer 4K scene and a 4K float colour image alongside two integer maps were exercised. Float tests check HDR/negative values, alpha, half-float input, native precision tags and refusal to clip float images into PNG. A GPU exposure test compares values above one with a clipped reference.
- GIMP 3.2.6 loaded a generated RGBA16 PNG, exported unchanged data, applied a foreground colour and exported with an explicit RGB ICC profile. Clay Form's decoder preserved the unchanged scalar/alpha values within one 16-bit unit. It recovered the expected sRGB colour from GIMP's linear foreground colour and retained the ICC profile through another PNG export/import. The GIMP foreground fill is opaque; the unchanged fixture separately verifies nonopaque alpha. A second real-GIMP EXR fixture preserves negative/HDR values and alpha to within 1e-6 and confirms full-float samples. It exercises the dedicated straight-out/associated-return GIMP alpha adaptation.

The manual GIMP fixture is reproducible with:

```sh
gimp -n -i -d -f -s --batch-interpreter=python-fu-eval \
  --batch="exec(open('scripts/validate-textures-gimp.py').read())" --quit
cargo test --locked --lib gimp_texture_colour_alpha_and_data_precision_review -- --ignored
gimp -n -i -d -f -s --batch-interpreter=python-fu-eval \
  --batch="exec(open('scripts/validate-textures-gimp-exr.py').read())" --quit
cargo test --locked --lib gimp_texture_float_exr_review -- --ignored
```

The script writes deterministic `/tmp/clay-gimp-*` fixtures. GIMP interoperability stays outside routine CI. The GPU pixel test and real-control interaction test run in CI.

Initial limits: RGB/roughness/metallic images only, up to 4096² and three 4K images per scene, 0–1 painting, surface occlusion, four-pixel gutter padding. Whole-scene Blender round-trip, UDIM painting, normal/height image painting, and topology reprojection are separate work. UV changes move image placement without rebaking pixels. Native files use losslessly compressed pixel payloads with a 2 GiB file cap. Float EXR support covers non-deep single-layer RGB/RGBA, linear Rec.709 colour and raw data channels; full OCIO/ACES, arbitrary AOVs and deep files remain separate work.

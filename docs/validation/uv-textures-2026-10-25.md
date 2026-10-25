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

## OpenUSD, FBX and pressure follow-up

OpenUSD export was parsed with Pixar OpenUSD Python bindings and accepted by `usdchecker`. Binary FBX 7400 was re-imported by Blender 5.2.2. The fixture checks geometry/axes, face-corner UVs, normals, material bindings, vertex colours and relative texture resolution. Source EXR pixels preserve negative/HDR values and coverage exactly; preview pixels composite coverage onto the base material with opaque alpha. Tests also verify UV masks/removal metadata, stale-UV rejection, cancellation, overwrite confirmation and separate companion folders on re-export. Interactive Vulkan checks click both export menu items and Cancel; picker captures were visually inspected. Maya and Cinema 4D were not available for native import validation.

```sh
cargo test --locked --lib interchange_manual_review -- --ignored
python3 scripts/validate-interchange-usd.py /tmp/clay-interchange-review
usdchecker /tmp/clay-interchange-review/model.usda
ALSOFT_DRIVERS=null blender -b -noaudio --python-exit-code 1 --python scripts/validate-interchange-fbx.py -- /tmp/clay-interchange-review
```

On this Arch host, Pixar's Python bindings need `LD_PRELOAD=/usr/lib/libjemalloc.so.2` to avoid a static-TLS loader error. Blender's audio shutdown hung in the sandbox after its validation assertions completed; the null audio driver avoids that unrelated dependency. Export fixtures remain manual and require installed Blender/OpenUSD tools.

The previous 4K regression's absolute 16 MiB compressed-project assertion failed after tighter UV packing increased the painted pixel footprint. The reopened pixels were exact and the file was 19,102,237 bytes versus 512 MiB raw image storage. The check now requires better than 16:1 compression on this fixture, retaining a meaningful bound without assuming a particular atlas footprint.

Pressure regressions verify capability discovery for Huion and OpenTabletDriver, rejection of keyboard endpoints, normalized light/firm samples, held pressure, contact release and native pressure retention across frames without new events. Standard freehand displacement at 25% pressure is one quarter of full-pressure displacement with unchanged radius. The host exposes two accessible Huion pressure endpoints, while the virtual OpenTabletDriver artist tablet requires an additional permission rule. That narrow udev rule passes `udevadm verify`; physical feel still needs the artist's check in the updated app.

## Selection responsiveness and direct texture painting

Selecting a UV island previously activated 3D face highlighting that rebuilt the complete polygon list for each selected face. Highlighting now builds topology once per batch. A focused 1,024-polygon selection test records two polygon builds for the complete viewport frame (44 ms including selection and CPU viewport rendering on the development host); it checks that selection leaves geometry revision and history unchanged. UV mask overlays are also batched, and fully transparent triangle overlays are omitted.

**Create & paint on model** creates the chosen image and activates its channel in the 3D view. **Paint on model** and the Paint brush's target controls exit component selection, activate the chosen Paint preset and close the UV editor. The brush panel exposes paint colour/scalar controls alongside the target and mode.

Airbrush uses time-based exponential deposition; spray evaluates seeded droplets in brush coordinates with adjustable flow, density and dot size. Regular strokes keep their existing movement-based deposition. Focused tests cover preset compatibility, changing density/seed/size, flow timing, stationary buildup, release, no hover painting, Undo/Redo, Escape cancellation and unchanged mesh positions. Interactive Vulkan checks click the new creation action and both mode choices, paint on the model, inspect the produced images and exercise Undo. Airbrush/spray and their controls were visually inspected in the rendered captures at `/tmp/clay-paint-modes`.

CI's existing universal-transform interaction check also exposed a small-viewport obstruction after the new docked brush controls reduced viewport width. The compact transform strip now keeps the pivot handle accessible at 640 × 480; the real-control regression passes. Three reported lint errors in the brush-control changes and tablet test placement were corrected.

## Layered PSD, material capture and texture-paint responsiveness

- **Edit in external paint app** now creates a native layered Photoshop PSD with a locked, visible UV guide and an initially selected paint layer. The application is configurable. Reload composites the saved PSD through a GIMP 3 Python worker, excludes the reserved guide layer even inside groups, and never saves over the source PSD. Native 16-bit integer and 32-bit float output retain source precision. The installed GIMP 3.2.6 PSD exporter reduces float to 16-bit; Reload rejects that downgrade. Photoshop itself was unavailable for testing.
- The ignored `layered_gimp_roundtrip_excludes_visible_guide_and_preserves_precision` test uses the actual installed GIMP reader and exporter. It checks ordinary and HDR/negative/alpha values, guide ordering and active paint selection, an actual edited PSD save, guide exclusion after grouping, missing-guide rejection, float-downgrade rejection and unchanged source bytes after reload. Its reproducible GIMP fixture is `scripts/validate-psd-exchange.py`. The visible guide preview was inspected at `/tmp/clay-psd-guide-preview.png`.
- **Capture vertex paint** writes base colour; **Bake material maps** writes unlit base colour, roughness and metallic together, compositing existing texture coverage. Background completion checks the source document before committing. Focused tests cover interpolation, HDR preservation, cancellation, one-step Undo/Redo and rejection of stale results. The rendered texture-control check clicks Bake and its replacement confirmation, then verifies all three maps and Undo/Redo. `/tmp/clay-uv-textures.png` was visually inspected.
- Dab rasterization clips each triangle to the projected brush square before visiting image pixels. In the focused 2K triangle reproduction, work fell from **2,098,176 samples / 1.897 s** to **5,460 samples / 5.86 ms**, preserving identical in-footprint sample coverage. These timings measure that raster step, not the complete application frame. Native little-endian pixels copy directly into the GPU page layout; the renderer retains texture storage and uploads only changed row ranges. A regression covers unchanged pages, multiple changed rows and partial final pages. Existing rendered model-painting, Airbrush and Spray checks pass after the upload change.
- The prior graphics CI failure in material Fill all was reproduced locally. The quick colour picker could round-trip values even without an edit; its temporary values now commit only on an explicit edit. The material interaction check scrolls the outer sidebar to reach the palette below the additional paint controls.

Full tests, lint and graphics remain assigned to PR CI; no release build was run.

## Painting, material and workflow review implementation

The engineering and UX review led to these connected changes:

| Reviewed problem | Result |
| --- | --- |
| Vertex paint could reach a covered, same-facing surface | Vertex and texture painting share projected front-surface eligibility, including other visible models. |
| Stroke spacing and airbrush buildup depended on event timing | Paint uses fixed distance samples with pressure interpolation and elapsed-time substeps; weighted falloff/masks enter the exponential deposition rate. |
| Texture target became stale after model/history/topology changes | Invalid targets reconcile to vertices on model changes, history, geometry commits and UV edits. |
| Sampling and Fill ignored the selected image | Sampling filters unlit image colour and scalar maps with coverage; float colour uses a separate linear HDR brush value so sampling can paint the same values back. Fill follows the active target with separate protected/all modes. |
| Shared images copied on every dab | Linked uses resolve the active image during drawing and synchronize at completion; save/autosave snapshots canonicalize all linked references. |
| Texture-only edits rebuilt geometry and every image upload | Display geometry and GPU mesh buffers are reused; dirty history updates shared 128 KiB atlas chunks and affected upload rows. |
| Each dab scanned/rebuilt full surface state | A stroke-local world-space hierarchy finds candidate triangles, including distant occluders; normals, eligibility and working buffers are reused. Vertex changes visit touched entries. |
| Release rebuilt all UV gutters | Cached UV-to-gutter correspondence copies only changed edge sources. New layouts and replacement/undo images initialize all gutter destinations before resuming incremental updates. |
| Partial material channels rebuilt untouched layers | Colour/roughness/metallic edits retain untouched layer storage and values. |
| Shared sliders rebased every model on every frame | Sliders keep a local draft and commit on release; repeated material values reuse the rebased result. |
| Brush colour, shared material editing and personal presets had ambiguous scope | Brush controls affect future strokes; shared project editing shows painted uses; personal-library saving is explicit and separate from project Undo. |
| Texture setup replaced existing colour | New images default to capturing the existing surface, with transparent and solid-value alternatives and a UV-readiness action. |
| Painting controls competed for screen space | Focus Paint temporarily leaves one sidebar and restores the previous layout; model/target/precision/shared-use status and inspection modes are grouped together. Creation, bake and external settings fold away. |
| Precision was confused with format | Storage choices say 16-bit integer or 32-bit float; PNG/EXR export and PSD/XCF working formats are separate. |
| Bake silently reduced map dimensions and used nearest-neighbour filtering | Existing maps preserve their own dimensions/precision by default; overrides show before/after and linked users. Area downsampling and bilinear enlargement filter linear premultiplied colour. |
| External editing always generated another temporary PSD | Reopen, new snapshot and file relinking operate on persistent working documents. Saved projects keep them in an adjacent `.textures` directory. |
| Float PSD could not be saved without precision loss in default GIMP | Automatic format uses float XCF with GIMP; explicit incompatible float PSD/GIMP output is rejected before creation. Native float PSD and EXR remain available. |
| Generic background status lacked cancellation/recovery | Operations identify capture, bake or layer reload, expose Cancel and bake progress, retain stale-result protection, and offer Retry/relink recovery. |

Focused regressions cover protected float pixels as exact no-ops, covered surfaces, pressure/time partitioning, shared-image allocation reuse, canonical snapshots, gutter cache invalidation, target-aware Fill, sampler coverage/scalars, project/library Undo separation, focus-layout persistence, bake filtering/precision/cancellation and working-file relinking. The spatial fixture narrows 260 triangles to four active candidates while retaining an occluder 100 units in front. The atlas fixture changes only the chunk intersecting an edited float pixel; this is structural evidence, not an end-to-end latency benchmark.

`float_xcf_working_document_survives_save_reload` ran through installed GIMP 3.2.6: native float PSD → layered XCF → ordinary XCF Save → guide-free EXR reload. Negative values, HDR values, fractional alpha and precision survive; source XCF bytes remain unchanged by reload. The conversion uses GIMP's documented [file_save API](https://developer.gimp.org/api/3.0/libgimp/func.file_save.html). Layered interoperability tests remain manual; routine CI retains GPU pixel/depth/clipping and real-control interaction checks.

First-dab hierarchy/normal setup and the first gutter-occupancy calculation after changing UV layout or image dimensions still have a setup cost. The linked 2D image view remains a preview: direct 2D brush painting, UDIMs, layered-BSDF baking and full colour-management pipelines remain separate features. No release build was run; full tests, formatting, lint and configured graphics checks run on the PR commit in CI.

Rendered interaction checks exercise texture creation/bake replacement, target and scalar sampling, Focus Paint/Restore, material copy/reset/layer edits and explicit library save/hide. Captures are reviewed for visible controls and usable layout; the GPU pixel test separately checks unlit/scalar output and incremental image updates.

Sidebar resize follow-up: the reported black strip and clipped brush tiles were reproduced with a four-column brush-control menu. Shared grid column widths expanded the content beyond the panel's painted bounds. Live controls now pack at their natural widths, and brush tiles reflow without cached column widths. Long labels and the stroke-mode dropdown wrap at narrow widths. The `brush_sidebar_resize_offscreen_review` interaction regression drags the sidebar from 600 pixels through 340, 220, 180 and 500 pixels, checks the resulting bounds and control positions, and saves captures for visual inspection. It runs in graphics CI; the narrow and wide captures were inspected locally.

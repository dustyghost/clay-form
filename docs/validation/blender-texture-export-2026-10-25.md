# Blender texture export regression

A plain clay surface exported 245 shader nodes with 70 geometry attribute reads. Blender 5.2.2 EEVEE reported “uses too many attributes” and displayed pink. The exporter also omitted face-corner UVs and image paint, so repairing the shader alone could not restore the painted appearance.

The exporter now folds constants, omits inactive unlinked layers, deduplicates varying fields and packs four scalar values per shader attribute. Source residuals remain metadata. Simple image-painted clay uses five nodes; shared/layered evaluation sits inside a named group. UVs and packed 16-bit PNG or 32-bit EXR images preserve paint coverage, including removed UV faces. PNG colour exports use working sRGB; float colour uses linear Rec.709. Image alpha is coverage over the underlying material, not transparency of the object.

Manual checks reopen files after the exporter's temporary images have been deleted and render in both EEVEE and CPU Cycles. The texture fixture checks UV coordinates, removed faces, embedded image format/precision, alpha wiring, visible colours and absence of pink pixels. The layered fixture exercises five independent varying fields (including signed residuals), packing through both RGB and alpha, and edits a shared emission definition after export. These are interoperability reviews, excluded from routine CI; focused Rust tests cover snapshot metadata, stale-UV rejection, layer reconstruction and lighting snapshots.

```sh
cargo test --locked --lib persistence::material_export::tests
cargo test --locked --lib texture_blender_export_manual_review -- --ignored
blender -b -noaudio --python-exit-code 1 --python scripts/validate-texture-export-blender.py -- /tmp/clay-texture-export
cargo test --locked --lib material_blender_export_manual_review -- --ignored
blender -b /tmp/clay-layer-export/layered.blend -noaudio --python-exit-code 1 --python scripts/validate-layered-export-blender.py -- /tmp/clay-layer-export
```

For a diagnostic session, set `CLAY_EXPORT_PROJECT` to its latest `.clay` stage and `CLAY_EXPORT_REVIEW` to a temporary output directory when running `texture_blender_export_manual_review`; it produces `recovered.blend` without modifying the source.

Blender renderers and lighting are not identical to Clay Form's viewport approximation. Extremely complex painted materials requiring more than twelve packed shader attributes are rejected explicitly rather than producing an unusable preview. The native `.clay` project remains the full editable source.

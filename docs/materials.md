# Material channel contract

Issues [#9](https://github.com/dustyghost/clay-form/issues/9) and [#61](https://github.com/dustyghost/clay-form/issues/61) provide vertex painting and shared layered surfaces for later texture painting and Blender exchange.

## Storage and shared definitions

`Mesh.materials` is empty for implicit default clay, or contains one evaluated `Material` per vertex. A material contains a base surface and three ordered additional surfaces. Each surface has linear RGB base colour, roughness, metallic, IOR, specular level, linear RGB emission and emission strength, plus subsurface weight, RGB radius and scale. Additional layers have weight and an Add amount: 0 is Mix, 1 is Add. Intermediate values preserve continuous painting and topology interpolation.

Colour, roughness, metallic, specular, emission colour and layer controls range from 0 to 1. IOR ranges from 1 to 4; emission strength from 0 to 100; subsurface radius and scale from 0 to 10 in model units. All values must be finite. Defaults preserve older projects: IOR 1.5, specular 0.5, no emission, scattering or additional layers. New projects use format version 2, so older application versions reject them rather than discard unfamiliar material data. Version 1 projects remain readable. Sparse default fields are omitted in JSON. The existing 128 MB project-file limit still applies; failed saves retain the previous file.

`SceneSettings.materials` holds up to 256 named definitions with stable IDs. Vertices retain sparse influence weights independently for base colour, roughness, metallic and the remaining surface/layer controls. Editing a named definition applies its parameter delta in proportion to those weights across every submodel and stored subdivision level. Undo restores the definition and affected values together. Copies have new identities. Legacy painted values without identity links remain independent until repainted; loading an older file does not guess shared assignments. Personal swatch templates and palette colours also persist in preferences; the open project's definition takes precedence over a personal template with the same identity.

A brush stroke interpolates the evaluated parameters and influences, not discrete face assignments. Weight is strength × pressure × falloff/alpha × stencil × (1 − mask); overlapping symmetry copies use the strongest influence. Colour-only painting changes only base colour and its binding, retaining layer and other channel bindings. Erasing fades towards default clay. The sampler copies raw base colour into a detached brush material without editing a shared definition or the model. Existing vertex groups, masks and visibility stay independent.

## Layers and rendering

A layer combines linear-light results as:

`result = previous × (1 − weight × (1 − add)) + layer × weight`

Mix at weight 1 replaces the preceding surface. Add accumulates the weighted surface and can exceed unit energy. This is an opaque surface stack, not a complete shader graph: transmission, anisotropy, texture maps and displacement are not authored here. Layer parameters interpolate between painted materials in their fixed stack positions.

The CPU and GPU use the same isotropic GGX approximation, adjustable dielectric IOR/specular and authored emission. Viewport lighting is a separate preference: up to eight independently adjustable directional or point lights, colour, intensity, camera-relative/world orientation and one neutral fill. Ambient and emission are accumulated once per surface, with direct contributions summed in linear light. The material swatches use fixed neutral studio lighting so they remain comparable. Both swatches and the viewport update when the shared definition changes. Subsurface preview uses a radius-dependent wrapped diffuse approximation to keep interaction fast; it does not compute actual mesh thickness or multiple scattering. Blender export connects weight/radius/scale to Principled subsurface scattering. Display conversion to sRGB happens once after layer composition. Masks and group colours are display overlays.

## Blender mapping and exports

The implementation uses ordinary [Principled BSDF inputs](https://docs.blender.org/manual/en/4.5/render/shader_nodes/shader/principled.html), Math, Mix Shader and Add Shader nodes. The **Export** menu offers OBJ, PLY and Blender. These exports capture the active model at its current subdivision level, including faces currently hidden for sculpting. Save `.clay` to retain the complete scene, brushes, masks, groups, history-independent editing state and subdivision stack.

| Data | Blender `.blend` | PLY | OBJ |
| --- | --- | --- | --- |
| Current geometry | Yes; Y-up converted to Z-up | Yes; source Y-up coordinates | Yes |
| Split surface normals | Yes | Recomputed by importer | Existing OBJ behaviour |
| Base vertex colour | Linear FLOAT POINT channels → Base Color | 8-bit sRGB RGB vertex properties | Not included |
| Roughness / metallic | Scalar attributes → matching Principled inputs | Not included | Not included |
| IOR / specular | Principled IOR / Specular IOR Level | Not included | Not included |
| Emission | Linear colour / strength → matching Principled inputs | Not included | Not included |
| Subsurface scattering | Principled Subsurface Weight / Radius / Scale; full Blender scattering | Not included | Not included |
| Three weighted layers | Standard shader nodes implementing Mix/Add | Not included | Not included |
| Shared definitions and paint blending | Named node groups, vertex weights and residual parameter attributes | Not included | Not included |

Blender export requires Blender 4.0 or newer on `PATH`. Conversion runs in a cancellable worker using a private snapshot and temporary files. The destination is replaced atomically only after successful conversion; the chooser confirms overwrites. The exported model has a fitted camera and neutral light for inspection by default. **Include viewport lights** snapshots the current rig instead, replacing the neutral preview lamp and converting all vectors from Y-up to Z-up. Camera-relative lights are resolved into world space before the worker starts. Directional lights map to Blender Sun energy equal to viewport intensity. Point power is `intensity * 16 * 4π`, preserving the viewport’s inverse-square coefficient. Ambient fill maps to a neutral World Background strength of `fill * 0.1`; it is an approximation of the viewport’s material-dependent fill. Removing all viewport lights exports an ambient-only setup when the option is enabled. The option is unavailable for OBJ/PLY.

In Blender, open the material node tree and enter a group named after the Clay Form material. Its labelled Value nodes are the shared definition. The generated evaluation tree adds weighted definitions to residual POINT attributes, retaining mixed and partially painted strokes. The residuals are data, not colour images. Parameter values are clamped to the same ranges as Clay Form. The exchange version and source coordinate convention are stored as metadata. Importing edits back into Clay Form remains future exchange work; `.blend` is not a substitute for the native project.

Full render identity is not promised: Blender's multiscattering, lighting, shadows, reflections and display transforms differ from the viewport approximation. External arbitrary shader graphs require an explicit future conversion or baking policy; they must not silently flatten into this channel contract.

## Topology and verification

Exact remappings copy material values and influences. Subdivision, adaptive split/collapse, modeled/sliced vertices and voxel remeshing interpolate the same source weights as existing material fields. Remeshing retains the bounded surface search and cancellation checks. Thickness and mirroring copy values. Detail projection retains target painting. History accounting includes material storage and conservatively estimates influence allocations.

Focused tests cover legacy defaults, channel-only painting, layer Mix/Add arithmetic, shared edits through subdivision and undo, native round trips, PLY bytes and Blender residual reconstruction. GPU checks compare layered pixels to the CPU reference and exercise small storage-buffer batches. UI checks click swatches, resize the crowded panel, rename, paint, sample and fill. Blender conversion and render review are manual interoperability checks, excluded from routine CI.

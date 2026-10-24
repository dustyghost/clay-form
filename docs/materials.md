# Material channel contract

Issue [#9](https://github.com/dustyghost/clay-form/issues/9) establishes the shared foundation for texture painting (#11) and Blender exchange (#21). This version stores independent continuous material values per mesh vertex; it does not assign material IDs or use vertex-group display colours.

## Storage and blending

`Mesh.materials` is either empty (implicit default clay) or has one `Material` per vertex. Each value contains `base_color: [f32; 3]`, `roughness: f32`, and `metallic: f32`. Channels must be finite and between 0 and 1. Empty arrays are omitted from project JSON, and missing fields in older projects load with the default. The first actual paint change allocates the vertex array. History memory accounting includes its payload.

Base colour uses scene-linear RGB with sRGB/Rec.709 primaries. The colour picker presents display colour and converts at its boundary; scalar channels are linear data, with no colour conversion. Topology interpolation and brush blending occur in linear channel space. A dab blends each enabled channel toward its target by strength × pressure response × falloff/alpha × stencil × (1 − mask). Overlapping symmetry copies use the strongest influence. Material mode blends parameter values, not two complete BSDF lobes or a discrete preset identity.

The example materials are value presets. Their values travel with the brush and project; changing a preset does not retroactively alter earlier strokes. Saved brush presets provide reusable user-defined materials without a second registry or hidden shared assignments.

## Blender compatibility matrix

The channel definitions follow [Blender's Principled BSDF inputs](https://docs.blender.org/manual/en/4.5/render/shader_nodes/shader/principled.html). The validation fixture uses Blender 5.2.2 LTS; it is not a production exchange implementation.

| Channel / structure | Current representation | Blender mapping / future exchange policy |
| --- | --- | --- |
| Base colour | Vertex RGB, linear, 0–1 | FLOAT_COLOR POINT attribute `clay_base_color` → Principled Base Color. Preserve linear values; use the sRGB transfer function only for colour images/UI. |
| Roughness | Vertex scalar, 0–1 | FLOAT POINT attribute `clay_roughness` → Roughness. GGX perceptual roughness; the viewport clamps shading roughness to 0.045 to bound highlights without changing stored values. |
| Metallic | Vertex scalar, 0–1 | FLOAT POINT attribute `clay_metallic` → Metallic. Intermediate values interpolate the metal/dielectric model. |
| Opacity | Not stored; opaque | Principled Alpha = 1. Future alpha is linear 0–1 coverage, independent of transmission. Reject/report unsupported transparency rather than dropping it. |
| Emission | Not stored; zero authored emission | Future linear RGB plus nonnegative strength → Emission Color/Strength. Studio fill is viewport lighting, not authored emission. The validation fixture uses emission only to reproduce that fixed fill. |
| Normal | Geometric/split surface normals | No painted normal channel. Future texture normals use tangent-space +Y, linear/non-colour data, with explicit tangent basis and UV seam handling; other conventions require conversion. |
| Height/displacement | Not stored | Future signed scalar in model-space length units; zero is no displacement, positive follows outward normal. Export must declare scene unit scale and whether displacement is baked geometry or a shader input. |
| Material slots / face assignments | No independent slot registry | A continuous vertex parameter field maps to one shared Principled material reading attributes. Existing polygroups remain face groups, not material slots. Blender face slots must be preserved separately or explicitly converted in #21. |
| Material blends | Continuous parameter interpolation | Export the evaluated fields as attributes; optional UV baking belongs to the texture/baking work. This is not arbitrary Mix Shader equivalence. |
| Textures, layered materials, node graphs, procedurals | Unsupported in this version | #21 must preserve the external source, offer an explicit bake/conversion, or report omission. Do not silently flatten them into these five floats. |

The vertex foundation imposes no UV requirement. Texture painting can use the same channel definitions with an image source and explicit colour/data interpretation. Data from future material slots or images must not overwrite vertex masks/groups.

## Transfer and rendering

Exact index mappings preserve material data through extraction, deletion and visibility splitting. New modeled/sliced vertices interpolate the same source weights as mask/group data. Thickness and mirroring copy the source values. Subdivision stores the same vertex-prefix contract as geometry, preserving high-level residual detail when switching levels. Adaptive split/collapse interpolate endpoints. Voxel remesh uses the existing bounded surface BVH, nearest-triangle barycentric sampling, cancellation and a search budget; failure rejects the worker result. Detail projection retains target material values.

Both GPU and CPU viewport paths use an isotropic GGX metallic/roughness approximation, dielectric IOR 1.5, one camera-relative directional light and a constant neutral fill. Shading runs in linear light, then converts once to display sRGB for the existing unorm compositor. Mask dimming and polygroup/guide overlays are display overlays. Default clay now uses this same shading, so the first painted stroke does not switch the rest of the model to a different lighting model.

The GPU regression checks rendered channels against a numeric reference, interpolation, mask dimming and small storage-buffer batches. Manual example captures and a controlled Blender fixture are documented in [validation notes](validation/material-paint-2026-10-24/README.md). Full Blender render equivalence is not promised: multiscattering, lighting, shadows, normal interpolation and display transforms can differ.

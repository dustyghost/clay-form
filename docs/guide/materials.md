# Paint colours and materials

[Documentation home](../../README.md#documentation) · [Brush controls](sculpting.md)

Choose **Paint** in the brush library, or **Add brush → Paint** in the brush library when opening an older project that has no Paint preset. Convert a parametric primitive to a sculpt mesh first.

Choose **Focus Paint workspace** to keep one painting sidebar and give the model more screen space. **Restore workspace** in the top toolbar returns your previous layout, including after restarting the app. The model selector switches the active model directly from this workspace.

![Focused Paint workspace with image target and inspection controls](../images/paint-focused-workspace.png)

In **Brush settings**, the Paint target shows the active model and whether strokes update **Vertices** or an image. Image targets show their dimensions and precision, including **32-bit float**, independently of export format. **Set up texture painting…** creates an image workflow; **UV / image view** opens the linked UV editor. Switching to a model without the selected image returns the target to Vertices.

Use **Brush colour**, the palette and **Sample colour** for future strokes. These controls do not recolour existing paint or edit a shared material. Image painting currently exposes base colour only; roughness and metallic remain available as vertex material properties. With Vertices selected, **Paint channels** offers **Whole material**, **Colour only**, **Roughness only** or **Metallic only**.

Open **Material presets and shared editor** to choose a rendered material swatch. Drag **Drag to resize materials** to change the swatch area; larger libraries scroll within it. Choosing a material affects future strokes. Open **Edit shared project material** deliberately when you want to update existing painted uses; it shows how many models and vertices share that definition.

| Example | Appearance | Roughness | Metallic |
| --- | --- | --- | --- |
| Matte clay | Warm, broad shading | 0.85 | 0 |
| Glazed ceramic | Teal with a sharp highlight | 0.18 | 0 |
| Rubber | Dark, matte surface | 0.92 | 0 |
| Satin aluminium | Pale metal with a broad highlight | 0.38 | 1 |
| Copper | Warm metal | 0.24 | 1 |
| Red plastic | Red with a glossy highlight | 0.30 | 0 |
| Cool clay | Cool grey, broad matte shading | 0.90 | 0 |
| Beeswax | Warm amber wax with soft scattering | 0.38 | 0 |
| Ivory wax | Pale wax with stronger scattering | 0.32 | 0 |
| Warm skin | Warm skin with a subtle surface sheen | 0.62 | 0 |
| Deep skin | Deep brown skin with a subtle surface sheen | 0.58 | 0 |

These are editable starting values. Ceramic uses a simple glossy dielectric; satin aluminium uses isotropic roughness, without directional brush marks or a separate coating layer. Inside **Edit shared project material**, edit **Shared colour**, **Roughness**, **Metallic** or **Material name** to update the named project material and its painted uses. Shared slider changes apply when you release the pointer; Escape cancels a drag. **Copy material** creates an independent project definition for future strokes, leaving earlier paint linked to its original material.

Project material edits, copies and resets stay in the `.clay` project and support Undo. **Save to personal library** explicitly publishes the current settings for future projects. Personal library saves and palette changes persist in preferences independently of project Undo.

**Reset material** restores a built-in material's original settings, or a custom material's saved starting settings. It keeps the name and updates painted uses. **Remove from project** removes the definition and detaches its painted uses while preserving their appearance; Undo restores it. Personal presets remain available. **Hide personal library preset** hides/removes the preset for future projects without changing current project paint. **Restore built-in materials** makes hidden built-ins available again.

Click a palette swatch to select a brush colour. **+ Colour** saves it to your personal palette; right-click a swatch to remove it. **Sample colour** picks the visible surface's colour, including image paint, without lighting, mask shading or vertex-group tint. It samples once; **Escape** cancels.

**Application** chooses Regular stroke, Airbrush or Spray dots, while **Stroke path** controls Freehand, Line or stamps. Continuous **Flow / second** is available for Freehand; hold the pen still to build up paint. Spray offers density and dot size as a percentage of brush radius. Use **Inspect** to switch between Lit material, Unlit colour, Roughness, Metallic and UV checker without editing paint.

![Paint brush settings and copper example](../images/material-copper.png)

**Strength**, pressure, radius, falloff, alpha, stencil and symmetry work with painting. Repeated strokes approach the chosen values. Mirrored copies do not double the strength where their footprints overlap. **Subtract**, or the configured reverse modifier, blends the selected channels toward default clay. **Escape** cancels an active stroke; release records one Undo step.

**Fill all vertices** applies the selected channels at full strength to every vertex of the active model, including masked and hidden vertices. It uses the chosen material even when Subtract is selected, updates every stored subdivision level, and creates one Undo step. Other submodels are unchanged.

**Fill unmasked vertices** applies the selected channels to the active model at the current Strength. Set Strength to 1 for the exact material values. The fill respects partial masks and hidden faces and updates stored subdivision levels in one undoable operation. The fill action can also be added to a custom menu or assigned a shortcut.

Painting never moves vertices or changes topology, including when Adaptive sculpt or automatic voxel remeshing is enabled. Masks protect paint independently of vertex groups. Selecting Paint turns off the vertex-group colour overlay so strokes are visible. **P** toggles vertex-group colours for inspection; it never changes the paint. Existing custom P bindings are preserved, and the toggle can be reassigned in keyboard settings.

![A local teal stroke over red plastic](../images/material-painted-stroke.png)

## Detail, topology and saving

Paint is stored on vertices, so the mesh resolution determines the finest colour boundary you can draw. Subdivide before painting fine details. For image detail independent of mesh density, use [UVs and texture painting](textures.md). Vertex painting and texture images are separate targets.

Subdivision and newly created modeling vertices interpolate the painted channels. Slicing, extraction, mirroring, thickness and merging carry the values to their corresponding vertices. Adaptive edge splits interpolate endpoints; edge collapse averages them. Coarse topology can soften small painted features. Voxel remeshing samples the nearest original triangle and interpolates its values; close or overlapping sheets can sample the other surface, so inspect the result and Undo if needed. Detail projection keeps the target's paint while changing its shape.

Changing subdivision levels retains higher-level paint detail and propagates edits to the lower-level samples. A coarse-level edit can change the interpolation underneath finer detail; channel values remain within their supported ranges.

Save as **`.clay`** to preserve paint, brush settings, masks, groups and subdivision levels. Undo/Redo includes material edits. OBJ import recognizes conventional `v x y z r g b [a]` vertex colours as sRGB, with opaque alpha only. OBJ material libraries and shader graphs are not imported. OBJ export reports a limitation for painted meshes instead of silently losing roughness and metallic values; keep the `.clay` project. Blender import and round-trip editing remain separate work.

## Shading

The viewport light widget starts hidden each time you open the app. Turn on **View → Lighting → Show light controls** when you want it, and use **Close light controls** to hide it completely. The **Light** widget beneath the viewport axes lets you drag the light around the model directly in view. Its gold marker shows the light direction; a hollow marker is behind the model. **Front / back** flips sides, **Directional** and **Point** change the light type, and point lights have a **Distance** slider. **Follow camera** controls whether the light moves with the view. Escape cancels a light drag. Choose which light to move from its light selector. You can also collapse the **Light** header or turn off **Show light controls** in the Lighting panel. These controls change illumination without moving the camera or painting the model.

![Viewport light controls and Lighting panel](../images/lighting-controls.png)

Open **View → Lighting** and use **Add light** or **Remove light** to manage up to eight lights. Select a light from the list to edit it independently as a **Directional light** or **Point light**. Removing the last light leaves ambient fill and material emission. Use **Drag to move light**, or the azimuth and elevation sliders, to reposition it. Point lights also have a distance control and distance falloff. Change each light’s colour and intensity; **Ambient fill** applies to the whole scene. Turn off **Light follows camera** to keep a light fixed in world space. **Reset studio lighting** restores one studio light and neutral fill. Your lights are saved in preferences.

Lighting controls do not edit paint or geometry.

The default viewport lighting uses a studio light and neutral fill. Roughness changes highlight width; metallic changes the diffuse/specular balance and tints metal reflections. The same material appearance is used for the active and inactive painted models. Unpainted inactive models retain their blue-grey selection tint.

The initial surface is opaque. UV image painting currently exposes base colour only. Existing roughness and metallic images remain preserved and rendered; their authoring controls are deferred in [#70](https://github.com/dustyghost/clay-form/issues/70). Normal maps, height/displacement maps, anisotropy and arbitrary shader graphs are not editable material channels yet. The viewport does not simulate scene reflections, cast shadows or indirect lighting. See the [material channel contract](../materials.md) for Blender mapping and validation.

## Shared surfaces and layers

Editing a named material now updates surfaces painted with it, in proportion to each stroke's contribution. **Copy material** creates an independent version before you experiment. Rename it with **Material name**. These definitions travel with the project, and Undo restores changes to both the definition and painted surfaces.

Open **Surface options** for IOR, specular level, emission colour and emission strength. Open **Material layers** to combine up to three additional surfaces over the base. Each layer has the same surface controls, a weight and **Mix** or **Add** blending. **Start from** supplies a surface preset; **Move up** changes the order and **Remove layer**, at the top of an expanded layer, deletes it and closes the gap in the stack. Empty slots are hidden, and **Add layer** creates a new layer. Undo restores a removed layer and its effect on painted surfaces. A layer with weight zero remains available for editing. Mix at full weight replaces the earlier surface; Add accumulates another contribution. Transmission is not available; image paint is managed through the Paint target and UV / image view.

Open **Subsurface scattering** for weight, RGB scattering radius and scale. Beeswax, Ivory wax, Warm skin and Deep skin include starting settings and a subtle specular layer; all colours and controls remain editable. The viewport uses a fast soft-diffuse approximation, while Blender export uses full Principled scattering. The approximation does not account for actual model thickness, so check final translucency in Blender.

![Ivory wax and the expanded material library](../images/material-ivory-wax.png)

![Warm skin preset](../images/material-warm-skin.png)

## Exporting paint

Choose **Export → PLY…** for geometry and base vertex colours, or **Export → Blender…** for the full painted material stack and editable named definitions. Blender export needs Blender 4.0 or newer installed on `PATH`. Export runs in the background and can be cancelled. Choose a destination in the file dialog; replacing a file requires confirmation.

**Export → Blender… → Include viewport lights** optionally exports your current lights and ambient fill. It starts unchecked; leaving it off keeps the neutral preview light used by Blender export. Directional lights become Sun lights and point lights retain their positions, colours and distance falloff. Camera-relative lights are frozen at their current world positions. Blender’s shadows, reflections and material shading can make the final appearance differ. OBJ and PLY do not export this light setup.

Both formats export the active model at its current mesh level, including hidden faces. PLY does not contain roughness, metallic, emission or layers. Keep a `.clay` project for the full scene and subdivision levels. In Blender, edit the labelled values inside the node group named after your material. Changes coming back from Blender are part of the later exchange flow.

# Navigation and tablet input

[Documentation home](../../README.md#documentation)

- [Orbit and pan](#orbit-and-pan)
- [Zoom](#zoom)
- [Perspective view](#perspective-view)
- [Surface display](#surface-display)
- [Huion tablets](#huion-tablets)

## Orbit and pan

- Left drag or pen tip on the clay: sculpt. Start a left/pen drag in empty space to orbit; it stays an orbit gesture even if you cross the clay. Right drag: orbit. Middle drag: pan. Wheel: zoom.
- The thin frame inside the viewport reserves a 20-pixel navigation strip, even when the model fills the screen. Start a left/pen drag there to orbit, hold **Ctrl** and drag down to zoom out, or hold **Alt** to pan. The gesture stays navigation when you cross back over clay; border taps do not sculpt, select, or cut.
- The top-right orientation gizmo shows positive and negative X/Y/Z axes. Drag it to orbit, or tap an axis to snap to that view. Taps tolerate up to 6 screen pixels of mouse/pen movement and keep the axis you first pressed; larger drags orbit. Dimmed axes point away from you.
- **Alt + left/pen drag pans the view when starting off the model.** Starting on the model sculpts, with Alt reversing the brush as usual. Alt + right drag also pans; releasing Alt during a navigation drag returns to orbiting. Shift temporarily uses Smooth.

Vertical orbit continues over the poles and through full turns; there is no top/bottom angle stop.

## Zoom

Use the toolbar **− / +** buttons, **View → Zoom in / Zoom out**, or the mouse wheel over the viewport. Default keys are **=** to zoom in and **−** to zoom out, configurable in Key settings. The View menu also has a zoom slider (20–800%). Click the toolbar percentage or View → Reset zoom to return to 100% while keeping rotation and pan; Reset view resets all camera navigation. Zoom is session-only and does not affect geometry or undo history.

Hold **Ctrl** during an orbit drag to zoom instead of rotating: drag up to zoom in, down to zoom out. This works with right drag, empty-space left/pen drag, and gizmo dragging, including switching the modifier mid-drag. Starting a left/pen drag with Ctrl held also zooms over the clay without sculpting. Change the modifier (or disable it) under **Settings → Key settings → Hold to zoom while dragging**. Existing settings gain Ctrl as the default. **Alt-drag starting off the model pans** horizontally and vertically without changing rotation, zoom, or geometry; starting on the model sculpts with the reverse modifier. The gesture stays sculpting or navigation until release, even when crossing the model edge. Set or disable it under **Hold to pan off the model**; existing preferences gain Alt as the default. Zoom takes priority if pan and zoom share a modifier. Inside the navigation frame, Ctrl+Shift visibility selection retains priority, so Alt can still switch that gesture to Hide. The border strip always remains available for navigation.

## Perspective view

Enable **View → Perspective view** to judge depth and foreshortening. Turn it off for orthographic comparisons against drawings. **Field of view** ranges from 20° to 75° (45° initially): wider angles emphasize depth, while narrower angles flatten proportions. Switching projection or changing the angle adjusts zoom to keep the visible models at approximately the same apparent size, within the normal zoom limits. Orbit and pan stay in place. The eye distance is fitted to the visible scene when changing projection, including larger imported models.

Wheel, toolbar and drag zoom enlarge the image without moving the eye through the model, so close sculpting views remain usable. Brush size follows the depth of the surface under the cursor. Picking, symmetry guides, selections, transform controls and cuts use the displayed projection. Geometry behind the camera is clipped; a path cut through geometry crossing the eye reports an error instead of applying an ambiguous cut.

The perspective toggle supports custom menus and shortcuts as **Toggle perspective view**; it has no default key binding. Assign one in **Settings → Key settings**, or right-click the checkbox to add it to a custom menu. Projection and field of view are session-only: they do not modify the project or undo history. Changing them ends the current drag and discards unapplied stamp, selection and cut previews. **Reset view** restores orthographic viewing and the default angle along with rotation, pan and zoom.

### Example: compare orthographic and perspective views

1. In a practice project, use **Primitive → Create primitive…** to replace the starting sphere with a **Cube**.
2. Enable **Wireframe** and right-drag until three faces are visible. Open **View** and use its left-sidebar icon to keep the controls visible.
3. Leave **Perspective view** off and inspect the parallel edges.
4. Enable it and set **Field of view** to **65°**. The nearer face appears larger and parallel edges recede toward one another. Toggle the checkbox to compare without changing the cube.

![Orthographic cube with parallel edges and Perspective view unchecked in the View sidebar](../images/view-orthographic.png)

*Orthographic: parallel edges remain parallel. The cube's orientation matches the next image.*

![The same cube in perspective with the View sidebar showing a 65 degree field of view](../images/view-perspective.png)

*Perspective at 65°: depth changes the apparent size of faces. The zoom percentage adjusts to preserve approximate framing; the geometry stays unchanged.*

## Surface display

In **View**, enable **Faint back-facing surfaces** to see the reverse sides of open surfaces translucently. **Back-face opacity** adjusts their visibility (15% by default). Front-facing surfaces remain opaque; backs behind solid foreground surfaces stay hidden. This is a session-only viewing option and does not change normals or sculpting/picking.

Wireframe options beside the Wireframe button select visible surfaces (edges hidden by other surfaces), front-facing edges, or transparent viewing of the whole frame. Triangle diagonals can be shown separately. Primitives use quad faces where possible; enable Triangle topology in the primitive editor to use triangles throughout. Quad faces survive subdivision, project saving, and OBJ export. Poles, caps, icospheres, and adaptive or voxel remeshing still use triangles.

## Huion tablets

The tablet's normal desktop driver provides pen cursor movement and tip/button input. On Linux, Clay Form additionally discovers devices named Huion under `/sys/class/input` and reads normalized `ABS_PRESSURE` from their `/dev/input/event*` device without taking exclusive control. The sidebar shows connection errors and live pressure. Native touch/pen pressure events are also accepted where eframe provides them. Otherwise strokes use full pressure.

For the connected GS1331 (`256c:006d`), install the included device-specific rule from a normal terminal at the repository or extracted release root:

```sh
sudo install -m 0644 packaging/linux/72-clay-form-huion.rules /etc/udev/rules.d/72-clay-form-huion.rules
sudo udevadm control --reload-rules
```

Unplug and reconnect the tablet, then restart Clay Form. The rule grants access to the active local desktop user for this model's tablet event devices using [systemd's uaccess mechanism](https://cgit.freedesktop.org/systemd/systemd/tree/src/login/70-uaccess.rules). It does not grant access to all input devices. Other models need their vendor/model IDs in a separate rule.

Your normal user needs read access to the tablet event device. If the sidebar reports permission denied, configure device-specific access through your distribution's input/udev facilities. Do not run the application as root. Device access and pressure have not yet been verified with a physical Huion tablet; model names/drivers that do not contain “Huion” need a discovery extension. Tilt, eraser detection, and tablet express-key configuration are not implemented.

## Related guides

[Key settings and workspace](workspace.md) · [Sculpting and brushes](sculpting.md) · [GPU rendering](../gpu-rendering.md)

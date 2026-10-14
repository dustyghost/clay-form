# Navigation and tablet input

[Documentation home](../../README.md#documentation)

- [Orbit and pan](#orbit-and-pan)
- [Zoom](#zoom)
- [Surface display](#surface-display)
- [Huion tablets](#huion-tablets)

## Orbit and pan

- Left drag or pen tip on the clay: sculpt. Start a left/pen drag in empty space to orbit; it stays an orbit gesture even if you cross the clay. Right drag: orbit. Middle drag: pan. Wheel: zoom.
- The top-right orientation gizmo shows positive and negative X/Y/Z axes. Drag it to orbit, or tap an axis to snap to that view. Taps tolerate up to 6 screen pixels of mouse/pen movement and keep the axis you first pressed; larger drags orbit. Dimmed axes point away from you.
- **Alt + left/pen drag pans the view when starting off the model.** Starting on the model sculpts, with Alt reversing the brush as usual. Alt + right drag also pans; releasing Alt during a navigation drag returns to orbiting. Shift temporarily uses Smooth.

Vertical orbit continues over the poles and through full turns; there is no top/bottom angle stop.

## Zoom

Use the toolbar **− / +** buttons, **View → Zoom in / Zoom out**, or the mouse wheel over the viewport. Default keys are **=** to zoom in and **−** to zoom out, configurable in Key settings. The View menu also has a zoom slider (20–800%). Click the toolbar percentage or View → Reset zoom to return to 100% while keeping rotation and pan; Reset view resets all camera navigation. Zoom is session-only and does not affect geometry or undo history.

Hold **Ctrl** during an orbit drag to zoom instead of rotating: drag up to zoom in, down to zoom out. This works with right drag, empty-space left/pen drag, and gizmo dragging, including switching the modifier mid-drag. Starting a left/pen drag with Ctrl held also zooms over the clay without sculpting. Change the modifier (or disable it) under **Settings → Key settings → Hold to zoom while dragging**. Existing settings gain Ctrl as the default. **Alt-drag starting off the model pans** horizontally and vertically without changing rotation, zoom, or geometry; starting on the model sculpts with the reverse modifier. The gesture stays sculpting or navigation until release, even when crossing the model edge. Set or disable it under **Hold to pan off the model**; existing preferences gain Alt as the default. Zoom takes priority if pan and zoom share a modifier. Ctrl+Shift visibility selection retains priority, so Alt can still switch that gesture to Hide.

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

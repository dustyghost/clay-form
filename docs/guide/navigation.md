# Navigation and tablet input

[Documentation home](../../README.md#documentation)

Move the view without changing the sculpt.

- [Orbit and pan](#orbit-and-pan)
- [Zoom](#zoom)
- [Perspective view](#perspective-view)
- [Surface display](#surface-display)
- [Huion tablets](#huion-tablets)

## Orbit and pan

### Mouse and pen gestures

| Action | Gesture |
| --- | --- |
| Sculpt | Left/pen drag starting on clay |
| Orbit | Right drag, or left/pen drag starting in empty space |
| Pan | Middle drag, Alt + right drag, or Alt + left/pen drag starting off the model |
| Zoom | Wheel, or Ctrl + drag |
| Smooth temporarily | Hold Shift while sculpting |
| Reverse the brush | Hold Alt while sculpting |

A drag keeps the role it started with. An orbit drag stays navigation when it crosses clay. Alt-drag starting **on** clay reverses the brush instead of panning.

### Navigate when the model fills the view

The thin inset frame reserves a **20-pixel navigation strip**. Start a pen/left drag there to orbit, add **Ctrl** to zoom, or **Alt** to pan. Border taps do not sculpt, select, or cut.

### Snap to an axis

Tap X, Y, or Z on the top-right orientation gizmo. Dimmed axes face away from you. Drag the gizmo to orbit; vertical orbit can continue over the poles.

## Zoom

Use the wheel, toolbar **− / +**, or **View → Zoom in / Zoom out**. Default keys are **− / =**. Zoom ranges from **20–800%**.

| Reset | What changes |
| --- | --- |
| Click the toolbar percentage, or **Reset zoom** | Zoom returns to 100%; rotation and pan stay |
| **Reset view** or **F** | Rotation, pan, zoom, and projection return to defaults |

For pen zoom, hold **Ctrl** and drag up to enlarge or down to shrink. This also works during an orbit or gizmo drag.

Change the zoom/pan modifiers in **Settings → Key settings**. Zoom takes priority if both use the same modifier; Ctrl+Shift selection retains priority inside the navigation frame.

## Perspective view

Enable **View → Perspective view** for depth and foreshortening. Leave it off for parallel projection when comparing against drawings.

**Field of view** ranges from **20–75°**, initially **45°**. Wider angles emphasize depth; narrower angles flatten proportions. Switching modes or angles adjusts framing approximately, within the zoom limits.

### Example: compare orthographic and perspective views

1. In a practice project, create a **Cube** primitive.
2. Enable **Wireframe** and orbit until three faces are visible.
3. Dock **View** on the left using its sidebar icon.
4. Toggle **Perspective view**, then set **Field of view** to **65°**.

![Orthographic cube with parallel edges and the Perspective view checkbox off](../images/view-orthographic.png)

*Orthographic: parallel edges stay parallel.*

![The same cube with Perspective view enabled and a 65 degree field of view](../images/view-perspective.png)

*Perspective: nearer faces look larger. Geometry and orientation are unchanged.*

### What the view settings affect

Picking, brush size, selections, transforms, guides, and cuts follow the displayed projection. Zoom enlarges the image without moving the camera eye through the model.

View settings are session-only and do not add undo steps. Changing projection or field of view ends the current drag and cancels unapplied selections, stamps, and cuts. **Reset view** returns to orthographic.

Assign **Toggle perspective view** in Key settings or add it to a custom menu; it has no default shortcut. Geometry behind the camera is clipped. A path cut through geometry crossing the eye reports an error; switch to orthographic and redraw it.

## Surface display

### Choose a wireframe mode

Open **Wireframe options** beside the Wireframe button.

| Mode | Shows |
| --- | --- |
| Visible surfaces | Edges not hidden by another surface |
| Front-facing edges | Front-facing topology |
| Transparent | The whole frame, including the back |

Enable triangle diagonals separately. Primitives use quads where possible; adaptive and voxel remeshing produce triangles.

### Inspect open surfaces

Enable **View → Faint back-facing surfaces** to see the back of an open surface. Set **Back-face opacity**, initially **15%**, to control translucency. Backs behind opaque foreground clay stay hidden. This changes display only, not normals or picking.

## Huion tablets

### Check input

Your desktop tablet driver supplies cursor and tip/button input. On Linux, Clay Form also looks for Huion devices under `/sys/class/input` and reads pressure from their `/dev/input/event*` devices. Native pen/touch pressure is accepted where the window system provides it; otherwise strokes use full pressure.

The **Tablet & navigation** panel shows live pressure and connection errors. Tilt, eraser detection, and express-key configuration are not implemented. Physical Huion pressure has not yet been verified.

### Allow pressure input on a GS1331

For the `256c:006d` device, run from the repository or extracted release folder:

```sh
sudo install -m 0644 packaging/linux/72-clay-form-huion.rules /etc/udev/rules.d/72-clay-form-huion.rules
sudo udevadm control --reload-rules
```

Reconnect the tablet and restart Clay Form. The rule grants the active desktop user access to this device through [systemd uaccess](https://cgit.freedesktop.org/systemd/systemd/tree/src/login/70-uaccess.rules).

Other models need their own vendor/model rule. If pressure reports permission denied, grant your normal user device-specific access; do not run the app as root. Devices whose names do not contain “Huion” need a discovery extension.

## Related guides

[Brushes](sculpting.md) · [Shortcuts](workspace.md#shortcuts) · [GPU rendering](../gpu-rendering.md)

# Reference images

[Documentation home](../../README.md#documentation)

## Add a reference

Drop a PNG or JPEG file onto the model view, or open **References**, enter its path, and choose **Add image**. Each image is embedded in the project; moving or deleting the source file afterward does not break it. JPEG orientation metadata is applied when loading photographs. PNG transparency is preserved.

Adding an image enters **Arrange images**. Drag an unlocked image to move it; drag its lower-right corner to resize it while retaining its aspect ratio. Choose **Done** in the strip above the viewport to return to sculpting. **Escape** discards an unfinished drag and finishes arranging. Losing focus, switching tools, or resizing the viewport cancels an unfinished drag. Completed placements remain saved in the document.

## Sculpt with references

Images remain fixed to the screen as the camera orbits, pans, and zooms. Outside Arrange mode, all images are passive: the pen and mouse sculpt or navigate through them normally. Right-drag orbiting, middle-drag panning, and wheel zoom also work while arranging. Primary drags in Arrange mode only place images; they never sculpt.

In **References**, select an image from the list to adjust its **Opacity** and **Size**, **Center image**, or **Remove image**. **Lock position and size** protects its placement while arranging. Each row has a visibility checkbox. **Show references** temporarily hides or shows the whole collection without changing the saved individual visibility settings.

Images stay within the model view and fit smaller viewports. Their stored positions and widths are proportions of the viewport; their aspect ratios stay intact. Later images appear over earlier ones. Use the list to hide overlapping images or select their controls.

## Example: keep two studies beside the sculpt

1. Drop two PNG or JPEG study images onto the viewport, one at a time.
2. In **Arrange images**, move one toward the upper left and the other toward the lower right. Drag each lower-right corner to size it without covering the working area.
3. Select the second image in **References**, reduce **Opacity** to **70%**, and enable **Lock position and size**.
4. Choose **Done**, then orbit the sphere. The studies stay fixed on screen while the model turns. Save a `.clay` project to keep the embedded images and their placements.

![Two sample reference images beside a sphere with the References panel open on the right](../images/reference-images.png)

*Two synthetic sample images demonstrate placement. Front study is selected; Side study at the lower right is locked and uses 70% opacity. Use your own drawings or photographs for sculpting.*

## Save and customize

Image data, placement, opacity, individual visibility, and locks save with `.clay` projects and recovery copies. Reference edits mark the project modified but leave sculpt geometry, mesh Undo/Redo, and the camera unchanged. Reference edits are separate from sculpt Undo/Redo; Escape cancels a placement before release. Arrange mode and the overall Show references switch are session-only. Older projects load with no references.

The References menu uses the same compact pin and left/right sidebar icons as other menus. Keep it floating or dock it as a collapsible sidebar section. **Reference image controls**, **Add reference image at path**, **Arrange reference images**, and **Toggle reference images** are available in custom menus and configurable shortcuts; no new default keys are assigned.

Limits: 8 references per project, 2048 × 2048 pixels and 16 MB per source file, with a combined budget of 8 megapixels and 16 MB of embedded PNG data. Unsupported, malformed, or oversized images report an error without adding an image. References are screen overlays in this phase; attaching images to front/side planes is future work.

## Related guides

[Navigation and tablet input](navigation.md) · [Workspace and shortcuts](workspace.md) · [Files and recovery](files.md)

# Make your first sculpt

[Documentation home](../../README.md#documentation)

Turn a sphere into a small moon: carve a few craters, soften their edges, and save your own pocket planet. A mouse is enough; a pen works too. Allow about ten minutes.

## 1. Start with a sphere

1. Choose **New sphere**. Save any work you want to keep before replacing it.
2. Select **Standard** in **Brush library**.
3. In **Brush settings**, choose **Add**, **Freehand**, **Soft** alpha, **Radius 60**, and **Strength 0.25**. Turn **Stroke stabilisation** off for this exercise.
4. Turn **Symmetry** off so each crater appears only where you draw. For a single clay color, turn off **View → Show vertex group colors**.

You should have an unmarked sphere and a circular brush preview when you hover over it. Your panels may be arranged differently.

<a id="2-make-a-mark"></a>

## 2. Carve three craters

1. Hold **Alt**, start on the front of the sphere, and draw a short, slow curl. Release the pointer, then Alt. Standard pushes inward while Alt is held.
2. Move to another spot. Reduce **Radius** to **35** and draw a second, smaller curl with Alt held.
3. Add a third crater nearby, leaving some untouched surface between them. Different sizes and uneven spacing give the moon character.

Look for shallow hollows rather than deep tunnels. You can build depth with another short pass. If a crater becomes too deep, press **Ctrl+Z**, lower Strength, and try again. Each completed stroke is one undo step.

If clay rises instead, check that Standard is set to **Add**: Alt reverses its current operation. If the view rotates, the stroke started off the model; start the next one on clay.

## 3. Soften the rims

Hold **Shift** and make a light pass around one crater's edge. Release Shift to return to Standard. Aim to soften the transition while keeping the hollow visible.

Too much smoothing can erase the crater. Undo that pass and use a shorter stroke. Shift uses your last Smooth preset, so its size and strength may differ from Standard; see [brush settings](sculpting.md#choose-a-brush) if you need to adjust it.

For a raised rim, release Alt and Shift and trace a short Standard stroke beside part of a crater. Leave some gaps rather than drawing a perfect ring.

<a id="3-inspect-the-shape"></a>

## 4. Inspect the shape

Orbit slightly to see whether the hollows have depth and the rims catch the light.

| To… | Use… |
| --- | --- |
| Orbit | Right drag, or left/pen drag starting in empty space |
| Zoom | Mouse wheel, or Ctrl + drag |
| Pan | Middle drag, or Alt + drag starting in empty space |
| Return to the default view | F |

If the model fills the screen, start a navigation drag in the thin border strip. See [Navigation](navigation.md) for pen gestures and perspective.

<a id="4-undo-and-compare"></a>

Compare a change with **Ctrl+Z**, then restore it with **Ctrl+Shift+Z**. Open **Edit** to see which action will be undone. Brush settings and camera movements are outside sculpt history.

## 5. Save your work

Choose **Save** (**Ctrl+S**), browse to a folder, and name the project `pocket-moon.clay`. Later saves update that file directly; **Save as…** (**Ctrl+Shift+S**) makes a separate version. Reopen with **Open** or **Ctrl+O**.

Keep the `.clay` project for further sculpting. **Export OBJ** saves the active mesh for other applications. See [Files and recovery](files.md).

Your moon is ready when you can see three distinct hollows from an angled view and have a saved project. Try adding one tiny crater across the edge of a larger one, or leave the far side smooth.

## Try next

| I want to… | Guide |
| --- | --- |
| Sculpt a crisp crease | [Detail brush](sculpting.md#detail-brush) |
| Protect part of the model | [Mask lasso](selection.md#example-protect-a-region-with-a-lasso) |
| Start from a primitive | [Primitives](modeling.md#parametric-primitives) |
| Cut away part of the form | [Slice](modeling.md#slice) |
| Work beside a drawing | [Reference images](references.md) |
| Keep favorite tools visible | [Workspace](workspace.md) |


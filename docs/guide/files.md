# Files, OBJ import, and recovery

[Documentation home](../../README.md#documentation)

- [Save and open projects](#projects-and-brush-files)
- [Import a mesh](#obj-import)
- [Export a mesh](#export-an-obj)
- [Recover recent work](#recovery-saves)

## Projects and brush files

### Save or open a project

1. Enter a relative or absolute path in the toolbar, such as `my-sculpt.clay`.
2. Choose **Save** or **Open**.

The parent folder must exist. Saving replaces an existing file at that path. New, Open, and Quit prompt when work is unsaved.

| Format | Contains |
| --- | --- |
| `.clay` | Geometry, submodels, masks, groups, levels, primitive recipes, brushes, symmetry, and reference images |
| `.brush.json` | One brush preset |
| `.brushes.json` | A brush library |
| `.obj` | An exported mesh, including supported quad faces and face groups |

Camera position and undo history are session-only. [Workspace preferences](workspace.md#where-preferences-are-stored) save separately.

## OBJ import

### Example: import the sample pyramid

1. Download [pyramid.obj](../examples/pyramid.obj), or use `docs/examples/pyramid.obj` from a checkout.
2. Open **Submodels**, enter its path in **OBJ path**, and click **Import OBJ**.
3. Wait for the new submodel to appear. Use the original sphere's eye icon to hide it when inspecting the pyramid.
4. Save a `.clay` project to keep both models, or undo the import.

![Imported pyramid on its own with the original Sphere hidden and the pyramid selected in Submodels](../images/obj-import.png)

*The sphere is hidden, not removed. The imported pyramid has a quad base and four triangular sides.*

### What import preserves

| Preserved | Not imported |
| --- | --- |
| Coordinates, scale, winding, shared vertices | UVs, materials, supplied normals, vertex colors |
| Triangles and quads | Points, lines, free-form geometry |
| OBJ face groups | External material files |

Import does not center, resize, or weld the mesh. Multiple OBJ objects combine into one submodel, named from the filename. The current project path and existing models stay unchanged. Import is cancellable and creates one undo step.

### Polygon and group support

Simple planar polygons with up to **256 corners** are triangulated, including concave shapes. Triangulate non-planar, self-intersecting, or degenerate polygons in your source app; errors identify the line.

Positive/negative indices, UV/normal index forms, comments, line continuations, and UTF-8 names are accepted. Normals are regenerated. OBJ `g` names become full-weight vertex groups; if a face has several groups, the first supplies its color. Long names receive unique shortened versions.

### Import limits

Maximum **128 MB**, **500,000 vertices**, **1,000,000 triangles** after triangulation, **64 groups**, and **64 scene submodels**. Errors and cancellation leave the original scene intact.

## Export an OBJ

Select the submodel, then choose **Export OBJ**. The file is written beside the project path with an `.obj` extension.

Export uses the active subdivision level. Apply temporary thickness first if you want the shell included. Use `.clay` to retain the full editable project.

## Recovery saves

### Example: make a recovery checkpoint

1. Make a small sculpt edit and release the pointer.
2. Open **Settings → Recovery saves**.
3. Click **Save recovery now** and wait for the copy to appear.

![Recovery controls showing automatic saves, retention, a manual save button, and a Recover button](../images/recovery-dialog.png)

*Recovery protects unsaved edits without changing your normal save path.*

### Choose the schedule

| Setting | Default | Range |
| --- | --- | --- |
| Automatic recovery | Enabled | On / off |
| Interval | 2 minutes | 1–60 minutes |
| Retained copies across projects | 5 | 1–20 |

Copies include the full project, including never-saved work and references. Saving waits for strokes, drags, transforms, and background mesh operations to finish. Unchanged content does not consume another version.

### Restore a copy

Startup offers retained copies after an interrupted session. You can also open them from Recovery saves.

1. Choose **Recover** beside a copy.
2. Handle any Save/Discard/Cancel prompt for your current work.
3. Choose a new filename and save. The restored project's initial path is `recovered.clay`.

**Continue without restoring** leaves the copies available. Saving a project successfully, or deliberately discarding it on New/Open/Quit, clears that session's copies.

### Find recovery files

Copies live in `~/.config/clay-form/recovery/`, or `$XDG_CONFIG_HOME/clay-form/recovery/` when set. Recovery preferences are global, independent of workspaces.

Writes run in the background using temporary files and atomic replacement. A failed save leaves earlier copies available and shows a persistent error; retries use your interval. Test screenshots use temporary folders rather than your personal recovery directory.

## Related guides

[Submodels](modeling.md#submodels) · [Brush presets](sculpting.md#save-presets-and-brush-icons) · [Workspace](workspace.md)

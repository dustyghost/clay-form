# Files, OBJ import, and recovery

[Documentation home](../../README.md#documentation)

- [Projects and brush files](#projects-and-brush-files)
- [OBJ import](#obj-import)
- [Recovery saves](#recovery-saves)

## Projects and brush files

Type a relative or absolute file path in the toolbar, then use Save or Open. `.clay` projects are versioned JSON containing geometry, masks, and the brush library. Presets use `.brush.json`; libraries use `.brushes.json`. Existing files at the selected save path are replaced. Saving uses a temporary sibling file followed by rename. Parent folders must already exist.

New, Open and closing the window prompt when the project has unsaved changes. The prompt has a separate save path, protecting the file you intend to open. Export OBJ writes a mesh beside the project path, using the `.obj` extension. Projects restore the sculpt and presets; camera position and undo history are session-only. OBJ import adds a new submodel through the Submodels menu.

## OBJ import

Open **Submodels**, enter a file in **OBJ path**, and click **Import OBJ**. The importer adds and selects a new sculptable submodel, keeping the existing scene, brush settings, and project save path. Coordinates, scale, winding, and shared vertex indices are preserved; import does not center, resize, weld, or move the source. Multiple OBJ objects are combined into this one submodel. The filename supplies a unique submodel name. Import runs in the existing cancellable mesh worker and creates one Undo step; errors and cancellation keep the original scene. **Import OBJ at path** is also available in custom menus and configurable shortcuts.

Triangles and quads are supported, with quads retained for topology tools. Simple planar n-gons (up to 256 corners) are triangulated, including concave polygons. Degenerate, self-intersecting, and non-planar n-gon faces are rejected with a line number; triangulate these in the source application. Positive/negative indices, UV/normal index forms, comments, line continuations, and UTF-8 names are supported. The parser follows the polygonal syntax in the [original Wavefront Appendix B1](https://github.com/mokiat/java-data-front/blob/master/documents/OBJ%20Specification.txt).

OBJ `g` names become vertex groups with full weights on their member vertices. If a face belongs to multiple groups, all memberships are retained as vertex selections; its first group supplies the face category/color. Long group names are shortened with unique suffixes. UVs, supplied normals, materials, smoothing flags, and vertex colors are not retained; normals are generated from the sculpt surface. External material files are not opened. Point/line-only and free-form geometry are unsupported and rejected rather than partly imported.

Limits: 128 MB per file, 500,000 vertices, 1,000,000 triangles after triangulation, 64 groups, and 64 scene submodels. Ordinary exported Clay Form surfaces can be imported with their coordinates, quad topology, and face groups; exported group names use their existing safe OBJ spelling. Projects and recovery copies include the imported geometry normally.

### Example: import the sample pyramid

1. Download [pyramid.obj](../examples/pyramid.obj), or use `docs/examples/pyramid.obj` from a local checkout.
2. Start a practice project, open **Submodels**, and put the file's path in **OBJ path**. Click **Import OBJ** and wait for it to finish.
3. Inspect the new active submodel. The starting sphere remains in the scene; the pyramid has one quad base and four triangular sides, with **Base** and **Sides** vertex groups.
4. Toggle the sphere's eye icon to inspect the pyramid alone. Save as a new `.clay` project to retain both models, or undo the import to remove the pyramid.

![Imported pyramid in front of the original sphere with the new submodel selected in the Submodels sidebar](../images/obj-import.png)

*The capture imports the same pyramid geometry from a temporary test file, so its filename differs from `pyramid.obj`. Import preserves source coordinates; the overlap with the sphere is expected.*

## Recovery saves

**Settings → Recovery saves** controls automatic recovery (enabled by default, every **2 minutes**, keeping the newest **5 copies across projects**). Choose an interval from 1–60 minutes and retain 1–20 versions. Copies include the entire project, including never-saved work, submodels, masks, groups, subdivision levels, primitives, brushes, and symmetry. Snapshots wait for strokes, parameter drags, transforms, and background mesh operations to finish. Serialization and disk writing run in a background worker; unchanged content does not consume another version.

Recovery files live beside preferences in `~/.config/clay-form/recovery/` (or `$XDG_CONFIG_HOME/clay-form/recovery/`). They never change the manual project path or clear its modified state. Writes use flushed temporary files and atomic replacement; incomplete temporary files are ignored. A failed write leaves earlier copies available and shows a persistent error with access to recovery settings. Automatic retries use the configured interval.

After an interrupted session, startup offers retained copies, newest first. **Recover** restores a modified project with the save path `recovered.clay`; choose your desired path before saving. Existing work gets the normal Save/Discard/Cancel prompt. **Continue without restoring** keeps the copies available under Settings. Successfully saving a project, or deliberately discarding it on New/Open/Quit, clears that session's copies. Recovery preferences are global, independent of workspaces. **Recovery saves** and **Save recovery now** are also available in custom menus and configurable shortcuts.

### Example: make a recovery checkpoint

1. Start a practice project, make a short sculpt stroke, and release the pointer.
2. Open **Settings → Recovery saves**. Leave automatic recovery enabled at **2 minutes**, keeping **5** versions, or choose your preferred interval and count.
3. Click **Save recovery now** and wait for a copy to appear in the list. This does not change your normal save path or clear the modified marker.
4. To try restoration, select that copy's **Recover** button and handle the usual unsaved-changes prompt. Save the restored project under a new filename, such as `practice-recovered.clay`.

![Recovery settings showing the interval, retention count, Save recovery now, and a retained copy with a Recover button](../images/recovery-dialog.png)

*A retained checkpoint is available without simulating a crash. This test capture uses a temporary recovery folder; normal sessions use the configuration folder described above.*

## Related guides

[Submodels](modeling.md#submodels) · [Brush presets](sculpting.md#brushes-alphas-and-symmetry) · [Workspace preferences](workspace.md#key-settings-and-workspace)

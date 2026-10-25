"""Blender FBX import validation after interchange_manual_review.
blender -b -noaudio --python-exit-code 1 --python scripts/validate-interchange-fbx.py -- /tmp/clay-interchange-review
"""
import json
import sys
from pathlib import Path
import bpy

directory = Path(sys.argv[sys.argv.index('--') + 1]).resolve()
source = json.loads((directory / 'source.clay').read_text())['mesh']
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
bpy.ops.import_scene.fbx(filepath=str(directory / 'model.fbx'))
obj = next(o for o in bpy.context.scene.objects if o.type == 'MESH')
mesh = obj.data
assert len(mesh.vertices) == len(source['vertices'])
assert len(mesh.polygons) == len(source['triangles'])
assert mesh.has_custom_normals
for actual, (x, y, z) in zip(mesh.vertices, source['vertices']):
    assert max(abs(a-b) for a, b in zip(obj.matrix_world @ actual.co, (x, -z, y))) < 1e-5
for f, face in enumerate(mesh.polygons):
    assert list(face.vertices) == source['triangles'][f]
    for c, loop in enumerate(face.loop_indices):
        expected = (0, 0) if source['uv']['removed'][f] else source['uv']['corners'][f][c]
        assert max(abs(a-b) for a, b in zip(mesh.uv_layers.active.data[loop].uv, expected)) < 1e-6
    assert face.material_index == (0 if source['uv']['removed'][f] else 1)
assert mesh.color_attributes
assert Path(directory / obj['clay_metadata']).is_file()
paths = [Path(bpy.path.abspath(n.image.filepath)) for m in mesh.materials for n in m.node_tree.nodes if n.type == 'TEX_IMAGE']
assert len(paths) >= 3, paths
assert all(p.is_file() for p in paths), paths
assert any(p.name == 'color.exr' for p in paths), paths
print('FBX geometry, axes, UVs, normals, material slots, vertex colours and relative textures validated')

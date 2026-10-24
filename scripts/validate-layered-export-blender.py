"""Manual interoperability review, after material_blender_export_manual_review.
blender -b /tmp/clay-layer-export/layered.blend -noaudio --python-exit-code 1 --python scripts/validate-layered-export-blender.py -- /tmp/clay-layer-export
"""
import bpy
import json
import sys
from pathlib import Path

out = Path(sys.argv[sys.argv.index('--') + 1])
data = json.loads((out / 'exchange.json').read_text())
obj = next(o for o in bpy.data.objects if o.type == 'MESH')
assert len(obj.data.vertices) == len(data['vertices'])
assert len(obj.data.polygons) == len(data['faces'])
assert obj.data.has_custom_normals
for p, original in zip(obj.data.vertices, data['vertices']):
    x, y, z = original
    assert max(abs(a-b) for a, b in zip(p.co, (x, -z, y))) < 1e-6
for c, values in enumerate(data['channels']):
    attribute = obj.data.attributes[f'clay_residual_{c:02}']
    assert attribute.domain == 'POINT' and attribute.data_type == 'FLOAT'
    assert max(abs(v.value-ref) for v, ref in zip(attribute.data, values)) < 1e-6
for asset in data['assets']:
    definition = next(g for g in bpy.data.node_groups if g.get('clay_material_id') == asset['id'])
    values = [n for n in definition.nodes if n.type == 'VALUE']
    assert len(values) == 70
    for node, value in zip(values, asset['values']):
        assert abs(node.outputs[0].default_value-value) < 1e-6
    for g in range(4):
        weights = obj.data.attributes[f"clay_weight_{asset['id']}_{g}"].data
        assert max(abs(v.value-w[g]) for v, w in zip(weights, asset['weights'])) < 1e-6
material = obj.data.materials[0]
assert sum(n.type == 'BSDF_PRINCIPLED' for n in material.node_tree.nodes) == 4
assert sum(n.type == 'ADD_SHADER' for n in material.node_tree.nodes) == 3
for n in material.node_tree.nodes:
    if n.type == 'BSDF_PRINCIPLED':
        assert all(n.inputs[name].is_linked for name in ['Subsurface Weight', 'Subsurface Radius', 'Subsurface Scale'])
assert material.node_tree.nodes.get('Material Output').inputs['Surface'].is_linked
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.device = 'CPU'
scene.cycles.samples = 8
scene.cycles.use_denoising = False
scene.render.resolution_x = scene.render.resolution_y = 256
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = 'PNG'

def render(name):
    scene.render.filepath = str(out / name)
    bpy.ops.render.render(write_still=True)
    image = bpy.data.images.load(scene.render.filepath, check_existing=False)
    image.colorspace_settings.name = 'Non-Color'
    pixels = list(image.pixels)
    bpy.data.images.remove(image)
    return pixels

before = render('blender-layered-before.png')
definition.nodes['Layer 1 · Emission strength'].outputs[0].default_value = 20
for channel, value in zip('RGB', (1, 0, 0)):
    definition.nodes[f'Layer 1 · Emission {channel}'].outputs[0].default_value = value
bpy.context.view_layer.update()
after = render('blender-layered-edited.png')
changed = sum(max(abs(a-b) for a, b in zip(before[i:i+3], after[i:i+3])) > 0.1 for i in range(0, len(before), 4))
assert changed > 1000, changed
(out / 'validation.json').write_text(json.dumps({'vertices': len(obj.data.vertices), 'channels': 70, 'shared_definitions': len(data['assets']), 'edited_render_pixels_changed': changed, 'blender_version': bpy.app.version_string}, indent=2))
print('Layered material export and live shared editing validated', flush=True)

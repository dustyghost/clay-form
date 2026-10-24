"""Validate optional viewport lighting after lighting_blender_export_manual_review."""
import bpy
import json
import math
import sys
from pathlib import Path
from mathutils import Vector

root = Path(sys.argv[sys.argv.index('--') + 1])
expected = json.loads((root / 'lighting.json').read_text())
bpy.ops.wm.open_mainfile(filepath=str(root / 'viewport.blend'))
lights = sorted((obj for obj in bpy.context.scene.objects if obj.type == 'LIGHT'), key=lambda obj: obj.name)
assert len(lights) == len(expected['lights']) == 2
for obj, source in zip(lights, expected['lights']):
    x, y, z = source['vector']
    vector = Vector((x, -z, y))
    assert obj.data.type == source['kind']
    assert max(abs(a-b) for a, b in zip(obj.data.color, source['colour'])) < 1e-5
    energy = source['intensity']
    if source['kind'] == 'POINT':
        assert (obj.location-vector).length < 1e-5
        energy *= 16 * 4 * math.pi
    else:
        emitted = obj.rotation_euler.to_quaternion() @ Vector((0, 0, -1))
        assert (emitted + vector.normalized()).length < 1e-5
    assert abs(obj.data.energy-energy) < 1e-3
strength = bpy.context.scene.world.node_tree.nodes['Background'].inputs['Strength'].default_value
assert abs(strength-expected['ambient_fill']*0.1) < 1e-6
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.samples = 8
scene.render.resolution_x = scene.render.resolution_y = 256
scene.render.resolution_percentage = 100
scene.render.filepath = str(root / 'viewport-lights.png')
bpy.ops.render.render(write_still=True)
bpy.ops.wm.open_mainfile(filepath=str(root / 'neutral.blend'))
assert len([obj for obj in bpy.context.scene.objects if obj.type == 'LIGHT']) == 1
assert not bpy.context.scene.get('clay_viewport_lights_included', False)
bpy.ops.wm.open_mainfile(filepath=str(root / 'ambient.blend'))
assert not any(obj.type == 'LIGHT' for obj in bpy.context.scene.objects)
assert bpy.context.scene['clay_viewport_lights_included']
assert abs(bpy.context.scene.world.node_tree.nodes['Background'].inputs['Strength'].default_value - 0.04) < 1e-6
(root / 'validation.json').write_text(json.dumps({'lights': 2, 'camera_relative_direction': 'passed', 'point_position_colour_power': 'passed', 'unchecked_neutral_preview': 'passed', 'ambient_only': 'passed', 'blender_version': bpy.app.version_string}, indent=2))
print('Optional viewport lighting export validated', flush=True)

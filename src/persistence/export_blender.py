"""Clay Form material exchange v1. Runs in Blender 4.0+ with a private snapshot.
Shared definition groups + POINT attributes retain editable, blended material uses.
Only ordinary Principled, Math, Mix Shader and Add Shader nodes are required.
"""
import bpy
import json
import math
import sys
from pathlib import Path
from mathutils import Vector

input_path, output_path = sys.argv[sys.argv.index('--') + 1:][:2]
data = json.loads(Path(input_path).read_text())
if bpy.app.version < (4, 0, 0):
    raise RuntimeError('Blender 4.0 or newer is required')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
mesh = bpy.data.meshes.new(data['name'])
# Convert Clay Form Y-up into Blender Z-up without changing handedness.
mesh.from_pydata([(x, -z, y) for x, y, z in data['vertices']], [], data['faces'])
mesh.update()
obj = bpy.data.objects.new(data['name'], mesh)
bpy.context.collection.objects.link(obj)
bpy.context.view_layer.objects.active = obj
obj.select_set(True)
for face in mesh.polygons:
    face.use_smooth = True
mesh.normals_split_custom_set([(x, -z, y) for face in data['normals'] for x, y, z in face])
obj['clay_exchange_version'] = data['version']
obj['clay_source_axes'] = 'Y-up; Blender positions are (x, -z, y)'
material = bpy.data.materials.new('Clay painted surface')
material.use_nodes = True
mesh.materials.append(material)
tree = material.node_tree
tree.nodes.clear()

def node(kind, label=''):
    n = tree.nodes.new(kind)
    n.label = label
    return n

def attribute(name, values):
    attr = mesh.attributes.new(name, 'FLOAT', 'POINT')
    attr.data.foreach_set('value', values)
    n = node('ShaderNodeAttribute', name)
    n.attribute_name = name
    return n.outputs['Fac']

def math_node(operation, a, b, label=''):
    n = node('ShaderNodeMath', label)
    n.operation = operation
    for socket, value in zip(n.inputs, (a, b)):
        if isinstance(value, (int, float)):
            socket.default_value = value
        else:
            tree.links.new(value, socket)
    return n.outputs[0]

properties = ['Colour R', 'Colour G', 'Colour B', 'Roughness', 'Metallic', 'IOR', 'Specular', 'Emission R', 'Emission G', 'Emission B', 'Emission strength', 'Subsurface weight', 'Radius R', 'Radius G', 'Radius B', 'Subsurface scale']
labels = ['Base · ' + p for p in properties]
for i in range(3):
    labels.extend([f'Layer {i+1} · ' + p for p in properties + ['Weight', 'Add (0 = Mix)']])

def limits(c):
    prop = c if c < 16 else (c - 16) % 18
    return (1, 4) if prop == 5 else (0, 100) if prop == 10 else (0, 10) if 12 <= prop <= 15 else (0, 1)

def group(c):
    return 0 if c < 3 else 1 if c == 3 else 2 if c == 4 else 3

assets = []
for asset in data['assets']:
    # Edit the labelled Value nodes inside this shared group to change every use.
    definition = bpy.data.node_groups.new(asset['name'], 'ShaderNodeTree')
    definition['clay_material_id'] = asset['id']
    definition['clay_layer_order'] = 'Base, Layer 1, Layer 2, Layer 3'
    output = definition.nodes.new('NodeGroupOutput')
    output.location = (550, 0)
    for c, (label, value) in enumerate(zip(labels, asset['values'])):
        socket = definition.interface.new_socket(name=label, in_out='OUTPUT', socket_type='NodeSocketFloat')
        n = definition.nodes.new('ShaderNodeValue')
        n.label = label
        n.name = label
        n.outputs[0].default_value = value
        n.location = ((c // 13) * 180 - 500, -(c % 13) * 90)
        definition.links.new(n.outputs[0], output.inputs[c])
    instance = node('ShaderNodeGroup', asset['name'])
    instance.node_tree = definition
    weights = [attribute(f"clay_weight_{asset['id']}_{g}", [w[g] for w in asset['weights']]) for g in range(4)]
    assets.append((instance, weights))

channels = []
for c, values in enumerate(data['channels']):
    value = attribute(f'clay_residual_{c:02}', values)
    for definition, weights in assets:
        value = math_node('ADD', value, math_node('MULTIPLY', definition.outputs[c], weights[group(c)]))
    lo, hi = limits(c)
    channels.append(math_node('MINIMUM', math_node('MAXIMUM', value, lo), hi, labels[c]))

def rgb(offset):
    n = node('ShaderNodeCombineColor')
    n.mode = 'RGB'
    for i in range(3):
        tree.links.new(channels[offset+i], n.inputs[i])
    return n.outputs[0]

def surface(offset, label):
    n = node('ShaderNodeBsdfPrincipled', label)
    for name, value in [('Base Color', rgb(offset)), ('Roughness', channels[offset+3]), ('Metallic', channels[offset+4]), ('IOR', channels[offset+5]), ('Specular IOR Level', channels[offset+6]), ('Emission Color', rgb(offset+7)), ('Emission Strength', channels[offset+10]), ('Subsurface Weight',channels[offset+11]), ('Subsurface Radius',rgb(offset+12)), ('Subsurface Scale',channels[offset+15])]:
        tree.links.new(value, n.inputs[name])
    return n.outputs['BSDF']

def weighted(shader, weight, label):
    # An unconnected first shader is zero, so this scales the second BSDF.
    n = node('ShaderNodeMixShader', label)
    tree.links.new(weight, n.inputs[0])
    tree.links.new(shader, n.inputs[2])
    return n.outputs[0]

shader = surface(0, 'Base surface')
for i in range(3):
    offset = 16 + i*18
    weight, add = channels[offset+16:offset+18]
    base_weight = math_node('SUBTRACT', 1, math_node('MULTIPLY', weight, math_node('SUBTRACT', 1, add)))
    n = node('ShaderNodeAddShader', f'Layer {i+1}: Mix / Add')
    tree.links.new(weighted(shader, base_weight, f'Layer {i+1} base weight'), n.inputs[0])
    tree.links.new(weighted(surface(offset, f'Layer {i+1} surface'), weight, f'Layer {i+1} weight'), n.inputs[1])
    shader = n.outputs[0]
output = node('ShaderNodeOutputMaterial')
tree.links.new(shader, output.inputs['Surface'])
# Group asset nodes separately and place the generated evaluation chain in columns.
for i, n in enumerate(tree.nodes):
    n.location = ((i // 25) * 230, -(i % 25) * 100)
material['clay_mapping'] = 'Linear RGB; Principled IOR/specular/emission; three weighted Mix/Add layers. Edit named definition groups.'
# A neutral scene and fitted camera make the exported model immediately inspectable.
scene = bpy.context.scene
scene.world.color = (0.12, 0.12, 0.12)
scene.view_settings.view_transform = 'Standard'
center = sum((v.co for v in mesh.vertices), Vector()) / max(1, len(mesh.vertices))
radius = max((v.co-center).length for v in mesh.vertices) if mesh.vertices else 1
radius = max(radius, 0.01)
bpy.ops.object.camera_add(location=center+Vector((2, -3, 2))*radius)
camera = bpy.context.object
camera.rotation_euler = (center-camera.location).to_track_quat('-Z', 'Y').to_euler()
camera.data.type = 'ORTHO'
camera.data.ortho_scale = radius*2.5
scene.camera = camera
bpy.ops.object.light_add(type='SUN', location=center+Vector((2, -3, 4))*radius)
light = bpy.context.object
light.rotation_euler = (center-light.location).to_track_quat('-Z', 'Y').to_euler()
light.data.energy = 2.5
bpy.ops.object.select_all(action='DESELECT')
obj.select_set(True)
bpy.context.view_layer.objects.active = obj
bpy.ops.wm.save_as_mainfile(filepath=str(Path(output_path).resolve()), check_existing=False)
print('Clay material export complete', flush=True)

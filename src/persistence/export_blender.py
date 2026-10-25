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
# Keep unused source fields as metadata, not shader inputs. The former exporter
# read all 70 fields even for plain clay, exceeding EEVEE's attribute limit.
from array import array
root_tree = material.node_tree
root_tree.nodes.clear()
properties = ['Colour R', 'Colour G', 'Colour B', 'Roughness', 'Metallic', 'IOR', 'Specular', 'Emission R', 'Emission G', 'Emission B', 'Emission strength', 'Subsurface weight', 'Radius R', 'Radius G', 'Radius B', 'Subsurface scale']
labels = ['Base · ' + p for p in properties]
for i in range(3):
    labels.extend([f'Layer {i+1} · ' + p for p in properties + ['Weight', 'Add (0 = Mix)']])

def group(c):
    return 0 if c < 3 else 1 if c == 3 else 2 if c == 4 else 3

def limits(c):
    prop = c if c < 16 else (c - 16) % 18
    return (1, 4) if prop == 5 else (0, 100) if prop == 10 else (0, 10) if 12 <= prop <= 15 else (0, 1)

# A linked definition can activate a currently zero-weight layer after export.
live_layers = [i for i in range(3) if any(data['channels'][32+i*18]) or any(any(w[3] for w in a['weights']) for a in data['assets'])]
complex_material = bool(data['assets'] or live_layers)
if complex_material:
    tree = bpy.data.node_groups.new('Clay layered surface', 'ShaderNodeTree')
    tree.interface.new_socket(name='Surface', in_out='OUTPUT', socket_type='NodeSocketShader')
    instance = root_tree.nodes.new('ShaderNodeGroup')
    instance.node_tree = tree
    instance.label = 'Painted layers · shared material controls inside'
    instance.location = (0, 0)
    out = root_tree.nodes.new('ShaderNodeOutputMaterial')
    out.location = (340, 0)
    root_tree.links.new(instance.outputs['Surface'], out.inputs['Surface'])
else:
    tree = root_tree

node_columns = {}
def node(kind, label='', column=0):
    n = tree.nodes.new(kind)
    n.label = label
    row = node_columns.get(column, 0)
    n.location = (column * 300, -row * 210)
    n.width = 240 if kind in ('ShaderNodeTexImage', 'ShaderNodeBsdfPrincipled') else 180
    node_columns[column] = row + 1
    return n

def connect(value, socket):
    if isinstance(value, (int, float, tuple, list)):
        socket.default_value = value[:3] if socket.type == 'VECTOR' and isinstance(value, (tuple, list)) else value
    else:
        tree.links.new(value, socket)

# Deduplicate varying fields and pack four float values per shader attribute.
# Constants go directly into sockets. Original POINT values remain inspectable.
packed, varying = [], {}
def attribute(name, values):
    attr = mesh.attributes.new(name, 'FLOAT', 'POINT')
    attr.data.foreach_set('value', values)
    if not values or all(v == values[0] for v in values):
        return values[0] if values else 0.
    key = array('f', values).tobytes()
    if key in varying:
        return varying[key]
    if not packed or len(packed[-1]['fields']) == 4:
        if len(packed) >= 12:
            raise RuntimeError('This painted material needs more independent vertex data than Blender material preview supports. Reduce independent material layers or bindings before exporting; no incomplete blend was saved.')
        name = f'clay_paint_data_{len(packed):02}'
        attr_node = node('ShaderNodeAttribute', 'Packed vertex paint', -4)
        attr_node.attribute_name = name
        split = node('ShaderNodeSeparateColor', 'Paint components', -3)
        split.mode = 'RGB'
        tree.links.new(attr_node.outputs['Color'], split.inputs[0])
        packed.append({'name':name, 'fields':[], 'sockets':list(split.outputs[:3])+[attr_node.outputs['Alpha']]})
    pack = packed[-1]
    socket = pack['sockets'][len(pack['fields'])]
    pack['fields'].append(values)
    varying[key] = socket
    return socket

def math_node(operation, a, b, label=''):
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return {'ADD':lambda:a+b, 'SUBTRACT':lambda:a-b, 'MULTIPLY':lambda:a*b, 'MINIMUM':lambda:min(a,b), 'MAXIMUM':lambda:max(a,b)}[operation]()
    if operation == 'MULTIPLY':
        if isinstance(a, (int,float)) and a == 0 or isinstance(b, (int,float)) and b == 0: return 0.
        if isinstance(a, (int,float)) and a == 1: return b
        if isinstance(b, (int,float)) and b == 1: return a
    if operation == 'ADD':
        if isinstance(a, (int,float)) and a == 0: return b
        if isinstance(b, (int,float)) and b == 0: return a
    n = node('ShaderNodeMath', label, -2)
    n.operation = operation
    connect(a, n.inputs[0]); connect(b, n.inputs[1])
    return n.outputs[0]

assets = []
for asset in data['assets']:
    definition = bpy.data.node_groups.new(asset['name'], 'ShaderNodeTree')
    definition['clay_material_id'] = asset['id']
    definition['clay_layer_order'] = 'Base, Layer 1, Layer 2, Layer 3'
    output = definition.nodes.new('NodeGroupOutput')
    output.location = (850, 0)
    for c, (label, value) in enumerate(zip(labels, asset['values'])):
        definition.interface.new_socket(name=label, in_out='OUTPUT', socket_type='NodeSocketFloat')
        n = definition.nodes.new('ShaderNodeValue')
        n.label = n.name = label
        n.outputs[0].default_value = value
        layer = 0 if c < 16 else 1+(c-16)//18
        row = c if c < 16 else (c-16)%18
        n.location = (layer*200, -row*90)
        definition.links.new(n.outputs[0], output.inputs[c])
    instance = node('ShaderNodeGroup', asset['name'], -5)
    instance.node_tree = definition
    instance.hide = True
    weights = [attribute(f"clay_weight_{asset['id']}_{g}", [w[g] for w in asset['weights']]) for g in range(4)]
    assets.append((instance, weights))

needed = set(range(16))
for i in live_layers: needed.update(range(16+i*18, 34+i*18))
channels = {}
for c, values in enumerate(data['channels']):
    if c not in needed:
        attr = mesh.attributes.new(f'clay_residual_{c:02}', 'FLOAT', 'POINT')
        attr.data.foreach_set('value', values)
        continue
    value = attribute(f'clay_residual_{c:02}', values)
    for definition, weights in assets:
        value = math_node('ADD', value, math_node('MULTIPLY', definition.outputs[c], weights[group(c)]))
    lo, hi = limits(c)
    channels[c] = math_node('MINIMUM', math_node('MAXIMUM', value, lo), hi, labels[c])

for pack in packed:
    attr = mesh.color_attributes.new(pack['name'], 'FLOAT_COLOR', 'POINT')
    fields = pack['fields'] + [[0.]*len(mesh.vertices)]*(4-len(pack['fields']))
    attr.data.foreach_set('color', [v for row in zip(*fields) for v in row])
material['clay_shader_attribute_count'] = len(packed)

def rgb(offset):
    values = [channels[offset+i] for i in range(3)]
    if all(isinstance(v, (int,float)) for v in values): return tuple(values)+(1.,)
    n = node('ShaderNodeCombineColor', labels[offset].split(' · ')[0], -1)
    n.mode = 'RGB'
    for i, value in enumerate(values): connect(value, n.inputs[i])
    return n.outputs[0]

# UV/image paint is embedded so moving the .blend cannot lose its textures.
images = {}
if data.get('uv'):
    uv = mesh.uv_layers.new(name='Clay UV')
    uv.data.foreach_set('uv', [v for face in data['uv'] for corner in face for v in corner])
    uv_node = node('ShaderNodeUVMap', 'Paint UVs', -2)
    uv_node.uv_map = uv.name
    mapped = data.get('mapped', [True]*len(mesh.polygons))
    coverage = 1.
    if not all(mapped):
        attr = mesh.attributes.new('clay_uv_mapped', 'FLOAT', 'FACE')
        attr.data.foreach_set('value', [float(v) for v in mapped])
        n = node('ShaderNodeAttribute', 'Mapped faces', -3)
        n.attribute_name = attr.name
        coverage = n.outputs['Fac']
    for texture in data.get('textures', []):
        image = bpy.data.images.load(str((Path(input_path).parent / texture['path']).resolve()), check_existing=False)
        image.colorspace_settings.name = 'Non-Color' if texture['channel'] != 'Color' else 'Linear Rec.709' if texture['linear'] else 'sRGB'
        image.alpha_mode = 'PREMUL' if texture['linear'] and texture['path'].endswith('.exr') else 'STRAIGHT'
        image.pack()
        image.filepath = '//'+Path(texture['path']).name
        n = node('ShaderNodeTexImage', 'Painted '+texture['channel'].lower(), -1)
        n.image = image
        n.extension = 'EXTEND'
        n.interpolation = 'Linear'
        tree.links.new(uv_node.outputs['UV'], n.inputs['Vector'])
        images[texture['channel']] = (n.outputs['Color'], math_node('MULTIPLY', n.outputs['Alpha'], coverage))

def image_over(value, name):
    if name not in images: return value
    colour, alpha = images[name]
    n = node('ShaderNodeMixRGB', name+' paint coverage', 0)
    n.blend_type = 'MIX'
    if isinstance(value,(int,float)): value=(value,value,value,1.)
    connect(alpha,n.inputs[0]); connect(value,n.inputs[1]); connect(colour,n.inputs[2])
    return n.outputs[0]

def surface(offset, label):
    n = node('ShaderNodeBsdfPrincipled', label, 1)
    for name, value in [('Base Color', image_over(rgb(offset),'Color')), ('Roughness',image_over(channels[offset+3],'Roughness')), ('Metallic',image_over(channels[offset+4],'Metallic')), ('IOR',channels[offset+5]), ('Specular IOR Level',channels[offset+6]), ('Emission Color',rgb(offset+7)), ('Emission Strength',channels[offset+10]), ('Subsurface Weight',channels[offset+11]), ('Subsurface Radius',rgb(offset+12)), ('Subsurface Scale',channels[offset+15])]:
        connect(value,n.inputs[name])
    return n.outputs['BSDF']

def weighted(shader, weight, label):
    if isinstance(weight,(int,float)) and weight == 1: return shader
    n = node('ShaderNodeMixShader', label, 2)
    connect(weight,n.inputs[0]); connect(shader,n.inputs[2])
    return n.outputs[0]

shader = surface(0, 'Base surface')
for i in live_layers:
    offset = 16+i*18
    weight, add = channels[offset+16], channels[offset+17]
    base_weight = math_node('SUBTRACT',1,math_node('MULTIPLY',weight,math_node('SUBTRACT',1,add)))
    n = node('ShaderNodeAddShader', f'Layer {i+1}: Mix / Add', 3)
    connect(weighted(shader,base_weight,f'Layer {i+1} base weight'),n.inputs[0])
    connect(weighted(surface(offset,f'Layer {i+1} surface'),weight,f'Layer {i+1} weight'),n.inputs[1])
    shader = n.outputs[0]
output = node('NodeGroupOutput' if complex_material else 'ShaderNodeOutputMaterial', column=4 if complex_material else 2)
connect(shader,output.inputs['Surface'])
material['clay_mapping'] = 'Linear painted values; packed vertex fields; only active unlinked layers. Image alpha is paint coverage, not surface transparency. Edit named shared definition groups inside Clay layered surface.'
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
lighting = data.get('lighting')
if lighting is None:
    # Preserve the neutral preview light when viewport lighting was not requested.
    bpy.ops.object.light_add(type='SUN', location=center+Vector((2, -3, 4))*radius)
    light = bpy.context.object
    light.rotation_euler = (center-light.location).to_track_quat('-Z', 'Y').to_euler()
    light.data.energy = 2.5
else:
    world = scene.world or bpy.data.worlds.new('Clay ambient fill')
    scene.world = world
    world.use_nodes = True
    background = world.node_tree.nodes.get('Background')
    background.inputs['Color'].default_value = (1, 1, 1, 1)
    background.inputs['Strength'].default_value = lighting['ambient_fill'] * 0.1
    world['clay_ambient_fill'] = lighting['ambient_fill']
    for index, source in enumerate(lighting['lights']):
        x, y, z = source['vector']
        vector = Vector((x, -z, y))
        lamp = bpy.data.lights.new(f'Clay light {index+1}', source['kind'])
        light = bpy.data.objects.new(lamp.name, lamp)
        bpy.context.collection.objects.link(light)
        lamp.color = source['colour']
        lamp['clay_intensity'] = source['intensity']
        if source['kind'] == 'POINT':
            light.location = vector
            # Viewport irradiance is intensity * 16 / distance squared.
            # Blender point power is distributed over a sphere (4*pi steradians).
            lamp.energy = source['intensity'] * 16 * 4 * math.pi
            lamp.shadow_soft_size = 0
        else:
            light.rotation_euler = (-vector).to_track_quat('-Z', 'Y').to_euler()
            lamp.energy = source['intensity']
            lamp.angle = 0
    scene['clay_viewport_lights_included'] = True
bpy.ops.object.select_all(action='DESELECT')
obj.select_set(True)
bpy.context.view_layer.objects.active = obj
bpy.ops.wm.save_as_mainfile(filepath=str(Path(output_path).resolve()), check_existing=False)
print('Clay material export complete', flush=True)

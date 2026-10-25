"""Static binary FBX via Blender's maintained exporter (Blender 4.0+)."""
import json
import sys
from pathlib import Path
import bpy

input_path, output_path = map(Path, sys.argv[sys.argv.index('--') + 1:][:2])
data = json.loads(input_path.read_text())
if bpy.app.version < (4, 0, 0):
    raise RuntimeError('Blender 4.0 or newer is required')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
mesh = bpy.data.meshes.new(data['name'])
# FBX exporter converts this Z-up mesh back to Y-up at export.
mesh.from_pydata([(x, -z, y) for x, y, z in data['vertices']], [], data['faces'])
mesh.update()
obj = bpy.data.objects.new(data['name'], mesh)
bpy.context.collection.objects.link(obj)
bpy.context.view_layer.objects.active = obj
obj.select_set(True)
for face in mesh.polygons:
    face.use_smooth = True
mesh.normals_split_custom_set([(x, -z, y) for face in data['normals'] for x, y, z in face])
obj['clay_metadata'] = data['metadata']
obj['clay_export_notes'] = 'Base PBR preview; save .clay for complete editing state. One source unit = one metre.'
uv = data['uv']
removed = uv.get('removed', [])
if uv['corners']:
    layer = mesh.uv_layers.new(name='st')
    for f, face in enumerate(mesh.polygons):
        for c, loop in enumerate(face.loop_indices):
            layer.data[loop].uv = (0, 0) if removed and removed[f] else uv['corners'][f][c]
    seams = {tuple(sorted(edge)) for edge in uv['seams']}
    for edge in mesh.edges:
        edge.use_seam = tuple(sorted(edge.vertices)) in seams

# FBX carries these colours independently of shader translation.
colours = mesh.color_attributes.new(name='ClayBaseColor', type='FLOAT_COLOR', domain='CORNER')
for loop in mesh.loops:
    colours.data[loop.index].color = (*data['materials'][loop.vertex_index]['base_color'], 1)

materials = data['materials']
count = len(materials)
base = bpy.data.materials.new('Clay base surface')
base.use_nodes = True
principled = base.node_tree.nodes.get('Principled BSDF')
principled.inputs['Base Color'].default_value = (*[sum(m['base_color'][c] for m in materials) / count for c in range(3)], 1)
for prop, socket in [('roughness', 'Roughness'), ('metallic', 'Metallic')]:
    principled.inputs[socket].default_value = sum(m[prop] for m in materials) / count
mesh.materials.append(base)
if data['maps']:
    painted = base.copy()
    painted.name = 'Clay painted surface'
    mesh.materials.append(painted)
    tree = painted.node_tree
    principled = tree.nodes.get('Principled BSDF')
    for source in data['maps']:
        image = bpy.data.images.load(str(input_path.parent / source['path']), check_existing=False)
        image.colorspace_settings.name = 'Non-Color' if source['linear'] else 'sRGB'
        texture = tree.nodes.new('ShaderNodeTexImage')
        texture.image = image
        texture.extension = 'EXTEND'
        socket = {'Color': 'Base Color', 'Roughness': 'Roughness', 'Metallic': 'Metallic'}[source['channel']]
        tree.links.new(texture.outputs['Color'], principled.inputs[socket])
    for f, face in enumerate(mesh.polygons):
        face.material_index = 0 if removed and removed[f] else 1

# External relative textures are portable and retain 16-bit PNG / 32-bit EXR.
# FBX's legacy material model may need roughness/metallic reassignment in the DCC.
bpy.ops.export_scene.fbx(filepath=str(output_path.resolve()), use_selection=True,
    object_types={'MESH'}, use_mesh_modifiers=False, mesh_smooth_type='OFF',
    use_custom_props=True, add_leaf_bones=False, bake_anim=False,
    axis_forward='-Z', axis_up='Y', apply_unit_scale=True,
    path_mode='RELATIVE', embed_textures=False)
print('Clay FBX export complete', flush=True)

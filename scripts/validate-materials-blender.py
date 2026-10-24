"""Manual Blender material mapping/render fixture, not an application exchange tool.
Run: blender -b --factory-startup --python scripts/validate-materials-blender.py -- /tmp/clay-material-review
The input materials.json is written by material_examples_manual_capture.
"""
import bpy
import json
import math
import sys
from pathlib import Path
from mathutils import Vector

out = Path(sys.argv[sys.argv.index('--') + 1])
examples = json.loads((out / 'materials.json').read_text())
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.device = 'CPU'
scene.cycles.samples = 32
scene.cycles.use_denoising = False
scene.render.resolution_x = 256
scene.render.resolution_y = 256
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = 'PNG'
scene.view_settings.view_transform = 'Standard'
scene.view_settings.look = 'None'
scene.view_settings.exposure = 0
scene.view_settings.gamma = 1
scene.world.use_nodes = True
scene.world.node_tree.nodes.get('Background').inputs['Strength'].default_value = 0
bpy.ops.object.camera_add(location=(0, 0, 5))
camera = bpy.context.object
camera.rotation_euler = (0, 0, 0) # Camera looks down local -Z.
camera.data.type = 'ORTHO'
camera.data.ortho_scale = 2.85
scene.camera = camera
bpy.ops.object.light_add(type='SUN', location=(0, 0, 5))
light = bpy.context.object
light.data.energy = 2.5
light.data.angle = 0
light.rotation_euler = (-Vector((-0.5, 0.7, 1)).normalized()).to_track_quat('-Z', 'Y').to_euler()
bpy.ops.mesh.primitive_plane_add(size=2)
plate = bpy.context.object

def material_for(obj, name, data):
    material = bpy.data.materials.new(name)
    material.use_nodes = True
    obj.data.materials.clear()
    obj.data.materials.append(material)
    tree = material.node_tree
    principled = tree.nodes.get('Principled BSDF')
    principled.inputs['IOR'].default_value = 1.5
    for key, socket, kind in [('base_color', 'Base Color', 'FLOAT_COLOR'), ('roughness', 'Roughness', 'FLOAT'), ('metallic', 'Metallic', 'FLOAT')]:
        attribute_name = 'clay_' + key
        if attribute_name in obj.data.attributes:
            obj.data.attributes.remove(obj.data.attributes[attribute_name])
        attribute = obj.data.attributes.new(attribute_name, kind, 'POINT')
        value = data[key]
        for item in attribute.data:
            if kind == 'FLOAT_COLOR':
                item.color = (*value, 1)
                assert max(abs(a-b) for a,b in zip(item.color[:3], value)) < 1e-6
            else:
                item.value = value
                assert abs(item.value-value) < 1e-6
        node = tree.nodes.new('ShaderNodeAttribute')
        node.attribute_name = attribute_name
        tree.links.new(node.outputs['Color' if kind == 'FLOAT_COLOR' else 'Fac'], principled.inputs[socket])
    # Match the viewport's explicit constant studio fill, without an HDRI or bounces.
    ambient = 0.10 * (1-data['metallic']) + 0.16 * data['metallic']
    principled.inputs['Emission Color'].default_value = (*data['base_color'], 1)
    principled.inputs['Emission Strength'].default_value = ambient
    return material

def reference_srgb(data):
    # Same published GGX equations as the app, with a front-facing flat plate.
    light = Vector((-0.5, 0.7, 1)).normalized()
    halfway = (light + Vector((0, 0, 1))).normalized()
    nl, nh, vh = light.z, halfway.z, halfway.z
    a2 = max(data['roughness'], 0.045)**4
    distribution = a2 / (math.pi * (nh*nh*(a2-1)+1)**2)
    geometry = 2*nl / (nl + math.sqrt(a2+(1-a2)*nl*nl))
    result = []
    for base in data['base_color']:
        f0 = 0.04*(1-data['metallic']) + base*data['metallic']
        fresnel = f0 + (1-f0)*(1-vh)**5
        specular = distribution*geometry*fresnel/(4*nl)
        diffuse = (1-fresnel)*(1-data['metallic'])*base/math.pi
        ambient = base*(0.10*(1-data['metallic'])+0.16*data['metallic'])
        value = ambient+(diffuse+specular)*nl*2.5
        result.append(12.92*value if value <= 0.0031308 else 1.055*value**(1/2.4)-0.055)
    return result

samples = []
for index, (name, data) in enumerate(examples):
    material_for(plate, name, data)
    scene.render.filepath = str(out / f'blender-plate-{index}.png')
    bpy.ops.render.render(write_still=True)
    # Read saved Standard/sRGB pixels as raw channels for a reproducible comparison.
    image = bpy.data.images.load(scene.render.filepath, check_existing=False)
    image.colorspace_settings.name = 'Non-Color'
    pixel = list(image.pixels[(128*256+128)*4:(128*256+128)*4+3])
    reference = reference_srgb(data)
    error = max(abs(a-b) for a,b in zip(pixel, reference))
    assert error <= 4/255, (name, error, pixel, reference)
    samples.append({'name':name, 'srgb':pixel, 'material':data, 'clay_reference_srgb':reference, 'max_srgb_error':error})
    bpy.data.images.remove(image)
(out / 'blender-samples.json').write_text(json.dumps({'blender':bpy.app.version_string, 'samples':samples}, indent=2))
bpy.data.objects.remove(plate, do_unlink=True)
# Six real Principled materials in one reusable gallery fixture.
for index, (name, data) in enumerate(examples):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.8, location=((index%3-1)*2.2, (0.5-index//3)*2.2, 0))
    obj=bpy.context.object
    obj.name=name
    for polygon in obj.data.polygons: polygon.use_smooth=True
    material_for(obj, name, data)
camera.data.ortho_scale=7.4
scene.render.resolution_x=960
scene.render.resolution_y=640
scene.render.filepath=str(out/'blender-material-gallery.png')
bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=str(out/'material-fixture.blend'))

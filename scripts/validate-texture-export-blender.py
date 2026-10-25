"""Manual Blender UV/embedded image regression, after texture_blender_export_manual_review.
blender -b -noaudio --python-exit-code 1 --python scripts/validate-texture-export-blender.py -- /tmp/clay-texture-export
"""
import bpy
import json
import sys
from pathlib import Path

out = Path(sys.argv[sys.argv.index('--') + 1])
report = []
for name in ('png', 'exr'):
    bpy.ops.wm.open_mainfile(filepath=str(out / (name + '.blend')))
    data = json.loads((out / (name + '.json')).read_text())
    obj = next(o for o in bpy.data.objects if o.type == 'MESH')
    uv = obj.data.uv_layers.active
    assert uv and len(uv.data) == len(obj.data.loops)
    expected = [q for face in data['uv'] for q in face]
    assert all(max(abs(a-b) for a,b in zip(v.uv,q)) < 1e-6 for v,q in zip(uv.data, expected))
    mapped = obj.data.attributes['clay_uv_mapped']
    assert [bool(v.value) for v in mapped.data] == data['mapped']
    material = obj.data.materials[0]
    nodes = material.node_tree.nodes
    assert len(nodes) <= 8, len(nodes)
    assert sum(n.type == 'BSDF_PRINCIPLED' for n in nodes) == 1
    tex = next(n for n in nodes if n.type == 'TEX_IMAGE')
    assert tex.image.packed_file and list(tex.image.size) == [32, 32]
    assert not (out / data['textures'][0]['path']).exists(), 'Validate without companion images'
    assert tex.image.colorspace_settings.name == ('sRGB' if name == 'png' else 'Linear Rec.709')
    assert tex.image.alpha_mode == ('STRAIGHT' if name == 'png' else 'PREMUL')
    assert tex.image.is_float, 'Retain 16-bit PNG / float EXR precision'
    assert bytes(tex.image.packed_file.data).startswith(b'\x89PNG' if name == 'png' else b'v/1\x01')
    assert any(link.from_node == tex and link.from_socket.name == 'Alpha' for link in material.node_tree.links)
    # A fresh process reopened the packed image after the exporter deleted its
    # temporary directory. Rendering must work in preview as well as Cycles.
    scene = bpy.context.scene
    scene.render.resolution_x = scene.render.resolution_y = 192
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'
    for engine in ('BLENDER_EEVEE' if bpy.app.version >= (5,0,0) or bpy.app.version < (4,2,0) else 'BLENDER_EEVEE_NEXT', 'CYCLES'):
        scene.render.engine = engine
        if engine == 'CYCLES':
            scene.cycles.device = 'CPU'
            scene.cycles.samples = 4
            scene.cycles.use_denoising = False
        scene.render.filepath = str(out / f'{name}-{engine}.png')
        bpy.ops.render.render(write_still=True)
        image = bpy.data.images.load(scene.render.filepath, check_existing=False)
        image.colorspace_settings.name = 'Non-Color'
        pixels = list(image.pixels)
        pink = sum(pixels[i] > .9 and pixels[i+1] < .05 and pixels[i+2] > .9 for i in range(0,len(pixels),4))
        red = sum(pixels[i] > pixels[i+2]*1.4 and pixels[i] > .2 for i in range(0,len(pixels),4))
        blue = sum(pixels[i+2] > pixels[i]*1.04 and pixels[i+2] > .2 for i in range(0,len(pixels),4))
        assert pink < 10, pink
        assert red > 100 and blue > 100, (red,blue)
        report.append({'file':name,'engine':engine,'nodes':len(nodes),'pink':pink,'red':red,'blue':blue})
(out / 'validation.json').write_text(json.dumps(report, indent=2))
print('Packed PNG/float EXR, UVs, paint coverage and preview/Cycles renders validated', flush=True)

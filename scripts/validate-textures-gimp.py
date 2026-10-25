# Run with: gimp -n -i -d -f -s --batch-interpreter=python-fu-eval \
#   --batch="exec(open('scripts/validate-textures-gimp.py').read())" --quit
# Writes only deterministic /tmp/clay-gimp-* fixtures.
import struct, zlib
from pathlib import Path
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
raw = (b'\0' + struct.pack('>4H', 12345, 23456, 34567, 45678) * 32) * 32
Path('/tmp/clay-gimp-input.png').write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>2I5B', 32, 32, 16, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))
from gi.repository import Gimp, Gio, Gegl
image = Gimp.file_load(Gimp.RunMode.NONINTERACTIVE, Gio.File.new_for_path('/tmp/clay-gimp-input.png'))
assert image is not None
assert image.set_color_profile(Gimp.ColorProfile.new_rgb_srgb())
print('Input precision:', image.get_precision())
procedure = Gimp.get_pdb().lookup_procedure('file-png-export')
def export(path):
    config=procedure.create_config()
    config.set_property('run-mode',Gimp.RunMode.NONINTERACTIVE)
    config.set_property('image',image)
    config.set_property('include-color-profile',True)
    config.set_property('file',Gio.File.new_for_path(path))
    result=procedure.run(config)
    print('PNG export:',result.index(0))
    assert result.index(0)==Gimp.PDBStatusType.SUCCESS
export('/tmp/clay-gimp-data.png')
Gimp.context_set_foreground(Gegl.Color.new('rgba(0.8, 0.1, 0.2, 0.5)'))
assert image.get_layers()[0].fill(Gimp.FillType.FOREGROUND)
export('/tmp/clay-gimp-edited.png')
image.delete()

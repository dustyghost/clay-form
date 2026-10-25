# Run with GIMP python-fu-eval, as documented in docs/validation/uv-textures-2026-10-25.md.
import struct
from pathlib import Path
from gi.repository import Gimp, Gio
s=lambda x: x.encode()+b'\0'
def attr(name,kind,data): return s(name)+s(kind)+struct.pack('<I',len(data))+data
channels=b''.join(s(c)+struct.pack('<iB3xii',2,0,1,1) for c in ['A','B','G','R'])+b'\0'
header=struct.pack('<II',20000630,2)+attr('channels','chlist',channels)+attr('compression','compression',b'\0')+attr('dataWindow','box2i',struct.pack('<4i',0,0,31,31))+attr('displayWindow','box2i',struct.pack('<4i',0,0,31,31))+attr('lineOrder','lineOrder',b'\0')+attr('pixelAspectRatio','float',struct.pack('<f',1))+attr('screenWindowCenter','v2f',struct.pack('<2f',0,0))+attr('screenWindowWidth','float',struct.pack('<f',1))+b'\0'
row=b''.join(struct.pack('<f',v)*32 for v in [.5,.12345678,4.5,-.25])
start=len(header)+32*8
Path('/tmp/clay-gimp-float-input.exr').write_bytes(header+b''.join(struct.pack('<Q',start+y*(8+len(row))) for y in range(32))+b''.join(struct.pack('<ii',y,len(row))+row for y in range(32)))
image=Gimp.file_load(Gimp.RunMode.NONINTERACTIVE,Gio.File.new_for_path('/tmp/clay-gimp-float-input.exr'))
assert image is not None
print('EXR image precision:',image.get_precision())
procedure=Gimp.get_pdb().lookup_procedure('file-exr-export')
config=procedure.create_config()
config.set_property('run-mode',Gimp.RunMode.NONINTERACTIVE)
config.set_property('image',image)
config.set_property('file',Gio.File.new_for_path('/tmp/clay-gimp-float-return.exr'))
result=procedure.run(config)
print('EXR export:',result.index(0))
assert result.index(0)==Gimp.PDBStatusType.SUCCESS
image.delete()

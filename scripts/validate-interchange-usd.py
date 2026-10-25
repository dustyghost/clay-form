"""Validate the manual interchange fixture with Pixar's OpenUSD bindings.
python scripts/validate-interchange-usd.py /tmp/clay-interchange-review
"""
import json
import sys
from pathlib import Path
from pxr import Usd, UsdGeom, UsdShade

directory = Path(sys.argv[1]).resolve()
source = json.loads((directory / 'source.clay').read_text())['mesh']
stage = Usd.Stage.Open(str(directory / 'model.usda'))
assert stage and stage.GetDefaultPrim().GetPath() == '/Model'
assert UsdGeom.GetStageUpAxis(stage) == 'Y'
assert UsdGeom.GetStageMetersPerUnit(stage) == 1
mesh = UsdGeom.Mesh(stage.GetPrimAtPath('/Model/Mesh'))
assert list(mesh.GetFaceVertexIndicesAttr().Get()) == [i for t in source['triangles'] for i in t]
for actual, expected in zip(mesh.GetPointsAttr().Get(), source['vertices']):
    assert max(abs(a-b) for a, b in zip(actual, expected)) < 1e-6
assert mesh.GetNormalsInterpolation() == 'faceVarying'
assert len(mesh.GetNormalsAttr().Get()) == len(source['triangles']) * 3
primvars = UsdGeom.PrimvarsAPI(mesh)
uv = primvars.GetPrimvar('st')
assert uv.GetInterpolation() == 'faceVarying'
for f, face in enumerate(source['uv']['corners']):
    for c, expected in enumerate(face):
        expected = (0, 0) if source['uv']['removed'][f] else expected
        assert max(abs(a-b) for a, b in zip(uv.Get()[f*3+c], expected)) < 1e-6
assert list(primvars.GetPrimvar('clay:uvMapped').Get()) == [not v for v in source['uv']['removed']]
assert list(primvars.GetPrimvar('clay:uvMask').Get()) == [v for f in source['uv']['masks'] for v in f]
binding = UsdShade.MaterialBindingAPI(mesh)
assert binding.ComputeBoundMaterial()[0].GetPath() == '/Model/Materials/Base'
subset = stage.GetPrimAtPath('/Model/Mesh/Painted')
assert UsdShade.MaterialBindingAPI(subset).ComputeBoundMaterial()[0].GetPath() == '/Model/Materials/Painted'
assert list(subset.GetAttribute('indices').Get()) == [i for i, v in enumerate(source['uv']['removed']) if not v]
textures = []
for prim in stage.Traverse():
    attr = prim.GetAttribute('inputs:file')
    if attr:
        asset = attr.Get()
        assert not Path(asset.path).is_absolute()
        assert Path(asset.resolvedPath).is_file(), asset
        textures.append(asset.path)
assert len(textures) == 3 and any(p.endswith('color.exr') for p in textures)
print('OpenUSD geometry, axes, UVs, masks, bindings and relative textures validated')

import hashlib, importlib.util, json, sys
from pathlib import Path
from types import SimpleNamespace
spec=importlib.util.spec_from_file_location('capacity','scripts/benchmark-capacity.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
label=sys.argv[1]
assert label in ['before','after']
fixture=sys.argv[2] if len(sys.argv)>2 else 'organic'
assert fixture in ['organic','hidden']
prefix='' if fixture=='organic' else 'hidden-'
exe=Path('target/display-reuse/baseline-test' if label=='before' else 'target/release/deps/clay_form-28c669c6ca701e80').resolve()
original=json.loads(Path(f'target/display-reuse/gpu-{label}/environment.json').read_text())
metadata=m.metadata
def from_binary(args):
 info=metadata(args)
 info['checkout_source_sha256']=info['source_sha256']
 info['source_sha256']=original['source_sha256']
 info['revision']=original['revision']
 info['binary_sha256']=hashlib.sha256(exe.read_bytes()).hexdigest()
 info['binary_origin']=f'Saved optimized test executable for gpu-{label}; no rebuild in this run.'
 return info
m.metadata=from_binary;m.build=lambda:str(exe)
args=SimpleNamespace(output=Path(f'target/display-reuse/{prefix}repeat-{label}'),fixtures=[fixture],levels=[4],repeats=3,gpu=True,workload='interactive',mesh_vertices=2000000,mesh_triangles=4000000,max_rss_mib=3072,min_available_mib=768,timeout=600)
raise SystemExit(m.run(args))

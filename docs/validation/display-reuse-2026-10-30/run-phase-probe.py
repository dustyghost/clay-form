import importlib.util,json,os,sys
from pathlib import Path
from types import SimpleNamespace
s=importlib.util.spec_from_file_location('capacity','scripts/benchmark-capacity.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=False)
a=SimpleNamespace(gpu=False,max_rss_mib=3072,min_available_mib=768,timeout=600)
(out/'environment.json').write_text(json.dumps(m.metadata(a),indent=2)+'\n')
exe=m.build();m.TEST='viewport::gpu::profile::capacity::gpu_load_profile_display_stages'
e=os.environ.copy();e.update(CLAY_DISPLAY_PROFILE=str((out/'profile.json').resolve()),CLAY_DISPLAY_LEVEL='3')
r=m.bounded_run(exe,e,out/'profile.log',a);print(r,flush=True);assert r['status']=='measured'

"""Local comparison only; run with Blender --background --factory-startup -t 1.

Arguments after --: source.clay output-directory target [refine-belly].
The refinement inserts coplanar triangle centroids without moving the surface.
No Blender dependency is added to Clay Form.
"""
import collections
import copy
import gc
import hashlib
import json
from pathlib import Path
import resource
import sys
import time

import bpy

args = sys.argv[sys.argv.index("--") + 1:]
source_path, output_path, target_arg = args[:3]
refine = len(args) > 3 and args[3] == "refine-belly"
output = Path(output_path)
output.mkdir(parents=True, exist_ok=True)
target = int(target_arg)
tag = f"quadriflow-{target}-{'refined' if refine else 'original'}"
if Path(source_path).resolve() == (output / (tag + ".clay")).resolve():
    raise ValueError("Choose a separate output directory; the source must not be overwritten")
source_bytes = Path(source_path).read_bytes()
source_hash = hashlib.sha256(source_bytes).hexdigest()
project = json.loads(source_bytes)
del source_bytes
source = project.pop("mesh")
vertices = source["vertices"]
triangles = source["triangles"]
if refine:
    refined = []
    for a, b, c in triangles:
        center = [(vertices[a][k] + vertices[b][k] + vertices[c][k]) / 3 for k in range(3)]
        if abs(center[0]) < .4 and abs(center[1]) < .4 and center[2] > 0:
            children = [(a, b, c)]
            for _ in range(2):
                next_children = []
                for x, y, z in children:
                    center = [(vertices[x][k] + vertices[y][k] + vertices[z][k]) / 3 for k in range(3)]
                    v = len(vertices)
                    vertices.append(center)
                    next_children.extend(((x, y, v), (y, z, v), (z, x, v)))
                children = next_children
            refined.extend(children)
        else:
            refined.append((a, b, c))
    triangles = refined
counts = {"vertices": len(vertices), "triangles": len(triangles)}
for obj in list(bpy.data.objects):
    bpy.data.objects.remove(obj, do_unlink=True)
mesh = bpy.data.meshes.new("Source surface")
mesh.from_pydata(vertices, [], triangles)
mesh.update()
obj = bpy.data.objects.new("Retopology comparison", mesh)
bpy.context.collection.objects.link(obj)
bpy.context.view_layer.objects.active = obj
obj.select_set(True)
del source, vertices, triangles
if refine:
    del refined
gc.collect()
print("START", json.dumps({"source": counts, "target": target, "refined": refine}), flush=True)
started = time.perf_counter()
settings = dict(use_mesh_symmetry=False, use_preserve_sharp=False,
                use_preserve_boundary=True, preserve_attributes=False,
                smooth_normals=False, mode="FACES", target_faces=target, seed=0)
status = bpy.ops.object.quadriflow_remesh(**settings)
elapsed = time.perf_counter() - started
if status != {"FINISHED"}:
    raise RuntimeError(f"QuadriFlow did not finish: {status}")
result = obj.data
vertices = [list(v.co) for v in result.vertices]
faces = [list(f.vertices) for f in result.polygons]
edges = collections.Counter()
winding = collections.Counter()
neighbours = [set() for _ in vertices]
for face in faces:
    for a, b in zip(face, face[1:] + face[:1]):
        edge = tuple(sorted((a, b)))
        edges[edge] += 1
        winding[edge] += 1 if a < b else -1
        neighbours[a].add(b)
        neighbours[b].add(a)
unused = set(range(len(vertices)))
components = 0
while unused:
    todo = [unused.pop()]
    components += 1
    while todo:
        for v in neighbours[todo.pop()]:
            if v in unused:
                unused.remove(v)
                todo.append(v)
degrees = collections.Counter(v for edge in edges for v in edge)
stats = dict(source_sha256=source_hash, blender_version=bpy.app.version_string,
             blender_build_hash=bpy.app.build_hash.decode(), source=counts,
             refinement="9 coplanar children per belly triangle (centroid abs(x/y)<0.4, z>0)" if refine else "none",
             settings=settings, elapsed_seconds=elapsed,
             max_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
             vertices=len(vertices), polygons=len(faces),
             face_sizes=dict(collections.Counter(map(len, faces))),
             belly_vertices=sum(abs(x) < .4 and abs(y) < .4 and z > 0 for x, y, z in vertices),
             components=components, euler=len(vertices) - len(edges) + len(faces),
             inconsistent_or_open_edges=sum(count != 2 or winding[e] != 0 for e, count in edges.items()),
             edge_incidence=dict(collections.Counter(edges.values())),
             vertex_valence=dict(collections.Counter(degrees.values())))
(output / (tag + ".json")).write_text(json.dumps(stats, indent=2) + "\n")
with (output / (tag + ".obj")).open("w") as f:
    for v in vertices:
        f.write("v " + " ".join(format(x, ".9g") for x in v) + "\n")
    for face in faces:
        f.write("f " + " ".join(str(v + 1) for v in face) + "\n")
triangles = [[face[0], face[i], face[i + 1]] for face in faces for i in range(1, len(face) - 1)]
project["mesh"] = dict(vertices=vertices, triangles=triangles,
                       quads=[face for face in faces if len(face) == 4],
                       mask=[0.0] * len(vertices), vertex_groups=[],
                       polygroups=[0] * len(triangles))
project["mesh_settings"] = copy.deepcopy(project["mesh_settings"])
project["mesh_settings"].update(lower=[], upper=[], dynamic=False)
project["mesh_settings"]["adaptive"]["topology"] = "Quads"
(output / (tag + ".clay")).write_text(json.dumps(project, separators=(",", ":")))
print("RESULT", json.dumps(stats), flush=True)

"""Create a minimal audit repair seed from every inverted cell in a saved graph.

Usage: make-inversion-seed.py graph.json NEW-patch.json
No geometry labels or coordinate regions select the cells. Include every face
incident to a flexible edge, retaining all other values and real locks.
"""
import json
from pathlib import Path
import sys

graph = json.loads(Path(sys.argv[1]).read_text())
if any(abs(v) > 1 for xy in graph["offsets"] for v in xy):
    raise ValueError("Seed export requires subdivided unit grid offsets")


def rotate(v, r):
    x, y = v
    return ((x, y), (-y, x), (-x, -y), (y, -x))[r % 4]


inverted = []
for f, (edges, rotations) in enumerate(zip(graph["faces"], graph["rotations"])):
    a, b = [rotate(graph["offsets"][edges[k]], rotations[k]) for k in (0, 1)]
    if a[0] * b[1] - a[1] * b[0] < 0:
        inverted.append(f)
flexible = {e for f in inverted for e in graph["faces"][f]
            if graph["allow"][e] != [0, 0]
            and graph["edge_faces"][e][0] != graph["edge_faces"][e][1]}
faces = sorted({f for e in flexible for f in graph["edge_faces"][e] if f < len(graph["faces"])})
edges = sorted({e for f in faces for e in graph["faces"][f]})
ids = {e: i for i, e in enumerate(edges)}
domains = [sum(1 << v for v in range(9)
               if all((e in flexible and graph["allow"][e][a] != 0)
                      or (v // 3 - 1, v % 3 - 1)[a] == graph["offsets"][e][a] for a in (0, 1)))
           for e in edges]
patch = dict(version=1, depth=0, radius=0, source_faces=[graph["source_faces"][f] for f in faces],
             graph_edges=edges, faces=[[ids[e] for e in graph["faces"][f]] for f in faces],
             rotations=[graph["rotations"][f] for f in faces], domains=domains,
             preferred=[(graph["offsets"][e][0] + 1) * 3 + graph["offsets"][e][1] + 1 for e in edges],
             inversions=len(inverted))
with Path(sys.argv[2]).open("x") as stream:
    json.dump(patch, stream)
print(f"{len(inverted)} inversions, {len(flexible)} flexible edges, {len(faces)} incident faces")

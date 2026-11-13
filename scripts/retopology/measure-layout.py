#!/usr/bin/env python3
"""Geometry-independent quad metrics and strict opposite-edge closed cycles.

Usage: measure-layout.py candidate.audit.json metrics.json
       measure-layout.py --self-test
No anatomical regions, guide directions or world-axis filters are used.
These finite diagnostics do not certify surface accuracy or self-intersections.
"""
import json
import math
import sys
from pathlib import Path


def closed_cycles(quads):
    corners = {}
    edges = set()
    for face in quads:
        for k, v in enumerate(face):
            a, b = face[k - 1], face[(k + 1) % 4]
            ring = corners.setdefault(v, {})
            ring.setdefault(a, set()).add(b)
            ring.setdefault(b, set()).add(a)
            edges.add(tuple(sorted((v, b))))
    opposite = {}
    for v, ring in corners.items():
        if len(ring) != 4 or any(len(adj) != 2 for adj in ring.values()):
            continue
        for a, adjacent in ring.items():
            other = set(ring) - {a} - adjacent
            if len(other) == 1:
                opposite[(a, v)] = next(iter(other))
    links = {e: set() for e in edges}
    for (a, v), b in opposite.items():
        links[tuple(sorted((a, v)))].add(tuple(sorted((v, b))))
    seen = set()
    result = []
    for start in sorted(edges):
        if start in seen:
            continue
        component = set()
        todo = [start]
        while todo:
            edge = todo.pop()
            if edge in component:
                continue
            component.add(edge)
            todo.extend(links[edge] - component)
        seen.update(component)
        if not all(len(links[e]) == 2 for e in component):
            continue
        a, b = start
        vertices = []
        for _ in component:
            vertices.append(a)
            a, b = b, opposite[(a, b)]
        assert (a, b) == start
        result.append(vertices)
    return result


def measure(mesh):
    positions, quads = mesh["positions"], mesh["quads"]
    ratios = []
    for face in quads:
        lengths = [math.dist(positions[a], positions[b])
                   for a, b in zip(face, face[1:] + face[:1])]
        if min(lengths) <= 0:
            raise ValueError("Degenerate quad edge")
        ratios.append(max(lengths) / min(lengths))
    ordered = sorted(ratios)
    index = 0.95 * (len(ordered) - 1)
    low = int(index)
    high = math.ceil(index)
    p95 = ordered[low] * (high - index) + ordered[high] * (index - low) if high != low else ordered[low]
    loops = closed_cycles(quads)
    return {
        "quads": len(quads), "vertices": len(positions),
        "closed_edge_loops_total": len(loops),
        "simple_closed_edge_loops": sum(len(set(c)) == len(c) for c in loops),
        "closed_loops": [{"vertices": c, "center": [sum(positions[v][axis] for v in c) / len(c)
                          for axis in range(3)]} for c in loops],
        "edge_ratio_max": max(ratios), "edge_ratio_p95": p95,
        "quality": mesh.get("quality"),
    }


def self_test():
    count, rings = 16, 12
    for closed in [False, True]:
        quads = []
        for row in range(rings if closed else rings - 1):
            following = (row + 1) % rings
            for i in range(count):
                j = (i + 1) % count
                quads.append([row * count + i, row * count + j,
                              following * count + j, following * count + i])
        cycles = closed_cycles(quads)
        # Open ends stop longitudinal traces; a toroidal grid closes both axes.
        assert len(cycles) == (rings + count if closed else rings - 2)
        edges = {tuple(sorted((a, b))) for f in quads for a, b in zip(f, f[1:] + f[:1])}
        assert all(tuple(sorted((a, b))) in edges for c in cycles for a, b in zip(c, c[1:] + c[:1]))
    print("Cylinder and toroidal-grid loop checks passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    else:
        source, output = map(Path, sys.argv[1:])
        report = measure(json.loads(source.read_text()))
        with output.open("x") as file:
            json.dump(report, file, indent=2)
            file.write("\n")
        print({k: v for k, v in report.items() if k not in ("closed_loops", "quality")})

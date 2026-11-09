#!/usr/bin/env python3
"""Offline MeshLab preconditioning comparison; never changes the source file.

Run with isolated PyMeshLab 2025.7.post1, one thread and the shared build lock.
This is a benchmark dependency, not an application backend.
"""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
import resource
import time

import numpy as np
import pymeshlab as ml


def metrics(vertices, faces):
    points = vertices[faces]
    edges = np.stack((points[:, 1] - points[:, 0], points[:, 2] - points[:, 1],
                      points[:, 0] - points[:, 2]), axis=1)
    lengths = np.linalg.norm(edges, axis=2)
    areas = np.linalg.norm(np.cross(edges[:, 0], -edges[:, 2]), axis=1) / 2
    quality = 4 * np.sqrt(3) * areas / (lengths * lengths).sum(axis=1)
    unique_edges, counts = np.unique(np.sort(np.concatenate(
        (faces[:, [0, 1]], faces[:, [1, 2]], faces[:, [2, 0]])), axis=1),
        axis=0, return_counts=True)
    return {
        'vertices': len(vertices), 'triangles': len(faces), 'area': float(areas.sum()),
        'euler': len(vertices) - len(unique_edges) + len(faces),
        'boundary_edges': int((counts == 1).sum()),
        'edges_over_two_faces': int((counts > 2).sum()),
        'zero_area_faces': int((areas <= 1e-15).sum()),
        'triangle_quality_p05_p50_p95': np.percentile(quality, [5, 50, 95]).tolist(),
        'edge_length_p05_p50_p95': np.percentile(lengths, [5, 50, 95]).tolist(),
        'longest_shortest_edge_ratio_p50_p95_max': [float(np.percentile(
            lengths.max(axis=1) / lengths.min(axis=1), p)) for p in [50, 95, 100]],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--target', type=int, default=10000)
    parser.add_argument('--edge-factor', type=float, default=.5)
    parser.add_argument('--surface-factor', type=float, default=.05)
    args = parser.parse_args()
    if args.target < 4 or not 0 < args.edge_factor <= 1 or not 0 < args.surface_factor <= .15:
        parser.error('target must be at least 4; edge-factor in (0,1], surface-factor in (0,.15]')
    args.output.mkdir(parents=True, exist_ok=False)
    started = time.perf_counter()
    source_bytes = args.source.read_bytes()
    doc = json.loads(source_bytes)
    vertices = np.asarray(doc['mesh']['vertices'], dtype=np.float64)
    faces = np.asarray(doc['mesh']['triangles'], dtype=np.int32)
    before = metrics(vertices, faces)
    h = np.sqrt(before['area'] / args.target)
    ms = ml.MeshSet()
    ms.add_mesh(ml.Mesh(vertex_matrix=vertices, face_matrix=faces), 'original')
    original_topology = ms.get_topological_measures()
    ms.add_mesh(ml.Mesh(vertex_matrix=vertices, face_matrix=faces), 'working')
    parameters = dict(iterations=5, adaptive=False, selectedonly=False,
                      featuredeg=180., checksurfdist=True, splitflag=True,
                      collapseflag=True, swapflag=True, smoothflag=True,
                      reprojectflag=True)
    print('START', json.dumps({'spacing': h, 'edge_target': h * args.edge_factor,
                              'max_local_surface_distance': h * args.surface_factor}), flush=True)
    ms.meshing_isotropic_explicit_remeshing(
        **parameters, targetlen=ml.PureValue(h * args.edge_factor), maxsurfdist=ml.PureValue(h * args.surface_factor))
    out_vertices = ms.current_mesh().vertex_matrix().copy()
    out_faces = ms.current_mesh().face_matrix().copy()
    after = metrics(out_vertices, out_faces)
    output_topology = ms.get_topological_measures()
    distances = {}
    for name, points, target in [
        ('original_vertices_to_working', vertices, 1),
        ('working_vertices_to_original', out_vertices, 0),
        ('original_centroids_to_working', vertices[faces].mean(axis=1), 1),
        ('working_centroids_to_original', out_vertices[out_faces].mean(axis=1), 0),
    ]:
        ms.add_mesh(ml.Mesh(vertex_matrix=points), name)
        distances[name] = ms.get_hausdorff_distance(
            sampledmesh=ms.current_mesh_id(), targetmesh=target, savesample=False,
            samplevert=True, sampleedge=False, sampleface=False, samplenum=len(points))
        ms.delete_current_mesh()
    record = dict(source=str(args.source.resolve()), source_sha256=hashlib.sha256(source_bytes).hexdigest(),
                  pymeshlab=importlib.metadata.version('pymeshlab'), target_quads=args.target,
                  original_spacing=float(h), parameters=parameters,
                  edge_target=float(h * args.edge_factor), max_local_surface_distance=float(h * args.surface_factor),
                  before=before, after=after, original_topology=original_topology,
                  working_topology=output_topology, sampled_distances=distances,
                  elapsed_seconds=time.perf_counter() - started,
                  peak_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                  note='Vertex and face-centroid samples in both directions; not a continuous Hausdorff or self-intersection certificate.')
    (args.output / 'preconditioning.json').write_text(json.dumps(record, indent=2) + '\n')
    assert after['boundary_edges'] == before['boundary_edges'] == 0
    assert after['edges_over_two_faces'] == after['zero_area_faces'] == 0
    assert after['euler'] == before['euler']
    assert output_topology['is_mesh_two_manifold']
    assert output_topology['connected_components_number'] == original_topology['connected_components_number']
    assert after['triangle_quality_p05_p50_p95'][0] > before['triangle_quality_p05_p50_p95'][0]
    assert max(float(d['max']) for d in distances.values()) < .15 * h
    doc['mesh'] = dict(vertices=out_vertices.tolist(), triangles=out_faces.tolist(),
                       quads=[], mask=[0.] * len(out_vertices), vertex_groups=[],
                       polygroups=[0] * len(out_faces))
    with (args.output / 'even.clay').open('x') as f:
        json.dump(doc, f, separators=(',', ':'))
    print('RESULT', json.dumps(record), flush=True)


if __name__ == '__main__':
    main()

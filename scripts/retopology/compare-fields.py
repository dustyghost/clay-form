#!/usr/bin/env python3
"""Swap pinned reference fields into Rust, retaining common fine geometry.

Run under the shared lock and a memory/no-swap cgroup. This tests unconstrained
uniform fields, not complete upstream-pipeline parity. Rejected fitting uses
the complete pre-projection candidate for geometric measurements.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('reference_solver', type=Path)
    parser.add_argument('original_snapshot', type=Path)
    parser.add_argument('output', type=Path)
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument('--native-only', action='store_true',
                        help='If field snapshots are byte-identical, run the shared downstream pipeline once.')
    selection.add_argument('--reference-only', action='store_true',
                           help='Evaluate a new reference-field variant without repeating an unchanged native baseline.')
    args = parser.parse_args()
    for name in ('source', 'reference_solver', 'original_snapshot', 'output'):
        setattr(args, name, getattr(args, name).resolve())
    args.output.mkdir(exist_ok=False)
    repo = Path(__file__).resolve().parents[2]
    audit = repo / 'crates/clay-retopology/target/debug/examples/audit'
    record = {'target_quads': 10000, 'guides': 'none', 'detail': 'uniform',
              'files': {name: {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
                        for name, path in [('source', args.source), ('audit', audit),
                                           ('reference_solver', args.reference_solver),
                                           ('original_snapshot', args.original_snapshot)]},
              'runs': [], 'results': {}}

    def save():
        (args.output / 'runs.json').write_text(json.dumps(record, indent=2) + '\n')

    def run(name, command, limit):
        command = [str(c) for c in command]
        print('START', name, flush=True)
        start = time.perf_counter()
        timed_out = False
        with (args.output / (name + '.log')).open('x') as log:
            child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                     start_new_session=True, cwd=repo)
            try:
                child.wait(timeout=limit)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
        record['runs'].append(dict(name=name, command=command, exit=child.returncode,
                                  timed_out=timed_out, seconds=time.perf_counter() - start,
                                  limit_seconds=limit,
                                  cumulative_child_peak_rss_kib=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss))
        save()
        print('DONE', name, child.returncode, round(record['runs'][-1]['seconds'], 3), flush=True)
        return child.returncode == 0 and not timed_out

    native = args.output / 'native.fields.json'
    shared = args.output / 'shared.input'
    reference = args.output / 'reference.output'
    imported = args.output / 'reference.fields.json'
    for name, command in [
        ('capture', [audit, 'capture-field', args.source, native, 10000]),
        ('export', [audit, 'export-field', native, shared]),
        ('reference', [args.reference_solver, shared, reference]),
        ('import', [audit, 'import-field', native, reference, imported]),
    ]:
        if not run(name, command, 60):
            raise SystemExit('Field preparation failed: ' + name)

    a = json.loads(native.read_text())
    b = json.loads(imported.read_text())
    identity = {'source_equal': a['source'] == b['source'],
                'field_arrays_equal': {k: a['field'][k] == b['field'][k]
                                       for k in a['field'] if k not in ('directions', 'origins')}}
    record['input_identity'] = identity
    record['field_parity'] = {
        'directions_equal': a['field']['directions'] == b['field']['directions'],
        'origins_equal': a['field']['origins'] == b['field']['origins'],
        'snapshot_bytes_equal': native.read_bytes() == imported.read_bytes(),
        'maximum_direction_distance': max(math.dist(x, y) for x, y in zip(a['field']['directions'], b['field']['directions'])),
        'maximum_origin_distance': max(math.dist(x, y) for x, y in zip(a['field']['origins'], b['field']['origins'])),
    }
    save()
    if not identity['source_equal'] or not all(identity['field_arrays_equal'].values()):
        raise SystemExit('Field swap changed shared input; comparison invalid')
    if args.native_only and not record['field_parity']['snapshot_bytes_equal']:
        raise SystemExit('Native-only run requires byte-identical field snapshots')
    # Fourfold directions: sign and quarter-turn choices describe the same cross.
    angles = []
    for q, r, n in zip(a['field']['directions'], b['field']['directions'], a['field']['normals']):
        tangent = [n[1]*q[2]-n[2]*q[1], n[2]*q[0]-n[0]*q[2], n[0]*q[1]-n[1]*q[0]]
        agreement = max(abs(sum(x*y for x, y in zip(q, r))),
                        abs(sum(x*y for x, y in zip(tangent, r))))
        angles.append(math.degrees(math.acos(min(1., agreement))))
    angles.sort()
    record['direction_difference_degrees'] = {name: angles[round((len(angles)-1)*fraction)]
                                               for name, fraction in [('median', .5), ('p95', .95), ('maximum', 1.)]}
    record['sampled_vertices'] = len(a['field']['normals'])
    record['sampled_triangles'] = len(a['field']['surface']['triangles'])
    record['spacing'] = a['field']['spacing'][0]
    del a, b, angles
    save()

    cases = ([('native', native)] if args.native_only else [('reference', imported)]
             if args.reference_only else [('native', native), ('reference', imported)])
    record['downstream_cases'] = [name for name, _ in cases]
    for name, fields in cases:
        raw = args.output / (name + '.raw.json')
        trace = args.output / (name + '-repair')
        if not run(name + '-integerize', [audit, 'integerize-field', fields, raw], 150):
            record['results'][name] = {'status': 'integerize_failed'}
            save()
            continue
        run(name + '-repair', [audit, 'repair-trace', raw, trace, 60], 75)
        completion_path = trace / 'completion.json'
        completion = json.loads(completion_path.read_text()) if completion_path.exists() else {}
        if not completion.get('repaired'):
            record['results'][name] = {'status': 'repair_failed', 'repair': completion}
            save()
            continue
        snapshot = trace / 'attempted.json'
        passed = run(name + '-resume', [audit, 'resume', snapshot, args.output / (name + '.accepted.obj')], 40)
        report_path = args.output / (name + '.audit.json')
        if not run(name + '-candidate', [audit, 'diagnose-candidate', snapshot, report_path], 40):
            record['results'][name] = {'status': 'candidate_diagnostic_failed', 'passed_own_source_checks': passed}
            save()
            continue
        report = json.loads(report_path.read_text())
        positions = report['positions'] if passed else report['before_surface_fit']['positions']
        mesh = dict(positions=positions, quads=report['quads'], accepted=passed,
                    candidate_stage='projected' if passed else 'before_surface_projection')
        mesh_path = args.output / (name + '.mesh.json')
        mesh_path.write_text(json.dumps(mesh))
        obj = args.output / (name + '.candidate.obj')
        with obj.open('x') as out:
            for p in positions:
                out.write('v ' + ' '.join(map(str, p)) + '\n')
            for q in report['quads']:
                out.write('f ' + ' '.join(str(v + 1) for v in q) + '\n')
        quality_path = args.output / (name + '.quality.json')
        if not run(name + '-measure', [audit, 'diagnose-obj', args.original_snapshot, obj, quality_path], 40):
            raise SystemExit('Candidate measurement failed: ' + name)
        passed_original = run(name + '-validate', [audit, 'validate-obj', args.original_snapshot, obj], 40)
        mesh['accepted'] = passed and passed_original
        mesh_path.write_text(json.dumps(mesh))
        run(name + '-layout', [sys.executable, repo / 'scripts/retopology/measure-layout.py',
                              mesh_path, args.output / (name + '.layout.json')], 30)
        record['results'][name] = {'status': 'passes_original_checks' if passed and passed_original else 'geometry_rejected',
                                  'passed_own_source_checks': passed, 'passed_original_checks': passed_original,
                                  'repair': completion, 'fit_error': report['surface_fit_error'],
                                  'quality_against_original': json.loads(quality_path.read_text())}
        save()


if __name__ == '__main__':
    main()

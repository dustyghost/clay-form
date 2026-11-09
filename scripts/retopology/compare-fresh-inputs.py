#!/usr/bin/env python3
"""Bounded, serial original/even-input comparison of Rust and Blender QuadriFlow.

Run under the shared build lock and a memory/no-swap cgroup. Rejected native
outputs use the full pre-projection candidate, never a partly projected mesh.
All four outputs are measured against the untouched original sculpt.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('original', type=Path)
    parser.add_argument('even', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.original = args.original.resolve()
    args.even = args.even.resolve()
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    repo = Path(__file__).resolve().parents[2]
    audit = repo / 'crates/clay-retopology/target/debug/examples/audit'
    record = {'target_quads': 10000, 'guides': 'none', 'detail': 'automatic uniform',
              'audit_sha256': hashlib.sha256(audit.read_bytes()).hexdigest(),
              'inputs': {k: {'path': str(p.resolve()), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
                         for k, p in [('original', args.original), ('even', args.even)]},
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
        item = dict(name=name, command=command, exit=child.returncode, timed_out=timed_out,
                    seconds=time.perf_counter() - start, limit_seconds=limit,
                    cumulative_child_peak_rss_kib=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
        record['runs'].append(item)
        save()
        print('DONE', json.dumps(item), flush=True)
        return child.returncode == 0 and not timed_out

    reference = args.output / 'native-original.raw.json'
    for name, source in [('original', args.original), ('even', args.even)]:
        raw = args.output / ('native-' + name + '.raw.json')
        trace = args.output / ('native-' + name + '-repair')
        key = 'native-' + name
        if not run(key + '-capture', [audit, 'capture-raw', source, raw, 10000], 150):
            record['results'][key] = {'status': 'capture_failed'}
            save()
            continue
        run(key + '-repair', [audit, 'repair-trace', raw, trace, 60], 75)
        completion_path = trace / 'completion.json'
        completion = json.loads(completion_path.read_text()) if completion_path.exists() else {}
        if not completion.get('repaired'):
            record['results'][key] = {'status': 'repair_failed', 'repair': completion}
            save()
            continue
        snapshot = trace / 'attempted.json'
        accepted = args.output / (key + '.accepted.obj')
        passed = run(key + '-resume', [audit, 'resume', snapshot, accepted], 40)
        report_path = args.output / (key + '.audit.json')
        if not run(key + '-candidate', [audit, 'diagnose-candidate', snapshot, report_path], 40):
            record['results'][key] = {'status': 'candidate_diagnostic_failed', 'passed_own_source_checks': passed}
            save()
            continue
        report = json.loads(report_path.read_text())
        positions = report['positions'] if passed else report['before_surface_fit']['positions']
        mesh = dict(positions=positions, quads=report['quads'], accepted=passed,
                    candidate_stage='projected' if passed else 'before_surface_projection')
        mesh_path = args.output / (key + '.mesh.json')
        mesh_path.write_text(json.dumps(mesh))
        obj = args.output / (key + '.candidate.obj')
        with obj.open('x') as f:
            for p in positions: f.write('v ' + ' '.join(map(str, p)) + '\n')
            for q in report['quads']: f.write('f ' + ' '.join(str(v + 1) for v in q) + '\n')
        quality_path = args.output / (key + '.quality.json')
        run(key + '-measure-original', [audit, 'diagnose-obj', reference, obj, quality_path], 40)
        passed_original = run(key + '-validate-original', [audit, 'validate-obj', reference, obj], 40)
        mesh['accepted'] = passed and passed_original
        mesh_path.write_text(json.dumps(mesh))
        run(key + '-layout', [sys.executable, repo / 'scripts/retopology/measure-layout.py',
                             mesh_path, args.output / (key + '.layout.json')], 30)
        record['results'][key] = {'status': 'passes_original_checks' if passed and passed_original else 'geometry_rejected',
                                  'passed_own_source_checks': passed, 'passed_original_checks': passed_original,
                                  'repair': completion, 'fit_error': report['surface_fit_error'],
                                  'quality_against_original': json.loads(quality_path.read_text())}
        save()

    for name, source in [('original', args.original), ('even', args.even)]:
        key = 'upstream-' + name
        directory = args.output / key
        directory.mkdir()
        if not run(key, ['/usr/bin/blender', '--background', '--factory-startup', '-t', 1,
                         '--python', repo / 'scripts/compare-retopology-blender.py', '--',
                         source, directory, 10000], 120):
            record['results'][key] = {'status': 'reference_failed'}
            save()
            continue
        obj = directory / 'quadriflow-10000-original.obj'
        quality_path = args.output / (key + '.quality.json')
        run(key + '-measure-original', [audit, 'diagnose-obj', reference, obj, quality_path], 40)
        passed = run(key + '-validate-original', [audit, 'validate-obj', reference, obj], 40)
        positions, quads = [], []
        for line in obj.read_text().splitlines():
            fields = line.split()
            if fields and fields[0] == 'v': positions.append(list(map(float, fields[1:])))
            elif fields and fields[0] == 'f': quads.append([int(v) - 1 for v in fields[1:]])
        mesh_path = args.output / (key + '.mesh.json')
        mesh_path.write_text(json.dumps(dict(positions=positions, quads=quads, accepted=passed,
                                            candidate_stage='upstream_output')))
        run(key + '-layout', [sys.executable, repo / 'scripts/retopology/measure-layout.py',
                             mesh_path, args.output / (key + '.layout.json')], 30)
        record['results'][key] = {'status': 'passes_original_checks' if passed else 'geometry_rejected',
                                  'quality_against_original': json.loads(quality_path.read_text())}
        save()


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Evaluate a corrected reference integer grid with unchanged native downstream stages.

Run under the shared build lock and a memory/no-swap cgroup. A failed fit is
measured using the complete pre-fit candidate, never a partially modified fit.
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
    parser.add_argument('fields', type=Path)
    parser.add_argument('grid', type=Path)
    parser.add_argument('original_snapshot', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    for key in ('fields', 'grid', 'original_snapshot', 'output'):
        setattr(args, key, getattr(args, key).resolve())
    args.output.mkdir(exist_ok=False)
    repo = Path(__file__).resolve().parents[2]
    audit = repo / 'crates/clay-retopology/target/debug/examples/audit'
    record = {'stage': 'corrected_integer_grid_before_subdivision',
              'files': {key: {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
                        for key, path in [('fields', args.fields), ('grid', args.grid), ('audit', audit),
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

    name = 'reference'
    raw = args.output / (name + '.raw.json')
    trace = args.output / (name + '-repair')
    if not run(name + '-import', [audit, 'import-integer-grid', args.fields, args.grid, raw], 60):
        raise SystemExit('Reference integer import failed')
    run(name + '-repair', [audit, 'repair-trace', raw, trace, 60], 75)
    completion_path = trace / 'completion.json'
    completion = json.loads(completion_path.read_text()) if completion_path.exists() else {}
    if not completion.get('repaired'):
        record['results'][name] = {'status': 'repair_failed', 'repair': completion}
        save()
        return
    snapshot = trace / 'attempted.json'
    passed = run(name + '-resume', [audit, 'resume', snapshot, args.output / (name + '.accepted.obj')], 40)
    report_path = args.output / (name + '.audit.json')
    if not run(name + '-candidate', [audit, 'diagnose-candidate', snapshot, report_path], 40):
        record['results'][name] = {'status': 'candidate_diagnostic_failed', 'passed_own_source_checks': passed}
        save()
        return
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

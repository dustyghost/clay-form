#!/usr/bin/env python3
"""Check native cycle selection on rotated/scaled copies of a frozen snapshot.

Usage: check-cycle-selection.py audit-executable raw-snapshot.json output-directory
Requires NumPy. This checks selection only, not invariance of the full solver.
Run under the same shared lock and memory limits as the other audit commands.
"""
import copy
import json
import subprocess
import sys
from pathlib import Path

import numpy as np


def main():
    executable, source, output = map(Path, sys.argv[1:])
    output.mkdir(parents=True, exist_ok=True)
    original = json.loads(source.read_text())
    axis = np.array([0.3, -0.7, 0.5])
    axis /= np.linalg.norm(axis)
    angle = 0.713
    x, y, z = axis
    skew = np.array([[0, -z, y], [z, 0, -x], [-y, x, 0]])
    rotation = np.eye(3) + np.sin(angle) * skew + (1 - np.cos(angle)) * skew @ skew
    records = []
    reference = None
    for label, matrix, scale, translation in [
        ("original", np.eye(3), 1.0, [0, 0, 0]),
        ("rotated", rotation, 1.0, [0, 0, 0]),
        ("rotated-scaled-translated", rotation, 7.0, [12, -4, 3]),
    ]:
        snapshot = copy.deepcopy(original)
        field = snapshot["field"]
        field["surface"]["positions"] = (
            np.asarray(field["surface"]["positions"]) @ matrix.T * scale + translation
        ).tolist()
        for key in ["normals", "directions"]:
            field[key] = (np.asarray(field[key]) @ matrix.T).tolist()
        field["areas"] = (np.asarray(field["areas"]) * scale**2).tolist()
        field["spacing"] = (np.asarray(field["spacing"]) * scale).tolist()
        # Origins, original source and constraints are not read by selection.
        # These temporary diagnostic snapshots must not be used for mesh solving.
        temporary = output / f"{label}.selection-only.json"
        with temporary.open("x") as file:
            json.dump(snapshot, file)
        report_path = output / f"{label}.report.json"
        cycles_path = output / f"{label}.cycles.json"
        try:
            with (output / f"{label}.log").open("x") as log:
                subprocess.run(
                    [str(executable.resolve()), "select-cycles", str(temporary),
                     str(report_path), str(cycles_path)],
                    check=True, stdout=log, stderr=log, timeout=60,
                )
        finally:
            temporary.unlink()
        report = json.loads(report_path.read_text())
        paths = sorted(sorted(c["vertices"]) for c in report["selected"])
        actual = (report["seeds"], paths)
        if reference is None:
            reference = actual
        record = {
            "case": label, "rotation": matrix.tolist(), "scale": scale,
            "translation": translation, "selected_count": len(paths),
            "same_seeds": actual[0] == reference[0],
            "same_selected_vertex_sets": actual[1] == reference[1],
        }
        records.append(record)
        print(record, flush=True)
    with (output / "invariance.json").open("x") as file:
        json.dump(records, file, indent=2)
        file.write("\n")
    assert all(r["same_seeds"] and r["same_selected_vertex_sets"] for r in records)


if __name__ == "__main__":
    main()

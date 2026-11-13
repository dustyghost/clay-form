#!/usr/bin/env python3
"""Check native geometric hints after rotation, scaling and translation.

Usage: check-geometry-hints.py audit-executable source.clay target output-directory
Requires NumPy; checks hint generation, not full remeshing invariance.
"""
import json
import subprocess
import sys
from pathlib import Path

import numpy as np


def main():
    executable, source, target, output = sys.argv[1:]
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    mesh = json.loads(Path(source).read_text())["mesh"]
    axis = np.array([0.3, -0.7, 0.5])
    axis /= np.linalg.norm(axis)
    x, y, z = axis
    skew = np.array([[0, -z, y], [z, 0, -x], [-y, x, 0]])
    angle = 0.713
    rotation = np.eye(3) + np.sin(angle) * skew + (1 - np.cos(angle)) * skew @ skew
    reference = None
    records = []
    for label, matrix, scale, shift in [
        ("original", np.eye(3), 1.0, [0, 0, 0]),
        ("rotated", rotation, 1.0, [0, 0, 0]),
        ("rotated-scaled-translated", rotation, 7.0, [12, -4, 3]),
    ]:
        temporary = output / f"{label}.temporary.clay"
        transformed = {"mesh": {
            "vertices": (np.asarray(mesh["vertices"]) @ matrix.T * scale + shift).tolist(),
            "triangles": mesh["triangles"],
        }}
        with temporary.open("x") as file:
            json.dump(transformed, file)
        report_path = output / f"{label}.report.json"
        try:
            with (output / f"{label}.log").open("x") as log:
                subprocess.run(
                    [str(Path(executable).resolve()), "curvature-guides", str(temporary), target,
                     str(report_path), str(output / f"{label}.guides.json")],
                    check=True, stdout=log, stderr=log, timeout=60,
                )
        finally:
            temporary.unlink()
        actual = json.loads(report_path.read_text())
        if reference is None:
            reference = actual
        same_ids = [g["vertex"] for g in actual["guides"]] == [g["vertex"] for g in reference["guides"]]
        assert same_ids, f"Selected hint vertices differ for {label}"
        a = np.array([g["tangent"] for g in reference["guides"]]) @ matrix.T
        b = np.array([g["tangent"] for g in actual["guides"]])
        cosines = np.abs((a * b).sum(axis=1))
        radius_error = max(abs(a["radius"] * scale - b["radius"])
                           for a, b in zip(reference["guides"], actual["guides"]))
        record = {
            "case": label, "rotation": matrix.tolist(), "scale": scale, "translation": shift,
            "hint_count": len(b), "same_vertex_ids": same_ids,
            "minimum_direction_abs_cosine": float(cosines.min()),
            "maximum_scaled_radius_error": radius_error,
        }
        records.append(record)
        print(record, flush=True)
    with (output / "invariance.json").open("x") as file:
        json.dump(records, file, indent=2)
        file.write("\n")
    assert all(r["minimum_direction_abs_cosine"] > 1 - 1e-8
               and r["maximum_scaled_radius_error"] < 1e-8 for r in records)


if __name__ == "__main__":
    main()

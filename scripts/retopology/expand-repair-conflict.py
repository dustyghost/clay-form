"""Offline, geometry-agnostic boundary expansion on a frozen coarse graph.

Requires the pinned scalar reference generator and the native audit executable.
Only fixed cut edges named by a sufficient UNSAT core become flexible. Every
incident face is then included; actual coordinate locks are never released.
SAT is checked on the entire graph before exporting an integer candidate.
This is a diagnostic experiment, not an accepted retopology or runtime solver.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import time


def write(path, value):
    with path.open("x") as stream:
        json.dump(value, stream, indent=2)


def rotated(v, r):
    x, y = v
    return ((x, y), (-y, x), (-x, -y), (y, -x))[r % 4]


def measure(graph):
    areas, defects = [], []
    for i, (face, rotations) in enumerate(zip(graph["faces"], graph["rotations"])):
        v = [rotated(graph["offsets"][e], r) for e, r in zip(face, rotations)]
        if tuple(map(sum, zip(*v))) != (0, 0):
            defects.append(i)
        areas.append(v[0][0] * v[1][1] - v[0][1] * v[1][0])
    return areas, defects


def run(args):
    args.output.mkdir()
    graph = json.loads(args.graph.read_text())
    seed = json.loads(args.patch.read_text())
    flexible = {e for e, d in zip(seed["graph_edges"], seed["domains"]) if d.bit_count() > 1}
    previous_directory = args.resume_expansion
    visited = set()
    while previous_directory:
        if previous_directory in visited:
            raise ValueError("Cyclic expansion resume chain")
        visited.add(previous_directory)
        previous = json.loads((previous_directory / "inputs.json").read_text())
        for path in (args.graph, args.patch):
            if previous["sha256"][str(path)] != hashlib.sha256(path.read_bytes()).hexdigest():
                raise ValueError("Expansion resume input changed")
        flexible.update(previous.get("initial_flexible_edges", []))
        for path in sorted(previous_directory.glob("step-*/summary.json")):
            row = json.loads(path.read_text())
            flexible.update(row.get("released_edges", []))
        parent = previous["arguments"].get("resume_expansion")
        previous_directory = Path(parent) if parent and "initial_flexible_edges" not in previous else None
    original_areas, original_defects = measure(graph)
    if original_defects:
        raise ValueError("Input graph has closure defects")
    write(args.output / "inputs.json", {
        "arguments": {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
        "sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                   for p in (args.graph, args.patch, args.reference, args.audit)},
        "original_inversions": sum(a < 0 for a in original_areas),
        "initial_flexible_edges": sorted(flexible),
    })
    for step in range(args.steps):
        directory = args.output / f"step-{step:03}"
        directory.mkdir()
        faces = sorted({f for e in flexible for f in graph["edge_faces"][e]
                        if f < len(graph["faces"])})
        edges = sorted({e for f in faces for e in graph["faces"][f]})
        ids = {e: i for i, e in enumerate(edges)}
        record = {"step": step, "faces": len(faces), "flexible_edges": len(flexible),
                  "fixed_edges": len(edges) - len(flexible)}
        if len(faces) > args.max_faces:
            record["outcome"] = "face_limit"
            write(directory / "summary.json", record)
            print(json.dumps(record), flush=True)
            break
        write(directory / "correspondence.json", {"graph_faces": faces, "graph_edges": edges})
        with (directory / "input.txt").open("x") as stream:
            stream.write(f"{len(edges)} {len(faces)}\n")
            for e in edges:
                x, y = graph["offsets"][e]
                a, b = graph["allow"][e] if e in flexible else (0, 0)
                stream.write(f"{x} {y} {a} {b}\n")
            for f in faces:
                stream.write(" ".join(map(str, [ids[e] for e in graph["faces"][f]]
                                          + graph["rotations"][f])) + "\n")
        with (directory / "input.txt").open("rb") as inp, (directory / "input.cnf").open("xb") as cnf:
            subprocess.run([str(args.reference)], stdin=inp, stdout=cnf, check=True, timeout=15)
        started = time.monotonic()
        subprocess.run([str(args.audit), "explain-cnf", str(directory / "input.cnf"),
                        str(directory / "solve.json"), str(args.millis)],
                       check=True, timeout=args.millis / 1000 + 15, stdout=subprocess.DEVNULL)
        result = json.loads((directory / "solve.json").read_text())
        record.update(outcome=result["outcome"], seconds=time.monotonic() - started,
                      core_coordinates=len(result["unsat_core_units"]))
        if result["outcome"] == "unsat":
            core_edges = {edges[(abs(v) - 1) // 6] for v in result["unsat_core_units"]}
            release = {e for e in core_edges - flexible if graph["allow"][e] != [0, 0]
                       and graph["edge_faces"][e][0] != graph["edge_faces"][e][1]}
            record["released_edges"] = sorted(release)
            flexible.update(release)
            if not release:
                record["outcome"] = "intrinsic_or_locked_conflict"
        elif result["outcome"] == "sat":
            assignment = {abs(v): v > 0 for v in result["solution"]}
            candidate = copy.deepcopy(graph)
            for i, e in enumerate(edges):
                for axis in range(2):
                    p = graph["offsets"][e][axis]
                    values = [v for v in (-1, 0, 1)
                              if assignment[3 * (2 * i + axis) + v + 2] == (p != v)]
                    if len(values) != 1:
                        raise ValueError("Invalid scalar assignment")
                    value = values[0]
                    if (e not in flexible or graph["allow"][e][axis] == 0) and value != p:
                        raise ValueError("Fixed coordinate changed")
                    candidate["offsets"][e][axis] = value
            areas, defects = measure(candidate)
            new_inversions = [f for f, (old, new) in enumerate(zip(original_areas, areas))
                              if new < 0 <= old]
            if defects or new_inversions or any(areas[f] < 0 for f in faces):
                raise ValueError("Surrounding graph consistency failed")
            record.update(closure_defects=0, new_inversions=0,
                          remaining_inversions=sum(a < 0 for a in areas),
                          changed_edges=sum(a != b for a, b in zip(graph["offsets"], candidate["offsets"])),
                          patch_positive_before=sum(original_areas[f] > 0 for f in faces),
                          patch_positive_after=sum(areas[f] > 0 for f in faces),
                          patch_area_before=sum(original_areas[f] for f in faces),
                          patch_area_after=sum(areas[f] for f in faces))
            write(args.output / "candidate-graph.json", candidate)
        write(directory / "summary.json", record)
        print(json.dumps(record), flush=True)
        if record["outcome"] != "unsat":
            break


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("graph", "patch", "reference", "audit", "output"):
        parser.add_argument(name, type=lambda s: Path(s).resolve())
    parser.add_argument("--millis", type=int, default=2000)
    parser.add_argument("--steps", type=int, default=64)
    parser.add_argument("--max-faces", type=int, default=8000)
    parser.add_argument("--resume-expansion", type=lambda s: Path(s).resolve())
    run(parser.parse_args())

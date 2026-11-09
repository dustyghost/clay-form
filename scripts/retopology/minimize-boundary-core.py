"""Shrink a sufficient boundary conflict using a bounded independent SAT solver.

The correspondence JSON must contain graph_edges in the CNF's local edge order.
Unit clauses for guide locks and contracted self edges remain mandatory. Only
ordinary cut-edge values are removed. Every retained conflict is independently
UNSAT; budgets can leave a non-minimal sufficient core, never a claimed proof
of infeasibility of the full model.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def run(args):
    args.output.mkdir()
    graph = json.loads(args.graph.read_text())
    edges = json.loads(args.correspondence.read_text())["graph_edges"]
    hard, groups = [], {}
    variables = 0
    for line in args.cnf.read_text().splitlines():
        if not line or line[0] in "cp":
            continue
        values = [int(v) for v in line.split()]
        if values[-1] != 0 or 0 in values[:-1]:
            raise ValueError("Expected one complete clause per line")
        variables = max(variables, *(abs(v) for v in values))
        if len(values) == 2:
            scalar = (abs(values[0]) - 1) // 3
            edge = edges[scalar // 2]
            if (graph["allow"][edge][scalar % 2] != 0
                    and graph["edge_faces"][edge][0] != graph["edge_faces"][edge][1]):
                groups.setdefault(edge, []).append(line)
                continue
        hard.append(line)
    started = time.monotonic()
    records, cache = [], {}
    def solve(active):
        key = tuple(sorted(active))
        if key in cache:
            return cache[key]
        if len(records) >= args.queries or time.monotonic() - started > args.seconds:
            return "budget"
        lines = hard + [line for e in key for line in groups[e]]
        with (args.output / "query.cnf").open("w") as stream:
            stream.write(f"p cnf {variables} {len(lines)}\n" + "\n".join(lines) + "\n")
        before = time.monotonic()
        with (args.output / "query.log").open("wb") as log:
            try:
                code = subprocess.run([str(args.solver), str(args.output / "query.cnf")],
                                      stdout=log, stderr=log, timeout=2).returncode
            except subprocess.TimeoutExpired:
                code = "timeout"
        outcome = {10: "sat", 20: "unsat"}.get(code, "budget")
        records.append(dict(edges=list(key), outcome=outcome, exit=code,
                            seconds=time.monotonic() - before))
        cache[key] = outcome
        return outcome
    active = sorted(groups)
    if solve(active) != "unsat":
        raise ValueError("Full frozen boundary was not proved UNSAT")
    partitions = min(2, len(active))
    complete = not active
    while active:
        changed = False
        chunks = [active[i::partitions] for i in range(partitions)]
        for chunk in chunks:
            trial = sorted(set(active) - set(chunk))
            outcome = solve(trial)
            if outcome == "unsat":
                active = trial
                partitions = min(max(2, partitions - 1), len(active))
                changed = True
                break
            if outcome == "budget":
                break
        else:
            if partitions == len(active):
                complete = True
                break
            partitions = min(len(active), partitions * 2)
            continue
        if not changed:
            break
    lines = hard + [line for e in active for line in groups[e]]
    with (args.output / "core.cnf").open("x") as stream:
        stream.write(f"p cnf {variables} {len(lines)}\n" + "\n".join(lines) + "\n")
    result = dict(released_edges=active, original_boundary_edges=len(groups),
                  core_boundary_edges=len(active), sufficient_unsat=True,
                  deletion_minimal=complete, queries=records,
                  seconds=time.monotonic() - started,
                  sha256={str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                          for p in (args.graph, args.correspondence, args.cnf, args.solver)})
    with (args.output / "core.json").open("x") as stream:
        json.dump(result, stream, indent=2)
    print(json.dumps({k: v for k, v in result.items() if k not in ("queries", "sha256")}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("graph", "correspondence", "cnf", "solver", "output"):
        parser.add_argument(name, type=lambda s: Path(s).resolve())
    parser.add_argument("--seconds", type=float, default=60)
    parser.add_argument("--queries", type=int, default=160)
    run(parser.parse_args())

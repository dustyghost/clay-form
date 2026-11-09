#!/usr/bin/env python3
"""Summarize an audit repair-trace directory without loading its mesh snapshot.

Spatial clusters are diagnostic Euclidean proximity groups, not solver regions.
Only completed SAT attempts contribute to timing totals. Interrupted attempts
remain visible separately; a local UNSAT result does not prove global failure.
"""
import argparse
import collections
import json
import math
from pathlib import Path


def bounds(locations):
    points = [p["representative_center"] for p in locations]
    if not points:
        return {"count": 0}
    return {
        "count": len(points),
        "center": [sum(p[k] for p in points) / len(points) for k in range(3)],
        "min": [min(p[k] for p in points) for k in range(3)],
        "max": [max(p[k] for p in points) for k in range(3)],
    }


def clusters(locations, distance):
    # The traced unresolved sets are small; avoid a geometry-library dependency.
    remaining = set(range(len(locations)))
    result = []
    while remaining:
        first = min(remaining)
        remaining.remove(first)
        queue, members = [first], []
        while queue:
            i = queue.pop()
            members.append(i)
            near = sorted(j for j in remaining if math.dist(
                locations[i]["representative_center"],
                locations[j]["representative_center"],
            ) <= distance)
            remaining.difference_update(near)
            queue.extend(near)
        result.append(bounds([locations[i] for i in members]))
    return sorted(result, key=lambda c: -c["count"])


def summarize(directory, spacing):
    stages, checkpoints, attempts, skipped = [], [], [], []
    pending = None
    with (directory / "events.jsonl").open() as stream:
        for line in stream:
            event = json.loads(line)
            if event["event"] == "stage":
                stages.append({
                    "stage": event["stage"], "seconds": event["seconds"],
                    "inversions": len(event["inversions"]),
                    # Skip clustering thousands of raw pre-greedy inversions.
                    "clusters": clusters(event["inversions"], 2 * spacing)
                    if event["stage"] != "input" else None,
                })
            elif event["event"] == "checkpoint":
                checkpoints.append({k: v for k, v in event.items() if k != "inversions"})
                checkpoints[-1]["inversions"] = len(event["inversions"])
            elif event["event"] == "patch":
                patch = event["patch"]
                item = {k: v for k, v in patch.items() if not k.startswith("inverted_source_faces_")}
                item["inversions_before"] = bounds(event["locations_before"])
                item["inversions_after"] = bounds(event["locations_after"])
                if patch["phase"] == "start":
                    pending = item
                elif patch["phase"] == "finish":
                    attempts.append(item)
                    pending = None
                else:
                    skipped.append(item)
    outcomes = collections.defaultdict(lambda: {"count": 0, "total_seconds": 0.,
        "propagation_seconds": 0., "encoding_seconds": 0., "search_seconds": 0.})
    for item in attempts:
        stats = item["sat"]
        bucket = outcomes[stats["outcome"]]
        bucket["count"] += 1
        for key in bucket:
            if key != "count":
                bucket[key] += stats[key]
    completion = directory / "completion.json"
    return {
        "trace_directory": str(directory), "spacing": spacing,
        "cluster_distance": 2 * spacing,
        "note": "Representative field-triangle centers, not exact coarse-cell footprints. "
            "Euclidean clusters are for inspection only. SAT times exclude interrupted "
            "attempts and surrounding repair work; UNSAT refers to a fixed local problem.",
        "completion": json.loads(completion.read_text()) if completion.exists() else None,
        "stages": stages, "checkpoints": checkpoints, "outcomes": dict(outcomes),
        "interrupted_patch": pending, "skipped_patches": skipped,
        "largest_inversion_patches": sorted(attempts,
            key=lambda p: -p["inversions_before"]["count"])[:24],
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace_directory", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--spacing", type=float, required=True)
    args = parser.parse_args()
    if not math.isfinite(args.spacing) or args.spacing <= 0:
        parser.error("spacing must be positive and finite")
    summary = summarize(args.trace_directory, args.spacing)
    with args.output.open("x") as output:
        json.dump(summary, output, indent=2, allow_nan=False)
        output.write("\n")
    print(json.dumps({"stages": [(s["stage"], s["inversions"]) for s in summary["stages"]],
                      "outcomes": summary["outcomes"]}, indent=2))

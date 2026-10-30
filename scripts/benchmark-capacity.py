#!/usr/bin/env python3
"""Run bounded, isolated capacity workloads or compare two compatible result sets."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
TEST = "viewport::gpu::profile::capacity::gpu_load_profile_capacity"


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def proc_kib(path, key):
    try:
        for line in Path(path).read_text().splitlines():
            if line.startswith(key + ":"):
                return int(line.split()[1])
    except (OSError, ValueError):
        pass
    return None


def source_digest():
    digest = hashlib.sha256()
    paths = list((ROOT / "src").rglob("*.rs"))
    paths += [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "build.rs", "scripts/benchmark-capacity.py")]
    for path in sorted(p for p in paths if p.is_file()):
        digest.update(str(path.relative_to(ROOT)).encode() + b"\0" + path.read_bytes())
    return digest.hexdigest()


def metadata(args):
    cpu = next(line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name"))
    return {
        "schema": 1, "revision": command("git", "rev-parse", "HEAD"),
        "working_tree_dirty": bool(command("git", "status", "--porcelain", "--untracked-files=normal")),
        "source_sha256": source_digest(), "rustc": command("rustc", "--version"),
        "platform": platform.platform(), "cpu": cpu, "logical_cpus": os.cpu_count(),
        "mem_total_kib": proc_kib("/proc/meminfo", "MemTotal"),
        "mem_available_start_kib": proc_kib("/proc/meminfo", "MemAvailable"),
        "swap_free_start_kib": proc_kib("/proc/meminfo", "SwapFree"),
        "profile": "release", "gpu_requested": args.gpu,
        "environment": {key: os.environ.get(key) for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_PROFILE_RELEASE_OPT_LEVEL", "CARGO_PROFILE_RELEASE_LTO", "CARGO_BUILD_TARGET", "CLAY_GPU_SOFTWARE", "VK_DRIVER_FILES", "VK_ICD_FILENAMES")},
        "limits": {"rss_mib": args.max_rss_mib, "timeout_seconds": args.timeout,
                   "min_available_mib": getattr(args, "min_available_mib", 512)},
        "workload": getattr(args, "workload", "full"),
        "mesh_capacity": {"vertices": getattr(args, "mesh_vertices", 500_000),
                          "triangles": getattr(args, "mesh_triangles", 1_000_000)},
        "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }


def build():
    # Never overlap Cargo invocations. Forward compiler diagnostics, retain only artifact metadata.
    with subprocess.Popen(["cargo", "test", "--release", "--locked", "--lib", "--no-run", "--message-format=json"], cwd=ROOT, stdout=subprocess.PIPE, text=True) as proc:
        executable = None
        for line in proc.stdout:
            event = json.loads(line)
            if event.get("reason") == "compiler-message":
                print(event["message"].get("rendered", ""), file=sys.stderr, end="")
            if event.get("reason") == "compiler-artifact" and event.get("profile", {}).get("test") and event.get("executable"):
                executable = event["executable"]
        if proc.wait() or not executable:
            raise RuntimeError("optimized benchmark build failed")
    return executable


def bounded_run(executable, env, log, args):
    start = time.monotonic()
    reason = None
    peak_rss = peak_swap = 0
    minimum_available = None
    reserve = getattr(args, "min_available_mib", 512) * 1024
    with log.open("w") as stream, subprocess.Popen(
        [executable, TEST, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
        cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT,
    ) as proc:
        try:
            while proc.poll() is None:
                rss = proc_kib(f"/proc/{proc.pid}/status", "VmRSS")
                available = proc_kib("/proc/meminfo", "MemAvailable")
                swap = proc_kib(f"/proc/{proc.pid}/status", "VmSwap")
                peak_rss = max(peak_rss, rss or 0)
                peak_swap = max(peak_swap, swap or 0)
                if available is not None:
                    minimum_available = available if minimum_available is None else min(minimum_available, available)
                if time.monotonic() - start > args.timeout:
                    reason = "timeout"
                elif rss is not None and rss > args.max_rss_mib * 1024:
                    reason = "process RSS budget exceeded"
                elif available is not None and available < reserve:
                    reason = f"system available memory below {reserve // 1024} MiB"
                if reason:
                    proc.kill()
                    break
                time.sleep(0.1)
        finally:
            if proc.poll() is None:
                proc.kill()
            code = proc.wait()
    observed = {"elapsed_seconds": time.monotonic() - start,
                "sampled_peak_rss_bytes": peak_rss * 1024,
                "sampled_peak_swap_bytes": peak_swap * 1024,
                "minimum_available_bytes": None if minimum_available is None else minimum_available * 1024}
    if reason or code != 0:
        return {"status": "failed", "reason": reason or f"test exit {code}", "log": log.name, **observed}
    return {"status": "measured", "log": log.name, **observed}


def validate_request(args):
    if not 1 <= args.repeats <= 10 or not 256 <= args.max_rss_mib <= 16384 or not math.isfinite(args.timeout) or args.timeout <= 0:
        raise ValueError("invalid repeat, RSS or timeout budget")
    if not 256 <= args.min_available_mib <= 16384:
        raise ValueError("available-memory reserve must be 256–16384 MiB")
    if not 500_000 <= args.mesh_vertices <= 16_000_000 or not 1_000_000 <= args.mesh_triangles <= 32_000_000:
        raise ValueError("mesh budgets must be within the supported Settings ranges")
    if not args.levels or any(level not in range(5) for level in args.levels):
        raise ValueError("fixture levels must be 0–4")
    if args.workload not in ("full", "interactive"):
        raise ValueError("unknown workload")
    quads = 4704 * 4 ** max(args.levels)
    if args.mesh_vertices < quads + 2 or args.mesh_triangles < quads * 2:
        raise ValueError(f"requested fixture needs {quads + 2} vertices / {quads * 2} triangles; set --mesh-vertices and --mesh-triangles explicitly")


def run(args):
    if sys.platform != "linux":
        raise ValueError("the initial capacity harness uses Linux process memory accounting")
    validate_request(args)
    args.output.mkdir(parents=True, exist_ok=False)
    info = metadata(args)
    (args.output / "environment.json").write_text(json.dumps(info, indent=2) + "\n")
    executable = build()
    cases = []
    for name in args.fixtures:
        for level in args.levels:
            key = f"{name}-l{level}"
            available = proc_kib("/proc/meminfo", "MemAvailable")
            required_mib = max(1024, args.max_rss_mib + args.min_available_mib)
            if available is None or available < required_mib * 1024:
                item = {"status": "skipped", "reason": f"less than {required_mib} MiB available before workload"}
            else:
                print(f"Running {key} ({args.repeats} repetitions)", flush=True)
                env = os.environ.copy()
                # Each child gets explicit limits; never inherit an unrelated probe's settings.
                for key_to_remove in list(env):
                    if key_to_remove.startswith("CLAY_CAPACITY_"):
                        env.pop(key_to_remove)
                env.update(CLAY_CAPACITY_OUTPUT=str((args.output / f"{key}.json").resolve()),
                           CLAY_CAPACITY_FIXTURE=name, CLAY_CAPACITY_LEVEL=str(level), CLAY_CAPACITY_REPEATS=str(args.repeats),
                           CLAY_CAPACITY_VERTICES=str(args.mesh_vertices), CLAY_CAPACITY_TRIANGLES=str(args.mesh_triangles),
                           CLAY_CAPACITY_WORKLOAD=args.workload)
                if args.gpu:
                    env["CLAY_CAPACITY_GPU"] = "1"
                item = bounded_run(executable, env, args.output / f"{key}.log", args)
                if item["status"] == "measured" and not Path(env["CLAY_CAPACITY_OUTPUT"]).is_file():
                    item.update(status="failed", reason="benchmark produced no result file")
                progress = args.output / f"{key}.progress.json"
                if progress.is_file():
                    latest = json.loads(progress.read_text())
                    item.update(progress=progress.name, last_stage=latest["stage"], stage_status=latest["status"])
            cases.append({"case": key, **item})
            (args.output / "index.json").write_text(json.dumps(cases, indent=2) + "\n")
            print(f"  {item['status']}: {item.get('reason', 'complete')}", flush=True)
            if item["status"] != "measured":
                # Do not continue applying memory pressure after a failed bounded run.
                return 1
    return 0


def compare_results(before, after):
    for key in ("schema", "fixture", "level", "repeats", "dabs_per_stroke", "fixture_hash_fnv1a", "counts", "gpu", "viewport"):
        if before[key] != after[key]:
            raise ValueError(f"incompatible workload: {key}")
    for key in ("workload", "mesh_capacity"):
        if before.get(key) != after.get(key):
            raise ValueError(f"incompatible workload: {key}")
    if before["output_hashes_fnv1a"] != after["output_hashes_fnv1a"]:
        raise ValueError("stroke outputs changed; review correctness before comparing speed")
    if before["metrics"].keys() != after["metrics"].keys():
        raise ValueError("metric coverage changed")
    rows = []
    for name in sorted(before["metrics"]):
        a, b = before["metrics"][name], after["metrics"][name]
        if a["n"] != b["n"]:
            raise ValueError(f"sample count changed: {name}")
        for percentile in ("p50_ms", "p95_ms"):
            old, new = a[percentile], b[percentile]
            ratio = new / old if old else None
            rows.append((name, percentile, old, new, ratio))
    return rows


def compare(args):
    left = json.loads((args.before / "environment.json").read_text())
    right = json.loads((args.after / "environment.json").read_text())
    for key in ("schema", "cpu", "logical_cpus", "mem_total_kib", "platform", "rustc", "profile", "gpu_requested", "environment"):
        if left[key] != right[key]:
            raise ValueError(f"incompatible environment: {key}")
    indexes = [json.loads((path / "index.json").read_text()) for path in (args.before, args.after)]
    if any(item["status"] != "measured" for index in indexes for item in index):
        raise ValueError("cannot compare incomplete or failed runs")
    cases = [{item["case"] for item in index} for index in indexes]
    if cases[0] != cases[1]:
        raise ValueError("workload coverage changed")
    rows = []
    for key in sorted(cases[0]):
        before, after = [json.loads((path / f"{key}.json").read_text()) for path in (args.before, args.after)]
        rows.extend((key, *row) for row in compare_results(before, after))
    print("case,metric,percentile,before_ms,after_ms,after_over_before")
    failed = False
    for case, name, percentile, old, new, ratio in rows:
        ratio_text = f"{ratio:.3f}" if ratio is not None else "n/a"
        print(f"{case},{name},{percentile},{old:.6f},{new:.6f},{ratio_text}")
        if args.max_regression_percent is not None and ratio is not None:
            failed |= ratio > 1 + args.max_regression_percent / 100
    return int(failed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    run_parser = sub.add_parser("run")
    run_parser.add_argument("output", type=Path)
    run_parser.add_argument("--fixtures", nargs="+", choices=("sphere", "organic", "hidden"), default=["sphere", "organic", "hidden"])
    run_parser.add_argument("--levels", nargs="+", type=int, choices=range(5), default=[0, 1, 2, 3])
    run_parser.add_argument("--repeats", type=int, default=3)
    run_parser.add_argument("--gpu", action="store_true")
    run_parser.add_argument("--workload", choices=("full", "interactive"), default="full",
                            help="interactive excludes file I/O and additional subdivision")
    run_parser.add_argument("--mesh-vertices", type=int, default=500_000)
    run_parser.add_argument("--mesh-triangles", type=int, default=1_000_000)
    run_parser.add_argument("--min-available-mib", type=int, default=512)
    run_parser.add_argument("--max-rss-mib", type=int, default=1536)
    run_parser.add_argument("--timeout", type=float, default=600)
    compare_parser = sub.add_parser("compare")
    compare_parser.add_argument("before", type=Path)
    compare_parser.add_argument("after", type=Path)
    compare_parser.add_argument("--max-regression-percent", type=float)
    args = parser.parse_args()
    try:
        return run(args) if args.mode == "run" else compare(args)
    except (ValueError, RuntimeError, OSError, subprocess.CalledProcessError) as error:
        print(f"benchmark: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())

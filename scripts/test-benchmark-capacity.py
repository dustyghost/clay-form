"""Protect comparisons from silently accepting a cheaper or different workload."""
import copy
from contextlib import contextmanager
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import unittest
from unittest import mock
from types import SimpleNamespace
import tempfile

spec = importlib.util.spec_from_file_location("capacity", Path(__file__).with_name("benchmark-capacity.py"))
capacity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capacity)


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.before = dict(schema=1, fixture="organic", level=2, repeats=3,
                           dabs_per_stroke=12, fixture_hash_fnv1a="input",
                           counts={"active_vertices": 75266}, gpu=None, viewport=[800, 600],
                           output_hashes_fnv1a={"Standard": "output"},
                           metrics={"dab": {"n": 33, "p50_ms": 10., "p95_ms": 20.}})
        self.after = copy.deepcopy(self.before)

    def test_reports_both_percentiles_and_speed_ratio(self):
        self.after["metrics"]["dab"]["p50_ms"] = 5.
        self.after["metrics"]["dab"]["p95_ms"] = 8.
        rows = capacity.compare_results(self.before, self.after)
        self.assertEqual([r[-1] for r in rows], [0.5, 0.4])

    def test_rejects_changed_inputs_counts_adapter_and_sampling(self):
        for key in ("schema", "fixture_hash_fnv1a", "counts", "gpu", "repeats", "dabs_per_stroke", "workload", "mesh_capacity"):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.after)
                changed[key] = "different"
                with self.assertRaisesRegex(ValueError, "incompatible workload"):
                    capacity.compare_results(self.before, changed)

    def test_rejects_faster_but_different_results(self):
        self.after["output_hashes_fnv1a"]["Standard"] = "wrong"
        with self.assertRaisesRegex(ValueError, "outputs changed"):
            capacity.compare_results(self.before, self.after)

    def test_rejects_missing_metrics(self):
        self.after["metrics"] = {}
        with self.assertRaisesRegex(ValueError, "coverage changed"):
            capacity.compare_results(self.before, self.after)

    def test_rejects_dropped_samples(self):
        self.after["metrics"]["dab"]["n"] = 1
        with self.assertRaisesRegex(ValueError, "sample count changed"):
            capacity.compare_results(self.before, self.after)


class GuardTests(unittest.TestCase):
    def args(self, **updates):
        values = dict(repeats=1, max_rss_mib=3072, timeout=600, min_available_mib=768,
                      mesh_vertices=2_000_000, mesh_triangles=4_000_000, levels=[4], workload="interactive")
        return SimpleNamespace(**(values | updates))

    def test_large_level_requires_explicit_supported_capacity(self):
        capacity.validate_request(self.args())
        capacity.validate_request(self.args(mesh_vertices=1_204_226, mesh_triangles=2_408_448))
        for updates in [dict(mesh_vertices=500_000, mesh_triangles=1_000_000),
                        dict(mesh_vertices=1_204_225), dict(mesh_triangles=2_408_447),
                        dict(mesh_vertices=16_000_001), dict(levels=[5]),
                        dict(min_available_mib=0), dict(workload="unknown"),
                        dict(timeout=float("nan")), dict(timeout=float("inf"))]:
            with self.subTest(updates=updates), self.assertRaises(ValueError):
                capacity.validate_request(self.args(**updates))

    def test_guard_retains_observations_when_it_kills_a_workload(self):
        for rss, available, expected, code in [(4000, 5000, "RSS budget", -9),
                                                (1000, 600, "available memory", -9),
                                                (4000, 5000, "RSS budget", 0)]:
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temporary:
                proc = mock.MagicMock()
                proc.__enter__.return_value = proc
                proc.pid = 123
                proc.poll.side_effect = [None, code]
                proc.wait.return_value = code
                readings = {"VmRSS": rss * 1024, "VmSwap": 16, "MemAvailable": available * 1024}
                with mock.patch.object(capacity.subprocess, "Popen", return_value=proc), mock.patch.object(capacity, "proc_kib", side_effect=lambda path, key: readings[key]):
                    result = capacity.bounded_run("unused", {}, Path(temporary)/"run.log", self.args())
                proc.kill.assert_called_once()
                self.assertEqual(result["status"], "failed")
                self.assertIn(expected, result["reason"])
                self.assertEqual(result["sampled_peak_rss_bytes"], rss * 1024 * 1024)
                self.assertEqual(result["minimum_available_bytes"], available * 1024 * 1024)
                self.assertEqual(result["sampled_peak_swap_bytes"], 16 * 1024)


class ProvenanceTests(unittest.TestCase):
    def test_manifest_covers_shaders_path_crates_embedded_inputs_and_configs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            home = Path(temporary) / "home"
            tracked = ["Cargo.toml", "src/a.rs", "src/viewport/gpu/viewport.wgsl",
                       "crates/clay-retopology/Cargo.toml", "crates/clay-retopology/src/lib.rs",
                       "tests/fixtures/embedded.json", "legal/third-party.json"]
            untracked = ["src/new.wgsl", ".cargo/config.toml", "run-output/profile.json"]
            for name in tracked + untracked:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(name)
            config = home / ".cargo/config.toml"
            config.parent.mkdir(parents=True)
            config.write_text("[build]\njobs = 99\n")
            with mock.patch.object(capacity, "ROOT", root), mock.patch.object(capacity, "command", side_effect=lambda *args: "\0".join(untracked if "--others" in args else tracked)), mock.patch.object(capacity.Path, "home", return_value=home), mock.patch.dict(capacity.os.environ, {"CARGO_HOME": str(home / ".cargo")}):
                before = capacity.source_manifest()
                self.assertNotIn("run-output/profile.json", [item["path"] for item in before])
                self.assertIn("cargo-config:cargo-home/config.toml", [item["path"] for item in before])
                for name in tracked + untracked[:2]:
                    with self.subTest(name=name):
                        path = root / name
                        original = path.read_text()
                        path.write_text(original + " changed")
                        self.assertNotEqual(capacity.manifest_digest(before), capacity.manifest_digest(capacity.source_manifest()))
                        path.write_text(original)
                config.write_text("[build]\njobs = 1\n")
                self.assertNotEqual(before, capacity.source_manifest())

    def test_file_hash_streams_in_bounded_chunks(self):
        payload = b"x" * (2 * 1024 * 1024 + 13)
        stream = mock.MagicMock(wraps=io.BytesIO(payload))
        stream.__enter__.return_value = stream
        path = mock.Mock()
        path.open.return_value = stream
        self.assertEqual(capacity.file_digest(path), hashlib.sha256(payload).hexdigest())
        self.assertTrue(all(call.args == (1024 * 1024,) for call in stream.read.call_args_list))


class BuildSafetyTests(unittest.TestCase):
    def resources(self, available=4096, some=0., full=0., processes="1 init"):
        pressure = f"some avg10={some} avg60=0 total=0\nfull avg10={full} avg60=0 total=0\n"
        with mock.patch.object(capacity.Path, "read_text", return_value=pressure), mock.patch.object(capacity, "proc_kib", return_value=available * 1024 if available is not None else None), mock.patch.object(capacity, "command", side_effect=lambda *args: processes if args[0] == "ps" else "memory summary"), mock.patch("sys.stdout", new_callable=io.StringIO):
            return capacity.build_resources()

    def test_job_limit_reacts_to_memory_and_stalls(self):
        self.assertEqual(self.resources()["jobs"], 2)
        self.assertEqual(self.resources(available=1536)["jobs"], 1)
        self.assertEqual(self.resources(some=0.01)["jobs"], 1)
        self.assertEqual(self.resources(full=0.01)["jobs"], 1)
        for values in [dict(available=512), dict(available=None), dict(full=1.),
                       dict(processes="52 rustc"), dict(processes="53 cargo-clippy")]:
            with self.subTest(values=values), self.assertRaises(RuntimeError):
                self.resources(**values)

    def test_missing_pressure_does_not_start_an_unchecked_build(self):
        with mock.patch.object(capacity, "command", return_value="memory summary"), mock.patch.object(capacity, "proc_kib", return_value=4096 * 1024), mock.patch.object(capacity.Path, "read_text", return_value=""):
            with self.assertRaisesRegex(RuntimeError, "cannot establish memory pressure"):
                capacity.build_resources()

    @unittest.skipUnless(capacity.sys.platform == "linux", "Linux flock is only used for runs")
    def test_lock_is_nonblocking_and_released_after_failure(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(capacity.Path, "home", return_value=Path(temporary)):
            with self.assertRaisesRegex(ValueError, "failed"):
                with capacity.workload_lock():
                    with self.assertRaisesRegex(RuntimeError, "do not wrap"):
                        with capacity.workload_lock():
                            self.fail("nested lock must not succeed")
                    raise ValueError("failed")
            with capacity.workload_lock():
                pass

    def test_run_holds_lock_before_resource_checks_and_through_workload(self):
        events = []

        @contextmanager
        def lock():
            events.append("lock")
            try:
                yield
            finally:
                events.append("unlock")

        with mock.patch.object(capacity.sys, "platform", "linux"), mock.patch.object(capacity, "validate_request"), mock.patch.object(capacity, "workload_lock", lock), mock.patch.object(capacity, "build_resources", side_effect=lambda: events.append("resources") or {"jobs": 1}), mock.patch.object(capacity, "run_locked", side_effect=lambda *args: events.append("build_and_run") or 0):
            self.assertEqual(capacity.run(SimpleNamespace()), 0)
        self.assertEqual(events, ["lock", "resources", "build_and_run", "unlock"])

    def test_build_overrides_inherited_parallelism(self):
        proc = mock.MagicMock()
        proc.__enter__.return_value = proc
        proc.stdout = [json.dumps({"reason": "compiler-artifact", "profile": {"test": True}, "executable": "/tmp/test-binary"})]
        proc.wait.return_value = 0
        with mock.patch.dict(capacity.os.environ, {"CARGO_BUILD_JOBS": "99"}), mock.patch.object(capacity.subprocess, "Popen", return_value=proc) as popen:
            self.assertEqual(capacity.build(1), "/tmp/test-binary")
        argv = popen.call_args.args[0]
        self.assertEqual(argv[argv.index("-j") + 1], "1")
        with self.assertRaises(ValueError):
            capacity.build(3)

    def test_changed_source_after_build_prevents_execution_and_binary_is_recorded(self):
        for changed in (False, True):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                executable = root / "binary"
                executable.write_bytes(b"exact executable")
                manifest = [{"path": "src/a.rs", "sha256": "before"}]
                info = {"source_manifest": manifest}
                args = SimpleNamespace(output=root / "output", fixtures=[])
                with mock.patch.object(capacity, "metadata", return_value=info), mock.patch.object(capacity, "build", return_value=str(executable)), mock.patch.object(capacity, "source_manifest", return_value=[] if changed else manifest), mock.patch.object(capacity, "bounded_run") as run:
                    if changed:
                        with self.assertRaisesRegex(RuntimeError, "changed during compilation"):
                            capacity.run_locked(args, {"jobs": 1})
                    else:
                        self.assertEqual(capacity.run_locked(args, {"jobs": 1}), 0)
                        recorded = json.loads((args.output / "environment.json").read_text())
                        self.assertEqual(recorded["executable"]["sha256"], hashlib.sha256(b"exact executable").hexdigest())
                    run.assert_not_called()

    def test_resource_change_after_build_defers_workload_and_records_reason(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = root / "binary"
            executable.write_bytes(b"exact executable")
            info = {"source_manifest": []}
            args = SimpleNamespace(output=root / "output", fixtures=["sphere"], levels=[0],
                                   max_rss_mib=1536, min_available_mib=512)
            with mock.patch.object(capacity, "metadata", return_value=info), mock.patch.object(capacity, "build", return_value=str(executable)), mock.patch.object(capacity, "source_manifest", return_value=[]), mock.patch.object(capacity, "build_resources", side_effect=RuntimeError("another build/compiler is running")), mock.patch.object(capacity, "bounded_run") as run, mock.patch("sys.stdout", new_callable=io.StringIO):
                self.assertEqual(capacity.run_locked(args, {"jobs": 1}), 1)
            run.assert_not_called()
            index = json.loads((args.output / "index.json").read_text())
            self.assertEqual(index[0]["status"], "skipped")
            self.assertIn("another build", index[0]["reason"])


class EnvironmentComparisonTests(unittest.TestCase):
    def test_comparison_on_non_linux_keeps_legacy_pairs_and_checks_build_config(self):
        with tempfile.TemporaryDirectory() as temporary:
            before, after = [Path(temporary) / name for name in ("before", "after")]
            info = dict(schema=1, cpu="cpu", logical_cpus=2, mem_total_kib=8000000,
                        platform="Linux", rustc="rustc", profile="release", gpu_requested=False,
                        environment={"RUSTFLAGS": None})
            for directory in (before, after):
                directory.mkdir()
                (directory / "environment.json").write_text(json.dumps(info))
                (directory / "index.json").write_text("[]")
            args = SimpleNamespace(before=before, after=after, max_regression_percent=None)
            with mock.patch.object(capacity.sys, "platform", "darwin"), mock.patch("sys.stdout", new_callable=io.StringIO):
                self.assertEqual(capacity.compare(args), 0)
                info["environment"]["CARGO_PROFILE_RELEASE_LTO"] = None
                (after / "environment.json").write_text(json.dumps(info))
                self.assertEqual(capacity.compare(args), 0)
                info["cargo_configuration"] = {}
                (after / "environment.json").write_text(json.dumps(info))
                with self.assertRaisesRegex(ValueError, "configuration provenance"):
                    capacity.compare(args)
                (before / "environment.json").write_text(json.dumps(info))
                self.assertEqual(capacity.compare(args), 0)
                info["cargo_configuration"] = {"ancestor-0/.cargo/config.toml": "changed"}
                (after / "environment.json").write_text(json.dumps(info))
                with self.assertRaisesRegex(ValueError, "configuration provenance"):
                    capacity.compare(args)


if __name__ == "__main__":
    unittest.main()

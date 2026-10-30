"""Protect comparisons from silently accepting a cheaper or different workload."""
import copy
import importlib.util
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


if __name__ == "__main__":
    unittest.main()

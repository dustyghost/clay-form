"""Protect comparisons from silently accepting a cheaper or different workload."""
import copy
import importlib.util
from pathlib import Path
import unittest

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
        for key in ("fixture_hash_fnv1a", "counts", "gpu", "repeats", "dabs_per_stroke"):
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


if __name__ == "__main__":
    unittest.main()

"""Reject misleading request, workload and provenance comparisons."""

from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "benchmarks"))
try:
    import scaling
finally:
    sys.path.pop(0)


class BenchmarkScalingTests(unittest.TestCase):
    def test_default_matrix_covers_85_unique_request_and_scaling_cases(self):
        cases = scaling.workloads(scaling.ENGINE_CASES, scaling.MC_CASES,
                                  [10, 100, 1000], [64, 1000, 10000],
                                  [1, 4, 10], ["pricing", "mc"])
        self.assertEqual(len(cases), 85)
        self.assertEqual(len({case["name"] for case in cases}), len(cases))
        self.assertEqual({case["request_mode"] for case in cases if case["group"] == "pricing"},
                         {"value", "risk"})

    def test_result_rejects_missing_risk_wrong_size_and_wrong_requests(self):
        case = dict(case="portfolio", group="pricing", request_mode="risk", portfolio_trades=100)
        result = dict(case="portfolio", samples=5, repeats=20, request_mode="risk",
                      portfolio_trades=100, snapshot={"sensitivities": {"rate": 2.0}},
                      setup_ms=1.0, first_call_ms=2.0, median_call_ms=0.5)
        scaling.check_result(result, case, 5, 20)
        for change in [dict(portfolio_trades=10), dict(request_mode="value"),
                       dict(snapshot={"sensitivities": {}}), dict(repeats=1),
                       dict(median_call_ms=float("nan"))]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                scaling.check_result({**result, **change}, case, 5, 20)

    def test_value_only_result_requires_absent_sensitivity_rows(self):
        case = dict(case="dcf", group="pricing", request_mode="value", portfolio_trades=2)
        result = dict(case="dcf", samples=5, repeats=1000, request_mode="value",
                      portfolio_trades=2, snapshot={"sensitivities": {}},
                      setup_ms=1.0, first_call_ms=2.0, median_call_ms=0.5)
        scaling.check_result(result, case, 5, 1000)
        result["snapshot"]["sensitivities"]["rate"] = 0.0
        with self.assertRaises(ValueError):
            scaling.check_result(result, case, 5, 1000)

    def test_first_evaluation_summary_uses_paired_sums(self):
        results = [dict(setup_ms=1, first_call_ms=100, median_call_ms=3),
                   dict(setup_ms=10, first_call_ms=10, median_call_ms=2),
                   dict(setup_ms=100, first_call_ms=1, median_call_ms=1)]
        result = scaling.summarize(results)
        self.assertEqual(result["setup_plus_first_ms"], 101)
        self.assertEqual(result["round_medians_ms"], [3, 2, 1])

    def test_fixture_normalization_preserves_equation_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            trees = [Path(temporary) / side for side in ("old", "new")]
            for tree, scalar in zip(trees, ("DualFwd", "Recorded")):
                directory = tree / "benchmarks/src/bin"
                directory.mkdir(parents=True)
                for filename in scaling.DRIVERS:
                    (directory / filename).write_text(f"type BenchmarkScalar = {scalar};\nx * 2.0\n")
            old, new = map(scaling.fixture_hashes, trees)
            for filename in scaling.DRIVERS:
                self.assertEqual(old[filename]["numerical"], new[filename]["numerical"])
                self.assertNotEqual(old[filename]["raw"], new[filename]["raw"])
            (trees[1] / "benchmarks/src/bin" / scaling.DRIVERS[0]).write_text(
                "type BenchmarkScalar = Recorded;\nx * 3.0\n")
            self.assertNotEqual(old[scaling.DRIVERS[0]]["numerical"],
                                scaling.fixture_hashes(trees[1])[scaling.DRIVERS[0]]["numerical"])

    def test_numerical_comparison_rejects_label_and_nonfinite_errors(self):
        good = {"price": 3.0, "sensitivities": {"rate": 2.0}}
        for bad in [{"price": 3.0, "sensitivities": {}},
                    {"price": float("nan"), "sensitivities": {"rate": 2.0}},
                    {"price": 3.0, "sensitivities": {"rate": 2.1}}]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                scaling.compare(good, bad)

    def test_dependency_fingerprint_records_multiple_versions(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "Cargo.lock").write_text(
                'version = 4\n[[package]]\nname = "rand"\nversion = "0.8.5"\n'
                '[[package]]\nname = "rand"\nversion = "0.9.0"\n'
                '[[package]]\nname = "rayon"\nversion = "1.10.0"\n')
            versions = scaling.numerical_dependencies(directory)
            self.assertEqual(versions["rand"], ["0.8.5", "0.9.0"])
            self.assertEqual(versions["rayon"], ["1.10.0"])


if __name__ == "__main__":
    unittest.main()

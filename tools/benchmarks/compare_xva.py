"""Compare JSON dumps from the standalone XVA benchmark in two AD worktrees.

Run: python3 tools/benchmarks/compare_xva.py reference.json compiled.json
The benchmark writes dumps with --benchmark <paths> <samples> <output.json>.
"""

import json
import math
from pathlib import Path
import sys


def compare(reference_path, compiled_path, *, risk_rtol=1e-12, risk_atol=1e-9,
            price_rtol=1e-12, price_atol=1e-9, show_timing=True):
    """Compares every numerical output and published label before accepting timings."""
    reference = json.loads(Path(reference_path).read_text())
    compiled = json.loads(Path(compiled_path).read_text())
    if reference["paths"] != compiled["paths"]:
        raise ValueError("path counts differ")
    if reference.get("compilation_failures", 0) or compiled.get("compilation_failures", 0):
        raise ValueError("native compilation failed")

    def indexed(items, key):
        """Indexes output labels and rejects duplicates instead of silently overwriting them."""
        result = {key(item): item for item in items}
        if len(result) != len(items):
            raise ValueError("duplicate output labels")
        return result

    def difference(a, b, label):
        """Checks a finite price/cube value against the declared numerical tolerance."""
        if not all(map(math.isfinite, (a, b))) or not math.isclose(a, b, rel_tol=price_rtol, abs_tol=price_atol):
            raise ValueError(f"{label}: reference={a}, compiled={b}")
        return abs(a - b)

    cubes = [indexed(result["cubes"], lambda cube: cube["trade_id"]) for result in (reference, compiled)]
    if cubes[0].keys() != cubes[1].keys():
        raise ValueError("cube identifiers differ")
    cube_error = 0.0
    entries = 0
    for key, a in cubes[0].items():
        b = cubes[1][key]
        if len(a["npvs"]) != reference["paths"] or len(b["npvs"]) != compiled["paths"]:
            raise ValueError("cube path layout differs")
        if a["dates"] != b["dates"]:
            raise ValueError("cube dates differ")
        for left, right in zip(a["npvs"], b["npvs"], strict=True):
            if len(left) != len(a["dates"]) or len(right) != len(b["dates"]):
                raise ValueError("cube date layout differs")
            for x, y in zip(left, right, strict=True):
                cube_error = max(cube_error, difference(x, y, f"cube {key}"))
                entries += 1

    values = [indexed(result["values"], lambda value: (value["netting_set"], value["measure"])) for result in (reference, compiled)]
    if values[0].keys() != values[1].keys():
        raise ValueError("adjustment labels differ")
    value_error = max((difference(a["value"], values[1][key]["value"], key) for key, a in values[0].items()), default=0.0)

    risks = [indexed(result["sensitivities"], lambda risk: risk[0]) for result in (reference, compiled)]
    if risks[0].keys() != risks[1].keys():
        raise ValueError("risk labels differ")
    risk_error = 0.0
    for key, a in risks[0].items():
        b = risks[1][key][1]
        if not all(map(math.isfinite, (a[1], b))) or not math.isclose(a[1], b, rel_tol=risk_rtol, abs_tol=risk_atol):
            raise ValueError(f"{key}: reference={a[1]}, candidate={b}; risk_rtol={risk_rtol}")
        risk_error = max(risk_error, abs(a[1] - b) / max(1.0, abs(a[1])))
    print(f"Matched {entries} cube entries, {len(values[0])} adjustments and {len(risks[0])} labelled first risks.")
    print(f"Maximum errors: cube={cube_error:.6g}; adjustment={value_error:.6g}; scaled risk={risk_error:.6g}")
    for key in sorted(values[0]):
        print(f"{'.'.join(key)}: reference={values[0][key]['value']:.12g}, compiled={values[1][key]['value']:.12g}")
    if show_timing:
        print(f"Reference median={reference['median_run_ms']:.3f} ms; candidate median={compiled['median_run_ms']:.3f} ms; ratio={reference['median_run_ms']/compiled['median_run_ms']:.3f}x")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: compare_xva.py reference.json compiled.json")
    compare(*sys.argv[1:])

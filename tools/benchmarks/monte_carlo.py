"""Compare existing Monte Carlo APIs in separate old/new AD worktrees."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess

from compare_branches import command, metadata


CASES = ("equity-call", "equity-put", "equity-call-value", "script-equity",
         "script-rates", "script-digital", "build-brownian", "build-hullwhite", "build-lgm")


def compare(left, right, location="snapshot"):
    """Check every value, risk label, cashflow, generated path and date."""
    if isinstance(left, dict) and isinstance(right, dict):
        if left.keys() != right.keys():
            raise ValueError(f"{location}: keys differ")
        for key in left:
            compare(left[key], right[key], f"{location}.{key}")
    elif isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            raise ValueError(f"{location}: lengths differ")
        for index, (a, b) in enumerate(zip(left, right, strict=True)):
            compare(a, b, f"{location}[{index}]")
    elif isinstance(left, (int, float)) and isinstance(right, (int, float)):
        risk = "sensitivities" in location
        rtol, atol = (1e-8, 1e-8) if risk else (1e-11, 1e-8)
        if not all(map(math.isfinite, (left, right))) or not math.isclose(left, right, rel_tol=rtol, abs_tol=atol):
            raise ValueError(f"{location}: {left} vs {right}")
    elif left is None or right is None or type(left) is not type(right) or left != right:
        raise ValueError(f"{location}: {left!r} vs {right!r}")


def main():
    """Builds independent branch binaries and runs the configured comparison protocol."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=Path.cwd())
    parser.add_argument("--baseline-target", type=Path, required=True)
    parser.add_argument("--candidate-target", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("target/monte-carlo-comparison"))
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--threads", type=int, default=10)
    parser.add_argument("--paths", type=int, nargs="+", default=[1000, 10000])
    parser.add_argument("--case", choices=CASES, action="append")
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    if min(args.rounds, args.samples, args.threads, *args.paths) < 1:
        parser.error("counts must be positive")
    directories = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    targets = {"baseline": args.baseline_target.resolve(), "candidate": args.candidate_target.resolve()}
    if len(set(directories.values())) != 2 or len(set(targets.values())) != 2:
        parser.error("use separate worktrees and target directories")
    fixture = "benchmarks/src/bin/monte_carlo_benchmark.rs"
    hashes = {label: hashlib.sha256((directory / fixture).read_bytes()).hexdigest()
              for label, directory in directories.items()}
    numerical_hashes = {label: hashlib.sha256(re.sub(
        rb"type BenchmarkScalar = (?:Recorded|DualFwd);",
        b"type BenchmarkScalar = BenchmarkScalarType;", (directory / fixture).read_bytes())).hexdigest()
        for label, directory in directories.items()}
    if len(set(numerical_hashes.values())) != 1:
        raise ValueError("Monte Carlo numerical fixtures differ between branches")
    env = {**os.environ, "RAYON_NUM_THREADS": str(args.threads)}
    env.pop("QUANTSUPPORT_AD_BACKEND", None)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = {
        "purpose": "harness smoke check" if args.smoke else "same-API Monte Carlo comparison",
        "compiler": command(["rustc", "+stable", "--version", "--verbose"], directories["candidate"]),
        "machine": platform.platform(), "threads": args.threads, "fixture_sha256": hashes,
        "numerical_fixture_sha256": numerical_hashes,
        "build_environment": {key: value for key, value in env.items() if key in {
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"
        } or key.startswith("CARGO_PROFILE_RELEASE_")},
        "sources": {label: metadata(directory) for label, directory in directories.items()},
        "tolerances": {"price_rtol": 1e-11, "price_atol": 1e-8, "risk_rtol": 1e-8, "risk_atol": 1e-8},
        "cases": {},
    }
    for label, directory in directories.items():
        subprocess.run(["cargo", "+stable", "build", "--release", "--locked", "-p", "benchmarks", "--bin", "monte_carlo_benchmark"],
                       cwd=directory, env={**env, "CARGO_TARGET_DIR": str(targets[label])}, check=True)
    if manifest["sources"] != {label: metadata(directory) for label, directory in directories.items()}:
        raise ValueError("sources changed during build")
    for case in args.case or CASES:
        for paths in [16] if args.smoke else args.paths:
            rounds, samples = (1, 1) if args.smoke else (args.rounds, args.samples)
            repeats = 1 if args.smoke or case.startswith("script-") else 10
            name = f"{case}-{paths}"
            observations = {label: [] for label in directories}
            for index in range(rounds):
                order = ["baseline", "candidate"] if index % 2 == 0 else ["candidate", "baseline"]
                for label in order:
                    raw = output / f"{name}-{label}-{index}.json"
                    print(f"{name}: round {index + 1}/{rounds}, {label}", flush=True)
                    with (output / f"{name}-{label}-{index}.log").open("w") as log:
                        subprocess.run([str(targets[label] / "release/monte_carlo_benchmark"), case,
                                        str(paths), str(samples), str(repeats), str(raw)],
                                       cwd=directories[label], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
                    result = json.loads(raw.read_text())
                    if (result["case"], result["path_count"], result["samples"], result["repeats"]) != (case, paths, samples, repeats):
                        raise ValueError("fixture parameters differ")
                    observations[label].append(result)
                compare(observations["baseline"][-1]["snapshot"], observations["candidate"][-1]["snapshot"])
            summary = {label: {
                key: statistics.median(item[key] for item in results)
                for key in ("setup_ms", "first_call_ms", "median_call_ms")
            } for label, results in observations.items()}
            for label, results in observations.items():
                summary[label]["setup_plus_first_ms"] = statistics.median(
                    item["setup_ms"] + item["first_call_ms"] for item in results)
                summary[label]["round_medians_ms"] = [item["median_call_ms"] for item in results]
            manifest["cases"][name] = {"paths": paths, "rounds": rounds, "samples": samples, "repeats": repeats,
                                        "summary_ms": summary,
                                        "raw": {label: [str(output / f"{name}-{label}-{i}.json") for i in range(rounds)]
                                                for label in directories}}
            old, new = summary["baseline"]["median_call_ms"], summary["candidate"]["median_call_ms"]
            print(f"{name}: old={old:.3f} ms, new={new:.3f} ms, old/new={old / new:.3f}x; parity passed", flush=True)
            (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    if manifest["sources"] != {label: metadata(directory) for label, directory in directories.items()}:
        raise ValueError("sources changed during timing; discard results")
    print(f"Results: {output / 'manifest.json'}", flush=True)


if __name__ == "__main__":
    main()

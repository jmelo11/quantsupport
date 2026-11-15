"""Measure existing XvaEngine.run across old/new branches, including preparation and quote refresh."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess

from compare_branches import metadata, command
from compare_xva import compare


def fixture_hashes(directory):
    """Fingerprint shared XVA equations/data, excluding candidate-only preparation calls."""
    fixture = directory / "benchmarks/src/bin/xva_benchmark.rs"
    source = fixture.read_bytes()
    normalized = re.sub(rb"type BenchmarkScalar = (?:Recorded|DualFwd);",
                        b"type BenchmarkScalar = BenchmarkScalarType;", source)
    normalized = re.sub(rb"(?ms)^    // BEGIN CANDIDATE PREPARATION\n.*?^    // END CANDIDATE PREPARATION\n",
                        b"", normalized)
    files = [directory / "benchmarks/src/xva_fixture.rs",
             *sorted((directory / "benchmarks/data/xva").glob("*.json"))]
    return {"driver_raw": hashlib.sha256(source).hexdigest(),
            "numerical_driver": hashlib.sha256(normalized).hexdigest(),
            "shared_files": {str(path.relative_to(directory)): hashlib.sha256(path.read_bytes()).hexdigest()
                             for path in files}}


def main():
    """Build separate benchmark binaries and compare complete XVA results before accepting timings."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=Path.cwd())
    parser.add_argument("--baseline-target", type=Path, required=True)
    parser.add_argument("--candidate-target", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("target/prepared-xva-comparison"))
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--large-rounds", type=int, default=1)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--threads", type=int, default=10)
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--case", action="append", help="Run only the named case (repeatable)")
    args = parser.parse_args()
    if min(args.rounds, args.large_rounds, args.samples, args.threads) < 1:
        parser.error("counts must be positive")
    paths = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    targets = {"baseline": args.baseline_target.resolve(), "candidate": args.candidate_target.resolve()}
    if paths["baseline"] == paths["candidate"] or targets["baseline"] == targets["candidate"]:
        parser.error("use separate worktrees and target directories")
    fixtures = {label: fixture_hashes(path) for label, path in paths.items()}
    if fixtures["baseline"]["numerical_driver"] != fixtures["candidate"]["numerical_driver"] or fixtures["baseline"]["shared_files"] != fixtures["candidate"]["shared_files"]:
        raise ValueError("XVA numerical driver, loaders or market data differ between branches")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "RAYON_NUM_THREADS": str(args.threads)}
    env.pop("QUANTSUPPORT_AD_BACKEND", None)
    cases = [
        ("small-auto", 16, True, "--benchmark", 1.0, args.rounds),
        ("small-prepared", 16, True, "--benchmark-prepared", 1.0, args.rounds),
        ("margined-auto", 1000, True, "--benchmark", 1.0, args.rounds),
        ("margined-prepared", 1000, True, "--benchmark-prepared", 1.0, args.rounds),
        ("unmargined-auto", 1000, False, "--benchmark", 1.0, args.rounds),
        ("market-refresh", 1000, True, "--benchmark-refresh", 1.01, args.rounds),
        ("large-auto", 10000, True, "--benchmark", 1.0, args.large_rounds),
    ]
    if args.case:
        unknown = set(args.case) - {case[0] for case in cases}
        if unknown:
            parser.error(f"unknown cases: {sorted(unknown)}")
        cases = [case for case in cases if case[0] in args.case]
    manifest = {
        "purpose": "harness smoke check" if args.smoke else "same-API prepared XVA comparison",
        "compiler": command(["rustc", "+stable", "--version", "--verbose"], paths["candidate"]),
        "machine": platform.platform(), "threads": args.threads,
        "sources": {label: metadata(path) for label, path in paths.items()},
        "fixture_hashes": fixtures,
        "tolerances": {"price_rtol": 1e-11, "price_atol": 1e-7, "risk_rtol": 1e-8, "risk_atol": 1e-8},
        "cases": {},
    }
    # Complete all builds before starting any timed process.
    for label, directory in paths.items():
        subprocess.run(["cargo", "+stable", "build", "--release", "--locked", "-p", "benchmarks", "--bin", "xva_benchmark"],
                       cwd=directory, env={**env, "CARGO_TARGET_DIR": str(targets[label])}, check=True)
    if manifest["sources"] != {label: metadata(path) for label, path in paths.items()}:
        raise ValueError("sources changed during build")
    for name, count, margin, mode, multiplier, rounds in cases:
        if args.smoke:
            count, rounds, samples = 16, 1, 1
        else:
            samples = args.samples
        results = {label: [] for label in paths}
        for index in range(rounds):
            order = ["baseline", "candidate"] if index % 2 == 0 else ["candidate", "baseline"]
            raw = {}
            for label in order:
                raw[label] = output / f"{name}-{label}-{index}.json"
                arguments = [str(targets[label] / "release/xva_benchmark"), mode if label == "candidate" else "--benchmark",
                             str(count), str(samples), str(raw[label]), str(margin).lower(), str(multiplier)]
                print(f"{name}: round {index + 1}/{rounds}, {label}, {count} paths", flush=True)
                with (output / f"{name}-{label}-{index}.log").open("w") as log:
                    subprocess.run(arguments, cwd=paths[label], env=env, stdout=log,
                                   stderr=subprocess.STDOUT, check=True)
                result = json.loads(raw[label].read_text())
                if result["margin"] != margin or result["fx_multiplier"] != multiplier:
                    raise ValueError("fixture options do not match")
                if label == "candidate" and mode == "--benchmark-refresh" and not result["reused_code"]:
                    raise ValueError("market refresh recompiled rather than rebound the program")
                results[label].append(result)
            compare(raw["baseline"], raw["candidate"], show_timing=False, **manifest["tolerances"])
        summary = {}
        for label, observations in results.items():
            summary[label] = {
                key: statistics.median(result.get(key, 0.0) for result in observations)
                for key in ("setup_ms", "preparation_ms", "first_run_ms", "median_run_ms",
                            "initial_preparation_ms", "market_context_update_ms", "engine_market_update_ms")
            }
            summary[label]["first_evaluation_ms"] = statistics.median(
                result["preparation_ms"] + result["first_run_ms"] for result in observations)
            summary[label]["market_refresh_total_ms"] = statistics.median(
                result["market_context_update_ms"] + result["engine_market_update_ms"]
                + result["preparation_ms"] + result["first_run_ms"] for result in observations)
            summary[label]["setup_plus_first_ms"] = statistics.median(
                result["setup_ms"] + result["preparation_ms"] + result["first_run_ms"] for result in observations)
        manifest["cases"][name] = {"paths": count, "margin": margin, "mode": mode,
                                    "fx_multiplier": multiplier, "rounds": rounds,
                                    "samples": samples, "summary_ms": summary,
                                    "raw": {label: [str(output / f"{name}-{label}-{i}.json") for i in range(rounds)] for label in paths}}
        if not args.smoke:
            old, new = summary["baseline"], summary["candidate"]
            print(f"{name}: old run {old['median_run_ms']:.3f} ms; prepared reuse {new['median_run_ms']:.3f} ms; "
                  f"old first {old['first_evaluation_ms']:.3f} ms; new preparation + first {new['first_evaluation_ms']:.3f} ms", flush=True)
        (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    if manifest["sources"] != {label: metadata(path) for label, path in paths.items()}:
        raise ValueError("sources changed during timing; discard results")
    print(f"Results: {output / 'manifest.json'}", flush=True)


if __name__ == "__main__":
    main()

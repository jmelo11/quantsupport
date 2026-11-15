"""Compare existing pricing families after replacing the AD pipeline."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess

from compare_branches import command, metadata
from monte_carlo import compare

CASES = ("dcf", "portfolio", "bond", "loan", "tree", "fourier", "black-equity",
         "equity-forward", "fx-option", "fx-forward", "cds", "futures",
         "black-caplet", "black-cap", "hw-caplet", "hw-cap", "hw-swaption")


def fixture_hash(directory, filename):
    """Only the build-time scalar spelling differs across the two branches."""
    source = (directory / "benchmarks/src/bin" / filename).read_bytes()
    normalized = re.sub(rb"type BenchmarkScalar = (?:Recorded|DualFwd);",
                        b"type BenchmarkScalar = BenchmarkScalarType;", source)
    return {"raw": hashlib.sha256(source).hexdigest(),
            "numerical_fixture": hashlib.sha256(normalized).hexdigest()}


def main():
    """Builds independent branch binaries and runs the configured comparison protocol."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=Path.cwd())
    parser.add_argument("--baseline-target", type=Path, required=True)
    parser.add_argument("--candidate-target", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("target/compiled-engine-comparison"))
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--threads", type=int, default=10)
    parser.add_argument("--case", action="append", choices=CASES)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    if min(args.rounds, args.samples, args.threads) < 1:
        parser.error("counts must be positive")
    directories = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    targets = {"baseline": args.baseline_target.resolve(), "candidate": args.candidate_target.resolve()}
    if len(set(directories.values())) != 2 or len(set(targets.values())) != 2:
        parser.error("use separate worktrees and target directories")
    fixtures = {label: fixture_hash(directory, "engine_benchmark.rs") for label, directory in directories.items()}
    if len({value["numerical_fixture"] for value in fixtures.values()}) != 1:
        raise ValueError("numerical engine fixtures differ")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "RAYON_NUM_THREADS": str(args.threads)}
    env.pop("QUANTSUPPORT_AD_BACKEND", None)
    manifest = {"purpose": "harness smoke check" if args.smoke else "compiled-only AD vs old-tape public pricing APIs",
                "compiler": command(["rustc", "+stable", "--version", "--verbose"], directories["candidate"]),
                "threads": args.threads, "machine": platform.platform(), "fixture_hashes": fixtures,
                "sources": {label: metadata(directory) for label, directory in directories.items()},
                "tolerances": {"price_rtol": 1e-11, "price_atol": 1e-8, "risk_rtol": 1e-8, "risk_atol": 1e-8},
                "cases": {}}
    for label, directory in directories.items():
        subprocess.run(["cargo", "+stable", "build", "--release", "--locked", "-p", "benchmarks", "--bin", "engine_benchmark"],
                       cwd=directory, env={**env, "CARGO_TARGET_DIR": str(targets[label])}, check=True)
    if manifest["sources"] != {label: metadata(directory) for label, directory in directories.items()}:
        raise ValueError("sources changed during build")
    for case in args.case or CASES:
        rounds, samples = (1, 1) if args.smoke else (args.rounds, args.samples)
        repeats = 1 if args.smoke else (3 if case in {"tree", "fourier"} else 100)
        observations = {label: [] for label in directories}
        for index in range(rounds):
            order = ["baseline", "candidate"] if index % 2 == 0 else ["candidate", "baseline"]
            for label in order:
                raw = output / f"{case}-{label}-{index}.json"
                print(f"{case}: round {index+1}/{rounds}, {label}", flush=True)
                with (output / f"{case}-{label}-{index}.log").open("w") as log:
                    subprocess.run([str(targets[label] / "release/engine_benchmark"), case,
                                    str(samples), str(repeats), str(raw)], cwd=directories[label], env=env,
                                   stdout=log, stderr=subprocess.STDOUT, check=True)
                result = json.loads(raw.read_text())
                if result["case"] != case:
                    raise ValueError("case mismatch")
                observations[label].append(result)
            compare(observations["baseline"][-1]["snapshot"], observations["candidate"][-1]["snapshot"])
        summary = {label: {key: statistics.median(item[key] for item in results)
                           for key in ("setup_ms", "first_call_ms", "median_call_ms")}
                   for label, results in observations.items()}
        for label, results in observations.items():
            summary[label]["round_medians_ms"] = [item["median_call_ms"] for item in results]
        manifest["cases"][case] = {"rounds": rounds, "samples": samples, "repeats": repeats,
                                    "summary_ms": summary,
                                    "raw": {label: [str(output / f"{case}-{label}-{index}.json") for index in range(rounds)]
                                            for label in directories}}
        old, new = summary["baseline"]["median_call_ms"], summary["candidate"]["median_call_ms"]
        print(f"{case}: old={old:.6f} ms, new={new:.6f} ms, old/new={old/new:.3f}x; parity passed", flush=True)
        (output / "manifest.json").write_text(json.dumps(manifest, indent=2)+"\n")
    if manifest["sources"] != {label: metadata(directory) for label, directory in directories.items()}:
        raise ValueError("sources changed during timing; discard results")
    print(f"Results: {output / 'manifest.json'}", flush=True)


if __name__ == "__main__":
    main()

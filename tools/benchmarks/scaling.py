"""Compare request, portfolio, path and worker scaling against the old AD branch.

Both worktrees use the same benchmark equations and public pricing methods.
Builds finish before timing; fresh processes alternate between implementations.
Every paired numerical snapshot must match before its timings are accepted.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import tomllib

from compare_branches import command, metadata
from engines import CASES as ENGINE_CASES
from monte_carlo import compare


MC_CASES = ("equity-call", "script-equity", "script-rates", "script-digital", "build-lgm")
DRIVERS = ("engine_benchmark.rs", "monte_carlo_benchmark.rs")


def fixture_hashes(directory):
    """Fingerprint equations with only the explicit scalar alias normalized."""
    result = {}
    for filename in DRIVERS:
        source = (directory / "benchmarks/src/bin" / filename).read_bytes()
        normalized = re.sub(rb"type BenchmarkScalar = (?:Recorded|DualFwd);",
                            b"type BenchmarkScalar = BenchmarkScalarType;", source)
        result[filename] = {"raw": hashlib.sha256(source).hexdigest(),
                            "numerical": hashlib.sha256(normalized).hexdigest()}
    return result


def prepare_baseline(baseline, candidate, toolchain):
    """Install benchmark-only drivers in a dedicated old-implementation worktree."""
    if not (baseline / "src/ad/dual.rs").is_file():
        raise ValueError("--prepare-baseline requires a worktree containing the old DualFwd implementation")
    for filename in (*DRIVERS, "pricing_benchmark.rs"):
        source = (candidate / "benchmarks/src/bin" / filename).read_text()
        source = source.replace("type BenchmarkScalar = Recorded;", "type BenchmarkScalar = DualFwd;")
        destination = baseline / "benchmarks/src/bin" / filename
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(source)
    manifest = baseline / "benchmarks/Cargo.toml"
    source = manifest.read_text()
    if 'serde_json = ' not in source:
        source = source.replace("[dependencies]\n", '[dependencies]\nserde_json = "1"\n', 1)
        manifest.write_text(source)
    # Seed with the candidate lock so unrelated numerical dependencies stay fixed.
    shutil.copy2(candidate / "Cargo.lock", baseline / "Cargo.lock")
    subprocess.run(["cargo", f"+{toolchain}", "metadata", "--offline", "--format-version", "1"],
                   cwd=baseline, check=True, stdout=subprocess.DEVNULL)


def numerical_dependencies(directory):
    """Require the same RNG, parallel runtime, linear algebra and data dependencies."""
    packages = tomllib.loads((directory / "Cargo.lock").read_text())["package"]
    names = {"rand", "rand_chacha", "rayon", "rayon-core", "nalgebra", "chrono",
             "num-complex", "serde", "serde_json"}
    return {name: sorted(package["version"] for package in packages if package["name"] == name)
            for name in sorted(names)}


def workloads(engine_cases, mc_cases, portfolios, paths, threads, groups):
    """Keep request and portfolio scaling separate from parallel path scaling."""
    cases = []
    if "pricing" in groups:
        for engine in engine_cases:
            counts = [2, *portfolios] if engine == "portfolio" else [2]
            for count in dict.fromkeys(counts):
                for mode in ("value", "risk"):
                    size = f"-{count}" if engine == "portfolio" else ""
                    cases.append({"name": f"{engine}{size}-{mode}", "group": "pricing",
                                  "case": engine, "request_mode": mode,
                                  "portfolio_trades": count, "threads": 1})
    if "mc" in groups:
        for case in mc_cases:
            for count in paths:
                for workers in threads:
                    cases.append({"name": f"{case}-{count}-t{workers}", "group": "mc",
                                  "case": case, "paths": count, "threads": workers})
    return cases


def check_result(result, case, samples, repeats):
    """Reject a driver that silently ran different requests or workload dimensions."""
    if (result["case"], result["samples"], result["repeats"]) != (case["case"], samples, repeats):
        raise ValueError("driver case, samples or repetitions differ from the requested workload")
    if case["group"] == "pricing":
        if (result["request_mode"], result["portfolio_trades"]) != (case["request_mode"], case["portfolio_trades"]):
            raise ValueError("driver requests or portfolio size differ")
        risk = result["snapshot"]["sensitivities"]
        if bool(risk) != (case["request_mode"] == "risk"):
            raise ValueError("snapshot does not contain exactly the requested risk measures")
    elif result["path_count"] != case["paths"]:
        raise ValueError("driver path count differs")
    for key in ("setup_ms", "first_call_ms", "median_call_ms"):
        value = result[key]
        if not isinstance(value, (int, float)) or not 0 < value < float("inf"):
            raise ValueError(f"invalid timing: {key}={value}")


def summarize(results):
    """Retain round medians and paired complete first-evaluation boundaries."""
    summary = {key: statistics.median(item[key] for item in results)
               for key in ("setup_ms", "first_call_ms", "median_call_ms")}
    summary["setup_plus_first_ms"] = statistics.median(
        item["setup_ms"] + item["first_call_ms"] for item in results)
    summary["round_medians_ms"] = [item["median_call_ms"] for item in results]
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=Path.cwd())
    parser.add_argument("--baseline-target", type=Path, required=True)
    parser.add_argument("--candidate-target", type=Path, required=True)
    parser.add_argument("--prepare-baseline", action="store_true", help="Install benchmark-only fixtures in a dedicated old-AD worktree")
    parser.add_argument("--toolchain", default="stable")
    parser.add_argument("--output", type=Path, default=Path("target/ad-scaling-comparison"))
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--threads", type=int, nargs="+", default=[1, 4, 10])
    parser.add_argument("--paths", type=int, nargs="+", default=[64, 1000, 10000])
    parser.add_argument("--portfolio-trades", type=int, nargs="+", default=[10, 100, 1000])
    parser.add_argument("--group", choices=["pricing", "mc"], action="append")
    parser.add_argument("--engine-case", choices=ENGINE_CASES, action="append")
    parser.add_argument("--mc-case", choices=MC_CASES, action="append")
    parser.add_argument("--smoke", action="store_true", help="Validate the harness using one short process round; no performance claims")
    args = parser.parse_args()
    if min(args.rounds, args.samples, *args.threads, *args.paths, *args.portfolio_trades) < 1:
        parser.error("all counts must be positive")
    trees = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    targets = {"baseline": args.baseline_target.resolve(), "candidate": args.candidate_target.resolve()}
    if len(set(trees.values())) != 2 or len(set(targets.values())) != 2:
        parser.error("use separate worktrees and target directories")
    if args.prepare_baseline:
        prepare_baseline(trees["baseline"], trees["candidate"], args.toolchain)
    hashes = {side: fixture_hashes(tree) for side, tree in trees.items()}
    for filename in DRIVERS:
        if hashes["baseline"][filename]["numerical"] != hashes["candidate"][filename]["numerical"]:
            raise ValueError(f"numerical fixtures differ: {filename}")
    dependencies = {side: numerical_dependencies(tree) for side, tree in trees.items()}
    if dependencies["baseline"] != dependencies["candidate"]:
        raise ValueError("numerical dependency versions differ; prepare a dedicated baseline with the candidate lock")
    cases = workloads(args.engine_case or ENGINE_CASES, args.mc_case or MC_CASES,
                      args.portfolio_trades, [8] if args.smoke else args.paths,
                      args.threads, args.group or ["pricing", "mc"])
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.pop("QUANTSUPPORT_AD_BACKEND", None)
    manifest = {
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(), "complete": False,
        "purpose": "harness smoke check" if args.smoke else "old vs compiled AD request and workload scaling",
        "compiler": command(["rustc", f"+{args.toolchain}", "--version", "--verbose"], trees["candidate"]),
        "machine": {"platform": platform.platform(), "architecture": platform.machine(), "logical_cpus": os.cpu_count()},
        "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "build_environment": {key: value for key, value in env.items() if key in {
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"
        } or key.startswith("CARGO_PROFILE_RELEASE_")},
        "sources": {side: metadata(tree) for side, tree in trees.items()},
        "fixture_hashes": hashes,
        "numerical_dependencies": dependencies,
        "tolerances": {"price_rtol": 1e-11, "price_atol": 1e-8, "risk_rtol": 1e-8, "risk_atol": 1e-8},
        "cases": {},
    }
    binaries = set("engine_benchmark" if case["group"] == "pricing" else "monte_carlo_benchmark" for case in cases)
    for side, tree in trees.items():
        build = ["cargo", f"+{args.toolchain}", "build", "--release", "--locked", "-p", "benchmarks"]
        for binary in sorted(binaries):
            build.extend(["--bin", binary])
        subprocess.run(build, cwd=tree, env={**env, "CARGO_TARGET_DIR": str(targets[side])}, check=True)
    if manifest["sources"] != {side: metadata(tree) for side, tree in trees.items()}:
        raise ValueError("sources changed during build")
    rounds, samples = (1, 1) if args.smoke else (args.rounds, args.samples)
    for case in cases:
        if args.smoke:
            repeats = 1
        elif case["group"] == "pricing":
            repeats = 3 if case["case"] in {"tree", "fourier"} else max(1, 2000 // case["portfolio_trades"])
        else:
            repeats = 1 if case["case"].startswith("script-") else 5
        observations = {side: [] for side in trees}
        raw_paths = {side: [] for side in trees}
        for index in range(rounds):
            order = ["baseline", "candidate"] if index % 2 == 0 else ["candidate", "baseline"]
            for side in order:
                raw = output / f"{case['name']}-{side}-{index}.json"
                binary = "engine_benchmark" if case["group"] == "pricing" else "monte_carlo_benchmark"
                dimensions = ([str(samples), str(repeats), str(raw), case["request_mode"], str(case["portfolio_trades"])]
                              if case["group"] == "pricing" else [str(case["paths"]), str(samples), str(repeats), str(raw)])
                print(f"{case['name']}: {side}, round {index + 1}/{rounds}", flush=True)
                with raw.with_suffix(".log").open("w") as log:
                    subprocess.run([str(targets[side] / "release" / binary), case["case"], *dimensions],
                                   cwd=trees[side], env={**env, "RAYON_NUM_THREADS": str(case["threads"])},
                                   stdout=log, stderr=subprocess.STDOUT, check=True)
                result = json.loads(raw.read_text())
                check_result(result, case, samples, repeats)
                observations[side].append(result)
                raw_paths[side].append({"path": str(raw), "sha256": hashlib.sha256(raw.read_bytes()).hexdigest()})
            compare(observations["baseline"][-1]["snapshot"], observations["candidate"][-1]["snapshot"])
        summary = {side: summarize(results) for side, results in observations.items()}
        ratio = summary["baseline"]["median_call_ms"] / summary["candidate"]["median_call_ms"]
        manifest["cases"][case["name"]] = {**case, "rounds": rounds, "samples": samples,
                                               "repeats": repeats, "parity": "passed", "summary_ms": summary,
                                               "old_over_new": ratio, "raw": raw_paths}
        (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(f"{case['name']}: old/new={ratio:.3f}x; parity passed", flush=True)
    if manifest["sources"] != {side: metadata(tree) for side, tree in trees.items()}:
        raise ValueError("sources changed during timing; discard results")
    manifest["complete"] = True
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Completed {len(cases)} comparisons: {output / 'manifest.json'}", flush=True)


if __name__ == "__main__":
    main()

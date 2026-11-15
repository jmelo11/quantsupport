"""Benchmark the same pricing API from separate old/new AD worktrees.

Both builds finish before timing starts, use one pinned compiler, and have
separate artifact directories. No Cargo feature or environment backend switch
selects an implementation. Small --smoke runs validate the harness only.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess

from compare_xva import compare as compare_xva


def command(arguments, directory, env=None):
    """Runs a read-only command and returns its textual output."""
    return subprocess.check_output(arguments, cwd=directory, env=env, text=True)


def metadata(directory):
    """Records branch, source, lockfile and fixture hashes for reproducible comparisons."""
    patch = subprocess.check_output(["git", "diff", "--binary", "HEAD"], cwd=directory)
    untracked = command(["git", "ls-files", "--others", "--exclude-standard"], directory).splitlines()
    return {
        "directory": str(directory),
        "branch": command(["git", "branch", "--show-current"], directory).strip(),
        "commit": command(["git", "rev-parse", "HEAD"], directory).strip(),
        "dirty_status": command(["git", "status", "--short"], directory).splitlines(),
        "tracked_diff_sha256": hashlib.sha256(patch).hexdigest(),
        "untracked_sha256": {name: hashlib.sha256((directory / name).read_bytes()).hexdigest() for name in untracked if (directory / name).is_file()},
        "lock_sha256": hashlib.sha256((directory / "Cargo.lock").read_bytes()).hexdigest(),
        "fixture_sha256": hashlib.sha256((directory / "benchmarks/src/bin/pricing_benchmark.rs").read_bytes()).hexdigest(),
    }


def compare_pricer(reference, candidate):
    """Rejects nonfinite or mismatched prices and first risks across shared scenarios."""
    if reference["case"] != candidate["case"]:
        raise ValueError("pricer cases differ")
    if len(reference["scenarios"]) != len(candidate["scenarios"]):
        raise ValueError("scenario counts differ")
    for (pv_a, risk_a), (pv_b, risk_b) in zip(reference["scenarios"], candidate["scenarios"], strict=True):
        if not all(map(math.isfinite, (pv_a, pv_b))) or not math.isclose(pv_a, pv_b, rel_tol=1e-12, abs_tol=1e-8):
            raise ValueError(f"price mismatch: {pv_a} vs {pv_b}")
        for a, b in zip(risk_a, risk_b, strict=True):
            if not all(map(math.isfinite, (a, b))) or not math.isclose(a, b, rel_tol=1e-10, abs_tol=1e-8):
                raise ValueError(f"first-risk mismatch: {a} vs {b}")


def main():
    """Builds independent branch binaries and runs the configured comparison protocol."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, default=Path("target/branch-ad-comparison"))
    parser.add_argument("--baseline-target", type=Path)
    parser.add_argument("--candidate-target", type=Path)
    parser.add_argument("--toolchain", default="stable")
    parser.add_argument("--samples", type=int, default=9)
    parser.add_argument("--repeats", type=int, default=20_000)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--threads", type=int, default=10)
    parser.add_argument("--xva", action="store_true")
    parser.add_argument("--xva-paths", type=int, default=1000)
    parser.add_argument("--xva-samples", type=int, default=3)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    if min(args.samples, args.repeats, args.rounds, args.threads, args.xva_paths, args.xva_samples) <= 0:
        parser.error("benchmark counts must be positive")
    if args.smoke:
        args.samples, args.repeats, args.rounds = 1, 100, 1
        args.xva_paths, args.xva_samples = 16, 1
    directories = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    if directories["baseline"] == directories["candidate"]:
        parser.error("use different worktrees for old and new AD")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "RAYON_NUM_THREADS": str(args.threads)}
    # Refuse to carry a stale in-process selector into cross-branch benchmarks.
    env.pop("QUANTSUPPORT_AD_BACKEND", None)
    manifest = {
        "purpose": "harness smoke check" if args.smoke else "timing comparison",
        "toolchain": command(["rustc", f"+{args.toolchain}", "--version", "--verbose"], directories["candidate"]),
        "threads": args.threads,
        "machine": {"platform": platform.platform(), "architecture": platform.machine(), "logical_cpus": os.cpu_count()},
        "build_environment": {name: value for name, value in env.items() if name in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"} or name.startswith("CARGO_PROFILE_RELEASE_")},
        "worktrees": {label: metadata(path) for label, path in directories.items()},
        "results": {},
    }
    targets = {
        "baseline": args.baseline_target.resolve() if args.baseline_target else output / "builds/baseline",
        "candidate": args.candidate_target.resolve() if args.candidate_target else output / "builds/candidate",
    }
    if targets["baseline"] == targets["candidate"]:
        parser.error("old and new AD must use separate target directories")
    # Every build completes before either binary is timed.
    for label, directory in directories.items():
        build_env = {**env, "CARGO_TARGET_DIR": str(targets[label])}
        subprocess.run(["cargo", f"+{args.toolchain}", "build", "--release", "--locked", "-p", "benchmarks", "--bin", "pricing_benchmark"], cwd=directory, env=build_env, check=True)
        if args.xva:
            subprocess.run(["cargo", f"+{args.toolchain}", "build", "--release", "--locked", "-p", "benchmarks", "--bin", "xva_benchmark"], cwd=directory, env=build_env, check=True)
    if manifest["worktrees"] != {label: metadata(path) for label, path in directories.items()}:
        raise ValueError("sources changed while building; repeat on stable worktrees")
    for case in ["bond", "equity"]:
        observations = {label: [] for label in directories}
        for round_index in range(args.rounds):
            order = ["baseline", "candidate"] if round_index % 2 == 0 else ["candidate", "baseline"]
            for label in order:
                binary = targets[label] / "release/pricing_benchmark"
                result = json.loads(command([str(binary), case, str(args.samples), str(args.repeats)], directories[label], env))
                observations[label].append(result)
            compare_pricer(observations["baseline"][-1], observations["candidate"][-1])
        manifest["results"][case] = observations
        if args.smoke:
            print(f"{case}: pricing and first-risk parity passed; smoke timings are not performance evidence.", flush=True)
        else:
            old = statistics.median(item["median_us"] for item in observations["baseline"])
            new = statistics.median(item["median_us"] for item in observations["candidate"])
            print(f"{case}: baseline={old:.3f} us, candidate={new:.3f} us, ratio={old/new:.3f}x", flush=True)
    if args.xva:
        observations = {label: [] for label in directories}
        for round_index in range(args.rounds):
            order = ["baseline", "candidate"] if round_index % 2 == 0 else ["candidate", "baseline"]
            paths = {}
            for label in order:
                directory = directories[label]
                paths[label] = output / f"xva-{label}-{round_index}.json"
                with (output / f"xva-{label}-{round_index}.txt").open("w") as log:
                    subprocess.run([str(targets[label] / "release/xva_benchmark"), "--benchmark", str(args.xva_paths), str(args.xva_samples), str(paths[label])], cwd=directory, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
                observations[label].append(str(paths[label]))
            compare_xva(paths["baseline"], paths["candidate"], risk_rtol=1e-4, show_timing=False)
        if not args.smoke:
            times = {label: statistics.median(json.loads(Path(path).read_text())["median_run_ms"] for path in paths) for label, paths in observations.items()}
            print(f"xva: baseline={times['baseline']:.3f} ms, candidate={times['candidate']:.3f} ms, ratio={times['baseline']/times['candidate']:.3f}x", flush=True)
        manifest["results"]["xva"] = observations
        manifest["xva_risk_rtol"] = 1e-4
    if manifest["worktrees"] != {label: metadata(path) for label, path in directories.items()}:
        raise ValueError("sources changed during timing; discard these measurements")
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Recorded compiler, revisions, fixtures and raw results in {output / 'manifest.json'}", flush=True)


if __name__ == "__main__":
    main()

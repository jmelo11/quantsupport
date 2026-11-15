"""Compare full swap exposure cubes at unchanged trade, path and date counts."""

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


def main():
    """Build separate branch binaries and validate every cube fingerprint before timing claims."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=Path.cwd())
    parser.add_argument("--baseline-target", type=Path, required=True)
    parser.add_argument("--candidate-target", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("target/pfe-comparison"))
    parser.add_argument("--trades", type=int, nargs="+", default=[100, 1000])
    parser.add_argument("--paths", type=int, default=500)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--samples", type=int, default=1)
    parser.add_argument("--threads", type=int, default=10)
    args = parser.parse_args()
    if min(*args.trades, args.paths, args.rounds, args.threads) < 1 or args.samples < 0:
        parser.error("counts must be positive; zero warm samples selects first-call diagnostics")
    trees = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    targets = {"baseline": args.baseline_target.resolve(), "candidate": args.candidate_target.resolve()}
    if len(set(trees.values())) != 2 or len(set(targets.values())) != 2:
        parser.error("use separate worktrees and targets")
    fixtures = {side: {name: hashlib.sha256((tree / name).read_bytes()).hexdigest()
                       for name in ("benchmarks/src/pfe_fixture.rs", "benchmarks/src/bin/pfe_benchmark.rs")}
                for side, tree in trees.items()}
    if fixtures["baseline"] != fixtures["candidate"]:
        raise ValueError("PFE workload definitions differ across branches")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "RAYON_NUM_THREADS": str(args.threads)}
    manifest = {"compiler": command(["rustc", "+stable", "--version", "--verbose"], trees["candidate"]),
                "machine": platform.platform(), "threads": args.threads, "fixture_hashes": fixtures,
                "sources": {side: metadata(tree) for side, tree in trees.items()},
                "timing_boundary": "full decomposition, inspection, simulation and per-trade cubes",
                "samples": args.samples, "cases": {}}
    for side, tree in trees.items():
        subprocess.run(["cargo", "+stable", "build", "--release", "--locked", "-p", "benchmarks", "--bin", "pfe_benchmark"],
                       cwd=tree, env={**env, "CARGO_TARGET_DIR": str(targets[side])}, check=True)
    if manifest["sources"] != {side: metadata(tree) for side, tree in trees.items()}:
        raise ValueError("sources changed during build")
    for count in args.trades:
        for expire in (False, True):
            case = f"{'expiring' if expire else 'all'}-{count}"
            results = {side: [] for side in trees}
            for index in range(args.rounds):
                order = ("baseline", "candidate") if index % 2 == 0 else ("candidate", "baseline")
                for side in order:
                    raw = output / f"{case}-{side}-{index}.json"
                    log = output / f"{case}-{side}-{index}.log"
                    arguments = [str(targets[side] / "release/pfe_benchmark"), str(count), str(args.paths),
                                 str(expire).lower(), str(args.samples), str(raw)]
                    wrapper = ["/usr/bin/time", "-l"] if platform.system() == "Darwin" else []
                    print(f"{case}: {side}, round {index+1}/{args.rounds}, {args.paths} paths", flush=True)
                    with log.open("w") as stream:
                        subprocess.run(wrapper + arguments, cwd=trees[side], env=env, stdout=stream,
                                       stderr=subprocess.STDOUT, check=True)
                    data = json.loads(raw.read_text())
                    if (data["trades"], data["paths"], data["expire_requests"], data["samples"]) != (count, args.paths, expire, args.samples):
                        raise ValueError("workload parameters differ")
                    rss = re.search(r"(\d+)\s+maximum resident set size", log.read_text())
                    data["max_rss_bytes"] = int(rss.group(1)) if rss else None
                    results[side].append(data)
                before, after = (results[side][-1] for side in ("baseline", "candidate"))
                if before["claims"] != after["claims"]:
                    raise ValueError("claim count changed")
                compare(before["snapshot"], after["snapshot"])
            summary = {}
            for side, observations in results.items():
                summary[side] = {key: statistics.median(data[key] for data in observations)
                                 for key in ("setup_ms", "first_ms", "pipeline_setup_ms", "evaluation_ms", "claims", "requests")}
                summary[side]["observed_ms"] = statistics.median(
                    data["median_ms"] if args.samples else data["first_ms"] for data in observations)
                summary[side]["round_ms"] = [data["median_ms"] if args.samples else data["first_ms"] for data in observations]
                rss = [data["max_rss_bytes"] for data in observations if data["max_rss_bytes"] is not None]
                summary[side]["max_rss_bytes"] = statistics.median(rss) if rss else None
            manifest["cases"][case] = {"trades": count, "paths": args.paths, "expire_requests": expire,
                                        "rounds": args.rounds, "summary": summary,
                                        "cube_entries_per_run": results["baseline"][0]["snapshot"]["entries"],
                                        "fingerprints": [data["snapshot"]["fingerprint"] for data in results["candidate"]]}
            old, new = (summary[side]["observed_ms"] for side in ("baseline", "candidate"))
            print(f"{case}: {old:.3f} -> {new:.3f} ms ({old/new:.2f}x); cube parity passed", flush=True)
            (output / "manifest.json").write_text(json.dumps(manifest, indent=2)+"\n")
    if manifest["sources"] != {side: metadata(tree) for side, tree in trees.items()}:
        raise ValueError("sources changed during timing; discard results")
    print(f"Results: {output / 'manifest.json'}", flush=True)


if __name__ == "__main__":
    main()

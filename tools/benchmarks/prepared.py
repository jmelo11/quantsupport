"""Measure explicit DCF snapshot preparation, refresh and warm execution.

Compares candidate ordinary and prepared calls against the old ordinary tape API.
Prepared calls reuse an unchanged numeric snapshot; refresh recaptures the market.
These boundaries differ deliberately and do not isolate native-code speed alone.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess

from compare_branches import command, metadata
from monte_carlo import compare
from scaling import fixture_hashes, numerical_dependencies


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, default=Path.cwd())
    parser.add_argument('--baseline-target', type=Path, required=True)
    parser.add_argument('--candidate-target', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=Path('target/prepared-dcf-comparison'))
    parser.add_argument('--rounds', type=int, default=3)
    parser.add_argument('--samples', type=int, default=7)
    parser.add_argument('--trades', type=int, nargs='+', default=[1, 10, 100, 1000])
    parser.add_argument('--toolchain', default='stable')
    args = parser.parse_args()
    if min(args.rounds, args.samples, *args.trades) < 1:
        parser.error('counts must be positive')
    trees = {'baseline': args.baseline.resolve(), 'candidate': args.candidate.resolve()}
    targets = {'baseline': args.baseline_target.resolve(), 'candidate': args.candidate_target.resolve()}
    if len(set(trees.values())) != 2 or len(set(targets.values())) != 2:
        parser.error('use separate worktrees and build targets')
    dependencies = {key: numerical_dependencies(tree) for key, tree in trees.items()}
    if dependencies['baseline'] != dependencies['candidate']:
        raise ValueError('numerical dependencies differ')
    fixtures = {key: fixture_hashes(tree)['engine_benchmark.rs'] for key, tree in trees.items()}
    if fixtures['baseline']['numerical'] != fixtures['candidate']['numerical']:
        raise ValueError('ordinary pricing fixtures differ')
    sources = {key: metadata(tree) for key, tree in trees.items()}
    env = {**os.environ, 'RAYON_NUM_THREADS': '1'}
    env.pop('QUANTSUPPORT_AD_BACKEND', None)
    for key, tree in trees.items():
        build = ['cargo', f'+{args.toolchain}', 'build', '--release', '--locked', '-p', 'benchmarks', '--bin', 'engine_benchmark']
        if key == 'candidate':
            build += ['--bin', 'prepared_dcf_benchmark']
        subprocess.run(build, cwd=tree, env={**env, 'CARGO_TARGET_DIR': str(targets[key])}, check=True)
    if sources != {key: metadata(tree) for key, tree in trees.items()}:
        raise ValueError('sources changed during build')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = {
        'recorded_at_utc': datetime.now(timezone.utc).isoformat(), 'complete': False,
        'purpose': __doc__, 'sources': sources, 'numerical_dependencies': dependencies,
        'engine_fixtures': fixtures, 'prepared_fixture_sha256': digest(trees['candidate'] / 'benchmarks/src/bin/prepared_dcf_benchmark.rs'),
        'runner_sha256': digest(Path(__file__)),
        'compiler': command(['rustc', f'+{args.toolchain}', '--version', '--verbose'], trees['candidate']),
        'machine': {'platform': platform.platform(), 'logical_cpus': os.cpu_count()}, 'threads': 1,
        'build_environment': {key: value for key, value in env.items() if key in {'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'} or key.startswith('CARGO_PROFILE_RELEASE_')},
        'tolerances': {'price_rtol': 1e-11, 'price_atol': 1e-8, 'risk_rtol': 1e-8, 'risk_atol': 1e-8},
        'cases': {},
    }
    for trades in args.trades:
        for mode in ('value', 'risk'):
            observations = {key: [] for key in trees}
            paths = {key: [] for key in trees}
            for round_index in range(args.rounds):
                order = ['baseline', 'candidate'] if round_index % 2 == 0 else ['candidate', 'baseline']
                for key in order:
                    raw = output / f'{trades}-{mode}-{key}-{round_index}.json'
                    if key == 'baseline':
                        dimensions = ['dcf' if trades == 1 else 'portfolio', str(args.samples), str(max(2, 2000 // trades)), str(raw), mode, str(2 if trades == 1 else trades)]
                        binary = 'engine_benchmark'
                    else:
                        dimensions = [str(trades), str(args.samples), str(raw), mode]
                        binary = 'prepared_dcf_benchmark'
                    with raw.with_suffix('.log').open('w') as log:
                        subprocess.run([str(targets[key] / 'release' / binary), *dimensions], cwd=trees[key], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
                    data = json.loads(raw.read_text())
                    if data['request_mode'] != mode or data['samples'] != args.samples:
                        raise ValueError('driver ignored request dimensions')
                    if key == 'candidate' and data['trades'] != trades:
                        raise ValueError('candidate ignored book size')
                    if key == 'baseline' and data['portfolio_trades'] != (2 if trades == 1 else trades):
                        raise ValueError('baseline ignored book size')
                    if bool(data['snapshot']['sensitivities']) != (mode == 'risk'):
                        raise ValueError('unexpected risk output')
                    observations[key].append(data)
                    paths[key].append({'path': str(raw), 'sha256': digest(raw)})
                compare(observations['baseline'][-1]['snapshot'], observations['candidate'][-1]['snapshot'])
            result = {'raw': paths, 'snapshot': observations['candidate'][0]['snapshot'], 'changed_snapshot': observations['candidate'][0]['changed_snapshot']}
            result['old_ordinary_ms'] = statistics.median(item['median_call_ms'] for item in observations['baseline'])
            for metric in ('prepare_ms', 'first_prepared_ms', 'first_ordinary_ms'):
                result[metric] = statistics.median(item[metric] for item in observations['candidate'])
            for metric in ('prepared_warm', 'ordinary_warm', 'refresh', 'refresh_and_evaluate'):
                result[metric + '_ms'] = statistics.median(item[metric]['median_ms'] for item in observations['candidate'])
            if any(not math.isfinite(value) or value <= 0 for key, value in result.items() if key.endswith('_ms')):
                raise ValueError('invalid timing')
            result['warm_speedup_vs_old'] = result['old_ordinary_ms'] / result['prepared_warm_ms']
            result['warm_speedup_vs_ordinary'] = result['ordinary_warm_ms'] / result['prepared_warm_ms']
            result['refresh_evaluation_vs_old'] = result['refresh_and_evaluate_ms'] / result['old_ordinary_ms']
            manifest['cases'][f'{trades}-{mode}'] = result
            print(f'{trades} {mode}: old {result["old_ordinary_ms"]*1000:.3f} us, prepared {result["prepared_warm_ms"]*1000:.3f} us, refresh+evaluate {result["refresh_and_evaluate_ms"]*1000:.3f} us', flush=True)
    if sources != {key: metadata(tree) for key, tree in trees.items()}:
        raise ValueError('sources changed during timing')
    manifest['complete'] = True
    (output / 'results.json').write_text(json.dumps(manifest, indent=2) + '\n')


if __name__ == '__main__':
    main()

# Running benchmarks

Benchmark code lives in the `benchmarks` workspace crate. Comparison runners
live in `tools/benchmarks/`. Run commands from the repository root with stable
Rust and Python 3.11 or later.

Generated measurements, JSON reports, logs and plots belong under `target/`.
They are local run artifacts and are not kept in version control. Market and
configuration files under `benchmarks/data/` are benchmark inputs.

## Criterion benchmarks

```sh
RAYON_NUM_THREADS=10 cargo +stable bench --locked -p benchmarks --bench ad_core -- --noplot
RAYON_NUM_THREADS=10 cargo +stable bench --locked -p benchmarks --bench swap_pfe -- --noplot
RAYON_NUM_THREADS=10 cargo +stable bench --locked -p benchmarks --bench xva_pricing -- --noplot
```

| Bench | Workload |
| --- | --- |
| `ad_core` | Native gradients, compilation and explicitly requested Hessians. |
| `swap_pfe` | Complete exposure pipelines for 100/1,000 swaps and 500 paths, with both request-expiration modes. |
| `xva_pricing` | Prepared XVA at 16/1,000/10,000 paths and full market refresh at 1,000/10,000 paths. |

Criterion writes estimates and samples under `target/criterion/`, or under
`CARGO_TARGET_DIR/criterion/` when a custom target is set. Full XVA refresh
measurements can take several minutes.

## Request, portfolio and worker scaling

Use separate worktrees and build targets for the old and current implementations.
The old tape baseline is `f3b5e61`. The runner installs matching standalone
pricing/MC drivers in the dedicated baseline worktree and checks numerical
fixture hashes and dependency versions before measuring.

```sh
git worktree add --detach ../quantsupport.worktrees/benchmark-old f3b5e61
git worktree add --detach ../quantsupport.worktrees/benchmark-candidate HEAD
# Cargo.lock is ignored; retain the current dependency pins.
cp Cargo.lock ../quantsupport.worktrees/benchmark-candidate/Cargo.lock
python3 tools/benchmarks/scaling.py \
  --baseline ../quantsupport.worktrees/benchmark-old \
  --candidate ../quantsupport.worktrees/benchmark-candidate \
  --prepare-baseline \
  --baseline-target target/benchmarks/old \
  --candidate-target target/benchmarks/candidate \
  --rounds 3 --samples 7 \
  --output target/benchmarks/scaling
```

`--prepare-baseline` modifies only the dedicated baseline's benchmark drivers,
manifest and lockfile; it preserves the selected old pricing implementation.
The default matrix covers 17 ordinary pricing families with value-only and
value-plus-first-risk requests, additional portfolio sizes, and five MC
workloads at three path counts and three worker counts.

Use `--group pricing` or `--group mc` to select a group. `--engine-case`,
`--mc-case`, `--paths`, `--threads` and `--portfolio-trades` select smaller
workloads. `--smoke` validates the harness with short runs; its timings are
not performance evidence.

The output directory contains a manifest plus per-process JSON and logs.
Builds finish before timing, execution order alternates across process rounds,
and each paired snapshot must pass numerical checks. The runner records
setup, first call, warm samples, compiler, dependencies and source hashes,
and rejects sources changed during the run.

## Prepared DCF

After preparing the matched baseline drivers above:

```sh
python3 tools/benchmarks/prepared.py \
  --baseline ../quantsupport.worktrees/benchmark-old \
  --candidate ../quantsupport.worktrees/benchmark-candidate \
  --baseline-target target/benchmarks/old \
  --candidate-target target/benchmarks/candidate \
  --trades 1 10 100 1000 --rounds 3 --samples 7 \
  --output target/benchmarks/prepared-dcf
```

This compares the old ordinary API with current ordinary and explicitly
prepared valuations. It measures preparation, warm execution and changed-market
refresh separately. Original and changed-rate prices and risk labels must
match ordinary valuations before timings are accepted.

A prepared binding owns a fixed numerical snapshot. Warm execution does not
query the market; refresh explicitly after quotes, fixings, terms, dates or
discount-policy changes. The prepared API supports Value and Sensitivities
and limits each member to 2,048 graph operations. Its lifecycle is described
in the [AD implementation chapter](../book/src/implementation/ad-jit.md#explicit-prepared-cashflow-valuations).

## XVA and PFE

`prepared_xva.py` compares automatic/explicit preparation, repeated execution
and changed-market pricing through `XvaEngine::run`. `pfe.py` compares complete
swap exposure pipelines, including request construction and simulation.
Both require matching loaders, market inputs and driver definitions in the
old and candidate worktrees; the scaling preparation step installs only the
ordinary pricing/MC drivers. The runners reject mismatched fixtures.

```sh
python3 tools/benchmarks/prepared_xva.py --help
python3 tools/benchmarks/pfe.py --help
```

Both accept `--baseline`, `--candidate`, `--baseline-target`,
`--candidate-target` and `--output`. Keep outputs under `target/benchmarks/`.
XVA supports `--case`, `--rounds`, `--large-rounds`, `--samples`, `--threads`
and `--smoke`. PFE supports `--trades`, `--paths`, `--rounds`, `--samples`
and `--threads`; zero warm samples selects first-call diagnostics.

Other runners are `engines.py` for ordinary pricing families,
`monte_carlo.py` for stored-path/scripted pricing and path generation, and
`compare_branches.py` for scalar pricing comparisons. Use each script's
`--help` for its dimensions and output options.

## Interpreting a run

Compare matching requests, market inputs, seeds, grids and worker counts.
Check every price, risk label and applicable cashflow/path/cube before using
a timing. Nonfinite or missing outputs fail the comparison.

Report setup, compilation/first call, repeated execution and market refresh
as separate costs. Warm prepared calls reuse a bound snapshot, so compare
full refresh plus evaluation when the market changes on every call. Savings
from preparation and request sharing are part of the workload comparison;
they do not isolate native-code speed alone.

Stored-path equity keeps generated paths fixed and reports discount/FX risk.
Scripted model valuations can differentiate model inputs. PFE's cube-only
pipeline uses `f64` and measures request sharing and simulation rather than
AD compilation. Conditional scripts can use per-path captures. These risk
and execution boundaries must match across implementations.

Use multiple fresh process rounds and enough samples. Keep absolute times,
round variation and confidence intervals alongside speed ratios; small
single-machine differences and single-pair large cases have limited precision.

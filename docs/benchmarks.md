# Compiled-only AD versus the old branch

Latest completion run: see [27 November validation](#completion-validation-2026-11-27) for the current tests, 112 matched benchmark cases and 17 Criterion cases. The ordinary-pricer performance gate still fails.

**All-engine performance gate: failed.** All 42 comparisons pass their price,
risk and applicable path/cashflow checks. Warm MC/scripted valuations and XVA
benefit, while 15 of 17 standard-pricer cases and conditional scripts regress.
At 10,000 XVA paths, warm time falls from 69.2 s to 104 ms and the complete
first evaluation from 66.9 s to 12.9 s. At 1,000 margined paths, preparation
makes the first call slower. This branch completes scalar-backend removal;
these measurements do not justify universal replacement on the main branch.

The replacement branch removes the old `DualFwd`/nested-forward/reverse-tape
backend from all scalar consumers, including scripting, ordinary pricers and
XVA setup. The old implementation exists on `benchmark/old-ad-tape`.
Public pricing methods and first-risk numeric units are retained.

Benchmark drivers live in the workspace's `benchmarks` crate, under
`benchmarks/src/bin`. They are run with `cargo run --release -p benchmarks
--bin <name> -- <arguments>`. The CVA example demonstrates valuation and contains
no benchmark flags, sampling loops or timing output. The dedicated XVA driver
uses its own market/configuration fixture under `benchmarks/data/xva`.

The comparison calls the actual existing pricing methods on both branches.
Shared fixture code differs only in its explicit build-time scalar alias;
the runner compares normalized numerical-fixture hashes and records original
file hashes. There is no AD Cargo feature or runtime backend switch.

Coverage includes eighteen concrete pricing families: DCF, portfolio, bond,
loan, tree, Fourier, closed-form/MC equity, equity forward, FX option/forward,
CDS, futures, Black cap/caplet and Hull–White cap/caplet/swaption. Scripted
equity, rate and conditional digital valuations, Brownian/Hull–White/LGM path
builders and XVA receive separate cost boundaries and measurements.

The equity MC pricer consumes materialized paths and reports discount/FX risk;
it does not report derivatives through path generation. Supported scripts and
XVA prepare whole native path programs. Conditional scripts and other layouts
construct per-path IR and execute native gradients. Ordinary small pricers
still construct their current capture before looking up compatible native code.

First call includes compilation. Warm calls include market input gathering,
coefficient binding/capture where applicable, pricing and first-risk reporting.
Setup is recorded separately. There is no promise that a small one-off call can
amortize native compilation. All first-order calls carry no Hessian kernel or
nested-forward seed fields.

Numerical comparisons check every price/risk label and, for MC/XVA, all emitted
cashflows, variables and requested path/cube entries. The reported timings were
collected with Rust 1.100.0 release builds in separate target directories,
completed before timing. The comparison scripts use the stable channel for new runs.
Benchmarks run sequentially with fixed seeds/grids/requests and ten Rayon workers.
Raw JSON, process rounds, fixture/source/compiler hashes and tolerances are
retained under the output directories below.

```sh
python3 tools/benchmarks/engines.py \
  --baseline ../quantsupport.worktrees/old-ad-tape \
  --baseline-target target/old-ad-tape-1.100 \
  --candidate-target target/ad-migration-1.100 \
  --output target/compiled-only-engines-1.100
python3 tools/benchmarks/monte_carlo.py \
  --baseline ../quantsupport.worktrees/old-ad-tape \
  --baseline-target target/old-ad-tape-1.100 \
  --candidate-target target/ad-migration-1.100 \
  --output target/compiled-only-mc-1.100
python3 tools/benchmarks/prepared_xva.py \
  --baseline ../quantsupport.worktrees/old-ad-tape \
  --baseline-target target/old-ad-tape-1.100 \
  --candidate-target target/ad-migration-1.100 \
  --output target/compiled-only-xva-1.100
```

`--smoke` checks the harness and numerical agreement; its timings are not
performance evidence. See [calibration and second-order audit](../book/src/risk/aad.md#calibration-dependencies) for the remaining
calibration finite-difference maps and public second-order integration, and
[compilation walkthrough](../book/src/risk/aad.md#recording-compiling-and-executing-a-calculation) for a runnable capture/compile/replay example.

## Results: 2026-11-14

**The all-engine performance gate fails.** The branch now contains only compiled AD, but 15 of the 17 standard-pricer cases regress on warm calls. Conditional digital scripts also regress. The feature branch is suitable for reviewing this replacement experiment; these results do not justify replacing the main branch for every engine.

The comparisons below include real pricing and published first sensitivities through the existing public methods. All numerical checks and end-of-run source-provenance checks passed. `Old/new` is a speedup: values above 1 mean the compiled-only branch is faster.

Measured on Apple M1 Pro, macOS arm64, pinned Rust 1.100.0, Cranelift 0.137.1, release builds and ten Rayon workers. Cases run sequentially in alternating process order, with builds completed before timing. These are local desktop measurements. Each case uses three process rounds, except 10,000-path XVA (one round); each round has five warm samples for standard/MC pricing and three for XVA. Values below are medians of round medians.

The baseline is `benchmark/old-ad-tape` at `f3b5e61`. Only common fixtures, the shared normal-CDF correctness fix and the compiler pin were added above the old pricing implementation. The candidate is the uncommitted `feature/cranelift-parallel-ad` working tree above `eb5df3e`; exact tracked diff and untracked-file hashes are retained in each run manifest. The measurements precede commit `94aab62` and the subsequent benchmark-layout cleanup. Fixtures were moved from `examples` to the `benchmarks` crate afterward; their numerical definitions are retained. The comparison runners now invoke the benchmark binaries directly. A portable summary with the measured source hashes and round timings is [compiled_pipeline_results.json](../benchmarks/compiled_pipeline_results.json).

### Existing standard pricers

Warm numbers are microseconds per evaluation. Cold numbers are milliseconds for the first call in a fresh process, including compilation. Requests include value and first sensitivities. Tree/Fourier use three evaluations per warm sample; the other cases use 100.

| Case | Old warm µs | Compiled warm µs | Old/new | Old cold ms | Compiled cold ms |
| --- | --- | --- | --- | --- | --- |
| dcf | 4.752 | 12.003 | 0.40× | 0.041 | 1.288 |
| portfolio | 10.450 | 24.391 | 0.43× | 0.055 | 1.300 |
| bond | 23.963 | 29.857 | 0.80× | 0.077 | 1.238 |
| loan | 18.869 | 12.201 | 1.55× | 0.070 | 1.110 |
| tree | 4363.347 | 3087.528 | 1.41× | 7.128 | 548.884 |
| fourier | 10709.986 | 17253.361 | 0.62× | 14.750 | 2799.061 |
| black-equity | 4.120 | 7.159 | 0.58× | 0.063 | 0.728 |
| equity-forward | 2.602 | 4.586 | 0.57× | 0.050 | 0.543 |
| fx-option | 3.934 | 7.199 | 0.55× | 0.061 | 0.772 |
| fx-forward | 2.501 | 4.747 | 0.53× | 0.047 | 0.549 |
| cds | 7.008 | 16.070 | 0.44× | 0.059 | 1.922 |
| futures | 1.248 | 3.065 | 0.41× | 0.042 | 0.475 |
| black-caplet | 3.273 | 6.697 | 0.49× | 0.055 | 0.732 |
| black-cap | 10.999 | 29.826 | 0.37× | 0.058 | 3.643 |
| hw-caplet | 2.306 | 4.844 | 0.48× | 0.043 | 0.638 |
| hw-cap | 10.620 | 20.855 | 0.51× | 0.053 | 3.228 |
| hw-swaption | 10.836 | 19.505 | 0.56× | 0.057 | 1.920 |

Loan and tree benefit on warm calls. Ordinary pricers still construct their scalar capture and validate its structure before reusing native gradient code; for small formulas that overhead exceeds the old tape cost. Large Fourier captures also pay boundary packing and separate forward/VJP block execution, and regress in this fixture. Compilation makes every standard-pricer cold call slower.

### Standalone and scripted Monte Carlo

All timings are milliseconds per public call. Stored-path equity consumes materialized paths: path creation is setup, and its existing risk contract covers discount/FX factors. Its speedup includes removing unnecessary AD arithmetic from untracked payoff reduction; it does not represent AD through path generation. Straightforward scripts reuse a whole native path program. The conditional digital case deliberately exercises per-path capture and native gradients. Simulation-builder rows measure generation of complete path arrays, rather than pricing sensitivities. Small builder differences should not be interpreted as a large compiled-AD gain.

| Case / paths | Old warm ms | Compiled warm ms | Old/new | Old cold ms | Compiled cold ms |
| --- | --- | --- | --- | --- | --- |
| equity-call-1000 | 0.085 | 0.010 | 8.23× | 0.196 | 0.492 |
| equity-call-10000 | 0.833 | 0.070 | 11.99× | 1.400 | 0.577 |
| equity-put-1000 | 0.085 | 0.010 | 8.13× | 0.167 | 0.479 |
| equity-put-10000 | 0.834 | 0.069 | 12.08× | 1.381 | 0.563 |
| equity-call-value-1000 | 0.067 | 0.008 | 7.97× | 0.142 | 0.021 |
| equity-call-value-10000 | 0.665 | 0.067 | 9.97× | 1.185 | 0.090 |
| script-equity-1000 | 2.871 | 0.448 | 6.41× | 3.422 | 1.835 |
| script-equity-10000 | 17.187 | 1.061 | 16.20× | 22.853 | 2.531 |
| script-rates-1000 | 15.428 | 1.296 | 11.91× | 14.377 | 4.961 |
| script-rates-10000 | 90.703 | 2.285 | 39.69× | 93.450 | 5.868 |
| script-digital-1000 | 3.102 | 3.311 | 0.94× | 3.401 | 5.939 |
| script-digital-10000 | 19.358 | 21.981 | 0.88× | 20.343 | 24.869 |
| build-brownian-1000 | 0.766 | 0.729 | 1.05× | 0.831 | 0.797 |
| build-brownian-10000 | 7.720 | 7.450 | 1.04× | 8.094 | 7.703 |
| build-hullwhite-1000 | 1.224 | 1.188 | 1.03× | 1.289 | 1.260 |
| build-hullwhite-10000 | 12.289 | 12.059 | 1.02× | 12.625 | 12.421 |
| build-lgm-1000 | 1.106 | 1.078 | 1.03× | 1.158 | 1.128 |
| build-lgm-10000 | 11.016 | 10.671 | 1.03× | 11.142 | 11.013 |

Setup and the complete one-off boundary are shown separately below. These include context/model construction and materialized paths where the API consumes them; they are not excluded from the one-off totals. Totals are medians of paired sums, so they need not equal the sum of the displayed component medians.

| Case / paths | Old setup ms | Compiled setup ms | Old setup + first ms | Compiled setup + first ms |
| --- | --- | --- | --- | --- |
| equity-call-1000 | 0.857 | 0.829 | 1.053 | 1.321 |
| equity-call-10000 | 8.066 | 7.736 | 9.511 | 8.317 |
| equity-put-1000 | 0.862 | 0.832 | 1.033 | 1.312 |
| equity-put-10000 | 8.121 | 7.751 | 9.553 | 8.314 |
| equity-call-value-1000 | 0.862 | 0.830 | 1.008 | 0.851 |
| equity-call-value-10000 | 8.105 | 7.737 | 9.341 | 7.828 |
| script-equity-1000 | 0.071 | 0.078 | 3.482 | 1.955 |
| script-equity-10000 | 0.070 | 0.074 | 22.918 | 2.594 |
| script-rates-1000 | 0.152 | 0.154 | 14.538 | 5.115 |
| script-rates-10000 | 0.148 | 0.167 | 93.586 | 6.011 |
| script-digital-1000 | 0.070 | 0.076 | 3.470 | 6.026 |
| script-digital-10000 | 0.074 | 0.077 | 20.413 | 24.948 |
| build-brownian-1000 | 0.031 | 0.033 | 0.862 | 0.831 |
| build-brownian-10000 | 0.026 | 0.035 | 8.122 | 7.739 |
| build-hullwhite-1000 | 0.027 | 0.034 | 1.315 | 1.294 |
| build-hullwhite-10000 | 0.028 | 0.035 | 12.653 | 12.457 |
| build-lgm-1000 | 0.028 | 0.034 | 1.186 | 1.162 |
| build-lgm-10000 | 0.029 | 0.034 | 11.171 | 11.048 |

### Existing XVA engine

The existing `XvaEngine::run` evaluates the same 58-claim IRS/XCCY portfolio, calibrated rate factors, credit/funding curves and full cubes. All timings are milliseconds. `Compiled first` includes preparation even when it is requested explicitly before `run`. Warm runs regenerate draws, evaluate paths, populate cubes and report CVA/FVA plus first risks.

The gains measure the combined effect of moving deterministic model work out
of path loops, reusing a whole native path program and running compiled AD
inside the existing engine. Quote refresh must rebuild current coefficients
and their published-risk Jacobian. That preparation remains expensive even
when the whole-path machine code is retained, as the refresh table shows.

| Case | Paths | Old warm ms | Compiled warm ms | Old/new | Old first ms | Compiled first ms |
| --- | --- | --- | --- | --- | --- | --- |
| small-auto | 16 | 124.305 | 0.853 | 145.65× | 605.393 | 13430.413 |
| small-prepared | 16 | 184.378 | 0.777 | 237.41× | 254.974 | 13091.201 |
| margined-auto | 1000 | 6682.129 | 11.417 | 585.29× | 7150.638 | 13643.412 |
| margined-prepared | 1000 | 6848.667 | 11.893 | 575.86× | 7400.090 | 12920.771 |
| unmargined-auto | 1000 | 4433.706 | 13.278 | 333.91× | 4835.720 | 5041.714 |
| market-refresh | 1000 | 7106.274 | 11.829 | 600.73× | 7437.848 | 10293.981 |
| large-auto | 10000 | 69206.187 | 103.659 | 667.63× | 66897.317 | 12934.824 |

Cold setup, preparation and complete first evaluation:

| Case | Old setup ms | Compiled setup ms | Explicit preparation ms | Old setup + first ms | Compiled setup + first ms |
| --- | --- | --- | --- | --- | --- |
| small-auto | 29.452 | 42.627 | 0.000 | 634.845 | 13475.693 |
| small-prepared | 27.617 | 42.836 | 13089.417 | 282.484 | 13134.940 |
| margined-auto | 29.846 | 49.850 | 0.000 | 7180.107 | 13693.262 |
| margined-prepared | 30.187 | 50.371 | 12907.034 | 7430.205 | 12971.142 |
| unmargined-auto | 33.222 | 50.272 | 0.000 | 4865.574 | 5104.944 |
| market-refresh | 30.726 | 50.168 | 10282.426 | 7468.006 | 10344.149 |
| large-auto | 30.722 | 49.687 | 0.000 | 66928.039 | 12984.511 |

Auto mode reports preparation inside `first_run_ms`, so its explicit preparation column is zero; the first-evaluation and setup-plus-first columns include it. Explicit and automatic preparation prime the same owned program. The obsolete small-run size threshold was removed after it caused supported small simulations to rebuild per-path captures; a regression test exercises automatic native preparation at 64 paths. Unsupported claim strategies use compiler captures and native gradients without a legacy backend.


The refresh case first prepares the original market, changes FX spot by 1%, rebuilds current context/engine bindings, and then values the changed market. The candidate must report `reused_code=true`. The initial preparation is paid before this changed-market boundary:

| Version | Initial preparation ms | Context update ms | Engine update ms | Rebind preparation ms | First updated run ms | Refresh total ms |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.000 | 0.000 | 0.000 | 0.000 | 7437.848 | 7437.848 |
| candidate | 13276.099 | 31.667 | 7.760 | 10282.426 | 12.753 | 10333.408 |

### Numerical agreement and verification

All 42 benchmark cases pass: 17 standard pricing cases, nine MC cases at each of 1,000/10,000 paths, and seven XVA cases. Every label and numerical output is checked, including emitted script cashflows/variables, generated paths, dates and full XVA cubes. Price/path tolerance is `rtol=1e-11`, `atol=1e-8` (`1e-7` for XVA); risk tolerance is `rtol=1e-8`, `atol=1e-8`.

| Group | Max value/path/cube absolute error | Max risk scaled error |
| --- | --- | --- |
| Standard | 0 | 2.47631e-15 |
| Monte Carlo | 7.77618e-11 | 2.11524e-14 |
| XVA | 2.51457e-08 | 1.80218e-13 |

XVA adjustment absolute error is at most 7.27596e-12. Scaled risk error is `abs(new-old)/max(1,abs(old))`. The raw manifests record all round timings, fixture hashes, source/compiler provenance and tolerances. They reside at `target/compiled-only-{engines,mc,xva}-1.100/manifest.json`; per-process JSON/logs are alongside them.

Validation: 984 release library tests passed after backend removal; 207 focused scripting tests subsequently passed, including original error preservation during native capture; all three XVA preparation tests passed after removal of the dispatch threshold. The old branch passes 978 release library tests. Strict library/example Clippy and rustdoc, workspace/all-target checks, nine Book structure tests, Book build and runnable Rust documentation examples passed. The broader test-target Clippy sweep has documented lint debt and is not reported as passing.

The scalar backend replacement is complete. Public second-order pricing/XVA requests and the remaining rate/Hull–White calibration finite-difference maps are still listed in [calibration and second-order audit](../book/src/risk/aad.md#calibration-dependencies). The tested kernel-level second-order API does not imply those calibrated pricing features are finished.

## Verification after benchmark relocation

The drivers now build and run from the `benchmarks` crate on both branches.
All 17 standard-pricer, nine Monte Carlo and seven XVA smoke cases pass,
along with the two scalar pricing scenarios. The smoke XVA cases use 16 paths.
The XVA comparison also checks matching numerical-driver, loader and market-data
hashes, excluding only the branch-specific preparation hook and scalar alias.
These checks validate the relocation; they do not replace the full timing
measurements above. Raw smoke manifests are under `target/ad-cleanup-*`.

After field encapsulation, 251 focused tests pass: 36 compiled-AD tests, one
path-plan test, 207 scripting tests, three XVA preparation tests and two fixture
loading tests in each of the CVA and benchmark crates. Strict library/example
and benchmark Clippy, strict rustdoc, workspace/all-target checks, nine Book
structure tests and the Book build pass. The CVA application also runs normally
with `cargo run --release -p cva`, without benchmark flags or timing output.

## Swap PFE request sharing: 2026-11-15

The cube-only swap benchmark uses `ExposureEvaluator<f64>::evaluate`; it
performs no AD compilation or sensitivity calculation. Its original
preprocessing allocated one response block per claim, although most swaps
asked for identical market observations. At 1,000 swaps this expanded 32,804
claims into 32,804 response slots per date and path. Sharing complete identical
request blocks reduces that to 101 slots. All original claims and trade output
cubes are retained. Keys include every request field, observation/expiration
date and discount curve; whole scripted blocks retain contiguous tail layouts.

The fixture is unchanged: 100/1,000 mixed-maturity swaps, 500 paths, fixed seed
42 and 121 monthly dates over ten years. Both request-expiration modes run the
existing decomposition, inspection, LGM simulation and exposure-evaluation
methods. Trade/curve construction stays outside the measured pipeline, as in
the original Criterion benchmark; hashing and JSON serialization are also
outside the evaluation timer. Each run fingerprints every finite cube value,
keeps complete first-path rows and compares all per-trade sums and dimensions.
The 1,000-trade cases each check 60,500,000 entries. Shared and unshared results
match exactly under these checks. The tests also compare every small-fixture
cube entry directly against independently constructed unshared requests.

The following are single-run diagnostics on separate old/fixed branch binaries,
using pinned Rust 1.100.0 release builds and ten Rayon workers. They establish
the size of this regression and fix; they are not multi-round median estimates.
Peak resident memory includes the common snapshot hashing/serialization.

| Case / trades | Before seconds | Fixed seconds | Before/fixed | Peak RSS GB: before → fixed |
| --- | --- | --- | --- | --- |
| all-100 | 7.986 | 0.338 | 23.6× | 0.523 → 0.087 |
| expiring-100 | 3.658 | 0.253 | 14.4× | 0.447 → 0.086 |
| all-1000 | 80.863 | 1.499 | 53.9× | 3.412 → 0.678 |
| expiring-1000 | 40.333 | 1.379 | 29.2× | 2.862 → 0.674 |

The complete four-case Criterion benchmark also passes with ten samples per
case and all 500 paths. It completed in approximately one minute in this run.
Its 1,000-swap mean estimates are 1.579 s for all requests and 1.430 s with
expiration. Flat sampling uses equal iteration counts for long-running cases;
the sample count and pricing workload are retained. Criterion's five-second
measurement time is a target, so ten full 1.4-second evaluations necessarily
take at least fourteen seconds for that case.

Reproduce the ordinary benchmark:

```sh
RAYON_NUM_THREADS=10 cargo bench --locked -p benchmarks --bench swap_pfe -- --noplot
```

Compare the unchanged full workload on separate branches:

```sh
python3 tools/benchmarks/pfe.py \
  --baseline ../quantsupport.worktrees/old-ad-tape \
  --baseline-target target/old-ad-tape-1.100 \
  --candidate-target target/ad-migration-1.100 \
  --rounds 1 --samples 0 --output target/pfe-comparison
```

Zero warm samples selects first-call diagnostics. The runner defaults to three
process rounds with one warm sample per round when those overrides are omitted;
that fuller comparison takes substantially longer on the old branch. Driver
and fixture hashes must be identical before timing; source hashes are verified
afterward. See [pfe_results.json](../benchmarks/pfe_results.json) for portable
results/provenance, `target/pfe-comparison` for raw JSON/logs and
`target/pfe-criterion-fixed.log` for Criterion output.

Validation passes: 31 XVA tests, including three request-sharing regressions,
two scripted-product tests, strict library/benchmark Clippy and rustdoc, and
workspace/all-target compilation. Existing pricing entrypoints retain their
signatures. A claim index identifies a response block and can now be shared;
it is not a unique trade identifier. No market numbers are cached across runs.

## Running the XVA pricing benchmark

The `xva_pricing` Criterion target calls the production `XvaEngine::run` on a
five-year USD interest-rate swap and USD/CLP cross-currency swap. Their 58 claims
share one netting set. Rate models are calibrated to the benchmark's caplet
surface and swaption cube; credit and funding curves supply CVA and FVA weights.
The monthly grid, seed 42, trades and market observations stay fixed across path
counts. Each call returns the complete netting-set exposure cube, both
adjustments and their published first sensitivities.

Run the benchmark on stable Rust:

```sh
RAYON_NUM_THREADS=10 cargo +stable bench --locked -p benchmarks --bench xva_pricing -- --noplot
```

The `xva_pricing` group measures repeated complete runs at 16, 1,000 and 10,000
paths, with and without variation margin. Market construction, calibration,
explicit preparation and an initial run occur before sampling. The timed call
still generates every path, exposure, adjustment and sensitivity, and includes
result allocation and destruction. It does not return a cached valuation.

The `xva_market_refresh` group alternates the initial CLP/USD spot by +1% and
-1% at 1,000 and 10,000 paths. Its timed operation includes market
initialization, `update_market`, preparation of the new numerical binding and a
complete `run`. A check before sampling confirms that an FX update retains the
native path program. The fixture's inline test compares every refreshed cube
entry, adjustment and risk label with a newly constructed engine, checks that
prices actually change, and covers both collateral modes.

Each case uses ten flat samples, a one-second warm-up and a three-second target
measurement period. These are prepared-execution measurements. Creating new
engines inside the same process would still reuse compiler caches, so this
target does not label those calls as cold compilation. Full market refreshes
can take many seconds, so collecting ten samples for each refresh case takes
several minutes despite the three-second target.

Use the standalone driver in a new process to measure the complete first call:

```sh
cargo +stable build --release --locked -p benchmarks --bin xva_benchmark
RAYON_NUM_THREADS=10 target/release/xva_benchmark \
  --benchmark 1000 5 target/xva-first-call.json true
```

The JSON separates market/engine setup, the first `run` including its automatic
preparation, and repeated-run times. It also retains the complete numerical
outputs. With a custom `CARGO_TARGET_DIR`, use the binary in that directory.
The existing `tools/benchmarks/prepared_xva.py` compares this driver with the old
AD branch and checks all prices, risk labels and path entries before reporting
a speedup. Both drivers use the existing pricing API and the dedicated
benchmark fixture.

### Pricing results: 2026-11-15

The benchmark was run from the isolated `benchmark/xva-pricing-measurement`
worktree at `1f181ad`. It uses stable Rust 1.100.0, Cranelift 0.137.1, release
builds and ten Rayon workers on an Apple M1 Pro. These are local desktop
measurements with background applications active; the confidence intervals
show the variation observed in these ten samples. All timings are milliseconds
per complete call. The prepared and refresh rows have different cost boundaries
as described above.

| Operation | Paths | Mean ms | 95% confidence interval ms |
| --- | --- | --- | --- |
| Prepared, with margin | 16 | 0.899 | 0.880–0.920 |
| Prepared, with margin | 1,000 | 14.827 | 13.737–16.383 |
| Prepared, with margin | 10,000 | 118.450 | 113.350–123.906 |
| Prepared, without margin | 16 | 0.675 | 0.653–0.697 |
| Prepared, without margin | 1,000 | 11.006 | 10.612–11.422 |
| Prepared, without margin | 10,000 | 95.919 | 90.681–103.356 |
| Market refresh and pricing, with margin | 1,000 | 13068.527 | 12643.028–13536.047 |
| Market refresh and pricing, with margin | 10,000 | 11694.795 | 11207.192–12293.380 |

Repeated 1,000-path margined pricing averages 14.8 ms, while changing FX,
reconstructing the market and rebinding its derivatives takes 13.1 s. Retaining
the native path program therefore does not eliminate the cost of a full market
refresh. Increasing the path count does not make that setup cost disappear.
Use the relevant boundary when deciding whether prepared execution benefits a
workload. These results do not establish a universal gain for one-off pricing
or changed markets.

The portable [XVA pricing results](../benchmarks/xva_pricing_results.json)
retain the source and market-fixture hashes, compiler details, confidence
intervals, iteration counts and sample durations. Raw Criterion output is in
`target/xva-pricing-criterion.log`; estimates are under
`target/ad-migration-1.100/criterion`.

### Fresh-process comparison with the old AD branch

The unchanged standalone driver was also run on `benchmark/old-ad-tape` at
`f3b5e61` and the measured compiled commit `1f181ad`. The numerical driver and
market-fixture hashes match across branches. All 62,000 exposure entries at
1,000 paths and 620,000 entries at 10,000 paths match within the declared
price tolerance, along with both adjustments and all 60 labelled first risks.
The maximum absolute cube difference is 2.52e-8; the maximum adjustment
difference is 4.55e-13.

This comparison uses three warm samples per fresh process. The 1,000-path row
is the median of three process rounds with alternating execution order; the
10,000-path row has one process round, so its uncertainty is less well
characterized. These medians are separate measurements from the Criterion mean
estimates above. First-call times include automatic preparation and compilation;
market/engine construction is reported separately in the JSON.

| Paths | Old warm ms | Compiled warm ms | Old first call ms | Compiled first call ms |
| --- | --- | --- | --- | --- |
| 1,000 | 7734.755 | 10.461 | 8581.745 | 15316.591 |
| 10,000 | 79520.659 | 91.656 | 79435.924 | 15619.385 |

The compiled first call is slower at 1,000 paths and faster at 10,000 paths.
The large warm gain applies to repeated calls on an unchanged prepared market;
the full refresh measurements above retain the cost of rebuilding the market
and its derivative bindings. All paths use the production `XvaEngine::run`.

Reproduce this comparison from the committed benchmark worktree:

```sh
python3 tools/benchmarks/prepared_xva.py \
  --baseline ../old-ad-tape \
  --candidate . \
  --baseline-target ../../quantsupport/target/old-ad-tape-1.100 \
  --candidate-target ../../quantsupport/target/ad-migration-1.100 \
  --rounds 3 --large-rounds 1 --samples 3 \
  --case margined-auto --case large-auto \
  --output ../../quantsupport/target/xva-pricing-branch-comparison
```

Raw comparison output is in `target/xva-pricing-branch-comparison.log` and its
sibling directory. The portable JSON retains per-round times, adjustments,
source and output hashes, numerical-layout counts and the comparison tolerances.

## Request, portfolio and worker scaling: 2026-11-15

An additional **85 matched comparisons** exercise the old implementation at
`f3b5e61` and compiled implementation at `41cb74a`: 17 standard pricing families
with both value-only and value-plus-first-risk requests, six additional
portfolio/request sizes, and five MC workloads at three path counts and three
worker counts. All 255 paired process comparisons pass numerical checks.

Both implementations use stable Rust 1.100.0 release builds on the same Apple
M1 Pro with ten logical CPUs. The baseline contains benchmark-only driver and
manifest changes; its AD/pricing implementation is unchanged. The candidate
contains the same extended driver. Source hashes and normalized fixture hashes
are retained in [ad_scaling_results.json](../benchmarks/ad_scaling_results.json).

This run also holds numerical dependency versions fixed across branches:
`rand 0.8.5`, `rand_chacha 0.3.1`, `rayon 1.10.0`, `rayon-core 1.12.1`,
`nalgebra 0.34.1`, `chrono 0.4.41`, `num-complex 0.4.6`, `serde 1.0.228` and
`serde_json 1.0.149`. Seeding the old worktree from the candidate lockfile
retains those versions while Cargo updates its workspace package entries.
These are new measurements with that controlled dependency set; historical
numbers elsewhere on this page retain their original provenance.

Both builds finish before timing. Cases execute sequentially; process order
alternates between implementations on successive rounds. Each number is the
median of three fresh-process round medians, with seven warm samples per
process. Fixed contracts, quotes, path indices, seeds, grids and requests are
shared within each pair. First-call timing includes compilation where the
production API performs it; setup is recorded separately. Medians from three
process rounds are not confidence intervals. Small differences on this local
desktop should be assessed alongside the stored per-round timings.

### Value-only versus first-risk requests

Warm timings below are microseconds per public evaluation with one worker.
`Value` requests PV alone; `Risk` requests PV and all published first
sensitivities. Old/new above one means the compiled implementation is faster.
The default portfolio contains two copies of the same three-year swap.
Tree/Fourier use three calls per sample; other standard cases use 1,000.

| Case | Old value µs | Compiled value µs | Old/new | Old risk µs | Compiled risk µs | Old/new |
| --- | --- | --- | --- | --- | --- | --- |
| dcf | 3.888 | 4.371 | 0.89× | 5.022 | 11.989 | 0.42× |
| portfolio | 7.987 | 9.158 | 0.87× | 10.374 | 24.537 | 0.42× |
| bond | 22.874 | 24.939 | 0.92× | 23.954 | 30.286 | 0.79× |
| loan | 7.841 | 4.863 | 1.61× | 18.674 | 12.210 | 1.53× |
| tree | 3716.986 | 1223.181 | 3.04× | 4326.569 | 3065.208 | 1.41× |
| fourier | 7980.028 | 4091.208 | 1.95× | 11226.264 | 16322.347 | 0.69× |
| black-equity | 2.172 | 2.838 | 0.77× | 4.130 | 7.151 | 0.58× |
| equity-forward | 1.638 | 1.985 | 0.83× | 2.591 | 4.538 | 0.57× |
| fx-option | 1.809 | 2.681 | 0.67× | 3.831 | 7.148 | 0.54× |
| fx-forward | 1.408 | 1.963 | 0.72× | 2.497 | 4.631 | 0.54× |
| cds | 5.790 | 5.195 | 1.11× | 6.933 | 15.991 | 0.43× |
| futures | 0.701 | 1.074 | 0.65× | 1.242 | 2.982 | 0.42× |
| black-caplet | 1.706 | 2.490 | 0.69× | 3.269 | 6.689 | 0.49× |
| black-cap | 8.815 | 10.572 | 0.83× | 11.018 | 29.821 | 0.37× |
| hw-caplet | 1.708 | 2.277 | 0.75× | 2.318 | 4.809 | 0.48× |
| hw-cap | 9.040 | 8.900 | 1.02× | 10.635 | 20.649 | 0.52× |
| hw-swaption | 9.586 | 9.690 | 0.99× | 10.903 | 19.491 | 0.56× |

Five of 17 families improve on warm value-only calls; only loan and tree improve
when first sensitivities are requested. Fourier is faster for value alone and
slower with risk. Request selection therefore changes the performance outcome.
The first-risk gate still fails for 15 of 17 standard families.

### Portfolio size

Each portfolio repeats the same receive-fixed three-year swap, with all
members valued and aggregated through `PortfolioPricer`. This isolates
streaming aggregation and repeated per-member differentiation; it does not
represent a diversified instrument mix. All timings are milliseconds per
complete portfolio evaluation. Calls per sample are `2000 / trades` rounded
down, with a minimum of one.

| Trades | Old value ms | Compiled value ms | Old/new | Old risk ms | Compiled risk ms | Old/new |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | 0.0080 | 0.0092 | 0.87× | 0.0104 | 0.0245 | 0.42× |
| 10 | 0.0393 | 0.0450 | 0.87× | 0.0506 | 0.1196 | 0.42× |
| 100 | 0.3887 | 0.4421 | 0.88× | 0.5002 | 1.1948 | 0.42× |
| 1000 | 3.9019 | 4.4840 | 0.87× | 4.9924 | 11.9338 | 0.42× |

The 1,000-position risk run takes 4.99 ms on the old implementation and 11.93 ms
on the compiled implementation. Growing this repeated-swap book does not
amortize away the ordinary pricer's per-position capture and native-gradient
execution costs.

### Monte Carlo path and worker scaling

All timings are milliseconds per public call. Each worker column uses an
independent fresh process with the indicated `RAYON_NUM_THREADS`. Scripted
cases use one call per sample; stored-path equity and LGM path generation use
five. Rows retain complete outputs in the raw snapshots.

Stored-path equity reports its existing discount/FX risk while holding the
materialized paths fixed. Script rows include model-dependent first risk and
emitted cashflows/variables. The LGM builder measures generation of complete
path arrays; it does not price an instrument or calculate a pricing gradient.

| Workload | Paths | Old / compiled, 1 worker ms | Old/new | Old / compiled, 4 workers ms | Old/new | Old / compiled, 10 workers ms | Old/new |
| --- | --- | --- | --- | --- | --- | --- | --- |
| equity-call | 64 | 0.008 / 0.004 | 1.75× | 0.008 / 0.004 | 1.71× | 0.008 / 0.005 | 1.70× |
| equity-call | 1000 | 0.086 / 0.010 | 8.16× | 0.086 / 0.011 | 8.02× | 0.086 / 0.011 | 8.00× |
| equity-call | 10000 | 0.835 / 0.069 | 12.14× | 0.840 / 0.068 | 12.31× | 0.841 / 0.068 | 12.36× |
| script-equity | 64 | 0.737 / 0.216 | 3.41× | 0.622 / 0.289 | 2.15× | 0.765 / 0.370 | 2.07× |
| script-equity | 1000 | 6.268 / 0.384 | 16.31× | 5.232 / 0.441 | 11.87× | 2.835 / 0.469 | 6.05× |
| script-equity | 10000 | 59.142 / 1.974 | 29.97× | 23.134 / 1.247 | 18.55× | 19.976 / 1.052 | 18.98× |
| script-rates | 64 | 5.281 / 0.985 | 5.36× | 3.545 / 1.025 | 3.46× | 3.696 / 1.045 | 3.54× |
| script-rates | 1000 | 43.158 / 1.358 | 31.78× | 13.899 / 1.146 | 12.13× | 14.410 / 1.550 | 9.29× |
| script-rates | 10000 | 405.256 / 4.674 | 86.71× | 119.852 / 2.174 | 55.14× | 102.981 / 2.253 | 45.70× |
| script-digital | 64 | 0.740 / 0.547 | 1.35× | 0.628 / 0.527 | 1.19× | 0.822 / 1.126 | 0.73× |
| script-digital | 1000 | 6.224 / 5.587 | 1.11× | 2.848 / 3.168 | 0.90× | 3.415 / 3.792 | 0.90× |
| script-digital | 10000 | 58.917 / 53.682 | 1.10× | 22.951 / 26.060 | 0.88× | 20.368 / 25.469 | 0.80× |
| build-lgm | 64 | 0.072 / 0.077 | 0.93× | 0.072 / 0.071 | 1.01× | 0.071 / 0.071 | 1.00× |
| build-lgm | 1000 | 1.072 / 1.104 | 0.97× | 1.101 / 1.119 | 0.98× | 1.109 / 1.105 | 1.00× |
| build-lgm | 10000 | 11.102 / 10.990 | 1.01× | 11.043 / 11.095 | 1.00× | 11.342 / 11.795 | 0.96× |

At 10,000 paths, the prepared rate script improves from 405.26 to 4.67 ms with
one worker and from 102.98 to 2.25 ms with ten. The old/new gains are 86.7× and
45.7× respectively. The compiled serial-to-ten-worker gain for this workload
is about 2.1×, so the AD speedup and parallel speedup answer different questions.
The conditional digital retains per-path recording and is slower on the
compiled implementation with four and ten workers at 10,000 paths. Increasing
worker count is not a universal improvement for small prepared workloads.

### First evaluation and compilation costs

These selected cases retain the same timing boundary on each implementation.
`Setup + first` is the median of each process's paired sum, so it can differ
from the sum of separately reported component medians. All timings are
milliseconds; the full artifact includes these fields for every case.

| Case | Old first ms | Compiled first ms | Old setup + first ms | Compiled setup + first ms |
| --- | --- | --- | --- | --- |
| dcf-risk | 0.056 | 1.430 | 0.210 | 1.529 |
| loan-risk | 0.078 | 1.101 | 0.165 | 1.189 |
| tree-risk | 6.110 | 549.056 | 6.197 | 549.153 |
| fourier-risk | 15.459 | 2818.039 | 15.546 | 2818.128 |
| portfolio-1000-risk | 5.082 | 13.351 | 8.236 | 16.566 |
| script-rates-10000-t1 | 406.078 | 8.439 | 406.227 | 8.601 |
| script-rates-10000-t10 | 92.588 | 6.055 | 92.740 | 6.209 |
| script-digital-10000-t10 | 21.871 | 26.865 | 21.942 | 26.944 |

### Agreement, artifacts and reproduction

The audit compares 1,195,491 numerical value/path/cashflow entries and
417 sensitivity entries across all process pairs. It also checks
all dictionary keys, risk labels, array lengths and date/string fields. Maximum
absolute value error is 8.46e-11; maximum absolute sensitivity error is
1.43e-06, across differing input units and portfolio scales.
The largest tolerance fractions are 0.00342 for values and
4.34e-05 for sensitivities; one would be the rejection boundary.
Value tolerance is `rtol=1e-11, atol=1e-8`; risk tolerance is
`rtol=1e-8, atol=1e-8`. Nonfinite values and missing requested risk rows fail.

The portable JSON contains every case's dimensions, round medians, setup and
first-call boundaries, source/lock/fixture hashes and dependency versions.
Raw snapshots and logs are under `target/ad-scaling-comparison`, with hashes
retained in the portable artifact. End-of-run source checks pass. A separate
eight-case smoke run validates the harness and contributes no performance
numbers to the tables above.

The runner requires Python 3.11 or later. To reproduce from fresh, dedicated
worktrees after committing the extended benchmark driver:

```sh
git worktree add --detach ../quantsupport.worktrees/scaling-old f3b5e61
git worktree add --detach ../quantsupport.worktrees/scaling-candidate HEAD
# Cargo.lock is ignored by this repository; retain the candidate dependency pins.
cp Cargo.lock ../quantsupport.worktrees/scaling-candidate/Cargo.lock
python3 tools/benchmarks/scaling.py \
  --baseline ../quantsupport.worktrees/scaling-old \
  --candidate ../quantsupport.worktrees/scaling-candidate \
  --prepare-baseline \
  --baseline-target target/scaling-old \
  --candidate-target target/scaling-candidate \
  --rounds 3 --samples 7 \
  --output target/ad-scaling-comparison
```

`--prepare-baseline` copies only standalone benchmark drivers, adds their JSON
dependency and resolves workspace lock entries from the candidate lockfile.
The old AD/pricing sources remain at the selected baseline commit. The runner
requires identical numerical fixtures and pinned numerical dependency versions
before it builds. Use `--group pricing` or `--group mc` for one group, and
`--engine-case`, `--mc-case`, `--paths`, `--threads` and `--portfolio-trades` to
select a smaller matrix. `--smoke` checks correctness and wiring using a short
run; its timings are not performance evidence.

## Prepared DCF prototype (20 November 2026)

The explicit `DiscountedCashflowPricer::prepare` / `prepare_portfolio` APIs move
market requests, coupon resolution, capture, graph export, compatibility checks
and risk-label mapping into preparation. A binding owns a **fixed numerical
snapshot**; its warm `evaluate` calls execute native code and produce the
requested numeric report. Compatible portfolio members share code and aggregate
numbers directly. Ordinary value-only DCF calls also now skip AD recording.

The distinction between timing boundaries matters. The old implementation below
uses its ordinary public pricing API at `f3b5e61`. The candidate is `b4cc893` plus
the prototype changes, fingerprinted in
[`prepared_dcf_results.json`](../benchmarks/prepared_dcf_results.json).
The prepared benchmark uses the same 3-year, quarterly USD SOFR swap terms,
notional, dates and flat curve as the ordinary engine fixture; every PV and
requested risk label must agree. It also checks both original and changed-rate
snapshots against fresh candidate ordinary valuations. The table compares an
ordinary calculation with a reused snapshot; it does **not** measure a JIT-only
speedup against a similarly prepared tape. A cached final report could also
avoid repeated work when callers need exactly the same output.

Three fresh processes per case, seven warm samples per process, release builds,
Rust 1.100.0, macOS ARM64, one worker; builds finish before timing. Results are
medians of process medians. These measurements use independent baseline and
candidate artifact directories with matching numerical dependency versions.

### PV and all first sensitivities

| Positions | Old ordinary | Candidate ordinary | Prepared warm | Warm gain vs old | Refresh + evaluate | Cold preparation |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 swap | 4.799 µs | 11.432 µs | 0.260 µs | 18.5× | 8.261 µs | 2.061 ms |
| 10 swaps | 48.417 µs | 111.869 µs | 1.117 µs | 43.4× | 82.156 µs | 2.237 ms |
| 100 swaps | 481.058 µs | 1117.281 µs | 9.911 µs | 48.5× | 822.708 µs | 2.957 ms |
| 1000 swaps | 4.778 ms | 11.294 ms | 0.098 ms | 48.8× | 8.848 ms | 10.959 ms |

The prototype passes the warm targets of 5 µs per swap and 5 ms per 1,000-member
portfolio. It **does not close the gap for one valuation per changed market**:
refresh plus evaluation remains 1.7–1.9× the old ordinary risk cost. Refresh
still resolves and records the current financial calculation to bind all
coefficients safely, including current bootstrap Jacobians. Its cost cannot
be omitted when a market changes on every call.

Against the old risk timings, the first preparation amortizes after roughly 455
repeated single-swap calls or three portfolio calls at 1,000 positions. After a
market update, refresh amortizes after about two risk calls per snapshot.
These are measured-cost estimates; workloads with one price per market should
continue to compare the complete refresh boundary. This prototype is evidence
for removing setup from repeated execution, not by itself a reason to replace
the original tape for all workloads.

### Value-only calls

| Positions | Old ordinary | Candidate ordinary, no AD capture | Prepared warm |
| --- | ---: | ---: | ---: |
| 1 swap | 3.804 µs | 1.996 µs | 0.116 µs |
| 10 swaps | 38.947 µs | 19.817 µs | 0.851 µs |
| 100 swaps | 374.867 µs | 193.437 µs | 8.035 µs |
| 1000 swaps | 3.811 ms | 1.948 ms | 0.081 ms |

Skipping recording improves ordinary value-only calls by about 1.9× without
requiring snapshot reuse. The prepared API currently builds both value and risk
kernels, so its refresh cost is not appropriate for a single value-only call.

### Lifecycle, limits and reproduction

A snapshot never follows mutations to the provider automatically. Refresh after
quote, calibration, FX, fixing, date, schedule, position or discount-policy
changes. Refresh rebuilds the binding and checks exact graph compatibility;
it reuses executable code when compatible, otherwise compiles a new kernel.
A failed refresh invalidates the old binding until a successful retry. The
prototype supports Value and Sensitivities, with a limit of 2,048 graph operations
per member. Larger trades return an explicit error and retain the ordinary
pricer's partitioned-compilation route. No new parallel portfolio scheduler is
introduced by this prototype; the worker-scaling results above describe the
existing simulation paths.

With the dedicated baseline worktree and locked dependencies prepared using the
scaling instructions above:

```sh
python3 tools/benchmarks/prepared.py \
  --baseline ../quantsupport.worktrees/scaling-old \
  --candidate . \
  --baseline-target target/prepared-old \
  --candidate-target target/prepared-candidate \
  --trades 1 10 100 1000 --rounds 3 --samples 7 \
  --output target/prepared-dcf-comparison
```

The portable artifact records every raw file's hash and the source, driver,
lockfile, compiler and build settings. The Rust driver validates ordinary,
prepared and refreshed results before accepting its timings, and the Python
runner compares the old and new snapshots before publishing the report.

A separate instrumented release run of 5,000 warm ordinary risk calls helps
locate the remaining overhead: median market-request work 0.458 µs, recording
and coupon resolution 4.083 µs, graph export 2.917 µs, code-cache lookup and
structural checks 3.500 µs, native differentiation 0.125 µs, and the remaining
risk/report work 0.874 µs. Logging happens after the timers; the first call is
discarded. This diagnostic has different instrumentation from the performance
table and is not used in its ratios. Its source/patch and raw-log hashes are in
the portable artifact; the local instrumented worktree is
`../quantsupport.worktrees/prepared-dcf-profile`. The profile supports moving
recording, export and structural checks out of repeated execution. The next
step for changing-market performance is a typed, validated numeric binding
that refreshes current values and calibration maps without recapturing the full
cashflow calculation.

## Completion validation: 2026-11-27

The current `b4cc893` working tree, including the prepared DCF prototype,
was copied into an isolated candidate worktree and compared with the old tape
at `f3b5e61`. Source and fixture hashes, matching numerical dependency versions,
all commands, raw-result hashes and per-round timings are preserved in
[`merge_validation_results.json`](../benchmarks/merge_validation_results.json).
The historical artifacts and tables above retain their original measurements.

### Completed tests and checks

| Scope | Result |
| --- | --- |
| Core Rust unit tests and doctests | 1,003 unit tests and 75 doctests passed |
| Other workspace examples and portable Excel Rust tests | 117 passed; one existing test ignored |
| Python binding Rust harness | All five passed, including embedded Python contracts |
| Installed Python bindings | All 216 tests passed against the rebuilt checkout |
| Tooling and documentation | All 106 tests passed, including a strict Sphinx build |
| Notebook runner and studies | Nine runner tests and all 16 notebooks passed |
| Portable Excel workbook tests | 17 passed; 21 skip entries include Windows live tests and a PowerShell-dependent class |
| Formatting, strict library/benchmark Clippy, type/stub checks | Passed |
| Book HTML, runnable Rust Book examples and strict rustdoc | Passed |
| Crate publish dry run and risk-reprojection example | Passed |

Four Clippy findings in the new prepared DCF test module were fixed and its
12 tests were rerun successfully. A separate broader
`cargo clippy -p quantsupport --tests -- -D warnings` audit still reports
778 existing test-code lint diagnostics outside that module. CI's strict
library check uses `--lib` and passes; the broader audit is recorded as a
failure in the artifact rather than counted as a successful check.

This is local macOS ARM64 / Python 3.14 validation. Windows live Excel,
PowerShell packaging and the other OS/Python matrix jobs remain unverified
on this working tree. Rust's embedded Python harness was pointed at the
virtual environment using `PYO3_PYTHON` and `PYTHONPATH`, with the matching
Python library directory on macOS's `DYLD_LIBRARY_PATH`.

### Completed benchmarks

All **112 matched cases / 326 paired processes** passed numerical checks:
85 request/portfolio/path/worker cases, eight prepared DCF cases, eight
additional MC cases, seven XVA cases and four PFE cases. All **17 Criterion
cases** also completed: five native AD, four full PFE and eight XVA
prepared/market-refresh workloads.

Both release builds completed before measurement. Benchmarks ran sequentially
with Rust 1.100.0 and matched numerical dependencies on the same Apple M1 Pro.
Most comparisons use three fresh process pairs; the large XVA case uses one
pair with three warm samples. The four PFE branch comparisons are single-pair
first-call diagnostics, independently complemented by Criterion's ten-sample
complete-workload measurements. Raw files and execution logs remain under
`target/merge-validation-20261127/`.

The selected timings below are medians of process medians. Ordinary and
prepared DCF rows use one worker; the script and XVA rows use ten workers.

| Workload / cost boundary | Old | Current |
| --- | ---: | ---: |
| Ordinary DCF, value only | 3.973 µs | 2.447 µs |
| Ordinary DCF, value and first risk | 4.741 µs | 11.418 µs |
| Ordinary 1,000-swap portfolio, value only | 3.848 ms | 1.908 ms |
| Ordinary 1,000-swap portfolio, value and first risk | 4.952 ms | 11.744 ms |
| Single-swap risk: old ordinary versus reused prepared snapshot | 4.746 µs | 0.261 µs |
| Single-swap risk: old ordinary versus refresh and prepared evaluation | 4.746 µs | 8.363 µs |
| 1,000-swap risk: old ordinary versus reused prepared snapshot | 4.994 ms | 0.099 ms |
| Rate script, 10,000 paths | 63.447 ms | 1.889 ms |
| Margined XVA, 1,000 paths, warm unchanged market | 5515.857 ms | 8.289 ms |
| Margined XVA, 10,000 paths, warm unchanged market | 55457.479 ms | 63.906 ms |
| Full PFE pipeline, 1,000 swaps / 500 paths, first call | 77481.407 ms | 1121.633 ms |

The performance gate remains **failed**: 15 of 17 ordinary first-risk
pricing cases are slower. Prepared DCF improves repeated snapshot execution by
18.2× for one swap and
50.5× for 1,000 positions, while full
refresh plus evaluation remains
1.76× and
1.73× the old ordinary risk cost,
respectively. The 10,000-path XVA first evaluation, including preparation,
takes 54.77 s on the old branch and 12.84 s on the
candidate. These results support the specific repeated-execution workloads;
they do not establish a universal gain for ordinary or changed-market pricing.

Criterion separately measures mean full market-refresh costs of
9.67 s at 1,000 paths and
10.49 s at 10,000 paths,
with ten complete evaluations per case. The artifact retains those mean
estimates and 95% confidence intervals separately from the paired-process
median timings in the table.

The portable artifact contains the complete reproduction commands and exact
source hashes. Run the existing `scaling.py`, `prepared.py`, `monte_carlo.py`,
`prepared_xva.py` and `pfe.py` runners with separate old/candidate worktrees
and build targets as documented above; then run all three registered Criterion
benches. A smoke run remains a harness check and is excluded from these results.

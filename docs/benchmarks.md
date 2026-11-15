# Compiled-only AD versus the old branch

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

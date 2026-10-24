# 10,000 instruments at 10,000 origination dates

For opening and refreshing the object/pricing panes in this workload, see the
[pane responsiveness measurements](ribbon.md).

The packaged build prices the complete book in **4.75 seconds**, compared with
**12.35 seconds** for individual asynchronous worksheet formulas (62% less time).
A confirmation run took 4.57 seconds. Unchanged recalculation takes 2.17 seconds
and adds no pricing jobs, contexts or curve builds.

| Implementation | Workers | Cold: final worksheet values | Unchanged recalculation | Peak Excel RSS |
| --- | ---: | ---: | ---: | ---: |
| Pre-engine worksheet implementation | synchronous | 13.584 s | 11.518 s | 533 MiB |
| Individual asynchronous PV formulas | 8 | 12.353 s | 3.173 s | 1,108 MiB |
| Paired-context batch | 1 | 14.031 s | 2.221 s | 1,081 MiB |
| Paired-context batch, packaged build | 8 | **4.750 s** | **2.168 s** | 1,048 MiB |
| Paired-context batch, confirmation build | 8 | 4.567 s | 2.147 s | 1,052 MiB |

Each JSON file beside this report contains the full 10,000-value vector,
stage timings, construction counters, binary hashes and machine information.
These are desktop Excel measurements on one machine, not estimates or a Rust-only loop.

The book consists of 10,000 distinct receive-fixed, five-year USD SOFR swaps.
Origination dates advance daily from 1990-01-01, and every row has its own context
at that date. Notionals and fixed rates vary. Swaps start seven days after
origination to avoid requiring past fixings. Six shared synthetic deposit/OIS
calibration rates are used for independently dated SOFR curves.

All asynchronous runs submitted and completed 10,000 valuations, materialized
10,000 contexts and built 10,000 curves. The largest absolute difference from
the pre-engine reference was **2.33e-10 USD**. Cold and warm vectors were identical.
The pre-engine bindings were built against the same numerical core as the new
engine; user changes to that core were retained.

The batch workbook uses this formula in G2:

```excel
=QS.EVALUATE.RANGE(C2:C10001,F2:F10001)
```

The engine fans the paired handles out to context-affine workers, reuses scalar
jobs already requested by other worksheet cells or the pane, and assembles the
ordered spill. One worksheet subscription replaces 10,000 result subscriptions.
Completion dispatch yields through a 16 ms UI timer between bounded batches.
Random refresh addresses are sorted and combined into short contiguous ranges;
pane tables transfer at most 256 rows per page.

In the packaged eight-worker run, the blocking definition pass took **2.35 s**,
engine readiness was recorded at **4.736 s**, and the single result delivery pass
took **50.6 ms**. Background COM heartbeat latency was **1.75 ms median,
6.52 ms p95, 135 ms maximum**. The heartbeat starts after the synchronous definition
call, so it excludes that initial 2.35 s block. Initial native commit/dispatch
reached 127 ms, above the plan's 100 ms handler target. The one-worker run also
had a 1.91 s maximum heartbeat delay; its p95 was 4.95 ms. Large first calculations
therefore still have measurable pauses despite the throughput improvement.

Windows x64, Excel 16.0 build 20430, 16 logical CPUs, Python 3.13.7, pywin32 312,
psutil 7.2.2. Excel remained in Manual mode and its 2,000 ms RTD throttle was
preserved. `CalculationState` was not used as the completion barrier: the runner
waited for settled definitions, no pending jobs, and no pending worksheet delivery.

From the repository root:

```powershell
python -m pip install -r bindings/excel/examples/requirements-live.txt
powershell -NoProfile -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1 -Profile release -Dist target/excel-async-release
python bindings/excel/examples/scripts/benchmark_async.py --xll target/excel-async-release/quantsupport.xll --count 10000 --workers 8 --layout batch --output target/excel-benchmarks/batch.json --reference bindings/excel/benchmarks/2026-10-24/pre-engine-rows.json
```

Use `--layout rows` for individual formulas and `--workers 1` for the scaling
control. The runner creates a separate Excel instance and generates its workbook
beside its JSON report. Restart Excel before replacing or loading a new XLL pair;
the executing native module stays mapped until process exit.

Validation passed: 123 native library tests (one separate benchmark ignored),
14 function-catalog tests, 39 workbook/desktop Excel checks, 9 release-download
tests and the managed JSON contract suite. Live coverage includes nested scalar
and spill delivery, quote edits, duplicate/cleared inputs, stored results and
curves, Manual mode, paired-context order/transposition/deduplication, late
cancellation, workbook lifetime and real pane cashflow paging beyond 256 rows.

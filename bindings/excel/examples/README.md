# Excel examples

**Large valuation example:** [10,000 valuations in one batch](workbooks/origination_10000_batch.xlsx). It contains 10,000 different five-year USD SOFR swaps, 10,000 distinct origination dates, and a separate dated context in every row. `QS.EVALUATE.RANGE` pairs the context and trade ranges, prices across workers, and spills 10,000 values. Open **Start Here**, then **Origination**.

The [row-by-row version](workbooks/origination_10000_valuations.xlsx) keeps a separate PV formula in every row for comparison.

The folder is organized as follows:

- `workbooks/`: ready-to-open Excel examples.
- `scripts/`: workbook generators, API migration, automation helper and benchmark runner.
- `tests/`: portable checks and optional desktop Excel integration tests.

Open a workbook's **Start Here** tab, then follow the linked, numbered tabs from left to right. Each study includes three changes to try and explains where to read their effects.

Load the matching `quantsupport.xll` and `QuantSupport.Ribbon64.xll` in **64-bit Windows Excel** before opening the workbook. Use **QuantSupport > Refresh Pricing** to calculate definitions and deliver background results, including in Manual mode. Generated results are blank until Excel calculates; the files do not contain saved prices or live native objects. See the [add-in setup](../README.md#build-and-load).

| Workbook | What to follow |
| --- | --- |
| [10,000 valuations in one batch](workbooks/origination_10000_batch.xlsx) | Same dated contexts and trades, with one parallel batch and a 10,000-row spill. Inspect G2 for the paired context/trade formula. |
| [10,000 valuations at origination](workbooks/origination_10000_valuations.xlsx) | Price a different swap at each of 10,000 origination dates; inspect the context, curve contribution, trade and PV for every row. |
| [Context-first pricing](workbooks/context_pricing_example.xlsx) | Create a short context, add independent inputs, request direct loan values and run a historical batch without per-row contexts or evaluation handles. |
| [Portfolio aggregation](workbooks/portfolio_aggregation_example.xlsx) | Pass a range of six trade handles, reconcile PV, and inspect weighted coupon rates/spreads, distinct SOFR/TermSOFR3m fixing indices, sensitivities and separate interest/capital payments. |
| [Swap pricing](workbooks/swap_pricing_example.xlsx) | Bootstrap SOFR under USD collateral and value a receive-fixed USD swap. Bootstrap ESTR in a separate EUR context for curve comparisons. |
| [Loan pricing](workbooks/loan_pricing_example.xlsx) | Compare 12 agreements: repayment styles, floating and mixed coupons, prepayment, borrower credit, funding and stepped coupons. |
| [Bond pricing](workbooks/bond_pricing_example.xlsx) | Keep one bond contract and compare separately anchored positions using Z-spread or clean price. |
| [Historical pricing](workbooks/historical_pricing_example.xlsx) | Archive two dated markets and reprice one unchanged trade. Compare the requested timestamp with the actual selected market date. |
| [Interpolation and schedules](workbooks/interpolation_example.xlsx) | Compare three interpolation methods, check a bilinear surface against its formula, and compare adjusted and transposed schedules. |

## Reading the sheets

- **Inputs / Cases:** blue cells are editable; labels explain units and conventions. Loan settings and coupon fields are labelled in each case's block. Rates and spreads are annual decimals: enter `0.04` for 4%. Dates are ISO text such as `2026-02-24`.
- **Market / Markets:** market assumptions appear first, followed by the curve-construction steps that consume them.
- **Build / Archive / Pricing:** read the object sequence from top to bottom. Each grey object-reference cell has its purpose beside it and its Excel formula immediately below. Named cells such as `Swap_Context` make dependencies easier to trace in the formula bar or Excel's Name Manager.
- **Results:** green cells contain financial measures. Loan and bond rows link directly to their inputs and build steps. Their **Inspect one case** dropdown controls both the Cashflows and Risk tabs.
- **Cashflows / Risk / Curves:** tables have dedicated space to spill. Sensitivities are raw derivatives per unit market input; multiplying a decimal-rate sensitivity by `0.0001` approximates the effect of a +1 bp move.
- **Layouts** in the loan study shows the same small curve table with all four combinations of `transpose` and `headers`. The schedule study also shows a transposed date list.

Start with **Context-first pricing** for the shorter API. Its additions all reference the same root; the engine commits the owned additions after the definition pass. Every study now creates its market through a context and independent additions. The detailed studies use the library's explicit **instrument → trade → discount policy → pricer → evaluation → result** objects. FUNDING is an ordinary composite curve selected by a discount policy. Coupon schedules use independent period, rate and index arrays. Contract settings use named QS.LOAN.SETTINGS arguments.

The swap study keeps USD/SOFR and EUR/ESTR bootstrap configurations in separate contexts. Bootstrapping EUR OIS inside a USD collateral context would require a `Collateral:EUR/USD` curve calibrated from additional market data. The ESTR comparison context has no role in the USD swap's valuation.

Loan fair-rate columns correspond to zero-based coupon segments. Each `QS.REQUEST.FAIR.RATE(index)` evaluation returns one scalar. A mixed or stepped solve fixes earlier segments before solving the requested segment. Empty segment columns mean that segment does not exist; bounded floating coupons have no fair-rate solve.

## Regenerate and check

From the repository root:

```sh
python -m pip install -r bindings/excel/examples/requirements.txt
python bindings/excel/examples/scripts/create_examples.py
python -m unittest discover -s bindings/excel/examples/tests -p 'test_*.py'
```

Generators write to `workbooks/` by default. Regenerate the large workbook with `python bindings/excel/examples/scripts/create_origination_example.py`.

Generate the faster batch version with `python bindings/excel/examples/scripts/create_origination_example.py --layout batch --output bindings/excel/examples/workbooks/origination_10000_batch.xlsx`.

Use `--example context`, `portfolio`, `swap`, `historical`, `loans`, `bonds`, or `interpolation` to generate one file, and `--output-dir PATH` to choose a destination.

The portfolio workbook matches the [Python notebook](../../python/examples/sensitivity/portfolio_aggregation.ipynb).
Its default USD book has two deposits (100,000 at 4% and 300,000 at 6%), two
receive-fixed SOFR swaps (100,000 at a 10 bp floating spread and 300,000 at
30 bp), one 200,000 TermSOFR3m swap, and a 50,000 short SOFR swap.
All payments are discounted on SOFR; TermSOFR3m has its own projection curve.
The deposits combine at **400,000 and 5.5%**; the paid SOFR coupons combine at
**-400,000 and 25 bp**. The TermSOFR3m coupons and opposing directions remain
separate. Open **3 Build** to find the contiguous `Portfolio_Members` range
passed to `QS.PORTFOLIO.CREATE`, and **5 Coupons** to inspect the resulting terms.
**6 Cashflows** shows payment-date interest and capital; it does not allocate
interest across accrual periods for NII.

The tests require Rust/Cargo. They validate navigation, formula dependencies, registered signatures, and the native pricing workflow. They also extract the swap's actual worksheet quotes, configurations and context assumptions and bootstrap them through Rust on any platform, checking the original missing-collateral failure as a regression.

On Windows, install `pywin32` and add `--xll-path bindings/excel/dist/quantsupport.xll` for live checks of native handles, numerical results, selected-case tables, and unchanged recalculation. Actual worksheet execution requires Windows Excel.

Context and observation timestamp arrays feed `QS.SNAPSHOT.FROM.RANGE`;
no store-constructor chain or input record table is required.

Background pricing requires both matching XLLs. In Manual mode use **Refresh Pricing**; automation should call `async_excel.await_engine`, including dependent stored-result stages. The persistent RTD throttle is preserved. See [`scripts/benchmark_async.py`](scripts/benchmark_async.py) for the 10,000-instrument origination-date benchmark and [`tests/test_async_live.py`](tests/test_async_live.py) for desktop Excel delivery tests.

## Benchmark 10,000 distinct dated contexts

From the repository root, with Windows Excel, `pywin32` and `psutil` installed:

```sh
python -m pip install -r bindings/excel/examples/requirements-live.txt
python bindings/excel/examples/scripts/benchmark_async.py --xll bindings/excel/dist/quantsupport.xll --count 10000 --layout batch --workers 8 --output target/excel-benchmarks/origination-10000.json
```

The benchmark generates the same example beside its report and runs an isolated Excel instance. Use `--layout rows` to measure individual PV formulas. It checks that every price is numeric and that unchanged recalculation submits no new jobs. Add `--reference PATH.json` to compare all prices against a previous run. Inputs are synthetic; each swap starts seven days after its valuation date.

See the [measured results and raw reports](../benchmarks/2026-10-24/README.md): 4.75 seconds cold for the paired batch versus 12.35 seconds for individual asynchronous formulas on the measured machine.

The [pane responsiveness report](../benchmarks/2026-10-24/ribbon.md) covers opening,
refreshing and searching the same 10,000-context book. The optional
`tests/test_ribbon_large_live.py` regression uses a probe companion built with
`-p:QuantSupportLiveTests=true`; it verifies the full lists, selection retention
and safe disposal during a pending load.

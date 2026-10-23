# Guided Excel examples

Open a workbook's **Start Here** tab, then follow the linked, numbered tabs from left to right. Each study includes three changes to try and explains where to read their effects.

Load the matching `quantsupport.xll` in **64-bit Windows Excel** before opening the workbook. Press **Ctrl+Alt+F9** to rebuild all calculations. Generated results are blank until Excel calculates; the files do not contain saved prices or live native objects. See the [add-in setup](../README.md#build-and-load).

| Workbook | What to follow |
| --- | --- |
| [Context-first pricing](context_pricing_example.xlsx) | Create a short context, add independent inputs, request direct loan values and run a historical batch without per-row contexts or evaluation handles. |
| [Portfolio aggregation](portfolio_aggregation_example.xlsx) | Pass a range of six trade handles, reconcile PV, and inspect weighted coupon rates/spreads, distinct SOFR/TermSOFR3m fixing indices, sensitivities and separate interest/capital payments. |
| [Swap pricing](swap_pricing_example.xlsx) | Bootstrap SOFR under USD collateral and value a receive-fixed USD swap. Bootstrap ESTR in a separate EUR context for curve comparisons. |
| [Loan pricing](loan_pricing_example.xlsx) | Compare 12 agreements: repayment styles, floating and mixed coupons, prepayment, borrower credit, funding and stepped coupons. |
| [Bond pricing](bond_pricing_example.xlsx) | Keep one bond contract and compare separately anchored positions using Z-spread or clean price. |
| [Historical pricing](historical_pricing_example.xlsx) | Archive two dated markets and reprice one unchanged trade. Compare the requested timestamp with the actual selected market date. |
| [Interpolation and schedules](interpolation_example.xlsx) | Compare three interpolation methods, check a bilinear surface against its formula, and compare adjusted and transposed schedules. |

## Reading the sheets

- **Inputs / Cases:** blue cells are editable; labels explain units and conventions. Loan settings and coupon fields are labelled in each case's block. Rates and spreads are annual decimals: enter `0.04` for 4%. Dates are ISO text such as `2026-02-24`.
- **Market / Markets:** market assumptions appear first, followed by the curve-construction steps that consume them.
- **Build / Archive / Pricing:** read the object sequence from top to bottom. Each grey object-reference cell has its purpose beside it and its Excel formula immediately below. Named cells such as `Swap_Context` make dependencies easier to trace in the formula bar or Excel's Name Manager.
- **Results:** green cells contain financial measures. Loan and bond rows link directly to their inputs and build steps. Their **Inspect one case** dropdown controls both the Cashflows and Risk tabs.
- **Cashflows / Risk / Curves:** tables have dedicated space to spill. Sensitivities are raw derivatives per unit market input; multiplying a decimal-rate sensitivity by `0.0001` approximates the effect of a +1 bp move.
- **Layouts** in the loan study shows the same small curve table with all four combinations of `transpose` and `headers`. The schedule study also shows a transposed date list.

Start with **Context-first pricing** for the shorter API. Its additions all reference the same root; pricing takes the complete additions range. The other studies retain the library's explicit **instrument → trade → discount policy → pricer → evaluation → result** objects. FUNDING is an ordinary composite curve selected by a discount policy. Coupon schedules use independent period, rate and index arrays. Contract settings use named QS.LOAN.SETTINGS arguments.

The swap study keeps USD/SOFR and EUR/ESTR bootstrap configurations in separate contexts. Bootstrapping EUR OIS inside a USD collateral context would require a `Collateral:EUR/USD` curve calibrated from additional market data. The ESTR comparison context has no role in the USD swap's valuation.

Loan fair-rate columns correspond to zero-based coupon segments. Each `QS.REQUEST.FAIR.RATE(index)` evaluation returns one scalar. A mixed or stepped solve fixes earlier segments before solving the requested segment. Empty segment columns mean that segment does not exist; bounded floating coupons have no fair-rate solve.

## Regenerate and check

From the repository root:

```sh
python -m pip install -r bindings/excel/examples/requirements.txt
python bindings/excel/examples/create_examples.py
python -m unittest discover -s bindings/excel/examples -p 'test_*.py'
```

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

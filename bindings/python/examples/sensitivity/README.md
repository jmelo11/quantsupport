# Sensitivity and portfolio requests

Start with [How a portfolio works](portfolio.ipynb) for a step-by-step
introduction to `qs.Portfolio`. It builds a deposit and two opposing swap
positions, explains the context and request lifecycle, and checks the aggregate
PV, risk and payment reports against individual trades. It also demonstrates
position size overrides, repeated members, common pricing currency and
unsupported requests. All market inputs are defined inline.

The risk study combines a swap, bond and caplet, aggregates their quote
sensitivities and compares linear P&L with scenario revaluation. It follows the
Rust [`sensitivity`](../../../../examples/sensitivity) examples.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [portfolio.ipynb](portfolio.ipynb) | How does `qs.Portfolio` turn trades into aggregate results? | Instruments versus positions, context lifecycle, standard results, side and size overrides, PV/risk/payment reconciliation, repeated members, request and currency rules |
| [portfolio_risk.ipynb](portfolio_risk.ipynb) | How do I aggregate position values and risk and check the result against a market move? | Shared context, product-specific requests, labelled aggregation, signed DV01/vega, AAD, central differences, stress P&L, cashflow reports |
| [portfolio_aggregation.ipynb](portfolio_aggregation.ipynb) | How do I use `qs.Portfolio` while retaining coupon terms and fixing indices? | Standard results, weighted fixed rates/floating spreads, distinct SOFR/TermSOFR3m curves, pay/receive groups, payment-date interest/capital, numeric risk aggregation |

Run the introduction in a fresh kernel from the repository root with a binding
built from this checkout:

```bash
python bindings/python/examples/run_notebooks.py sensitivity/portfolio.ipynb
```

For the more detailed coupon study, open the aggregation notebook and run its
cells in order, or use:

```bash
python bindings/python/examples/run_notebooks.py sensitivity/portfolio_aggregation.ipynb
```

It creates an illustrative market directly from editable zero-rate pillars;
there are no downloaded market inputs. `POSITIONS` contains two deposits,
three receive-fixed swaps and one short swap, all in USD. The default deposit
coupons combine to 400,000 at **5.5%**, and paid SOFR coupons to -400,000 at
a **25 bp** spread. TermSOFR3m coupons stay separate at their own index, and the
short swap keeps its opposing coupon directions.

The notebook checks PV, every raw sensitivity and payment totals against
independent position evaluations, then displays positions, position PVs, detailed
coupons, the interest/capital summary and sensitivities. It also plots interest
and principal payments separately. Set `EXPORT_CSV = True` in its final code
cell to export five tables. Edit notionals, rates or spreads in `POSITIONS`
to explore weighting; changes to the book's structure may require adapting the
demonstration assertions. The summary uses payment dates, without accrual NII
allocation. Built-in pricers differentiate one trade at a time.
Sensitivities are raw derivatives per unit zero-rate input; multiply by `0.0001`
to estimate a +1 bp PV change for these decimal-rate pillars.

The matching [Excel workbook](../../../excel/examples/workbooks/portfolio_aggregation_example.xlsx)
uses the same dates, curves, instruments and directions. Its **Start Here** tab
explains the editable inputs and result sheets.

The mixed portfolio risk notebook imports its local [market_data.py](market_data.py)
directly; the introduction and aggregation notebooks define their markets inline.

Rate and volatility derivatives are checked with separate central revaluations.
Swap cashflows receive explicit trade/leg signs before aggregation; the signed
payment PV report reconciles the swap and bond rather than adding both swap
legs as receipts. The finite stress illustrates nonlinear hedge residuals.

For the separate historical ORE/CurveEngine study and its optional dependencies,
see [the external benchmark](../../../../examples/sensitivity/bechmark/sensitivity.ipynb):

```bash
python bindings/python/examples/run_notebooks.py benchmark
```

Follow the [shared setup](../README.md#install-and-run). Run this scope from the
repository root with:

```bash
python bindings/python/examples/run_notebooks.py sensitivity/portfolio_risk.ipynb
```

The benchmark compares recorded curve, PV and rate-risk results with explicit
tolerances and reports a remaining FX-risk-definition discrepancy separately.

For contract-level analytics, see [valuation](../valuation/README.md). For
legal netting and future exposure, see [CVA/XVA](../cva/README.md).

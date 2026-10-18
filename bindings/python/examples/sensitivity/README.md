# Sensitivity and portfolio requests

The portfolio study combines a swap, bond and caplet, aggregates their quote
sensitivities and compares linear P&L with scenario revaluation. It follows the
Rust [`sensitivity`](../../../../examples/sensitivity) examples.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [portfolio_risk.ipynb](portfolio_risk.ipynb) | How do I aggregate position values and risk and check the result against a market move? | Shared context, product-specific requests, labelled aggregation, signed DV01/vega, AAD, central differences, stress P&L, cashflow reports |

The notebook imports its local [market_data.py](market_data.py) directly.

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

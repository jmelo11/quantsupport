# Sensitivity and portfolio requests

Scope: portfolio valuation, market-quote derivatives, and scenarios,
corresponding to the Rust [`sensitivity`](../../../../examples/sensitivity) examples.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [portfolio_risk.ipynb](portfolio_risk.ipynb) | How do I aggregate position values and risk and check the result against a market move? | Shared context, product-specific requests, labelled aggregation, signed DV01/vega, AAD, central differences, stress P&L, cashflow reports |

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run). Run this scope from the
repository root with:

```bash
python bindings/python/examples/run_notebooks.py sensitivity/portfolio_risk.ipynb
```

For contract-level analytics, see [valuation](../valuation/README.md). For
legal netting and future exposure, see [CVA/XVA](../cva/README.md).

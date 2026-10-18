# Swap valuation

| Notebook | Workflow |
| --- | --- |
| [tour.ipynb](tour.ipynb) | Market inputs, conventions, contract/position terms, requests and result units |
| [par_swap.ipynb](par_swap.ipynb) | Solve the fixed coupon and reprice at zero PV |
| [currency_conversion.ipynb](currency_conversion.ipynb) | Value CLP payments in USD under USD collateral and reconcile PV |

The local [market_data.py](market_data.py) loads illustrative USD/SOFR inputs
from the Rust Hull–White snapshot and USD/CLP inputs from the bootstrap snapshot.
It loads curves without volatility inputs because these swap valuations use curves.

The studies explicitly verify signed discounted cashflows, reversed positions,
the fixed-leg annuity/par-coupon identity, FX reciprocity, and cancellation of
matching principal exchanges. CLP cashflows remain labelled CLP even though
the USD-collateralized native position PV is already reported in USD.

Follow the [shared setup](../../README.md#install-and-run), then run this product
folder from the repository root:

```bash
python bindings/python/examples/run_notebooks.py valuation/swaps
```

The [Rust swaps package](../../../../../examples/valuation/swaps) runs with
`cargo run -p valuation-swaps`. Return to the [valuation catalog](../README.md).

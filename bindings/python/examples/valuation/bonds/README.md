# Bond valuation

| Notebook | Workflow |
| --- | --- |
| [fixed_rate_bond.ipynb](fixed_rate_bond.ipynb) | Clean-price/Z-spread round trip, yield, duration, cashflows and curve risk |
| [stepped_coupon_bond.ipynb](stepped_coupon_bond.ipynb) | Amortization, issuer calls, calibrated tree/Fourier values, OAS and quote risk |

The local [market_data.py](market_data.py) reads the illustrative 2026-02-24
USD/SOFR curves and Black caplet surface. [data/bond_tree.json](data/bond_tree.json)
and [data/bond_fourier.json](data/bond_fourier.json) own the callable-bond model
and grids. Contract terms remain in the notebooks.

The callable-bond teaching grids use 48 time steps per year and 256 Fourier
points. Each engine's price differs by less than 0.01 per 100 principal from
its value with 96 steps per year and 512 Fourier points on this snapshot.
Refine both grids when adapting the example to a pricing tolerance. Only the
tree risk report requests sensitivities; Fourier comparisons and OAS inversion
request value and OAS.

Follow the [shared setup](../../README.md#install-and-run), then run this product
folder from the repository root:

```bash
python bindings/python/examples/run_notebooks.py valuation/bonds
```

The [Rust bonds package](../../../../../examples/valuation/bonds) runs with
`cargo run -p valuation-bonds`. Return to the [valuation catalog](../README.md).

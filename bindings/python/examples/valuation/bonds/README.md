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
points. The notebook actually revalues at 96 steps and 512 points and reports
the observed changes; this is a diagnostic, not proof of convergence.
Refine both grids and state width when adapting the example to a tolerance. Only the
tree risk report requests sensitivities; Fourier comparisons and OAS inversion
request value and OAS.

The straight-bond study reconstructs spread- and yield-discounted receipts and
checks duration/spread risk with independent bumps. The callable study verifies
its OAS inversion and compares the callable note with its matching noncallable
underlying under the same model and spread.

Follow the [shared setup](../../README.md#install-and-run), then run this product
folder from the repository root:

```bash
python bindings/python/examples/run_notebooks.py valuation/bonds
```

The [Rust bonds package](../../../../../examples/valuation/bonds) runs with
`cargo run -p valuation-bonds`. Return to the [valuation catalog](../README.md).

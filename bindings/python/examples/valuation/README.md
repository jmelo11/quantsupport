# Valuation

Scope: product construction, valuation methods, and product-specific analytics,
corresponding to the Rust [`valuation`](../../../../examples/valuation) examples.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [tour.ipynb](tour.ipynb) | How do I value a receive-fixed swap and read its results? | Dates, enums, contract/position terms, context, requests, result units |
| [product_valuations.ipynb](product_valuations.ipynb) | How do requests differ for swaps, bonds, caplets and caps? | Par rate, price/spread anchors, yield, duration, cashflows, Black option valuation |
| [stepped_coupon_bond.ipynb](stepped_coupon_bond.ipynb) | How do stepped coupons, amortization and issuer calls affect value and OAS? | Typed coupons/calls, calibrated Hull–White model, tree/Fourier engines, OAS risk |

[data/bond_tree.json](data/bond_tree.json) and
[data/bond_fourier.json](data/bond_fourier.json) hold the model and numerical
settings explained in the callable-bond narrative. Contract terms remain in the
notebooks.

The callable-bond defaults use 48 time steps per year and 256 Fourier state
points to keep the walkthrough interactive. On the supplied market snapshot,
each engine's price changes by less than 0.01 per 100 principal compared with
96 time steps per year and 512 Fourier points. This comparison is not a
convergence guarantee: refine both grids when adapting the example to a pricing
tolerance. Sensitivities are requested only for the tree risk report; the
Fourier comparison and price-to-OAS round trip request value and OAS.

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run). Run this scope from the
repository root with:

```bash
python bindings/python/examples/run_notebooks.py valuation/tour.ipynb valuation/product_valuations.ipynb valuation/stepped_coupon_bond.ipynb
```

Next: [portfolio sensitivity](../sensitivity/README.md) or
[scripted payoffs](../scripting/README.md).

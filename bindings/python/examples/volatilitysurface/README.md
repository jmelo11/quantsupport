# Volatility surfaces

Scope: constructed option-volatility markets and their pricing impact,
corresponding to the Rust [`volatilitysurface`](../../../../examples/volatilitysurface)
examples.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [curves_and_volatility.ipynb](curves_and_volatility.ipynb) | How do separate rate and volatility shocks affect a swap and a caplet? | Curve/surface configuration, forwards, smiles, Black-vol units, quote scenarios, full revaluation |

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run). Run this scope from the
repository root with:

```bash
python bindings/python/examples/run_notebooks.py volatilitysurface/curves_and_volatility.ipynb
```

For curve construction alone, see [bootstrap](../bootstrap/README.md). For
calibrating a short-rate model from this surface, see [Hull–White](../hullwhite/README.md).

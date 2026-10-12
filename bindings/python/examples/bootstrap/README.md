# Bootstrap

Scope: quote inputs, curve configurations, and constructed discount/forward
curves, corresponding to the Rust [`bootstrap`](../../../../examples/bootstrap)
examples.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [curves.ipynb](curves.ipynb) | How do deposit/OIS quotes become discount factors and forward rates? | QuoteStore, CurveConfiguration, MarketIndex, context lifecycle, nodes and pillars |

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run), then open the notebook
in a fresh kernel. Run this scope from the repository root with:

```bash
python bindings/python/examples/run_notebooks.py bootstrap/curves.ipynb
```

Next: [valuation](../valuation/README.md) or [volatility surfaces](../volatilitysurface/README.md).

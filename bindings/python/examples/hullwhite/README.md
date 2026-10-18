# Hull–White simulations

The simulation study calibrates a Gaussian short-rate model, generates paths
and examines their distribution and sampling precision. It follows the Rust
[`hullwhite`](../../../../examples/hullwhite) examples.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [simulations.ipynb](simulations.ipynb) | How do I generate rate paths and obtain distributions and sampling statistics? | SimulationConfiguration, calibration, horizon/grid, seed/path count, Simulation.paths and dates |

[data/simulation.json](data/simulation.json) contains the model assignment,
calibration source, and path-generation settings explained in the notebook.

The study reports calibration diagnostics and distinguishes Black quote
volatility from absolute model sigma. It reconstructs Gaussian mean/variance
recursions, checks repeated seeds, and separates sampling error from Euler
time-grid bias. Positive mean reversion is required here. Exported numerical
paths have no quote-risk pillars and are not differentiated exposure paths.

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run). Run this scope from the
repository root with:

```bash
python bindings/python/examples/run_notebooks.py hullwhite/simulations.ipynb
```

Next: [future portfolio exposure and XVA](../cva/README.md).

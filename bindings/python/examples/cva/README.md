# CVA, exposure, and XVA

Scope: netting-set exposure and valuation adjustments, corresponding to the
Rust [`cva`](../../../../examples/cva) and [`pfe`](../../../../examples/pfe) examples.
The Python walkthrough uses the summaries exposed by the XVA binding; it does
not imply that raw NPV cubes or PFE percentiles are exposed by that API.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [xva.ipynb](xva.ipynb) | How does legal netting change future exposure and XVA for the same positions? | XvaConfig, CsaTerms, NettingSet, calibrated LGM paths, signed EPE/ENE/EE profiles, XVA values and parameter risk |

[data/xva.json](data/xva.json) holds the model and simulation settings. Client
credit/funding terms and portfolio grouping remain visible in the notebook.

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run). Run this scope from the
repository root with:

```bash
python bindings/python/examples/run_notebooks.py cva/xva.ipynb
```

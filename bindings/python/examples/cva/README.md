# CVA, exposure, and XVA

Scope: netting-set exposure and valuation adjustments, corresponding to the
Rust [`cva`](../../../../examples/cva) and [`pfe`](../../../../examples/pfe) examples.
The Python walkthrough uses the summaries exposed by the XVA binding; it does
not imply that raw NPV cubes or PFE percentiles are exposed by that API.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [xva.ipynb](xva.ipynb) | How does legal netting change future exposure and XVA for the same positions? | XvaConfig, CsaTerms, NettingSet, calibrated LGM paths, signed EPE/ENE/EE profiles, XVA values and parameter risk |

[data/xva.json](data/xva.json) holds the model and simulation settings. Client
credit, funding, and variation-margin terms are defined directly in Python
in the notebook. The credit curve is bootstrapped
from the illustrative CDS quotes in [data/credit_quotes.json](data/credit_quotes.json)
using [data/credit_curves.json](data/credit_curves.json). A reusable
`TenorBasedZeroRateTermStructure` supplies funding spreads over SOFR;
`ZeroRateTermStructure` also supports dated pillars. The notebook compares netting and
variation-margin effects using identical market paths.

The ATM calibration basket stops at the portfolio's three-year horizon.
Some longer-expiry ATM strikes are outside the shared surface's quoted smile;
extending the basket requires market quotes covering those strikes.

Thresholds and minimum transfer amount are in collateral currency. Calls
settle on simulation dates; no settlement lag, margin period of risk, or
initial margin is modeled. Returned exposure profiles are raw, pre-margin;
CVA and FVA use post-margin pathwise exposures.

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run). Open
[xva.ipynb](xva.ipynb) in VS Code, select the Python environment containing the
rebuilt binding as its kernel, and run all cells. The notebook has been
validated with the `alm` Python 3.14 environment. An unrelated default
Jupyter kernel can load incompatible NumPy or binding versions.

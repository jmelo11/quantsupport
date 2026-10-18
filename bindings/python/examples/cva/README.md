# CVA, exposure, and XVA

The XVA study compares two offsetting swaps under shared and separate netting
agreements, then examines variation margin and credit/funding adjustments.
It follows the Rust [`cva`](../../../../examples/cva) and
[`pfe`](../../../../examples/pfe) examples using the exposure summaries
available through Python.

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
When extending it, verify that the new effective strikes remain inside the
shared surface's quoted smile, or choose an explicit extrapolation policy.

Thresholds and minimum transfer amount are in collateral currency. Calls
settle on simulation dates; no settlement lag, margin period of risk, or
initial margin is modeled. Returned exposure profiles are raw, pre-margin;
CVA and FVA use post-margin pathwise exposures. ENE is signed nonpositive and
`EE = EPE + ENE` at each date. FVA integrates signed exposure with right-endpoint
Actual/365 intervals, so netting need not reduce its magnitude. The high-level
example computes CVA/FVA, not DVA or PFE quantiles.

Risk coordinates need care: credit curves are snapshotted as numerical
survival pillars. Rows labelled `CVA.Cds_...` differentiate independent
**survival probabilities**, not the original CDS spreads; the credit bootstrap
Jacobian is not replayed. Funding-curve rows similarly differentiate derived
spread pillars without replaying the source curve calibration. This example
supplies dated funding spreads directly, in annual decimals. The notebook
annotates its risk table so quote-like labels cannot imply validated CDS hedges.

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run). Open
[xva.ipynb](xva.ipynb), select the environment containing the checkout-built
binding, and run all cells. To run without a browser:

```bash
python bindings/python/examples/run_notebooks.py cva/xva.ipynb
```

Assertions check profile signs/identities, raw netting, and unchanged raw
profiles when variation margin is disabled; the numerical comparison explains
why the corresponding XVA values still change.

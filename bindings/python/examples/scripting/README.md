# Scripting

Scope: dated payoff events and native/scripted comparisons, corresponding to
the Rust [`scripting`](../../../../examples/scripting) examples.

| Example | Financial question | Library concepts |
| --- | --- | --- |
| [scripted_cashflows.ipynb](scripted_cashflows.ipynb) | How do I express dated rate observations and payments and compare their native equivalent? | ScriptedProduct, event dates, RateIndex/cvg/pays, model resolution, path-averaged outputs, deterministic representation checks |

[stepped_cashflows.py](stepped_cashflows.py) generates repeated events for this
tutorial's six-year semiannual note. It omits issuer exercise; the
[callable-bond example](../valuation/bonds/stepped_coupon_bond.ipynb) explains that
separate valuation problem.

The notebook verifies deterministic native/scripted equivalence at matching
assumptions and reports repeated-seed stochastic estimates. The selected
result variable may use a fitted martingale control; raw payment PV sums are
not an exact finite-path identity for the adjusted result. Other returned
variables, such as observed rates, are not necessarily PVs.

The notebook imports its local [market_data.py](market_data.py) directly.

Follow the [shared setup](../README.md#install-and-run). Run this scope from the
repository root with:

```bash
python bindings/python/examples/run_notebooks.py scripting/scripted_cashflows.ipynb
```

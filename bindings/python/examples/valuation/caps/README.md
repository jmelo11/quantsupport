# Cap and caplet valuation

| Notebook | Workflow |
| --- | --- |
| [caps_and_caplets.ipynb](caps_and_caplets.ipynb) | Black prices for a SOFR caplet and quarterly cap strip, with curve/volatility risk |

The local [market_data.py](market_data.py) loads illustrative 2026-02-24
USD/SOFR curves and the Black caplet surface from the Rust Hull–White snapshot.
Rates and strikes are decimals; every example uses an absolute strike, so
`0.045` means 4.5%. Relative-strike interpretation is consumer-specific and is
not assumed here. Rate derivatives scale by `1e-4` for +1 bp P&L;
Black-volatility derivatives scale by `0.01` for +1 volatility point.

The notebook independently implements the Black formula with `math.erf`,
checks caplet/floorlet parity, and reconstructs the cap from eight explicitly
dated caplets. It specifies the engine's Actual/360 expiry/accrual conventions;
this is a period-forward caplet study, not a daily-observed overnight payoff.

Follow the [shared setup](../../README.md#install-and-run), then run this product
folder from the repository root:

```bash
python bindings/python/examples/run_notebooks.py valuation/caps
```

The [Rust caps package](../../../../../examples/valuation/caps) runs with
`cargo run -p valuation-caps`. Return to the [valuation catalog](../README.md).

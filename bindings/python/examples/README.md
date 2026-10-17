# Learning QuantSupport from Python

These notebooks assume familiarity with the financial products, but no prior
knowledge of QuantSupport. Each follows a financial question through contract
terms, market construction, valuation, and interpretation of the result. Run
the cells from top to bottom; every notebook works in a fresh kernel.

## Catalog by Rust example scope

The folders mirror the repository's Rust example packages. Each contains its
own catalog, notebooks, `market_data.py` loader, and any configuration specific
to that scope.

| Scope | Catalog | Coverage |
| --- | --- | --- |
| `bootstrap` | [Curve construction](bootstrap/README.md) | Quotes, bootstrap configurations, nodes, pillars, and forwards |
| `valuation/swaps` | [Swaps](valuation/swaps/README.md) | First valuation, par coupon and base-currency conversion |
| `valuation/bonds` | [Bonds](valuation/bonds/README.md) | Straight-bond analytics and callable stepped bonds |
| `valuation/caps` | [Caps and caplets](valuation/caps/README.md) | Black option values and curve/volatility risk |
| `valuation/loans` | [Loans](valuation/loans/README.md) | Repayment, resets, prepayment, credit, valuation and risk |
| `volatilitysurface` | [Volatility markets](volatilitysurface/README.md) | Curves, surfaces, and independent rate/volatility valuation shocks |
| `hullwhite` | [Rate simulations](hullwhite/README.md) | Calibrated model setup and retrieving simulation results |
| `sensitivity` | [Portfolio and risk](sensitivity/README.md) | Valuation requests, aggregation, quote risk, and scenarios |
| `scripting` | [Payoff events](scripting/README.md) | Dated payoff scripts and representation checks |
| `cva` | [Exposure and XVA](cva/README.md) | Netting sets, credit/funding inputs, exposure profiles, and XVA |

## Suggested reading order

| Order | Notebook | What you will learn |
| --- | --- | --- |
| 1 | [From quotes to a swap](valuation/swaps/tour.ipynb) | Dates and conventions, product/position terms, market indices, the context lifecycle, requests, and result units |
| 2 | [Par swap](valuation/swaps/par_swap.ipynb) | Solve the fixed coupon and verify zero PV |
| 3 | [Fixed-rate bond](valuation/bonds/fixed_rate_bond.ipynb) | Price/Z-spread anchors, yield, duration, payments and quote risk |
| 4 | [Caps and caplets](valuation/caps/caps_and_caplets.ipynb) | Black option values and separate rate/volatility risk |
| 5 | [Loans](valuation/loans/loans.ipynb) | Repayment, resets, prepayment, par coupon/spread, credit and risk |
| 6 | [Currency conversion](valuation/swaps/currency_conversion.ipynb) | Read FX direction, value a CLP swap under USD collateral, and reconcile its base-currency PV |
| 7 | [Curves and volatility](volatilitysurface/curves_and_volatility.ipynb) | Configure and inspect curves and surfaces, query forwards and smiles, and explain market-shock P&L |
| 8 | [Simulations and their results](hullwhite/simulations.ipynb) | Model calibration, path count/seed/grid, retrieving paths, seeds and sampling error |
| 9 | [Portfolio valuation and risk](sensitivity/portfolio_risk.ipynb) | Aggregate quote risk and payments, distinguish DV01 from vega, and check AAD against full revaluation |
| 10 | [Callable stepped-coupon bond](valuation/bonds/stepped_coupon_bond.ipynb) | Typed terms, calibrated tree/Fourier engines, OAS and risk |
| 11 | [Scripted cashflows](scripting/scripted_cashflows.ipynb) | Dated observations/payments and the native-versus-scripted bond check |
| 12 | [Exposure and XVA](cva/xva.ipynb) | Netting sets, client terms, exposure profiles and adjustment risk |

The introduction is the recommended starting point. For a closer look at curve
construction before pricing, read [bootstrap/curves.ipynb](bootstrap/curves.ipynb).
For loans, follow [the loan walkthrough](valuation/loans/loans.ipynb) from a fixed
payment schedule through floating resets, prepayment, valuation and risk.
The later notebooks repeat their own small setup, so you can also open the
subject you need directly.

## Install and run

Use a Python environment with the binding built from this checkout. From the
repository root, inside an activated virtual environment or Conda environment:

```bash
python -m pip install maturin -r bindings/python/examples/requirements.txt
maturin develop -m bindings/python/Cargo.toml --release
cd bindings/python/examples
python -m jupyterlab
```

If you need a virtual environment, create one first with `python -m venv .venv`
and activate it (`source .venv/bin/activate` on macOS/Linux, or
`.venv\Scripts\Activate.ps1` in PowerShell). Use either that environment or your
Conda environment. Select a notebook kernel using the same interpreter into
which `maturin develop` installed the extension.

Open any notebook and run its cells in order. Each folder contains its own copy
of `market_data.py`, imported with `from market_data import load_market`.
That file resolves market and configuration paths relative to its own location.
No helper-package installation or notebook magic is needed. The scripting
notebook also uses its adjacent event builder.

Jupyter starts each kernel in its notebook's scope folder. If your editor uses
a different working directory, select the folder containing the notebook as
the kernel working directory. The runner below sets this automatically. No
notebook invokes Cargo, fetches live data, or requires another notebook's
variables. If the native binding and the usual notebook/plotting dependencies
are already available, no installation step is needed to run the examples.

To execute the entire collection without a browser, run from the repository
root (or use an absolute path to the runner from any other directory):

```bash
python bindings/python/examples/run_notebooks.py
```

To execute selected notebooks and save results outside the source directory:

```bash
python bindings/python/examples/run_notebooks.py hullwhite/simulations.ipynb sensitivity/portfolio_risk.ipynb --output-dir /tmp/quantsupport-notebooks
```

The runner discovers notebooks in every scope and uses a fresh kernel with the
current interpreter for each notebook. It validates the notebook format and
fails on any cell error.
Executed copies preserve the scope folders under `--output-dir`.
The runner leaves source notebooks unchanged by default. Pass `--write` to
refresh their stored outputs, or `--kernel NAME` to select a registered kernel
instead of the current interpreter. The callable-bond
example uses small teaching grids for its two calibrated numerical valuations
and OAS inversion; its narrative explains how to refine those grids. Build the
binding with `--release` as above for interactive pricing. The per-cell timeout
defaults to 600 seconds and can be changed with `--timeout`.

## How the pieces fit

`QuoteStore` contains observations, `FixingStore` contains historical index
observations, and `FxStore` contains spot exchange rates. Most examples are
forward-looking USD products on one reference date. The currency-conversion
example also supplies USD/CLP spot FX and a CLP curve under USD collateral.
These examples do not require historical fixings. Curve and volatility
configurations describe how to construct market objects from quote identifiers.
A `MarketIndex` is the key
that connects those objects to products and models; it is not a market value.

Create a `PricingContext` and enter `with context:`. Entering the block
automatically constructs the market and starts the differentiation tape;
exiting clears both, including when an exception occurs. Call `initialize()`
explicitly only when using the context without a `with` block. Product
specifications describe the contract and position; the native pricer supplies
the valuation method. `evaluate(trade, requests)` returns an `EvaluationResults` object with
scalar analytics and pandas tables. Save those results inside the context and
use them after it closes. Constructed curve, surface, and simulation views
should be used inside the active context. Keep contexts sequential because the
native automatic-differentiation tape is managed per thread.

Simulation configurations add a model and a path-generation setup. A raw rate
path is not a product value. Script evaluation and the XVA engine combine
models with future payments to answer different questions. Ordinary portfolio
valuation is a loop over trades plus labelled pandas aggregation; legal netting
is specified separately with `NettingSet`.

Python-defined instruments and pricers can also participate through
`register_pricer`; see the [binding extension guide](../README.md#extending-the-library-in-python).
The notebooks focus on native products and their existing Python entry points.

## Auxiliary files and market provenance

| File | Responsibility |
| --- | --- |
| `market_data.py` in each scope folder | Local loader imported directly by its notebooks; reads typed inputs/configurations without hidden pricing |
| [stepped_cashflows.py](scripting/stepped_cashflows.py) | Translate this tutorial's six-year semiannual note into repetitive script events; no optimal issuer exercise |
| [valuation/bonds/data/bond_tree.json](valuation/bonds/data/bond_tree.json), [valuation/bonds/data/bond_fourier.json](valuation/bonds/data/bond_fourier.json) | Native pricer/model schema and numerical grids |
| [hullwhite/data/simulation.json](hullwhite/data/simulation.json) | Calibrated Hull–White simulation settings |
| [cva/data/xva.json](cva/data/xva.json) | Calibrated LGM exposure-engine settings |
| [run_notebooks.py](run_notebooks.py) | Execute the documentation against an installed binding |

Most notebooks use the **illustrative 2026-02-24** snapshot already maintained in
[`examples/hullwhite/data`](../../../examples/hullwhite/data) at the repository
root: `quotes.json`, `curve_specs.json`, and `vol_specs.json`. The
[currency-conversion notebook](valuation/swaps/currency_conversion.ipynb) uses the
USD/CLP quote and curve files in
[`examples/bootstrap/data`](../../../examples/bootstrap/data) on the same
reference date, with an explicit illustrative spot rate of 935 CLP per USD.
Keep the repository layout when running the tutorials; copying only the
notebooks omits their helpers and source market data. Nothing downloads or
updates these quotes automatically.
The [loan notebook](valuation/loans/loans.ipynb) reads a separate illustrative EUR
snapshot from [`examples/valuation/loans/data`](../../../examples/valuation/loans/data),
shared with `cargo run -p valuation-loans`.

The market contains a SOFR curve and a Black caplet surface with absolute-strike
smiles. The simulation and callable-bond examples calibrate a short-rate
volatility schedule from that surface; a Black quoted volatility and the
model's absolute sigma are different quantities.

## Reading the numbers

- Rates and spreads are decimals: `0.04` is 4%, and `1e-4` is 1 bp.
- A Black volatility of `0.30` is 30%; +1 volatility point is `0.01`.
- `result.price` is signed position PV. Bond clean/dirty anchors are quoted per
  configured units, usually per 100 principal.
- `sensitivities.value` is a derivative per unit input quote. Multiply by the
  appropriate move before reporting DV01 or vega. The notebooks retain signs.
- Cashflow amounts are undiscounted payments, with currency and leg labels.
- Missing/unrequested analytics are not zero and should not be silently filled
  when reporting product-level measures.

The assertions test the financial interpretation: par repricing, spread-price
round trips, response to independent market shocks, reproducible paths, AAD
versus central differences, deterministic payoff equivalence, and exposure
netting. These checks illustrate how to validate adaptations of the examples;
their small path counts and numerical grids are teaching settings.
